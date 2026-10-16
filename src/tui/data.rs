//! What the TUI reads: nicks, posts, attachments. [`SqliteStore`] opens the
//! database **read-only** (every write goes through a headless job); [`MemStore`]
//! is an in-memory stand-in for tests and headless renders.

use crate::prelude::*;

/// One nick in the left bar.
#[derive(Clone, Debug, PartialEq)]
pub struct NickEntry {
    /// Nick without `@`.
    pub nick: String,
    /// Profile display name, when scraped.
    pub display_name: Option<String>,
    /// Stored posts and replies by this nick.
    pub post_count: i64,
}

/// Which rows a search keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KindFilter {
    /// Posts and replies.
    #[default]
    Any,
    /// Thread roots only.
    Posts,
    /// Replies only.
    Replies,
}

/// Body search (Search menu or the Shift-S modal). Every field is optional; all combine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchFilter {
    /// Words in the title or body (case-insensitive substring).
    pub keyword: String,
    /// Only these nicks (empty: all).
    pub nicks: Vec<String>,
    /// A post id: that post and every reply in its thread.
    pub post_id: String,
    /// `posted_at >= from` (ISO prefix, e.g. `2026-10-01`).
    pub from: String,
    /// `posted_at <= to` (inclusive prefix, e.g. `2026-10-15` covers the whole day).
    pub to: String,
    /// Posts / replies / both.
    pub kind: KindFilter,
}

impl SearchFilter {
    /// No criterion set.
    pub fn is_empty(&self) -> bool {
        self.keyword.trim().is_empty() && self.nicks.is_empty() && self.post_id.trim().is_empty()
            && self.from.trim().is_empty() && self.to.trim().is_empty() && self.kind == KindFilter::Any
    }
    /// Whether `p` passes the filter.
    pub fn matches(&self, p: &Post) -> bool {
        let kw = self.keyword.trim().to_lowercase();
        if !kw.is_empty() && !p.title.to_lowercase().contains(&kw) && !p.content.to_lowercase().contains(&kw) { return false; }
        if !self.nicks.is_empty() && !self.nicks.iter().any(|n| n.eq_ignore_ascii_case(&p.nick)) { return false; }
        let id = self.post_id.trim();
        if !id.is_empty() && p.post_id != id && p.parent_post_id != id && p.in_reply_to_id != id { return false; }
        let (from, to) = (self.from.trim(), self.to.trim());
        if !from.is_empty() && p.posted_at.as_str() < from { return false; }
        if !to.is_empty() && p.posted_at.get(..to.len()).unwrap_or(&p.posted_at) > to { return false; }
        match self.kind {
            KindFilter::Any => true,
            KindFilter::Posts => p.parent_post_id.is_empty(),
            KindFilter::Replies => !p.parent_post_id.is_empty(),
        }
    }
    /// One-line description for the Body title.
    pub fn describe(&self) -> String {
        let mut v = Vec::new();
        if !self.keyword.trim().is_empty() { v.push(format!("\"{}\"", self.keyword.trim())); }
        if !self.nicks.is_empty() { v.push(self.nicks.iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join(",")); }
        if !self.post_id.trim().is_empty() { v.push(format!("id {}", self.post_id.trim())); }
        if !self.from.trim().is_empty() || !self.to.trim().is_empty() { v.push(format!("{}..{}", self.from.trim(), self.to.trim())); }
        match self.kind { KindFilter::Any => {}, KindFilter::Posts => v.push("posts".into()), KindFilter::Replies => v.push("replies".into()) }
        if v.is_empty() { "everything".into() } else { v.join(" · ") }
    }
}

/// Read access to the scraped data.
pub trait Store {
    /// Nicks with their post counts, most posts first.
    fn nicks(&self) -> Result<Vec<NickEntry>>;
    /// Posts and replies by any of `nicks`, newest first.
    fn posts_for(&self, nicks: &[String]) -> Result<Vec<Post>>;
    /// Posts passing `f`, newest first.
    fn search(&self, f: &SearchFilter) -> Result<Vec<Post>>;
    /// One post by id.
    fn post(&self, id: &str) -> Result<Option<Post>>;
    /// Direct replies to `id` (its `in_reply_to_id`), oldest first.
    fn replies(&self, id: &str) -> Result<Vec<Post>>;
    /// Attachments stored for `id` (one per file).
    fn attachments(&self, id: &str) -> Result<Vec<Attachment>>;
    /// Bookmarked post ids (pending removals excluded).
    fn bookmarks(&self) -> Result<HashSet<String>>;
}

