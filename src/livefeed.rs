//! Live updates and the site Notifications pane.
//!
//! Live updates follow the open feed or thread on the LiveView socket.
//! `thread:insert`, `thread:update-post`, and `feed:delete` change the local
//! rows. `feed:refresh` and the trending events raise a "new posts" chip.
//! `refresh-posts` runs at most once every 30 seconds for the rows on screen.
//!
//! `/rs/notifications` is read-only. Opening it may mark items read. Each new
//! item is stored and raised as a Sync notice. Rules run on a live reply, and
//! on a site notification whose post is already stored.

use crate::prelude::*;
use crate::service::{CapabilityId, RedStar};
use std::time::{Duration, Instant};

/// How often visible posts may be refreshed.
pub const REFRESH_GAP: Duration = Duration::from_secs(30);

/// One site notification item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteNote {
    /// Site item id.
    pub id: String,
    /// Kind the page named, such as `reply`.
    pub kind: String,
    /// Actor nick, without `@`.
    pub actor: String,
    /// Related post id, when the item has one.
    pub post_id: String,
    /// Thread id (`data-thread`).
    pub thread_id: String,
    /// Visible text.
    pub body: String,
}

/// Whether a refresh poll is due. The first poll has no `last`.
pub fn poll_due(last: Option<Instant>, now: Instant) -> bool {
    match last {
        None => true,
        Some(t) => now.saturating_duration_since(t) >= REFRESH_GAP,
    }
}

/// Events carried on a Phoenix frame or a `phx_reply` payload.
pub fn events_in(v: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    collect_events(v, &mut out);
    if let Some(frame) = v.as_array()
        && frame.len() >= 5
    {
        collect_events(&frame[4], &mut out);
    }
    out
}

fn collect_events(v: &Value, out: &mut Vec<(String, Value)>) {
    let lists = [
        v.get("e").and_then(Value::as_array),
        v.pointer("/diff/e").and_then(Value::as_array),
        v.pointer("/response/diff/e").and_then(Value::as_array),
    ];
    for list in lists.into_iter().flatten() {
        for item in list {
            let Some(pair) = item.as_array() else { continue };
            let Some(name) = pair.first().and_then(Value::as_str) else { continue };
            let payload = pair.get(1).cloned().unwrap_or(json!({}));
            if !out.iter().any(|(n, p)| n == name && p == &payload) {
                out.push((name.to_string(), payload));
            }
        }
    }
}

fn live_name(name: &str) -> bool {
    matches!(
        name,
        "thread:insert" | "thread:update-post" | "feed:delete" | "feed:refresh" | "trending-current" | "trending-latest" | "clear-app-badge"
    )
}

/// Follow `thread` : store the page, then apply socket events until `wait` elapses.
pub async fn watch_thread(http: &reqwest::Client, jar: &Jar, thread: &str, wait: Duration) -> Result<Vec<String>> {
    RedStar::require(CapabilityId::Live)?;
    let thread = thread.trim().trim_start_matches('@');
    if thread.is_empty() {
        bail!("--live needs a thread id");
    }
    let (props, replies) = fetch_thread(http, jar, thread).await?;
    let mut posts = Vec::new();
    if let Some(root) = props.get("root") {
        posts.push(to_post(root, "", ""));
    }
    for reply in &replies {
        posts.push(to_post(reply, thread, ""));
    }
    let path = home()?.join(DB_REL_PATH);
    {
        let mut db = open_db(&path)?;
        save_posts(&mut db, &posts)?;
    }
    let url = thread_url(thread);
    let html = http.get(&url).send().await?.text().await?;
    let page = parse_lv(&url, &html).with_context(|| format!("reading the live page for {thread}"))?;
    let ids = {
        let db = open_db(&path)?;
        visible_ids(&db, Some(thread))?
    };
    let mut ids = ids;
    if ids.is_empty() {
        ids.push(thread.to_string());
    }
    watch_page(jar, &page, &path, &ids, wait).await
}

/// Follow the home feed (`/rs`) and apply what the socket pushes.
pub async fn watch_feed(http: &reqwest::Client, jar: &Jar, wait: Duration) -> Result<Vec<String>> {
    RedStar::require(CapabilityId::Live)?;
    let url = format!("{}/rs", base_url());
    let html = http.get(&url).send().await?.text().await?;
    let page = parse_lv(&url, &html).context("reading the live feed")?;
    let ids = feed_ids(&html);
    let path = home()?.join(DB_REL_PATH);
    watch_page(jar, &page, &path, &ids, wait).await
}

