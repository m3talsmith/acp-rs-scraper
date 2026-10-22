//! Spaces list. The core stores the rows and asks the loaded service.
//!
//! Joining a room is not built. A LiveKit token is not a column and is not stored.
//! `--dry-run` prints the request and does not call the service.

use acp_rs_plugin::{Budget, CapabilityId, Cursor, DynService, Page, Target};

use crate::prelude::*;

/// View key for the Spaces list.
pub const SPACES_KEY: &str = "spaces";

/// One stored space. No LiveKit column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceRow {
    /// Stable slug.
    pub slug: String,
    /// Related post id. Empty when the list row has none.
    pub post_id: String,
    /// Title.
    pub title: String,
    /// `live`, `scheduled`, `ended`, or another state the service named.
    pub state: String,
    /// The room was live when it was stored.
    pub live: bool,
    /// Scheduled time. Empty when the service did not send one.
    pub scheduled_at: String,
    /// End time. Empty when the service did not send one.
    pub ended_at: String,
    /// Recording link. A `wss://` URL is dropped.
    pub recording_url: String,
    /// When this row was stored.
    pub fetched_at: String,
}

/// `--dry-run` text. No service call.
pub fn dry_text() -> String {
    "dry-run: no connection opened\nspaces\nJoining a room is not built.\n".into()
}

/// Read spaces into the local table, or print the dry-run.
pub async fn run(args: &Args) -> Result<()> {
    if args.dry_run {
        println!("{}", dry_text());
        return Ok(());
    }
    let Some(live) = crate::plugins::live(CapabilityId::Spaces) else {
        bail!("{}", crate::service::SERVICE_ABSENT);
    };
    let pages = fetch(live.get()).await?;
    let fetched_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let rows: Vec<SpaceRow> = pages.into_iter().flat_map(|page| rows_of(&page, &fetched_at)).collect();
    let n = rows.len();
    save_rows(&rows)?;
    println!("spaces {n}");
    Ok(())
}

/// Page the loaded service. This does not open a socket of its own.
pub async fn fetch(service: &dyn DynService) -> Result<Vec<Page>> {
    let mut pages = Vec::new();
    let mut cursor = Cursor::default();
    let mut budget = Budget::unlimited();
    let mut seen = HashSet::new();
    loop {
        let page = service.spaces(&Target::Site, &cursor, &mut budget).await.map_err(|err| anyhow!(err.to_string()))?;
        let next = page.next.clone();
        pages.push(page);
        let Some(next) = next else { break };
        if !seen.insert(next.as_str().to_string()) {
            break;
        }
        cursor = next;
        if pages.len() > 10_000 {
            bail!("the service did not finish the spaces list");
        }
    }
    Ok(pages)
}

/// Rows from one page. A websocket URL is not stored.
pub fn rows_of(page: &Page, fetched_at: &str) -> Vec<SpaceRow> {
    page.items
        .iter()
        .filter_map(|item| {
            let slug = item.source_id.native.trim();
            if slug.is_empty() {
                return None;
            }
            let state = item.kind.clone();
            let recording = recording_url(&item.url);
            Some(SpaceRow {
                slug: slug.to_string(),
                post_id: item.parent.as_ref().map(|id| id.native.clone()).unwrap_or_default(),
                title: item.title.clone(),
                live: state.eq_ignore_ascii_case("live"),
                scheduled_at: if state.eq_ignore_ascii_case("scheduled") { item.posted_at.clone() } else { String::new() },
                ended_at: if state.eq_ignore_ascii_case("ended") {
                    item.posted_at.clone()
                } else {
                    item.edited_at.clone().unwrap_or_default()
                },
                state,
                recording_url: recording,
                fetched_at: fetched_at.to_string(),
            })
        })
        .collect()
}

fn recording_url(url: &str) -> String {
    let url = url.trim();
    if url.is_empty() || url.starts_with("wss://") || url.starts_with("ws://") {
        return String::new();
    }
    url.to_string()
}

