//! Bookmarks stored on this machine. They follow `--clear-all` and can be
//! joined with posts. Syncing a site list is not in this binary.

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

#[cfg(test)]
mod tests;
