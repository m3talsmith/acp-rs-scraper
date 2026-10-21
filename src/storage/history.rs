//! Saved searches in the encrypted database.
//!
//! A row is the query and the filters. It is not a copy of the posts that matched.

use crate::prelude::*;
use rusqlite::{params, Connection};

/// `search_history` in the same database as posts and nicks.
pub const SEARCH_HISTORY_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS search_history (
            id INTEGER PRIMARY KEY,
            created_at TEXT NOT NULL,
            keyword TEXT NOT NULL,
            nicks TEXT NOT NULL,
            post_id TEXT NOT NULL,
            from_date TEXT NOT NULL,
            to_date TEXT NOT NULL,
            kind TEXT NOT NULL,
            site INTEGER NOT NULL,
            sort_order TEXT NOT NULL,
            limit_n INTEGER
        );";

/// The action-history tree, one JSON document, in the same database.
pub const ACTION_TREE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS action_tree (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            body TEXT NOT NULL
        );";

/// One saved search. `limit` `None` keeps every match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedQuery {
    /// Words in the title or body.
    pub keyword: String,
    /// Only these nicks. Empty means all.
    pub nicks: Vec<String>,
    /// That post and its thread.
    pub post_id: String,
    /// `posted_at` lower bound.
    pub from: String,
    /// `posted_at` upper bound.
    pub to: String,
    /// `any`, `posts`, or `replies`.
    pub kind: String,
    /// The search ran on the site.
    pub site: bool,
    /// `newest` or `oldest`.
    pub order: String,
    /// How many matches to keep. `None` keeps all.
    pub limit: Option<usize>,
}

impl SavedQuery {
    /// One line for the history list.
    pub fn label(&self) -> String {
        let where_ = if self.site { "site" } else { "local" };
        let body = if self.keyword.trim().is_empty() { "everything".to_string() } else { format!("\"{}\"", self.keyword.trim()) };
        format!("{where_} {body}")
    }
    /// Whether `p` passes this query. Order and limit are applied by [`SavedQuery::apply`].
    pub fn matches(&self, p: &Post) -> bool {
        let kw = self.keyword.trim().to_lowercase();
        if !kw.is_empty() && !p.title.to_lowercase().contains(&kw) && !p.content.to_lowercase().contains(&kw) { return false; }
        if !self.nicks.is_empty() && !self.nicks.iter().any(|n| n.eq_ignore_ascii_case(&p.nick)) { return false; }
        let id = self.post_id.trim();
        if !id.is_empty() && p.post_id != id && p.parent_post_id != id && p.in_reply_to_id != id { return false; }
        let (from, to) = (self.from.trim(), self.to.trim());
        if !from.is_empty() && p.posted_at.as_str() < from { return false; }
        if !to.is_empty() && p.posted_at.get(..to.len()).unwrap_or(&p.posted_at) > to { return false; }
        match self.kind.as_str() {
            "posts" => p.parent_post_id.is_empty(),
            "replies" => !p.parent_post_id.is_empty(),
            _ => true,
        }
    }
    /// Keep the matches, then the order and the limit.
    pub fn apply(&self, posts: Vec<Post>) -> Vec<Post> {
        let mut v: Vec<Post> = posts.into_iter().filter(|p| self.matches(p)).collect();
        if self.order == "oldest" {
            v.sort_by(|a, b| a.posted_at.cmp(&b.posted_at));
        } else {
            v.sort_by(|a, b| b.posted_at.cmp(&a.posted_at));
        }
        if let Some(n) = self.limit { v.truncate(n); }
        v
    }
    /// The export object. Field names are this minor. The format version stays 2.
    pub fn to_json(&self) -> Value {
        json!({
            "keyword": self.keyword,
            "nicks": self.nicks,
            "post_id": self.post_id,
            "from": self.from,
            "to": self.to,
            "kind": self.kind,
            "site": self.site,
            "order": self.order,
            "limit": self.limit,
        })
    }
    /// Parse an export object or a `--search-json` value.
    pub fn from_value(v: &Value) -> Option<Self> {
        if !v.is_object() { return None; }
        let text = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let nicks = v.get("nicks").and_then(|x| x.as_array()).map(|a| {
            a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()
        }).unwrap_or_default();
        Some(Self {
            keyword: text("keyword"),
            nicks,
            post_id: text("post_id"),
            from: text("from"),
            to: text("to"),
            kind: { let k = text("kind"); if k.is_empty() { "any".into() } else { k } },
            site: v.get("site").and_then(|x| x.as_bool()).unwrap_or(false),
            order: { let o = text("order"); if o.is_empty() { "newest".into() } else { o } },
            limit: v.get("limit").and_then(|x| x.as_u64()).map(|n| n as usize),
        })
    }
}

/// Create the tables when they are missing.
pub fn ensure(c: &Connection) -> Result<()> {
    c.execute_batch(SEARCH_HISTORY_SCHEMA)?;
    c.execute_batch(ACTION_TREE_SCHEMA)?;
    Ok(())
}

/// Replace the saved action-history document.
pub fn save_action_body(c: &Connection, body: &str) -> Result<()> {
    ensure(c)?;
    c.execute(
        "INSERT INTO action_tree (id, body) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET body = ?1",
        params![body],
    )?;
    Ok(())
}

/// The saved action-history document, when one has been written.
pub fn load_action_body(c: &Connection) -> Result<Option<String>> {
    if !crate::maintenance::table_exists(c, "action_tree")? { return Ok(None); }
    c.query_row("SELECT body FROM action_tree WHERE id = 1", [], |r| r.get(0)).optional().map_err(Into::into)
}

