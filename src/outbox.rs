//! Offline drafts and the local send queue.
//!
//! A draft is stored in the encrypted database first. Each save writes a
//! `post_versions` row. Queuing and cancelling stay on this machine.
//! Sending a row is not in this binary.

use crate::prelude::*;

/// Ask again when a queued item is older than this.
pub const STALE_SECS: i64 = 24 * 60 * 60;

/// What a queued send is allowed to do.
const SENDABLE: &[&str] = &["new_post", "reply"];

/// A post that lives on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPost {
    /// Local id.
    pub id: String,
    /// Title. Empty for a reply.
    pub title: String,
    /// Markdown body.
    pub body: String,
    /// Thread id. Empty for a new post.
    pub thread_id: String,
    /// `draft`, `local`, `queued`, or `published`.
    pub state: String,
    /// Site id, once published.
    pub site_post_id: String,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub updated_at: String,
}

/// One outbox row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Queue id.
    pub id: String,
    /// `new_post` or `reply`.
    pub action: String,
    /// JSON payload.
    pub payload: String,
    /// `queued`, `sending`, `sent`, `failed`, or `cancelled`.
    pub status: String,
    /// RFC 3339.
    pub created_at: String,
    /// When the user last confirmed, if they did.
    pub confirmed_at: String,
    /// Send attempts.
    pub attempts: i64,
    /// Last failure. Empty when there is none.
    pub last_error: String,
    /// Site id after a send.
    pub result_post_id: String,
}

fn now_stamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn open() -> Result<Connection> {
    crate::crypto::cmd::prepare_db_quiet()?;
    let c = open_db(&home()?.join(DB_REL_PATH))?;
    ensure_posts_schema(&c)?;
    Ok(c)
}

