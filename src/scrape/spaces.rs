//! The read-only Spaces list (`/rs/spaces` and the `space` key on a post).
//!
//! A LiveKit URL or token is a secret. It is dropped before a row is stored,
//! printed, or logged. Joining a room stays gated until a capture exists.

use crate::prelude::*;

/// Database key for the Spaces list.
pub const SPACES_KEY: &str = "spaces";

/// One stored space. No LiveKit field exists on this type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceRow {
    /// Site slug (`strike-meeting`).
    pub slug: String,
    /// Post id of the space's feed post, when the page had one.
    pub post_id: String,
    /// Title from the page, or the slug.
    pub title: String,
    /// State string the page exposed (`live`, `scheduled`, `ended`, or empty).
    pub state: String,
    /// The page said this room is live.
    pub live: bool,
    /// Scheduled start, when the page exposed one.
    pub scheduled_at: String,
    /// End time, when the page exposed one.
    pub ended_at: String,
    /// Public recording page. Empty when the page had none.
    pub recording_url: String,
    /// When this row was written (RFC 3339).
    pub fetched_at: String,
}

impl SpaceRow {
    /// One list line. A secret never appears here.
    pub fn line(&self) -> String {
        let badge = if self.live {
            "LIVE"
        } else if self.state.is_empty() {
            "space"
        } else {
            self.state.as_str()
        };
        let title = if self.title.is_empty() { self.slug.as_str() } else { self.title.as_str() };
        let mut s = format!("{badge}\t{}\t{title}", self.slug);
        if !self.scheduled_at.is_empty() {
            s.push_str(&format!("\tscheduled {}", self.scheduled_at));
        }
        if !self.ended_at.is_empty() {
            s.push_str(&format!("\tended {}", self.ended_at));
        }
        if !self.recording_url.is_empty() {
            s.push_str(&format!("\trecording {}", self.recording_url));
        }
        debug_assert!(!s.contains("wss://") && !s.contains("ws://") && !s.to_ascii_lowercase().contains("livekit"));
        s
    }
}

/// `/rs/spaces`.
pub fn spaces_url() -> String {
    format!("{}/rs/spaces", base_url())
}

/// Read `/rs/spaces`, store the posts and the space rows, and print the list.
///
/// A second run stops when the first page is already known. `--full` is a
/// scrape flag and does not apply here; the first page is always fetched.
pub async fn read_spaces(http: &reqwest::Client, jar: &Jar) -> Result<usize> {
    let url = spaces_url();
    let known = known_space_posts()?;
    let known_ref = if known.is_empty() { None } else { Some(&known) };
    let fetched = fetch_feed(http, jar, &url, None, known_ref, true)
        .await
        .with_context(|| format!("reading Spaces ({url})"))?;
    let n = remember_feed(&fetched.items, SPACES_KEY, "Spaces")?;
    let db = home()?.join(DB_REL_PATH);
    let c = open_db(&db)?;
    let mut drafts = Vec::new();
    for item in &fetched.items {
        drafts.extend(drafts_from_post(item));
    }
    for bar in spacebars(&fetched.html) {
        drafts.extend(drafts_from_bar(&bar));
    }
    let rows = save_spaces(&c, &drafts)?;
    for row in &rows {
        println!("{}", row.line());
    }
    Ok(n)
}

/// Store space objects found on crawl pages. Secrets are not written.
pub fn store_from_posts(c: &Connection, items: &[Value]) -> Result<usize> {
    let mut drafts = Vec::new();
    for item in items {
        drafts.extend(drafts_from_post(item));
    }
    if drafts.is_empty() {
        return Ok(0);
    }
    save_spaces(c, &drafts).map(|rows| rows.len())
}

