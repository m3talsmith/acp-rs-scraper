//! Live updates and the site Notifications pane.
//!
//! The core does not open a websocket. `--live`, `--live-feed`, and
//! `--site-notifications` call the loaded [`acp_rs_plugin::CapabilityId::Live`]
//! capability once. The service keeps the socket. `--live-for` is printed on
//! `--dry-run` and stored as `site_state.live_for` before that call. A second
//! call sooner than 30 seconds does not call the service.
//!
//! A new reply is stored and rules run on it. A site notification's `raw`
//! column stays empty. [`acp_rs_plugin::CapabilityId::Notification`] turns each
//! new site notification into the notice the core keeps.

use std::time::{Duration, Instant};

use acp_rs_plugin::{CapabilityId, DynService, ItemRec, NoticeRec, ServiceId, SourceId};

use crate::prelude::*;

/// How often a live read may call the service.
pub const REFRESH_GAP: Duration = Duration::from_secs(30);

/// Native id for `--live-feed`.
pub const FEED_VIEW: &str = "feed";

/// Native id for `--site-notifications`.
pub const NOTES_VIEW: &str = "site-notifications";

/// One site notification item. `raw` is not a field. The column stays empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteNote {
    /// Site item id.
    pub id: String,
    /// Kind the service named, such as `reply`.
    pub kind: String,
    /// Actor nick, without `@`.
    pub actor: String,
    /// Related post id, when the item has one.
    pub post_id: String,
    /// Thread id.
    pub thread_id: String,
    /// Visible text.
    pub body: String,
}

/// Whether a refresh is due. The first poll has no `last`.
pub fn poll_due(last: Option<Instant>, now: Instant) -> bool {
    match last {
        None => true,
        Some(t) => now.saturating_duration_since(t) >= REFRESH_GAP,
    }
}

/// `--dry-run` text. No service call and no socket.
pub fn dry_text(kind: &str, seconds: u64) -> String {
    format!("dry-run: no connection opened\n{kind}\nfor {seconds}s\nThe service keeps the socket.\n")
}

/// Seconds from `--live-for`. The default is 1.
pub fn seconds(args: &Args) -> u64 {
    args.live_for.unwrap_or(1)
}

/// These flags are the live commands.
pub fn cli_requested(args: &Args) -> bool {
    args.live.is_some() || args.live_feed || args.site_notifications || args.live_for.is_some()
}

/// Follow a thread, the feed, or the site notification pane.
pub async fn run(args: &Args) -> Result<()> {
    if args.live.is_none() && !args.live_feed && !args.site_notifications {
        bail!("--live-for needs --live, --live-feed, or --site-notifications");
    }
    let seconds = seconds(args);
    let kind = kind_line(args)?;
    if args.dry_run {
        println!("{}", dry_text(&kind, seconds));
        return Ok(());
    }
    let path = home()?.join(DB_REL_PATH);
    {
        let db = open_db(&path)?;
        if let Some(prev) = crate::storage::db::site_state_get(&db, "live_polled_at")?
            && let Ok(prev) = prev.parse::<i64>()
        {
            let now = chrono::Utc::now().timestamp();
            if now.saturating_sub(prev) < REFRESH_GAP.as_secs() as i64 {
                println!("live refresh waits 30 seconds");
                return Ok(());
            }
        }
        crate::storage::db::site_state_set(&db, "live_for", &seconds.to_string())?;
    }
    let Some(found) = crate::plugins::live(CapabilityId::Live) else {
        bail!("{}", crate::service::SERVICE_ABSENT);
    };
    let service = found.get();
    if args.site_notifications && !service.has(CapabilityId::Notification) {
        bail!("{}", crate::service::SERVICE_ABSENT);
    }
    let view = view_id(service, args)?;
    let items = service.live(&view).await.map_err(|err| anyhow!(err.to_string()))?;
    let mut db = open_db(&path)?;
    crate::storage::db::site_state_set(&db, "live_polled_at", &chrono::Utc::now().timestamp().to_string())?;
    let lines = if args.site_notifications {
        store_notes(&db, &path, service, &items).await?
    } else {
        store_items(&mut db, &path, &items)?
    };
    for line in &lines {
        println!("{line}");
    }
    Ok(())
}