async fn watch_page(jar: &Jar, page: &LvPage, db_path: &Path, ids: &[String], wait: Duration) -> Result<Vec<String>> {
    let mut lv = Lv::connect(page, jar).await.context("joining the live view")?;
    let mut frames = drain(&mut lv, wait.min(Duration::from_millis(500)), true).await?;
    if poll_due(None, Instant::now()) && !ids.is_empty() {
        let reply = lv
            .push("event", json!({"type": "hook", "event": "refresh-posts", "value": {"ids": ids}}))
            .await
            .context("refresh-posts")?;
        frames.push(reply);
    }
    frames.extend(drain(&mut lv, Duration::from_millis(80), false).await?);
    let _ = lv.close().await;
    let mut db = open_db(db_path)?;
    let mut lines = Vec::new();
    for frame in &frames {
        lines.extend(apply_events(&mut db, db_path, &events_in(frame))?);
    }
    Ok(lines)
}

async fn drain(lv: &mut Lv, max: Duration, stop_on_event: bool) -> Result<Vec<Value>> {
    let start = tokio::time::Instant::now();
    let mut frames = Vec::new();
    loop {
        let left = max.saturating_sub(start.elapsed());
        if left.is_zero() {
            break;
        }
        let msg = match tokio::time::timeout(left, lv.ws.next()).await {
            Err(_) => break,
            Ok(m) => m,
        };
        let text = match &msg {
            Some(Ok(Message::Text(t))) => Some(t.to_string()),
            _ => None,
        };
        match lv.absorb(msg, None).await {
            Ok(_) => {}
            Err(e) if e.to_string().contains("websocket closed") => break,
            Err(e) => return Err(e),
        }
        if let Some(text) = text
            && let Ok(v) = serde_json::from_str::<Value>(&text)
        {
            let hit = events_in(&v).iter().any(|(name, _)| live_name(name));
            frames.push(v);
            if stop_on_event && hit {
                break;
            }
        }
    }
    Ok(frames)
}

fn feed_ids(html: &str) -> Vec<String> {
    let Some(feed) = props(html, FEED_PROPS_SEL) else { return Vec::new() };
    feed["posts"]
        .as_array()
        .map(|posts| posts.iter().filter_map(|p| p.get("id").and_then(Value::as_str).map(str::to_string)).collect())
        .unwrap_or_default()
}

fn visible_ids(c: &Connection, thread: Option<&str>) -> Result<Vec<String>> {
    ensure_posts_schema(c)?;
    let mut out = Vec::new();
    if let Some(thread) = thread {
        let mut st = c.prepare(
            "SELECT post_id FROM posts WHERE post_id = ?1 OR parent_post_id = ?1 ORDER BY posted_at LIMIT 50",
        )?;
        let rows = st.query_map(params![thread], |r| r.get::<_, String>(0))?;
        for id in rows {
            out.push(id?);
        }
    }
    Ok(out)
}

/// Apply live events. Each line is `live`, the kind, and the ids.
pub fn apply_events(c: &mut Connection, db: &Path, events: &[(String, Value)]) -> Result<Vec<String>> {
    ensure_posts_schema(c)?;
    let mut lines = Vec::new();
    for (name, payload) in events {
        match name.as_str() {
            "thread:insert" => lines.push(apply_insert(c, db, payload)?),
            "thread:update-post" => lines.push(apply_update(c, payload)?),
            "feed:delete" => lines.push(apply_delete(c, payload)?),
            "feed:refresh" | "trending-current" | "trending-latest" => {
                raise(c, "Live", "new posts")?;
                lines.push("live\trefresh\tnew posts".into());
            }
            _ => {}
        }
    }
    Ok(lines)
}