/// Read the queue when the database is already there. Does not create one.
fn open_existing() -> Result<Option<Connection>> {
    let path = home()?.join(DB_REL_PATH);
    if !path.is_file() {
        return Ok(None);
    }
    let c = open_db(&path)?;
    ensure_posts_schema(&c)?;
    Ok(Some(c))
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Save a draft or a keep-local post. The same id writes another version.
pub fn save_local(c: &mut Connection, id: &str, title: &str, body: &str, thread: &str, keep_local: bool) -> Result<String> {
    ensure_posts_schema(c)?;
    let id = id.trim();
    if id.is_empty() || id.contains(char::is_whitespace) {
        bail!("a draft needs an id");
    }
    let body = body.trim();
    if body.is_empty() {
        bail!("a draft needs a body");
    }
    let state = if keep_local { "local" } else { "draft" };
    let source = if keep_local { "local" } else { "draft" };
    let stamp = now_stamp();
    let existing: Option<(String,)> = c
        .query_row("SELECT created_at FROM local_posts WHERE id = ?1", rusqlite::params![id], |r| Ok((r.get(0)?,)))
        .optional()?;
    let created = existing.map(|t| t.0).unwrap_or_else(|| stamp.clone());
    c.execute(
        "INSERT INTO local_posts (id, title, body, thread_id, state, site_post_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET title = excluded.title, body = excluded.body, thread_id = excluded.thread_id,
            state = excluded.state, updated_at = excluded.updated_at",
        rusqlite::params![id, title.trim(), body, thread.trim(), state, created, stamp],
    )?;
    let hash = version_hash(body, &stamp);
    save_post_versions(c, &[PostVersion {
        post_id: id.to_string(),
        version_hash: hash,
        body_json: body.to_string(),
        edited_at: stamp.clone(),
        observed_at: now_stamp(),
        source: source.to_string(),
    }])?;
    let post = load_local(c, id)?;
    sync_queue_row(c, &post)?;
    let n = version_count(c, id)?;
    Ok(format!("draft {id} state={state} versions={n}"))
}

/// A draft also sits in the outbox as `draft`. Keep-local leaves the queue.
/// A row that is `sending` or `sent` is left alone.
fn sync_queue_row(c: &Connection, post: &LocalPost) -> Result<()> {
    let id = format!("Q-{}", post.id);
    let status: Option<String> = c.query_row("SELECT status FROM outbox WHERE id = ?1", rusqlite::params![id], |r| r.get(0)).optional()?;
    if post.state == "local" {
        if status.as_deref() == Some("draft") {
            c.execute("UPDATE outbox SET status = 'cancelled' WHERE id = ?1", rusqlite::params![id])?;
        }
        return Ok(());
    }
    match status.as_deref() {
        Some("sending" | "sent") => return Ok(()),
        Some("queued") => {
            let (action, payload) = queue_payload(post);
            c.execute("UPDATE outbox SET action = ?2, payload = ?3 WHERE id = ?1", rusqlite::params![id, action, payload])?;
            return Ok(());
        }
        _ => {}
    }
    let (action, payload) = queue_payload(post);
    let stamp = now_stamp();
    c.execute(
        "INSERT INTO outbox (id, action, payload, status, created_at, confirmed_at, attempts, last_error, result_post_id)
         VALUES (?1, ?2, ?3, 'draft', ?4, NULL, 0, '', NULL)
         ON CONFLICT(id) DO UPDATE SET action = excluded.action, payload = excluded.payload, status = 'draft',
            last_error = '', result_post_id = NULL",
        rusqlite::params![id, action, payload, stamp],
    )?;
    Ok(())
}

fn queue_payload(post: &LocalPost) -> (String, String) {
    let action = if post.thread_id.is_empty() { "new_post" } else { "reply" };
    let payload = json!({
        "local_id": post.id,
        "title": post.title,
        "body": post.body,
        "thread": post.thread_id,
    })
    .to_string();
    (action.to_string(), payload)
}

fn version_count(c: &Connection, id: &str) -> Result<i64> {
    Ok(c.query_row("SELECT count(*) FROM post_versions WHERE post_id = ?1", rusqlite::params![id], |r| r.get(0))?)
}

/// Drafts and keep-local posts, newest update first.
pub fn list_local(c: &Connection) -> Result<Vec<String>> {
    ensure_posts_schema(c)?;
    let mut st = c.prepare(
        "SELECT id, state, title, (SELECT count(*) FROM post_versions WHERE post_id = local_posts.id)
         FROM local_posts ORDER BY updated_at DESC, id",
    )?;
    let rows = st.query_map([], |r| Ok(format!("draft {} state={} versions={}", r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(3)?)))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    if out.is_empty() {
        out.push("draft none".into());
    }
    Ok(out)
}

fn load_local(c: &Connection, id: &str) -> Result<LocalPost> {
    c.query_row(
        "SELECT id, title, body, thread_id, state, ifnull(site_post_id, ''), created_at, updated_at FROM local_posts WHERE id = ?1",
        rusqlite::params![id],
        |r| {
            Ok(LocalPost {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                thread_id: r.get(3)?,
                state: r.get(4)?,
                site_post_id: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        },
    )
    .optional()?
    .with_context(|| format!("no local post {id}"))
}

/// Queue a local post. The outbox id is `Q-` plus the local id.
pub fn enqueue(c: &mut Connection, local_id: &str) -> Result<String> {
    ensure_posts_schema(c)?;
    let post = load_local(c, local_id.trim())?;
    if post.state == "published" {
        bail!("published post {} is not queued again", post.id);
    }
    let (action, payload) = queue_payload(&post);
    let id = format!("Q-{}", post.id);
    let existing: Option<String> = c.query_row("SELECT status FROM outbox WHERE id = ?1", rusqlite::params![id], |r| r.get(0)).optional()?;
    if let Some(status) = existing {
        if status == "queued" || status == "sending" {
            bail!("outbox {id} is already {status}");
        }
    }
    let stamp = now_stamp();
    c.execute(
        "INSERT INTO outbox (id, action, payload, status, created_at, confirmed_at, attempts, last_error, result_post_id)
         VALUES (?1, ?2, ?3, 'queued', ?4, NULL, 0, '', NULL)
         ON CONFLICT(id) DO UPDATE SET action = excluded.action, payload = excluded.payload, status = 'queued',
            created_at = excluded.created_at, confirmed_at = NULL, attempts = 0, last_error = '', result_post_id = NULL",
        rusqlite::params![id, action, payload, stamp],
    )?;
    c.execute("UPDATE local_posts SET state = 'queued', updated_at = ?2 WHERE id = ?1", rusqlite::params![post.id, stamp])?;
    Ok(format!("outbox {id} status=queued action={action}"))
}

/// Queue rows, newest first.
pub fn list_outbox(c: &Connection) -> Result<Vec<String>> {
    ensure_posts_schema(c)?;
    let mut st = c.prepare("SELECT id, status, action FROM outbox ORDER BY created_at DESC, id")?;
    let rows = st.query_map([], |r| Ok(format!("outbox {} status={} action={}", r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    if out.is_empty() {
        out.push("outbox none".into());
    }
    Ok(out)
}

/// Mark a queued or failed item cancelled. A sent item stays sent.
pub fn cancel(c: &mut Connection, id: &str) -> Result<String> {
    ensure_posts_schema(c)?;
    let item = load_item(c, id.trim())?;
    if item.status == "sent" {
        bail!("outbox {} is already sent", item.id);
    }
    if item.status == "sending" {
        bail!("outbox {} is sending; it is not cancelled until a read-back", item.id);
    }
    c.execute("UPDATE outbox SET status = 'cancelled' WHERE id = ?1", rusqlite::params![item.id])?;
    if let Some(local) = payload_local(&item.payload) {
        let _ = c.execute("UPDATE local_posts SET state = 'draft', updated_at = ?2 WHERE id = ?1 AND state = 'queued'", rusqlite::params![local, now_stamp()]);
    }
    Ok(format!("outbox {} status=cancelled", item.id))
}

fn load_item(c: &Connection, id: &str) -> Result<Item> {
    c.query_row(
        "SELECT id, action, payload, status, created_at, ifnull(confirmed_at, ''), attempts, last_error, ifnull(result_post_id, '')
         FROM outbox WHERE id = ?1",
        rusqlite::params![id],
        |r| {
            Ok(Item {
                id: r.get(0)?,
                action: r.get(1)?,
                payload: r.get(2)?,
                status: r.get(3)?,
                created_at: r.get(4)?,
                confirmed_at: r.get(5)?,
                attempts: r.get(6)?,
                last_error: r.get(7)?,
                result_post_id: r.get(8)?,
            })
        },
    )
    .optional()?
    .with_context(|| format!("no outbox item {id}"))
}

fn payload_local(payload: &str) -> Option<String> {
    serde_json::from_str::<Value>(payload).ok().and_then(|v| v.get("local_id").and_then(Value::as_str).map(str::to_string))
}

fn payload_fields(payload: &str) -> Result<(String, String, String)> {
    let v: Value = serde_json::from_str(payload).context("outbox payload")?;
    let title = v.get("title").and_then(Value::as_str).unwrap_or("").to_string();
    let body = v.get("body").and_then(Value::as_str).unwrap_or("").to_string();
    let thread = v.get("thread").and_then(Value::as_str).unwrap_or("").to_string();
    Ok((title, body, thread))
}

fn stale(created_at: &str, now: chrono::DateTime<chrono::Utc>) -> bool {
    let Ok(at) = chrono::DateTime::parse_from_rfc3339(created_at) else { return false };
    now.signed_duration_since(at).num_seconds() > STALE_SECS
}

fn find_sent(c: &Connection, body: &str, thread: &str) -> Result<Option<String>> {
    if thread.is_empty() {
        Ok(c.query_row("SELECT post_id FROM posts WHERE content = ?1 ORDER BY posted_at DESC LIMIT 1", rusqlite::params![body], |r| r.get(0)).optional()?)
    } else {
        Ok(c.query_row(
            "SELECT post_id FROM posts WHERE content = ?1 AND (parent_post_id = ?2 OR post_id = ?2) ORDER BY posted_at DESC LIMIT 1",
            rusqlite::params![body, thread],
            |r| r.get(0),
        )
        .optional()?)
    }
}

/// What `--outbox-send` should do. Sending does not leave this machine.
#[derive(Debug)]
pub enum SendStep {
    /// Print this and do not connect.
    Print(String),
}

/// Decide a send. A dry-run and a cancel print. A confirm of an enabled write marks `sending`.
pub fn prepare_send(c: &mut Connection, id: &str, confirmed: bool, dry_run: bool, now: chrono::DateTime<chrono::Utc>) -> Result<SendStep> {
    ensure_posts_schema(c)?;
    let item = load_item(c, id.trim())?;
    if !SENDABLE.contains(&item.action.as_str()) {
        bail!("outbox {} action {} is not sent. Only a text post or a reply leaves the queue.", item.id, item.action);
    }
    let (_title, body, thread) = payload_fields(&item.payload)?;
    let stale = stale(&item.created_at, now);
    let age = if stale { "older than 24 hours" } else { "within 24 hours" };
    if dry_run {
        return Ok(SendStep::Print(format!(
            "sending is not in this binary\noutbox {} status={} action={}\n{age}",
            item.id, item.status, item.action
        )));
    }
    if item.status == "sent" {
        return Ok(SendStep::Print(format!("outbox {} status=sent result={}", item.id, item.result_post_id)));
    }
    if item.status == "cancelled" {
        return Ok(SendStep::Print(format!("outbox {} status=cancelled", item.id)));
    }
    if item.status == "sending" {
        if let Some(found) = find_sent(c, &body, &thread)? {
            c.execute(
                "UPDATE outbox SET status = 'sent', result_post_id = ?2 WHERE id = ?1",
                rusqlite::params![item.id, found],
            )?;
            return Ok(SendStep::Print(format!("outbox {} status=sent result={found} read-back", item.id)));
        }
    }
    if !confirmed {
        let mut text = "nothing sent".to_string();
        if stale {
            text.push_str("\nolder than 24 hours");
        }
        if item.status == "sending" {
            text.push_str("\nread-back found no post");
        }
        return Ok(SendStep::Print(text));
    }
    if !crate::prefs::writes_enabled() {
        bail!("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent.");
    }
    Ok(SendStep::Print(crate::service::SERVICE_ABSENT.to_string()))
}

/// Finish a send after the site answers.
pub fn finish_send(c: &Connection, id: &str, result: &std::result::Result<String, String>) -> Result<()> {
    match result {
        Ok(post_id) => {
            c.execute("UPDATE outbox SET status = 'sent', result_post_id = ?2, last_error = '' WHERE id = ?1", rusqlite::params![id, post_id])?;
            if let Ok(item) = load_item(c, id) {
                if let Some(local) = payload_local(&item.payload) {
                    c.execute(
                        "UPDATE local_posts SET state = 'published', site_post_id = ?2, updated_at = ?3 WHERE id = ?1",
                        rusqlite::params![local, post_id, now_stamp()],
                    )?;
                }
            }
        }
        Err(error) => {
            let error = one_line(error);
            c.execute("UPDATE outbox SET status = 'failed', last_error = ?2 WHERE id = ?1", rusqlite::params![id, error])?;
        }
    }
    Ok(())
}

/// List drafts. An empty home says `draft none` and does not create a database.
pub fn show_drafts() -> Result<String> {
    match open_existing()? {
        None => Ok("draft none".into()),
        Some(c) => Ok(list_local(&c)?.join("\n")),
    }
}

/// List the outbox. An empty home says `outbox none` and does not create a database.
pub fn show_outbox() -> Result<String> {
    match open_existing()? {
        None => Ok("outbox none".into()),
        Some(c) => Ok(list_outbox(&c)?.join("\n")),
    }
}

/// Save a draft into the encrypted database.
pub fn save_fields(id: &str, title: &str, body: &str, thread: &str, keep_local: bool) -> Result<String> {
    let mut c = open()?;
    save_local(&mut c, id, title, body, thread, keep_local)
}

/// Queue a local post.
pub fn queue_local(id: &str) -> Result<String> {
    let mut c = open()?;
    enqueue(&mut c, id)
}

/// Cancel a queued item.
pub fn cancel_item(id: &str) -> Result<String> {
    let mut c = open()?;
    cancel(&mut c, id)
}

/// Confirm a send and mark it `sending`, or print why it did not send.
pub fn begin_send(id: &str, confirmed: bool, dry_run: bool) -> Result<SendStep> {
    let mut c = open()?;
    prepare_send(&mut c, id, confirmed, dry_run, chrono::Utc::now())
}

/// Store the result of a send that [`begin_send`] already marked `sending`.
pub fn complete_send(id: &str, result: &std::result::Result<String, String>) -> Result<()> {
    let c = open()?;
    finish_send(&c, id, result)
}

/// Whether the command line asked for the outbox or a draft.
pub fn cli_requested(args: &Args) -> bool {
    args.draft_save || args.draft_list || args.outbox_list || args.outbox_queue.is_some() || args.outbox_send.is_some() || args.outbox_cancel.is_some()
}

fn confirm(args: &Args, prompt: &str) -> Result<bool> {
    if args.dry_run {
        return Ok(false);
    }
    if args.yes {
        return Ok(true);
    }
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        eprint!("{prompt}");
        io::stderr().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        return Ok(matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"));
    }
    Ok(false)
}

/// Run a draft or outbox command. A send logs in only after confirm.
pub async fn run_cli(args: &Args) -> Result<()> {
    crate::crypto::cmd::prepare_db()?;
    if args.draft_save {
        let body = match (&args.body, &args.body_file) {
            (Some(_), Some(_)) => bail!("pass --body or --body-file, not both"),
            (Some(s), None) => s.clone(),
            (None, Some(path)) => std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
            (None, None) => bail!("a draft needs --body or --body-file"),
        };
        let id = args.draft_id.clone().unwrap_or_else(|| format!("D{:08x}", rand::random::<u32>()));
        let mut c = open()?;
        let line = save_local(&mut c, &id, args.draft_title.as_deref().unwrap_or(""), &body, args.draft_thread.as_deref().unwrap_or(""), args.keep_local)?;
        println!("{line}");
        return Ok(());
    }
    if args.draft_list {
        let c = open()?;
        for line in list_local(&c)? {
            println!("{line}");
        }
        return Ok(());
    }
    if args.outbox_list {
        let c = open()?;
        for line in list_outbox(&c)? {
            println!("{line}");
        }
        return Ok(());
    }
    if let Some(id) = args.outbox_queue.as_deref() {
        let mut c = open()?;
        println!("{}", enqueue(&mut c, id)?);
        return Ok(());
    }
    if let Some(id) = args.outbox_cancel.as_deref() {
        let mut c = open()?;
        println!("{}", cancel(&mut c, id)?);
        return Ok(());
    }
    if let Some(id) = args.outbox_send.as_deref() {
        let confirmed = confirm(args, "Send from the outbox? [y/N] ")?;
        let mut c = open()?;
        match prepare_send(&mut c, id, confirmed, args.dry_run, chrono::Utc::now())? {
            SendStep::Print(text) => {
                println!("{text}");
                Ok(())
            }
        }
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        ensure_posts_schema(&c).unwrap();
        c
    }

    #[test]
    fn a_second_save_keeps_another_version_and_the_queue_can_cancel() {
        let mut c = mem();
        let line = save_local(&mut c, "D1", "Hello", "shop notes", "", false).unwrap();
        assert!(line.contains("state=draft"), "{line}");
        assert!(line.contains("versions=1"), "{line}");
        let line = save_local(&mut c, "D1", "Hello", "shop notes revised", "", false).unwrap();
        assert!(line.contains("versions=2"), "{line}");
        let listed = list_local(&c).unwrap().join("\n");
        assert!(listed.contains("draft D1 state=draft versions=2"), "{listed}");
        let queued = enqueue(&mut c, "D1").unwrap();
        assert_eq!(queued, "outbox Q-D1 status=queued action=new_post");
        let line = save_local(&mut c, "L1", "Kept", "stays here", "", true).unwrap();
        assert!(line.contains("state=local"), "{line}");
        let sources: Vec<String> = {
            let mut st = c.prepare("SELECT source FROM post_versions WHERE post_id = 'D1' ORDER BY edited_at").unwrap();
            let rows = st.query_map([], |r| r.get(0)).unwrap();
            rows.collect::<Result<Vec<_>, _>>().unwrap()
        };
        assert!(sources.iter().all(|s| s == "draft"), "{sources:?}");
        let local_source: String = c.query_row("SELECT source FROM post_versions WHERE post_id = 'L1'", [], |r| r.get(0)).unwrap();
        assert_eq!(local_source, "local");
        assert_eq!(cancel(&mut c, "Q-D1").unwrap(), "outbox Q-D1 status=cancelled");
        let again = enqueue(&mut c, "D1").unwrap();
        assert!(again.contains("status=queued"), "{again}");
    }

    #[test]
    fn a_stale_item_and_a_crash_do_not_send_twice() {
        let mut c = mem();
        save_local(&mut c, "D1", "Hello", "shop notes", "THREAD", false).unwrap();
        enqueue(&mut c, "D1").unwrap();
        c.execute("UPDATE outbox SET created_at = '2020-01-01T00:00:00Z' WHERE id = 'Q-D1'", []).unwrap();
        let now = chrono::Utc::now();
        match prepare_send(&mut c, "Q-D1", false, true, now).unwrap() {
            SendStep::Print(text) => {
                assert!(text.contains("sending is not in this binary"), "{text}");
                assert!(text.contains("action=reply"), "{text}");
                assert!(text.contains("older than 24 hours"), "{text}");
                assert!(!text.contains("https://acp.us"), "{text}");
            }
        }
        match prepare_send(&mut c, "Q-D1", false, false, now).unwrap() {
            SendStep::Print(text) => {
                assert!(text.contains("nothing sent"), "{text}");
                assert!(text.contains("older than 24 hours"), "{text}");
            }
        }
        let status: String = c.query_row("SELECT status FROM outbox WHERE id = 'Q-D1'", [], |r| r.get(0)).unwrap();
        assert_eq!(status, "queued");
        match prepare_send(&mut c, "Q-D1", true, false, now).unwrap() {
            SendStep::Print(text) => assert!(text.contains("not in this binary"), "{text}"),
        }
        let status: String = c.query_row("SELECT status FROM outbox WHERE id = 'Q-D1'", [], |r| r.get(0)).unwrap();
        assert_eq!(status, "queued");
        c.execute("UPDATE outbox SET status = 'sending' WHERE id = 'Q-D1'", []).unwrap();
        c.execute(
            "INSERT INTO posts (post_id, parent_post_id, in_reply_to_id, nick, title, content, posted_at, body_state, body_json)
             VALUES ('POST', 'THREAD', '', 'ada', '', 'shop notes', '2026-10-21T00:00:00Z', 'full', '')",
            [],
        )
        .unwrap();
        match prepare_send(&mut c, "Q-D1", true, false, now).unwrap() {
            SendStep::Print(text) => assert!(text.contains("read-back") && text.contains("result=POST"), "{text}"),
        }
    }

    #[test]
    fn a_gated_action_is_not_sent() {
        let mut c = mem();
        c.execute(
            "INSERT INTO outbox (id, action, payload, status, created_at, attempts, last_error) VALUES ('Q-V', 'vote', '{}', 'queued', '2026-10-21T00:00:00Z', 0, '')",
            [],
        )
        .unwrap();
        let err = prepare_send(&mut c, "Q-V", true, false, chrono::Utc::now()).unwrap_err().to_string();
        assert!(err.contains("is not sent"), "{err}");
    }
}
