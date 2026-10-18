//! Bookmarks: local first (they work offline), synced with the site's own
//! bookmarks by `--sync-bookmarks` (pull the site's list, store its posts,
//! push local additions/removals). Stored in the `bookmarks` table of the
//! database so they follow `--clear-all` and can be joined with posts.

use crate::prelude::*;

/// One stored bookmark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    /// Post id.
    pub post_id: String,
    /// When it was added locally (or first seen on the site).
    pub created_at: String,
    /// The site has it too.
    pub remote: bool,
    /// Removed locally, pending removal on the site.
    pub deleted: bool,
}

/// Open the database for bookmark writes (creating the table; waits on a busy DB).
pub fn bookmarks_conn(db: &Path) -> Result<Connection> {
    let c = open_db(db)?;
    c.busy_timeout(std::time::Duration::from_secs(10))?;
    c.execute_batch(BOOKMARKS_SCHEMA)?;
    Ok(c)
}

/// Every bookmark row (including pending removals), newest first.
pub fn load_bookmarks(c: &Connection) -> Result<Vec<Bookmark>> {
    if !crate::maintenance::table_exists(c, "bookmarks")? { return Ok(Vec::new()); }
    let mut st = c.prepare("SELECT post_id, created_at, remote, deleted FROM bookmarks ORDER BY created_at DESC, post_id")?;
    let v = st.query_map([], |r| Ok(Bookmark { post_id: r.get(0)?, created_at: r.get(1)?, remote: r.get::<_, i64>(2)? != 0, deleted: r.get::<_, i64>(3)? != 0 }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Validate a post id (base64url-ish, as the site uses).
pub fn check_post_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("invalid post id {id:?}");
    }
    Ok(())
}

/// A validated, de-duplicated post id list (`--bookmark` / `--unbookmark`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostIds(pub Vec<String>);

impl std::ops::Deref for PostIds {
    type Target = [String];
    fn deref(&self) -> &[String] { &self.0 }
}

/// Parse a comma-separated post id list (clap value parser).
pub fn parse_post_id_arg(s: &str) -> std::result::Result<PostIds, String> { parse_post_ids(s).map(PostIds) }

/// Parse a comma-separated post id list.
pub fn parse_post_ids(s: &str) -> std::result::Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let v: Vec<String> = s.split(',').map(|x| x.trim().trim_start_matches("post-").to_string())
        .filter(|x| !x.is_empty() && seen.insert(x.clone())).collect();
    if v.is_empty() { return Err("expected at least one post id".into()); }
    for id in &v { check_post_id(id).map_err(|e| e.to_string())?; }
    Ok(v)
}

/// Bookmark `ids` locally (un-deleting a pending removal). Returns how many changed.
pub fn add_bookmarks(c: &Connection, ids: &[String], now: &str) -> Result<usize> {
    let mut n = 0;
    for id in ids {
        check_post_id(id)?;
        n += c.execute("INSERT INTO bookmarks (post_id, created_at) VALUES (?1, ?2)
                        ON CONFLICT(post_id) DO UPDATE SET deleted = 0 WHERE bookmarks.deleted = 1", params![id, now])?;
    }
    Ok(n)
}

/// Remove `ids` locally: rows the site has become pending removals, local-only rows go.
pub fn remove_bookmarks(c: &Connection, ids: &[String]) -> Result<usize> {
    let mut n = 0;
    for id in ids {
        n += c.execute("UPDATE bookmarks SET deleted = 1 WHERE post_id = ?1 AND remote = 1 AND deleted = 0", params![id])?;
        n += c.execute("DELETE FROM bookmarks WHERE post_id = ?1 AND remote = 0", params![id])?;
    }
    Ok(n)
}

/// What a sync does, from the local rows and the site's current list.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncPlan {
    /// Bookmark these on the site (local additions).
    pub push_add: Vec<String>,
    /// Unbookmark these on the site (local removals).
    pub push_remove: Vec<String>,
    /// New from the site: insert as remote.
    pub insert_remote: Vec<String>,
    /// Local rows the site also has: mark remote.
    pub set_remote: Vec<String>,
    /// Rows to drop: removed on the site, or pending removals the site no longer has.
    pub delete_rows: Vec<String>,
}