const COLS: &str = "post_id,parent_post_id,nick,title,content,posted_at,in_reply_to_id,body_state";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Post> {
    Ok(Post { post_id: r.get(0)?, parent_post_id: r.get(1)?, nick: r.get(2)?, title: r.get(3)?,
        content: r.get(4)?, posted_at: r.get(5)?, in_reply_to_id: r.get(6)?, media: Vec::new(),
        truncated: r.get::<_, String>(7)? == "truncated" })
}

/// The real database, opened read-only. A missing database reads as empty.
pub struct SqliteStore {
    path: PathBuf,
}

impl SqliteStore {
    /// Store for the database at `path` (opened per query, read-only).
    pub fn new(path: PathBuf) -> Self { SqliteStore { path } }
    fn conn(&self) -> Result<Option<Connection>> {
        if !self.path.exists() { return Ok(None); }
        let c = Connection::open_with_flags(&self.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)
            .with_context(|| format!("open {} read-only", self.path.display()))?;
        if !crate::maintenance::table_exists(&c, "posts")? { return Ok(None); }
        Ok(Some(c))
    }
    fn has_col(c: &Connection, col: &str) -> bool {
        c.prepare(&format!("SELECT {col} FROM posts LIMIT 0")).is_ok()
    }
    fn cols(c: &Connection) -> String {
        let mut s = COLS.to_string();
        if !Self::has_col(c, "in_reply_to_id") { s = s.replace("in_reply_to_id", "parent_post_id"); }
        if !Self::has_col(c, "body_state") { s = s.replace("body_state", "'unknown'"); }
        s
    }
    fn query(&self, where_: &str, order: &str, p: &[&dyn rusqlite::ToSql]) -> Result<Vec<Post>> {
        let Some(c) = self.conn()? else { return Ok(Vec::new()) };
        let mut st = c.prepare(&format!("SELECT {} FROM posts WHERE {where_} ORDER BY {order}", Self::cols(&c)))?;
        let v = st.query_map(p, row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }
}

impl Store for SqliteStore {
    fn nicks(&self) -> Result<Vec<NickEntry>> {
        let Some(c) = self.conn()? else { return Ok(Vec::new()) };
        let mut v: Vec<NickEntry> = list_nicks_conn(&c)?.into_iter()
            .map(|i| NickEntry { nick: i.nick, display_name: i.display_name, post_count: i.post_count }).collect();
        v.sort_by(|a, b| b.post_count.cmp(&a.post_count).then_with(|| a.nick.to_lowercase().cmp(&b.nick.to_lowercase())));
        Ok(v)
    }
    fn posts_for(&self, nicks: &[String]) -> Result<Vec<Post>> {
        if nicks.is_empty() { return Ok(Vec::new()); }
        let want: Vec<String> = nicks.iter().map(|n| n.to_lowercase()).collect();
        let j = serde_json::to_string(&want)?;
        self.query("lower(nick) IN (SELECT value FROM json_each(?1))", "posted_at DESC", &[&j])
    }
    fn search(&self, f: &SearchFilter) -> Result<Vec<Post>> {
        let kw = f.keyword.trim();
        let like = format!("%{}%", kw.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let pre = if kw.is_empty() { "1".to_string() } else { "(title LIKE ?1 ESCAPE '\\' OR content LIKE ?1 ESCAPE '\\')".into() };
        let rows = if kw.is_empty() { self.query("1", "posted_at DESC", &[])? } else { self.query(&pre, "posted_at DESC", &[&like])? };
        Ok(rows.into_iter().filter(|p| f.matches(p)).collect())
    }
    fn post(&self, id: &str) -> Result<Option<Post>> {
        Ok(self.query("post_id = ?1", "id", &[&id])?.into_iter().next())
    }
    fn replies(&self, id: &str) -> Result<Vec<Post>> {
        let Some(c) = self.conn()? else { return Ok(Vec::new()) };
        let col = if Self::has_col(&c, "in_reply_to_id") { "in_reply_to_id" } else { "parent_post_id" };
        self.query(&format!("{col} = ?1 AND post_id <> ?1"), "posted_at", &[&id])
    }
    fn attachments(&self, id: &str) -> Result<Vec<Attachment>> {
        let Some(c) = self.conn()? else { return Ok(Vec::new()) };
        if !crate::maintenance::table_exists(&c, "attachments")? { return Ok(Vec::new()); }
        let mut st = c.prepare("SELECT post_id,url,local_path,mime_type FROM attachments WHERE post_id = ?1 ORDER BY id DESC")?;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for a in st.query_map(params![id], |r| Ok(Attachment {
            post_id: r.get(0)?, url: r.get(1)?, kind: "file".into(),
            local_path: PathBuf::from(r.get::<_, String>(2)?), mime_type: r.get(3)?,
        }))? {
            let a = a?;
            if seen.insert(crate::media::canonical_url(&a.url).to_string()) { out.push(a); }
        }
        out.reverse();
        Ok(out)
    }
    fn bookmarks(&self) -> Result<HashSet<String>> {
        let Some(c) = self.conn()? else { return Ok(HashSet::new()) };
        if !crate::maintenance::table_exists(&c, "bookmarks")? { return Ok(HashSet::new()); }
        let mut st = c.prepare("SELECT post_id FROM bookmarks WHERE deleted = 0")?;
        let v = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<HashSet<String>>>()?;
        Ok(v)
    }
}

/// In-memory store (tests, headless renders).
#[derive(Default, Clone)]
pub struct MemStore {
    /// Every post.
    pub posts: Vec<Post>,
    /// Every attachment.
    pub atts: Vec<Attachment>,
    /// Bookmarked post ids.
    pub marks: Vec<String>,
}

impl Store for MemStore {
    fn nicks(&self) -> Result<Vec<NickEntry>> {
        let mut m: Vec<NickEntry> = Vec::new();
        for p in &self.posts {
            match m.iter_mut().find(|e| e.nick.eq_ignore_ascii_case(&p.nick)) {
                Some(e) => e.post_count += 1,
                None => m.push(NickEntry { nick: p.nick.clone(), display_name: None, post_count: 1 }),
            }
        }
        m.sort_by(|a, b| b.post_count.cmp(&a.post_count).then_with(|| a.nick.cmp(&b.nick)));
        Ok(m)
    }
    fn posts_for(&self, nicks: &[String]) -> Result<Vec<Post>> {
        let mut v: Vec<Post> = self.posts.iter().filter(|p| nicks.iter().any(|n| n.eq_ignore_ascii_case(&p.nick))).cloned().collect();
        v.sort_by(|a, b| b.posted_at.cmp(&a.posted_at));
        Ok(v)
    }
    fn search(&self, f: &SearchFilter) -> Result<Vec<Post>> {
        let mut v: Vec<Post> = self.posts.iter().filter(|p| f.matches(p)).cloned().collect();
        v.sort_by(|a, b| b.posted_at.cmp(&a.posted_at));
        Ok(v)
    }
    fn post(&self, id: &str) -> Result<Option<Post>> { Ok(self.posts.iter().find(|p| p.post_id == id).cloned()) }
    fn replies(&self, id: &str) -> Result<Vec<Post>> {
        let mut v: Vec<Post> = self.posts.iter().filter(|p| p.in_reply_to_id == id && p.post_id != id).cloned().collect();
        v.sort_by(|a, b| a.posted_at.cmp(&b.posted_at));
        Ok(v)
    }
    fn attachments(&self, id: &str) -> Result<Vec<Attachment>> {
        Ok(self.atts.iter().filter(|a| a.post_id == id).cloned().collect())
    }
    fn bookmarks(&self) -> Result<HashSet<String>> { Ok(self.marks.iter().cloned().collect()) }
}

/// Post ids linked from a body: `/rs/post/{root}` and `/rs/post/{root}/{reply}` URLs
/// and `#post-{id}` anchors, in order of appearance, without duplicates.
pub fn linked_ids(text: &str) -> Vec<String> {
    use std::sync::LazyLock;
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(
        r"(?:/rs/post/([A-Za-z0-9_-]{8,})(?:/([A-Za-z0-9_-]{8,}))?)|(?:#post-([A-Za-z0-9_-]{8,}))").expect("link re"));
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for c in RE.captures_iter(text) {
        // The most specific id: the reply in /rs/post/{root}/{reply}, else the root / anchor.
        let id = c.get(2).or(c.get(1)).or(c.get(3)).map(|m| m.as_str().to_string());
        if let Some(id) = id && seen.insert(id.clone()) { out.push(id); }
    }
    out
}