fn kind_line(args: &Args) -> Result<String> {
    if let Some(id) = args.live.as_deref() {
        let id = id.trim().trim_start_matches('@');
        if id.is_empty() {
            bail!("--live needs a thread id");
        }
        return Ok(format!("live {id}"));
    }
    if args.live_feed {
        return Ok("live feed".into());
    }
    if args.site_notifications {
        return Ok("site-notifications".into());
    }
    bail!("--live-for needs --live, --live-feed, or --site-notifications");
}

fn view_id(service: &dyn DynService, args: &Args) -> Result<SourceId> {
    let native = if let Some(id) = args.live.as_deref() {
        let id = id.trim().trim_start_matches('@');
        if id.is_empty() {
            bail!("--live needs a thread id");
        }
        id.to_string()
    } else if args.live_feed {
        FEED_VIEW.to_string()
    } else {
        NOTES_VIEW.to_string()
    };
    Ok(SourceId::new(ServiceId::new(service.id()), native))
}

/// Store live items. A `delete` kind removes the local post. A new row runs rules.
pub fn store_items(c: &mut Connection, db: &Path, items: &[ItemRec]) -> Result<Vec<String>> {
    crate::storage::db::ensure_posts_schema(c)?;
    let mut lines = Vec::new();
    let mut fresh = Vec::new();
    for item in items {
        let id = item.source_id.native.trim();
        if id.is_empty() {
            continue;
        }
        if item.kind.eq_ignore_ascii_case("delete") {
            c.execute("DELETE FROM posts WHERE post_id = ?1", params![id])?;
            lines.push(format!("live\tdelete\t{id}"));
            continue;
        }
        let existed: i64 = c.query_row("SELECT count(*) FROM posts WHERE post_id = ?1", params![id], |r| r.get(0))?;
        let post = item_to_post(item);
        if existed == 0 {
            crate::storage::db::save_posts(c, &[post.clone()])?;
            let parent = if post.parent_post_id.is_empty() { post.post_id.as_str() } else { post.parent_post_id.as_str() };
            raise(c, "Live", &format!("new reply {} on {parent}", post.post_id))?;
            lines.push(format!(
                "live\tinsert\t{}\t{parent}\t{}\t{}",
                post.post_id,
                post.nick,
                one_line(&post.content)
            ));
            fresh.push(post);
        } else {
            c.execute(
                "UPDATE posts SET
                    content = CASE WHEN ?1 <> '' THEN ?1 ELSE content END,
                    body_json = CASE WHEN ?2 <> '' THEN ?2 ELSE body_json END,
                    score = COALESCE(?3, score),
                    title = CASE WHEN ?4 <> '' THEN ?4 ELSE title END
                 WHERE post_id = ?5",
                params![post.content, post.body_json, post.score, post.title, id],
            )?;
            let score_bit = post.score.map(|n| format!("\tscore {n}")).unwrap_or_default();
            lines.push(format!("live\tupdate\t{id}{score_bit}"));
        }
    }
    if !fresh.is_empty() {
        let _ = crate::rules::evaluate(c, db, &fresh, "live", false)?;
    }
    Ok(lines)
}

/// A stored post from one live item. A websocket URL is not kept as the body.
pub fn item_to_post(item: &ItemRec) -> Post {
    let id = item.source_id.native.trim().to_string();
    let parent = item.parent.as_ref().map(|p| p.native.clone()).unwrap_or_default();
    let parent = if parent == id { String::new() } else { parent };
    let reply_to = item.in_reply_to.as_ref().map(|p| p.native.clone()).filter(|p| p != &id).unwrap_or_else(|| parent.clone());
    Post {
        post_id: id,
        parent_post_id: parent,
        in_reply_to_id: reply_to,
        nick: item.nick.trim_start_matches('@').to_string(),
        title: item.title.clone(),
        content: visible_body(&item.body),
        posted_at: item.posted_at.clone(),
        media: Vec::new(),
        truncated: item.truncated,
        body_json: item.body_json.clone(),
        score: item.score,
    }
}

fn visible_body(body: &str) -> String {
    let body = body.trim();
    if body.starts_with("wss://") || body.starts_with("ws://") {
        return String::new();
    }
    body.to_string()
}