fn apply_insert(c: &mut Connection, db: &Path, payload: &Value) -> Result<String> {
    let id = payload.get("id").and_then(Value::as_str).unwrap_or("");
    let thread = payload.get("thread_id").and_then(Value::as_str).unwrap_or("");
    let parent = if thread.is_empty() || thread == id { "" } else { thread };
    let post = to_post(payload, parent, "");
    if post.post_id.is_empty() {
        bail!("thread:insert had no post id");
    }
    save_posts(c, &[post.clone()])?;
    let detail = format!("new reply {} on {}", post.post_id, if parent.is_empty() { post.post_id.as_str() } else { parent });
    raise(c, "Live", &detail)?;
    let _ = crate::rules::evaluate(c, db, &[post.clone()], "live", false)?;
    Ok(format!("live\tinsert\t{}\t{}\t{}\t{}", post.post_id, if parent.is_empty() { &post.post_id } else { parent }, post.nick, one_line(&post.content)))
}

fn apply_update(c: &Connection, payload: &Value) -> Result<String> {
    let id = payload.get("id").and_then(Value::as_str).unwrap_or("");
    if id.is_empty() {
        bail!("thread:update-post had no post id");
    }
    let content = payload.get("body").map(|b| {
        let mut s = String::new();
        prosemirror_text(b, &mut s);
        s
    }).unwrap_or_default();
    let body_json = payload.get("body").filter(|b| b.is_object()).map(|b| b.to_string()).unwrap_or_default();
    let score = payload.get("score").and_then(Value::as_i64);
    let title = payload.get("title").and_then(Value::as_str).unwrap_or("");
    c.execute(
        "UPDATE posts SET
            content = CASE WHEN ?1 <> '' THEN ?1 ELSE content END,
            body_json = CASE WHEN ?2 <> '' THEN ?2 ELSE body_json END,
            score = COALESCE(?3, score),
            title = CASE WHEN ?4 <> '' THEN ?4 ELSE title END
         WHERE post_id = ?5",
        params![content, body_json, score, title, id],
    )?;
    let score_bit = score.map(|n| format!("\tscore {n}")).unwrap_or_default();
    Ok(format!("live\tupdate\t{id}{score_bit}"))
}

fn apply_delete(c: &Connection, payload: &Value) -> Result<String> {
    let id = payload.get("id").and_then(Value::as_str).unwrap_or("");
    if id.is_empty() {
        bail!("feed:delete had no post id");
    }
    c.execute("DELETE FROM posts WHERE post_id = ?1", params![id])?;
    Ok(format!("live\tdelete\t{id}"))
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

/// Read `/rs/notifications`, store the items, and mark them read when the socket says so.
pub async fn read_site_notifications(http: &reqwest::Client, jar: &Jar, wait: Duration) -> Result<Vec<String>> {
    RedStar::require(CapabilityId::SiteNotifications)?;
    let url = format!("{}/rs/notifications", base_url());
    let html = http.get(&url).send().await?.text().await?;
    let notes = parse_site_notifications(&html);
    let path = home()?.join(DB_REL_PATH);
    let db = open_db(&path)?;
    ensure_posts_schema(&db)?;
    db.execute_batch(crate::storage::schema::SITE_NOTIFICATIONS_SCHEMA)?;
    let mut fresh = Vec::new();
    for note in &notes {
        let existed: i64 = db.query_row("SELECT count(*) FROM site_notifications WHERE id = ?1", params![note.id], |r| r.get(0))?;
        db.execute(
            "INSERT INTO site_notifications (id, kind, actor, post_id, thread_id, created_at, read, body, raw)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, '')
             ON CONFLICT(id) DO UPDATE SET
               kind = excluded.kind, actor = excluded.actor, post_id = excluded.post_id,
               thread_id = excluded.thread_id, body = excluded.body",
            params![note.id, note.kind, note.actor, note.post_id, note.thread_id, chrono::Local::now().to_rfc3339(), note.body],
        )?;
        if existed == 0 {
            fresh.push(note.clone());
        }
    }
    let marked = mark_from_socket(jar, &url, &html, &db, wait).await?;
    let mut lines = Vec::new();
    if marked {
        lines.push("site-notification\tmarked read\tclear-app-badge".into());
    }
    let mut st = db.prepare("SELECT id, kind, actor, post_id, thread_id, read, body FROM site_notifications ORDER BY id")?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, String>(6)?,
        ))
    })?;
    for row in rows {
        let (id, _kind, actor, post_id, thread_id, read, body) = row?;
        let state = if read == 0 { "unread" } else { "read" };
        lines.push(format!("site-notification\t{state}\t{actor}\t{id}\t{thread_id}\t{body}"));
        let _ = (post_id, body);
    }
    for note in &fresh {
        raise(&db, "Site", &format!("{} {}", note.actor, note.body))?;
        if !note.post_id.is_empty()
            && let Ok(post) = load_post(&db, &note.post_id)
        {
            let _ = crate::rules::evaluate(&db, &path, &[post], "site-notification", false)?;
        }
    }
    Ok(lines)
}