/// Replace the stored list with `rows`.
pub fn save_rows(rows: &[SpaceRow]) -> Result<()> {
    let c = open_db(&home()?.join(DB_REL_PATH))?;
    c.execute_batch(SPACES_SCHEMA)?;
    c.execute("DELETE FROM spaces", [])?;
    for row in rows {
        c.execute(
            "INSERT INTO spaces (slug, post_id, title, state, live, scheduled_at, ended_at, recording_url, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.slug,
                row.post_id,
                row.title,
                row.state,
                row.live as i64,
                row.scheduled_at,
                row.ended_at,
                row.recording_url,
                row.fetched_at
            ],
        )?;
    }
    Ok(())
}

/// Stored spaces, newest fetch first.
pub fn list_rows(c: &Connection) -> Result<Vec<SpaceRow>> {
    c.execute_batch(SPACES_SCHEMA)?;
    let mut st = c.prepare(
        "SELECT slug, post_id, title, state, live, scheduled_at, ended_at, recording_url, fetched_at
         FROM spaces ORDER BY fetched_at DESC, slug",
    )?;
    let rows = st.query_map([], |r| {
        Ok(SpaceRow {
            slug: r.get(0)?,
            post_id: r.get(1)?,
            title: r.get(2)?,
            state: r.get(3)?,
            live: r.get::<_, i64>(4)? != 0,
            scheduled_at: r.get(5)?,
            ended_at: r.get(6)?,
            recording_url: r.get(7)?,
            fetched_at: r.get(8)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_rs_plugin::{ItemRec, ServiceId, SourceId};

    #[test]
    fn a_live_row_keeps_the_recording_and_drops_a_socket_url() {
        let mut item = ItemRec::new(SourceId::new(ServiceId::new("redstar"), "strike-meeting"), "");
        item.kind = "live".into();
        item.title = "Strike meeting".into();
        item.url = "https://example.test/recordings/strike".into();
        item.parent = Some(SourceId::new(ServiceId::new("redstar"), "ALICE0001"));
        let mut socket = item.clone();
        socket.url = "wss://example.test/livekit?token=secret".into();
        socket.source_id = SourceId::new(ServiceId::new("redstar"), "other");
        let page = Page { items: vec![item, socket], ..Page::default() };
        let rows = rows_of(&page, "2026-10-22T00:00:00Z");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].live);
        assert_eq!(rows[0].recording_url, "https://example.test/recordings/strike");
        assert_eq!(rows[0].post_id, "ALICE0001");
        assert!(rows[1].recording_url.is_empty(), "a livekit url is not stored");
        assert!(!dry_text().contains("https://acp.us"));
        assert!(dry_text().contains("no connection opened"));
    }

    #[test]
    fn the_table_round_trips_without_a_token_column() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(SPACES_SCHEMA).unwrap();
        let row = SpaceRow {
            slug: "strike-meeting".into(),
            post_id: "ALICE0001".into(),
            title: "Strike meeting".into(),
            state: "live".into(),
            live: true,
            scheduled_at: String::new(),
            ended_at: String::new(),
            recording_url: "https://example.test/recordings/strike".into(),
            fetched_at: "2026-10-22T00:00:00Z".into(),
        };
        c.execute(
            "INSERT INTO spaces (slug, post_id, title, state, live, scheduled_at, ended_at, recording_url, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![row.slug, row.post_id, row.title, row.state, 1_i64, row.scheduled_at, row.ended_at, row.recording_url, row.fetched_at],
        )
        .unwrap();
        let got = list_rows(&c).unwrap();
        assert_eq!(got, vec![row]);
        let cols: Vec<String> = c
            .prepare("SELECT name FROM pragma_table_info('spaces')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(!cols.iter().any(|name| name.contains("token") || name.contains("livekit")));
    }
}