/// Map a live item to a site notification. An empty id is dropped.
pub fn note_of(item: &ItemRec) -> Option<SiteNote> {
    let id = item.source_id.native.trim();
    if id.is_empty() {
        return None;
    }
    let post_id = item.parent.as_ref().map(|p| p.native.clone()).unwrap_or_default();
    let thread_id = item
        .in_reply_to
        .as_ref()
        .map(|p| p.native.clone())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| post_id.clone());
    Some(SiteNote {
        id: id.to_string(),
        kind: item.kind.clone(),
        actor: item.nick.trim_start_matches('@').to_string(),
        post_id,
        thread_id,
        body: one_line(&item.body),
    })
}

/// Store site notifications. `raw` is written as `''`. New rows raise a notice and can fire a rule.
pub async fn store_notes(c: &Connection, db: &Path, service: &dyn DynService, items: &[ItemRec]) -> Result<Vec<String>> {
    c.execute_batch(crate::storage::schema::SITE_NOTIFICATIONS_SCHEMA)?;
    let mut fresh = Vec::new();
    for item in items {
        let Some(note) = note_of(item) else { continue };
        let existed: i64 = c.query_row("SELECT count(*) FROM site_notifications WHERE id = ?1", params![note.id], |r| r.get(0))?;
        c.execute(
            "INSERT INTO site_notifications (id, kind, actor, post_id, thread_id, created_at, read, body, raw)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, '')
             ON CONFLICT(id) DO UPDATE SET
               kind = excluded.kind, actor = excluded.actor, post_id = excluded.post_id,
               thread_id = excluded.thread_id, body = excluded.body",
            params![
                note.id,
                note.kind,
                note.actor,
                note.post_id,
                note.thread_id,
                chrono::Local::now().to_rfc3339(),
                note.body
            ],
        )?;
        if existed == 0 {
            fresh.push((note, item.source_id.clone()));
        }
    }
    let mut lines = Vec::new();
    let mut st = c.prepare("SELECT id, actor, thread_id, read, body FROM site_notifications ORDER BY id")?;
    let rows = st.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?))
    })?;
    for row in rows {
        let (id, actor, thread_id, read, body) = row?;
        let state = if read == 0 { "unread" } else { "read" };
        lines.push(format!("site-notification\t{state}\t{actor}\t{id}\t{thread_id}\t{body}"));
    }
    for (note, source) in &fresh {
        let event = NoticeRec {
            kind: note.kind.clone(),
            title: "Site".into(),
            body: format!("{} {}", note.actor, note.body),
            source_id: Some(source.clone()),
        };
        let shaped = service.notify(&event).await.map_err(|err| anyhow!(err.to_string()))?;
        let title = if shaped.title.is_empty() { "Site" } else { shaped.title.as_str() };
        let body = if shaped.body.is_empty() { event.body.as_str() } else { shaped.body.as_str() };
        raise(c, title, body)?;
        if !note.post_id.is_empty()
            && let Ok(post) = load_post(c, &note.post_id)
        {
            let _ = crate::rules::evaluate(c, db, &[post], "site-notification", false)?;
        }
    }
    Ok(lines)
}