/// Insert `q` and, when `keep` is set, drop rows older than the newest `keep`.
pub fn insert(c: &Connection, q: &SavedQuery, keep: Option<usize>) -> Result<()> {
    ensure(c)?;
    let nicks = q.nicks.join(",");
    let now = chrono::Utc::now().to_rfc3339();
    c.execute(
        "INSERT INTO search_history (created_at, keyword, nicks, post_id, from_date, to_date, kind, site, sort_order, limit_n)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![now, q.keyword, nicks, q.post_id, q.from, q.to, q.kind, q.site, q.order, q.limit.map(|n| n as i64)],
    )?;
    if let Some(n) = keep {
        c.execute(
            "DELETE FROM search_history WHERE id NOT IN (SELECT id FROM search_history ORDER BY id DESC LIMIT ?1)",
            params![n as i64],
        )?;
    }
    Ok(())
}

/// Newest first.
pub fn list(c: &Connection) -> Result<Vec<SavedQuery>> {
    if !crate::maintenance::table_exists(c, "search_history")? { return Ok(Vec::new()); }
    let mut st = c.prepare(
        "SELECT keyword, nicks, post_id, from_date, to_date, kind, site, sort_order, limit_n
         FROM search_history ORDER BY id DESC")?;
    let rows = st.query_map([], |r| {
        let nicks: String = r.get(1)?;
        let limit: Option<i64> = r.get(8)?;
        Ok(SavedQuery {
            keyword: r.get(0)?,
            nicks: if nicks.is_empty() { Vec::new() } else { nicks.split(',').map(str::to_string).collect() },
            post_id: r.get(2)?,
            from: r.get(3)?,
            to: r.get(4)?,
            kind: r.get(5)?,
            site: r.get::<_, i64>(6)? != 0,
            order: r.get(7)?,
            limit: limit.map(|n| n as usize),
        })
    })?;
    rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
}

/// Delete the saved queries and the action-history branches. Posts, nicks, and preferences stay.
pub fn clear(c: &Connection) -> Result<()> {
    ensure(c)?;
    c.execute("DELETE FROM search_history", [])?;
    c.execute("DELETE FROM action_tree", [])?;
    Ok(())
}

/// The `search` object for an export, when this export is a search.
///
/// A full dump, a nick filter, and a basket do not get one.
pub fn export_object(args: &Args) -> Option<Value> {
    if args.post_ids.is_some() { return None; }
    if let Some(raw) = &args.search_json {
        let v: Value = serde_json::from_str(raw).ok()?;
        return SavedQuery::from_value(&v).map(|q| q.to_json());
    }
    let keyword = args.search.as_deref()?;
    let nicks = args.nick_list().unwrap_or_default();
    Some(SavedQuery {
        keyword: keyword.into(),
        nicks,
        post_id: String::new(),
        from: String::new(),
        to: String::new(),
        kind: "any".into(),
        site: false,
        order: args.order.to_string(),
        limit: args.limit,
    }.to_json())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn q(keyword: &str) -> SavedQuery {
        SavedQuery {
            keyword: keyword.into(), nicks: Vec::new(), post_id: String::new(), from: String::new(), to: String::new(),
            kind: "any".into(), site: false, order: "newest".into(), limit: None,
        }
    }

    #[test]
    fn action_branches_round_trip_and_clear_drops_them() {
        let c = Connection::open_in_memory().unwrap();
        let body = r#"{"current":1,"at":0,"branches":[{"name":"1","steps":[]}]}"#;
        save_action_body(&c, body).unwrap();
        assert_eq!(load_action_body(&c).unwrap().as_deref(), Some(body));
        insert(&c, &q("one"), None).unwrap();
        clear(&c).unwrap();
        assert!(load_action_body(&c).unwrap().is_none());
        assert!(list(&c).unwrap().is_empty());
    }

    #[test]
    fn cap_keeps_the_newest_rows_and_clear_drops_only_them() {
        let c = Connection::open_in_memory().unwrap();
        insert(&c, &q("one"), Some(2)).unwrap();
        insert(&c, &q("two"), Some(2)).unwrap();
        insert(&c, &q("three"), Some(2)).unwrap();
        let rows = list(&c).unwrap();
        assert_eq!(rows.iter().map(|r| r.keyword.as_str()).collect::<Vec<_>>(), ["three", "two"]);
        clear(&c).unwrap();
        assert!(list(&c).unwrap().is_empty());
    }

    #[test]
    fn a_search_export_carries_the_query_and_a_dump_does_not() {
        let search = Args::parse_from(["acp-red-star", "--search", "strike", "--export", "out.json", "--order", "oldest", "--limit", "5"]);
        let v = export_object(&search).unwrap();
        assert_eq!(v["keyword"], "strike");
        assert_eq!(v["order"], "oldest");
        assert_eq!(v["limit"], 5);
        let dump = Args::parse_from(["acp-red-star", "--export", "out.json"]);
        assert!(export_object(&dump).is_none());
        let nicks = Args::parse_from(["acp-red-star", "--export", "out.json", "--nicks", "alice"]);
        assert!(export_object(&nicks).is_none());
        let basket = Args::parse_from(["acp-red-star", "--export", "out.json", "--post-ids", "ALICE0001"]);
        assert!(export_object(&basket).is_none());
    }
}
