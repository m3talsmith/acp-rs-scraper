//! Backfill for bodies stored truncated (`--refetch-truncated`).
//!
//! Profile feeds send long posts and comments shortened: the item has
//! `body_truncated: true`, a cut `body` doc and a `body_text_summary`. The
//! thread page (`/rs/post/{thread_id}`, what "show more" opens) renders the
//! root and every reply in full, so one GET per thread completes all of its
//! stored rows. Rows stored before 1.26.0 have `body_state = 'unknown'`
//! and are checked the same way.

use crate::prelude::*;

/// Counts from [`refetch_truncated`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RefetchStats {
    /// Rows that were candidates (`body_state` not `full`).
    pub candidates: usize,
    /// Thread pages fetched.
    pub threads: usize,
    /// Rows whose stored text was shorter than the full body and was replaced.
    pub fixed: usize,
    /// Rows already complete (now marked `full`).
    pub already_full: usize,
    /// Rows not found on their thread page (deleted, or thread failed).
    pub missing: usize,
}

/// A stored row waiting for its full body.
#[derive(Debug, Clone)]
pub struct Pending {
    /// Site post id.
    pub post_id: String,
    /// Thread root id (`post_id` for roots).
    pub thread_id: String,
    /// Stored text.
    pub content: String,
}

/// Rows whose body is not known to be complete, optionally only for `nicks`
/// (case-insensitive). Ordered by thread so each thread is fetched once.
pub fn pending_bodies(db: &Path, nicks: &[String]) -> Result<Vec<Pending>> {
    if !db.exists() { return Ok(Vec::new()); }
    let c = crate::storage::db::open_db(db)?;
    ensure_posts_schema(&c)?;
    let want: HashSet<String> = nicks.iter().map(|n| n.trim_start_matches('@').to_lowercase()).collect();
    let mut st = c.prepare(
        "SELECT post_id, CASE WHEN parent_post_id = '' THEN post_id ELSE parent_post_id END, content, nick
         FROM posts WHERE body_state <> 'full' ORDER BY 2, posted_at")?;
    let rows = st.query_map([], |r| Ok((Pending { post_id: r.get(0)?, thread_id: r.get(1)?, content: r.get(2)? }, r.get::<_, String>(3)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (p, nick) = row?;
        if want.is_empty() || want.contains(&nick.to_lowercase()) { out.push(p); }
    }
    Ok(out)
}

/// Decide what to store for a row given its full thread-page item: `Some(text)`
/// when the full body differs from the stored one, `None` when it already matches.
/// Whitespace-only differences (old single-newline paragraph joins) count as
/// a change too, so paragraph breaks are restored.
pub fn completed_body(stored: &str, full: &str) -> Option<String> {
    (!full.is_empty() && full != stored).then(|| full.to_string())
}

/// Fetch the thread page of every pending row (sequentially, pausing
/// [`THREAD_FETCH_PAUSE_MS`] between threads) and store the full bodies.
pub async fn refetch_truncated(http: &reqwest::Client, jar: &Jar, db: &Path, nicks: &[String]) -> Result<RefetchStats> {
    let pending = pending_bodies(db, nicks)?;
    let mut stats = RefetchStats { candidates: pending.len(), ..Default::default() };
    let mut by_thread: Vec<(String, Vec<Pending>)> = Vec::new();
    for p in pending {
        match by_thread.last_mut() {
            Some((t, v)) if *t == p.thread_id => v.push(p),
            _ => by_thread.push((p.thread_id.clone(), vec![p])),
        }
    }
    info!("refetch: {} rows to check in {} threads", stats.candidates, by_thread.len());
    let c = crate::storage::db::open_db(db)?;
    for (i, (tid, rows)) in by_thread.iter().enumerate() {
        if i > 0 { polite_pause().await; }
        stats.threads += 1;
        let items = match fetch_thread(http, jar, tid).await {
            Ok((th, mut r)) => { if let Some(root) = th.get("root") { r.push(root.clone()); } r }
            Err(e) => { err!("warning: refetch thread {tid}: {e:#}"); stats.missing += rows.len(); continue; }
        };
        for row in rows {
            let Some(item) = items.iter().find(|x| s(x, "id") == row.post_id) else {
                verbose!("refetch: {} not on thread {tid}", row.post_id);
                stats.missing += 1;
                continue;
            };
            let parent = if row.thread_id == row.post_id { "" } else { row.thread_id.as_str() };
            let fp = to_post(item, parent, "");
            if fp.truncated { stats.missing += 1; continue; }
            match completed_body(&row.content, &fp.content) {
                Some(text) => {
                    c.execute("UPDATE posts SET content = ?1, body_state = 'full' WHERE post_id = ?2", params![text, row.post_id])?;
                    stats.fixed += 1;
                }
                None => {
                    c.execute("UPDATE posts SET body_state = 'full' WHERE post_id = ?1", params![row.post_id])?;
                    stats.already_full += 1;
                }
            }
        }
        if (i + 1) % 25 == 0 { info!("refetch: {}/{} threads", i + 1, by_thread.len()); }
    }
    info!("refetch: {} rows checked, {} fixed, {} already full, {} not found ({} threads)",
        stats.candidates, stats.fixed, stats.already_full, stats.missing, stats.threads);
    Ok(stats)
}

#[cfg(test)]
mod tests;