fn load_post(c: &Connection, id: &str) -> Result<Post> {
    c.query_row(
        "SELECT post_id, parent_post_id, in_reply_to_id, nick, title, content, posted_at, body_json, score FROM posts WHERE post_id = ?1",
        params![id],
        |r| {
            Ok(Post {
                post_id: r.get(0)?,
                parent_post_id: r.get(1)?,
                in_reply_to_id: r.get(2)?,
                nick: r.get(3)?,
                title: r.get(4)?,
                content: r.get(5)?,
                posted_at: r.get(6)?,
                media: Vec::new(),
                truncated: false,
                body_json: r.get(7)?,
                score: r.get(8)?,
            })
        },
    )
    .map_err(Into::into)
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn raise(c: &Connection, title: &str, body: &str) -> Result<()> {
    let notice = crate::notify::Notice::new(crate::notify::NoticeKind::Sync, title, body);
    crate::notify::print_stderr(&notice, false);
    crate::storage::notices::insert(
        c,
        &crate::storage::notices::NoticeRow {
            id: 0,
            created_at: chrono::Local::now().to_rfc3339(),
            kind: "sync".into(),
            title: title.into(),
            body: body.into(),
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: &str, nick: &str, body: &str, parent: Option<&str>) -> ItemRec {
        let mut item = ItemRec::new(SourceId::new(ServiceId::new("redstar"), id), body);
        item.kind = kind.into();
        item.nick = nick.into();
        item.posted_at = "2026-10-22T00:00:00Z".into();
        if let Some(parent) = parent {
            item.parent = Some(SourceId::new(ServiceId::new("redstar"), parent));
        }
        item
    }

    #[test]
    fn a_poll_waits_thirty_seconds() {
        let now = Instant::now();
        assert!(poll_due(None, now));
        assert!(!poll_due(Some(now), now));
        assert!(poll_due(Some(now - REFRESH_GAP), now));
        let text = dry_text("live A1", 1);
        assert!(text.contains("no connection opened"));
        assert!(text.contains("The service keeps the socket."));
        assert!(!text.contains("https://acp.us"));
    }

    #[test]
    fn an_insert_is_stored_and_a_rule_can_see_it() {
        let dir = std::env::temp_dir().join(format!("acp-live-{}-insert", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rs.database");
        let mut c = open_db(&path).unwrap();
        let lines = store_items(&mut c, &path, &[item("LIVE1", "reply", "bob", "a reply arrived live", Some("A1"))]).unwrap();
        assert!(lines[0].contains("LIVE1"), "{lines:?}");
        assert!(lines[0].contains("a reply arrived live"), "{lines:?}");
        let n: i64 = c
            .query_row("SELECT count(*) FROM posts WHERE post_id = 'LIVE1' AND parent_post_id = 'A1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        crate::rules::insert_user(
            &c,
            &crate::rules::NewRule {
                name: "live word".into(),
                enabled: true,
                priority: 1,
                cloned_from: None,
                template_key: None,
                trigger: crate::rules::Trigger::PostSynced,
                conditions: vec![crate::rules::Condition::Keyword { text: "arrived".into() }],
                actions: vec![crate::rules::RuleAction::Notify],
            },
        )
        .unwrap();
        store_items(&mut c, &path, &[item("LIVE2", "reply", "bob", "arrived again", Some("A1"))]).unwrap();
        let fired: i64 = c
            .query_row("SELECT count(*) FROM rule_firings WHERE post_id = 'LIVE2' AND outcome = 'fired'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fired, 1);
        let deleted = store_items(&mut c, &path, &[item("LIVE1", "delete", "", "", None)]).unwrap();
        assert!(deleted[0].contains("delete"), "{deleted:?}");
        let left: i64 = c.query_row("SELECT count(*) FROM posts WHERE post_id = 'LIVE1'", [], |r| r.get(0)).unwrap();
        assert_eq!(left, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_notification_keeps_the_text_and_leaves_raw_empty() {
        let row = note_of(&item("N1", "reply", "@ada", "ada replied on the strike thread", Some("A1"))).unwrap();
        assert_eq!(row.actor, "ada");
        assert_eq!(row.thread_id, "A1");
        assert!(row.body.contains("strike thread"));
        let socket = item("N2", "reply", "ada", "wss://example.test/socket?token=secret", Some("A1"));
        let post = item_to_post(&socket);
        assert!(post.content.is_empty(), "a socket url is not stored as the body");
        let dir = std::env::temp_dir().join(format!("acp-live-{}-note", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let c = open_db(&dir.join("rs.database")).unwrap();
        c.execute_batch(crate::storage::schema::SITE_NOTIFICATIONS_SCHEMA).unwrap();
        c.execute(
            "INSERT INTO site_notifications (id, kind, actor, post_id, thread_id, created_at, read, body, raw)
             VALUES ('N1', 'reply', 'ada', 'A1', 'A1', 't', 0, 'ada replied', '')",
            [],
        )
        .unwrap();
        let raw: String = c.query_row("SELECT raw FROM site_notifications WHERE id = 'N1'", [], |r| r.get(0)).unwrap();
        assert!(raw.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