/// Every space row, newest fetch last.
pub fn load_spaces(c: &Connection) -> Result<Vec<SpaceRow>> {
    if !table_exists(c, "spaces")? {
        return Ok(Vec::new());
    }
    let mut st = c.prepare(
        "SELECT slug, post_id, title, state, live, scheduled_at, ended_at, recording_url, fetched_at
         FROM spaces ORDER BY live DESC, slug",
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
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

fn known_space_posts() -> Result<HashSet<String>> {
    let db = home()?.join(DB_REL_PATH);
    if !db.is_file() {
        return Ok(HashSet::new());
    }
    let c = open_db(&db)?;
    Ok(load_site_feed(&c, SPACES_KEY)?.map(|f| f.post_ids.into_iter().collect()).unwrap_or_default())
}

fn save_spaces(c: &Connection, drafts: &[SpaceRow]) -> Result<Vec<SpaceRow>> {
    if drafts.is_empty() {
        return Ok(Vec::new());
    }
    c.execute_batch(SPACES_SCHEMA)?;
    let mut st = c.prepare(
        "INSERT INTO spaces (slug, post_id, title, state, live, scheduled_at, ended_at, recording_url, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(slug) DO UPDATE SET
            post_id = CASE WHEN excluded.post_id <> '' THEN excluded.post_id ELSE spaces.post_id END,
            title = CASE WHEN excluded.title <> '' THEN excluded.title ELSE spaces.title END,
            state = CASE WHEN excluded.state <> '' THEN excluded.state ELSE spaces.state END,
            live = CASE WHEN excluded.state <> '' THEN excluded.live
                        WHEN excluded.live <> 0 THEN 1 ELSE spaces.live END,
            scheduled_at = CASE WHEN excluded.scheduled_at <> '' THEN excluded.scheduled_at ELSE spaces.scheduled_at END,
            ended_at = CASE WHEN excluded.ended_at <> '' THEN excluded.ended_at ELSE spaces.ended_at END,
            recording_url = CASE WHEN excluded.recording_url <> '' THEN excluded.recording_url ELSE spaces.recording_url END,
            fetched_at = excluded.fetched_at",
    )?;
    for row in drafts {
        st.execute(params![
            row.slug,
            row.post_id,
            row.title,
            row.state,
            if row.live { 1 } else { 0 },
            row.scheduled_at,
            row.ended_at,
            row.recording_url,
            row.fetched_at,
        ])?;
    }
    let slugs: HashSet<&str> = drafts.iter().map(|r| r.slug.as_str()).collect();
    Ok(load_spaces(c)?.into_iter().filter(|r| slugs.contains(r.slug.as_str())).collect())
}

fn drafts_from_post(item: &Value) -> Vec<SpaceRow> {
    let post_id = s(item, "id");
    let mut out = Vec::new();
    if let Some(space) = item.get("space") {
        if let Some(row) = draft_from_space(space, &post_id, true) {
            out.push(row);
        }
    }
    if let Some(arr) = item.get("spaces").and_then(Value::as_array) {
        for space in arr {
            if let Some(row) = draft_from_space(space, &post_id, true) {
                out.push(row);
            }
        }
    }
    out
}

/// A `forum/SpaceBar` that is only a LiveKit secret is ignored.
fn drafts_from_bar(bar: &Value) -> Vec<SpaceRow> {
    draft_from_space(bar, "", false).into_iter().collect()
}

fn draft_from_space(space: &Value, post_id: &str, keep_bare_slug: bool) -> Option<SpaceRow> {
    let mut owned = space.clone();
    redact_space_secrets(&mut owned);
    let slug = match &owned {
        Value::String(s) if !s.trim().is_empty() => s.trim().to_string(),
        Value::Object(_) => object_slug(&owned)?,
        _ => return None,
    };
    let obj = owned.as_object();
    let (state, live) = obj.map(state_of).unwrap_or_else(|| (String::new(), false));
    let scheduled_at = obj.and_then(|o| text(o, &["scheduled_at", "starts_at"])).unwrap_or_default();
    let ended_at = obj.and_then(|o| text(o, &["ended_at", "ended"])).unwrap_or_default();
    let recording_url = obj.map(recording_of).unwrap_or_default();
    let title = obj.and_then(|o| text(o, &["title", "name"])).filter(|s| !s.is_empty()).unwrap_or_else(|| slug.clone());
    if !keep_bare_slug && state.is_empty() && !live && scheduled_at.is_empty() && ended_at.is_empty() && recording_url.is_empty() {
        return None;
    }
    Some(SpaceRow {
        slug,
        post_id: post_id.to_string(),
        title,
        state,
        live,
        scheduled_at,
        ended_at,
        recording_url,
        fetched_at: chrono::Local::now().to_rfc3339(),
    })
}

fn object_slug(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        let s = s.trim();
        return (!s.is_empty()).then(|| s.to_string());
    }
    for key in ["slug", "id", "name"] {
        if let Some(s) = v.get(key).and_then(Value::as_str) {
            let s = s.trim();
            if !s.is_empty() && !secret_text(s) {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn state_of(obj: &serde_json::Map<String, Value>) -> (String, bool) {
    let explicit = text(obj, &["state", "status"]).unwrap_or_default();
    let live_flag = obj.get("live").and_then(Value::as_bool).unwrap_or(false)
        || explicit.eq_ignore_ascii_case("live")
        || explicit.eq_ignore_ascii_case("live_now");
    let state = if !explicit.is_empty() {
        explicit
    } else if live_flag {
        "live".into()
    } else if text(obj, &["ended_at"]).is_some_and(|s| !s.is_empty()) {
        "ended".into()
    } else if text(obj, &["scheduled_at", "starts_at"]).is_some_and(|s| !s.is_empty()) {
        "scheduled".into()
    } else {
        String::new()
    };
    let live = live_flag || state.eq_ignore_ascii_case("live");
    (state, live)
}

fn text(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = obj.get(*key).and_then(Value::as_str) {
            let s = s.trim();
            if !s.is_empty() && !secret_text(s) {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn recording_of(obj: &serde_json::Map<String, Value>) -> String {
    for key in ["recording_url", "recordings_url", "recording"] {
        if let Some(s) = obj.get(key).and_then(Value::as_str) && public_link(s) {
            return s.to_string();
        }
        if let Some(s) = obj.get(key).and_then(|v| v.get("url")).and_then(Value::as_str) && public_link(s) {
            return s.to_string();
        }
    }
    if let Some(arr) = obj.get("recordings").and_then(Value::as_array) {
        for item in arr {
            let s = item.as_str().or_else(|| item.get("url").and_then(Value::as_str));
            if let Some(s) = s && public_link(s) {
                return s.to_string();
            }
        }
    }
    String::new()
}

fn public_link(s: &str) -> bool {
    if secret_text(s) {
        return false;
    }
    let lower = s.trim().to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")) && !lower.contains("token=")
}

fn secret_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.contains("token") || k.contains("livekit") || matches!(k.as_str(), "server_url" | "ws_url")
}

fn secret_text(s: &str) -> bool {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("wss://") || lower.starts_with("ws://") || lower.contains("livekit") {
        return true;
    }
    let parts: Vec<&str> = t.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| p.len() >= 8 && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
}

/// Remove LiveKit secrets from `v` in place.
pub fn redact_space_secrets(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.retain(|k, val| !secret_key(k) && !val.as_str().is_some_and(secret_text));
            for val in map.values_mut() {
                redact_space_secrets(val);
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_space_secrets(item);
            }
        }
        _ => {}
    }
}

/// `forum/SpaceBar` props on a page, with secrets already removed.
pub fn spacebars(html: &str) -> Vec<Value> {
    let Ok(sel) = Selector::parse(r#"div#forum\/SpaceBar[data-props]"#) else { return Vec::new() };
    let doc = Html::parse_document(html);
    let mut out = Vec::new();
    for el in doc.select(&sel) {
        let Some(raw) = el.value().attr("data-props") else { continue };
        let Ok(mut v) = serde_json::from_str::<Value>(raw) else { continue };
        redact_space_secrets(&mut v);
        out.push(v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "lk-secret-token-do-not-keep";

    #[test]
    fn a_live_room_keeps_its_recording_and_drops_the_token() {
        let raw = json!({
            "id": "SPC00001",
            "space": {
                "slug": "strike-meeting",
                "title": "Strike meeting",
                "state": "live",
                "live": true,
                "recording_url": "https://example.test/recordings/strike",
                "token": TOKEN,
                "livekit_url": "wss://livekit.example.test/room"
            }
        });
        let row = drafts_from_post(&raw).pop().unwrap();
        assert!(row.live);
        assert_eq!(row.state, "live");
        assert_eq!(row.recording_url, "https://example.test/recordings/strike");
        let line = row.line();
        assert!(line.starts_with("LIVE\tstrike-meeting"));
        assert!(line.contains("https://example.test/recordings/strike"));
        assert!(!line.contains(TOKEN));
        assert!(!line.contains("wss://"));
        let html = format!(
            r#"<div id="forum/SpaceBar" data-props="{}"></div>"#,
            esc(&json!({"slug": "strike-meeting", "token": TOKEN, "url": "wss://livekit.example.test/room"}).to_string())
        );
        let bars = spacebars(&html);
        assert_eq!(bars.len(), 1);
        let dumped = bars[0].to_string();
        assert!(!dumped.contains(TOKEN));
        assert!(!dumped.contains("wss://"));
        assert!(drafts_from_bar(&bars[0]).is_empty());
    }

    #[test]
    fn scheduled_and_ended_use_the_page_state() {
        let scheduled = json!({"id": "SPC00002", "space": {"slug": "later-forum", "state": "scheduled", "scheduled_at": "2026-10-22T15:00:00Z"}});
        let ended = json!({"id": "SPC00003", "space": {"name": "old-forum", "state": "ended", "ended_at": "2026-10-20T12:00:00Z"}});
        let s = drafts_from_post(&scheduled).pop().unwrap();
        let e = drafts_from_post(&ended).pop().unwrap();
        assert_eq!(s.state, "scheduled");
        assert!(!s.live);
        assert!(s.line().contains("scheduled"));
        assert_eq!(e.slug, "old-forum");
        assert!(e.line().contains("ended"));
        assert!(!e.line().contains(TOKEN));
    }

    #[test]
    fn a_live_row_can_end_and_the_token_is_not_stored() {
        let c = Connection::open_in_memory().unwrap();
        let live = json!({
            "id": "SPC00001",
            "space": {
                "slug": "strike-meeting",
                "title": "Strike meeting",
                "state": "live",
                "live": true,
                "recording_url": "https://example.test/recordings/strike",
                "token": TOKEN,
                "livekit_url": "wss://livekit.example.test/room"
            }
        });
        assert_eq!(store_from_posts(&c, &[live]).unwrap(), 1);
        assert!(load_spaces(&c).unwrap()[0].live);
        assert_eq!(store_from_posts(&c, &[json!({"id": "PLAIN"})]).unwrap(), 0);
        let ended = json!({
            "id": "SPC00001",
            "space": {"slug": "strike-meeting", "state": "ended", "live": false, "ended_at": "2026-10-21T19:00:00Z"}
        });
        store_from_posts(&c, &[ended]).unwrap();
        let row = load_spaces(&c).unwrap().pop().unwrap();
        assert!(!row.live);
        assert_eq!(row.state, "ended");
        assert_eq!(row.recording_url, "https://example.test/recordings/strike");
        let dumped: String = c.query_row(
            "SELECT slug || title || state || recording_url || scheduled_at || ended_at FROM spaces",
            [],
            |r| r.get(0),
        ).unwrap();
        assert!(!dumped.contains(TOKEN), "livekit token was stored");
        assert!(!dumped.contains("wss://"), "livekit url was stored");
    }

    fn esc(s: &str) -> String {
        s.replace('&', "&amp;").replace('"', "&quot;")
    }
}