async fn mark_from_socket(jar: &Jar, url: &str, html: &str, db: &Connection, wait: Duration) -> Result<bool> {
    let page = match parse_lv(url, html) {
        Ok(page) => page,
        Err(_) => return Ok(false),
    };
    let mut lv = match Lv::connect(&page, jar).await {
        Ok(lv) => lv,
        Err(_) => return Ok(false),
    };
    let frames = drain(&mut lv, wait.min(Duration::from_millis(500)), true).await?;
    let _ = lv.close().await;
    let marked = frames.iter().any(|frame| events_in(frame).iter().any(|(name, _)| name == "clear-app-badge"));
    if marked {
        db.execute("UPDATE site_notifications SET read = 1", [])?;
    }
    Ok(marked)
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

/// Items in a notifications page. The selector is `.js-notification`.
pub fn parse_site_notifications(html: &str) -> Vec<SiteNote> {
    let doc = Html::parse_document(html);
    let Ok(sel) = Selector::parse(".js-notification") else { return Vec::new() };
    doc.select(&sel)
        .filter_map(|el| {
            let id = el.value().attr("data-id").unwrap_or("").trim().to_string();
            if id.is_empty() {
                return None;
            }
            let body = el.text().collect::<Vec<_>>().join(" ");
            Some(SiteNote {
                id,
                kind: el.value().attr("data-kind").unwrap_or("").to_string(),
                actor: el.value().attr("data-actor").unwrap_or("").trim_start_matches('@').to_string(),
            post_id: el.value().attr("data-post").unwrap_or("").to_string(),
            thread_id: el.value().attr("data-thread").unwrap_or("").to_string(),
            body: one_line(&body),
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poll_waits_thirty_seconds() {
        let now = Instant::now();
        assert!(poll_due(None, now));
        assert!(!poll_due(Some(now), now));
        assert!(poll_due(Some(now - REFRESH_GAP), now));
    }

    #[test]
    fn an_insert_is_stored_and_a_rule_can_see_it() {
        let dir = std::env::temp_dir().join(format!("acp-live-{}-{}", std::process::id(), "insert"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rs.database");
        let mut c = open_db(&path).unwrap();
        let payload = json!({
            "id": "LIVE1",
            "thread_id": "A1",
            "author": {"username": "bob"},
            "body": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "a reply arrived live"}]}]},
            "inserted_at": "2026-10-21T20:00:00Z"
        });
        let lines = apply_events(&mut c, &path, &[("thread:insert".into(), payload)]).unwrap();
        assert!(lines[0].contains("LIVE1"), "{lines:?}");
        assert!(lines[0].contains("a reply arrived live"), "{lines:?}");
        let n: i64 = c.query_row("SELECT count(*) FROM posts WHERE post_id = 'LIVE1' AND parent_post_id = 'A1'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        let notices: i64 = c.query_row("SELECT count(*) FROM notices WHERE title = 'Live'", [], |r| r.get(0)).unwrap();
        assert_eq!(notices, 1);
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
        let again = json!({
            "id": "LIVE2",
            "thread_id": "A1",
            "author": {"username": "bob"},
            "body": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "arrived again"}]}]},
            "inserted_at": "2026-10-21T20:01:00Z"
        });
        apply_events(&mut c, &path, &[("thread:insert".into(), again)]).unwrap();
        let fired: i64 = c.query_row(
            "SELECT count(*) FROM rule_firings WHERE post_id = 'LIVE2' AND outcome = 'fired'",
            [],
            |r| r.get(0),
        )
        .unwrap();
        assert_eq!(fired, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn notifications_html_keeps_the_visible_text() {
        let html = r#"<div class="js-notification" data-id="N1" data-thread="A1" data-kind="reply" data-actor="ada">ada replied on the strike thread</div>"#;
        let notes = parse_site_notifications(html);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "N1");
        assert_eq!(notes[0].actor, "ada");
        assert_eq!(notes[0].thread_id, "A1");
        assert!(notes[0].body.contains("strike thread"));
        assert!(!html.contains("token") || !notes[0].body.contains("token"));
    }
}