/// Plan a sync. `site_complete`: the whole site list was read (only then can a
/// missing remote bookmark be treated as removed on the site). Local changes win.
pub fn plan_sync(local: &[Bookmark], site: &[String], site_complete: bool) -> SyncPlan {
    let site_set: HashSet<&String> = site.iter().collect();
    let local_ids: HashSet<&String> = local.iter().map(|b| &b.post_id).collect();
    let mut p = SyncPlan::default();
    for b in local {
        let on_site = site_set.contains(&b.post_id);
        match (b.deleted, on_site) {
            (false, true) => if !b.remote { p.set_remote.push(b.post_id.clone()) },
            (false, false) => if b.remote { if site_complete { p.delete_rows.push(b.post_id.clone()) } } else { p.push_add.push(b.post_id.clone()) },
            (true, true) => p.push_remove.push(b.post_id.clone()),
            (true, false) => p.delete_rows.push(b.post_id.clone()),
        }
    }
    for id in site { if !local_ids.contains(id) { p.insert_remote.push(id.clone()); } }
    p
}

/// Counts reported by a sync.
#[derive(Debug, Default)]
pub struct SyncStats {
    /// Bookmarks on the site.
    pub site: usize,
    /// New posts stored from the site's bookmarks.
    pub posts_saved: usize,
    /// Pushed to the site (added / removed).
    pub pushed: (usize, usize),
    /// New local rows from the site.
    pub pulled: usize,
    /// Rows dropped (removed on the site).
    pub dropped: usize,
    /// Pushes the site refused.
    pub failed: usize,
}

/// The LiveView hook that sets a bookmark: `bookmark {id, state:"1"|"0"}`.
pub fn bookmark_event(id: &str, on: bool) -> Value {
    json!({"type": "hook", "event": BOOKMARK_EVENT, "value": {"id": id, "state": if on { "1" } else { "0" }}})
}

/// Feed item → stored post (thread roots have no parent).
pub fn feed_item_post(v: &Value) -> Post {
    let id = s(v, "id");
    let tid = s(v, "thread_id");
    let parent = if tid.is_empty() || tid == id { String::new() } else { tid };
    to_post(v, &parent, "")
}

/// `--sync-bookmarks`: read the site's bookmarks (storing their posts), then push
/// local additions and removals. Needs a logged-in `http`/`jar`.
pub async fn sync_bookmarks(http: &reqwest::Client, jar: &Jar, db: &Path) -> Result<SyncStats> {
    let url = bookmarks_url();
    let page = fetch_feed(http, jar, &url, None, None, false).await
        .with_context(|| format!("reading the site's bookmarks ({url})"))?;
    let site: Vec<String> = page.items.iter().map(|v| s(v, "id")).filter(|i| !i.is_empty()).collect();
    let posts: Vec<Post> = page.items.iter().map(feed_item_post).filter(|p| !p.post_id.is_empty() && !p.nick.is_empty()).collect();
    let mut st = SyncStats { site: site.len(), posts_saved: save(&posts)?, ..Default::default() };
    let c = bookmarks_conn(db)?;
    let plan = plan_sync(&load_bookmarks(&c)?, &site, !page.stopped_incremental);
    let now = chrono::Local::now().to_rfc3339();
    for id in &plan.insert_remote { c.execute("INSERT OR IGNORE INTO bookmarks (post_id, created_at, remote) VALUES (?1, ?2, 1)", params![id, now])?; }
    for id in &plan.set_remote { c.execute("UPDATE bookmarks SET remote = 1 WHERE post_id = ?1", params![id])?; }
    for id in &plan.delete_rows { c.execute("DELETE FROM bookmarks WHERE post_id = ?1", params![id])?; }
    st.pulled = plan.insert_remote.len();
    st.dropped = plan.delete_rows.len();
    if !plan.push_add.is_empty() || !plan.push_remove.is_empty() {
        let resp = http.get(&url).send().await?.error_for_status()?;
        let final_url = resp.url().to_string();
        let lvp = parse_lv(&final_url, &resp.text().await?)?;
        let mut lv = Lv::connect(&lvp, jar).await?;
        for (id, add) in plan.push_add.iter().map(|i| (i, true)).chain(plan.push_remove.iter().map(|i| (i, false))) {
            let r = lv.push("event", bookmark_event(id, add)).await?;
            if r["status"] != "ok" { err!("warning: the site refused to {} bookmark {id}: {r}", if add { "add" } else { "remove" }); st.failed += 1; continue; }
            if add { c.execute("UPDATE bookmarks SET remote = 1 WHERE post_id = ?1", params![id])?; st.pushed.0 += 1; }
            else { c.execute("DELETE FROM bookmarks WHERE post_id = ?1", params![id])?; st.pushed.1 += 1; }
            verbose!("bookmark {id}: {} on the site", if add { "added" } else { "removed" });
        }
    }
    Ok(st)
}

#[cfg(test)]
mod tests;
