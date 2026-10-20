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
    /// Newest `posted_at` for this nick, when one is stored.
    pub latest: Option<String>,
}

/// Nick-bar filter: case-insensitive match of `q` against the nick and display name.
/// Substring matches come first, then fuzzy ones (the letters of `q` in order, e.g.
/// `hzd` → `hazaldin`); each group keeps the input order. A blank `q` keeps everything.
pub fn filter_nicks(all: &[NickEntry], q: &str) -> Vec<NickEntry> {
    let q = q.trim().trim_start_matches('@').to_lowercase();
    if q.is_empty() { return all.to_vec(); }
    let fuzzy = |s: &str| { let mut it = s.chars(); q.chars().all(|c| it.any(|x| x == c)) };
    let hay = |n: &NickEntry| (n.nick.to_lowercase(), n.display_name.as_deref().unwrap_or("").to_lowercase());
    let (mut sub, mut fz) = (Vec::new(), Vec::new());
    for n in all {
        let (a, b) = hay(n);
        if a.contains(&q) || b.contains(&q) { sub.push(n.clone()); }
        else if fuzzy(&a) || fuzzy(&b) { fz.push(n.clone()); }
    }
    sub.extend(fz);
    sub
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
    /// Which end of the match list to keep. Default newest.
    pub order: ScrapeOrder,
    /// How many matches to keep at that end. `None` keeps every match.
    pub limit: Option<usize>,
}

impl SearchFilter {
    /// No criterion set.
    pub fn is_empty(&self) -> bool {
        self.keyword.trim().is_empty() && self.nicks.is_empty() && self.post_id.trim().is_empty()
            && self.from.trim().is_empty() && self.to.trim().is_empty() && self.kind == KindFilter::Any
            && self.order == ScrapeOrder::Newest && self.limit.is_none()
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
    /// Research projects, newest first.
    fn research_projects(&self) -> Result<Vec<ResearchProject>> { Ok(Vec::new()) }
    /// Items of research project `name`, in the order added.
    fn research_items(&self, _name: &str) -> Result<Vec<ResearchItem>> { Ok(Vec::new()) }
    /// A saved site read (posts in site order), when this store has one.
    fn site_feed(&self, _key: &str) -> Result<Option<SiteFeed>> { Ok(None) }
    /// Stored edit versions of `post_id`, oldest first.
    fn post_versions(&self, _post_id: &str) -> Result<Vec<PostVersion>> { Ok(Vec::new()) }
}

/// The stored file of research item `research:<id>`, as an attachment (local files, page snapshots).
pub fn research_attachment(it: &ResearchItem) -> Option<Attachment> {
    (!it.local_path.is_empty()).then(|| Attachment {
        post_id: format!("research:{}", it.id), url: it.r#ref.clone(), kind: "file".into(),
        local_path: PathBuf::from(&it.local_path),
        mime_type: it.meta["mime_type"].as_str().or(it.meta["content_type"].as_str()).unwrap_or("application/octet-stream")
            .split(';').next().unwrap_or("").trim().to_string(),
        on_site: false,
    })
}

const COLS: &str = "post_id,parent_post_id,nick,title,content,posted_at,in_reply_to_id,body_state";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Post> {
    Ok(Post { post_id: r.get(0)?, parent_post_id: r.get(1)?, nick: r.get(2)?, title: r.get(3)?,
        content: r.get(4)?, posted_at: r.get(5)?, in_reply_to_id: r.get(6)?, media: Vec::new(),
        truncated: r.get::<_, String>(7)? == "truncated", body_json: r.get(8)?, score: r.get(9)? })
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
        let c = crate::storage::db::open_db_flags(&self.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
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
        if Self::has_col(c, "body_json") { s.push_str(",body_json"); } else { s.push_str(",''"); }
        if Self::has_col(c, "score") { s.push_str(",score"); } else { s.push_str(",NULL"); }
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
            .map(|i| NickEntry { nick: i.nick, display_name: i.display_name, post_count: i.post_count, latest: i.latest }).collect();
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
        // Attachment rows are readable before a posts table exists (an older
        // file, or a database that only has the attachments table). A missing
        // `on_site` column is treated as on the site. This store never alters.
        if !self.path.exists() { return Ok(Vec::new()); }
        let c = crate::storage::db::open_db_flags(&self.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        if let Some(n) = id.strip_prefix("research:").and_then(|n| n.parse::<i64>().ok()) {
            if !crate::maintenance::table_exists(&c, "research_items")? { return Ok(Vec::new()); }
            let pid: Option<i64> = c.query_row("SELECT project_id FROM research_items WHERE id = ?1", [n], |r| r.get(0)).optional()?;
            let Some(pid) = pid else { return Ok(Vec::new()) };
            return Ok(project_items(&c, pid)?.iter().filter(|i| i.id == n).filter_map(research_attachment).collect());
        }
        if !crate::maintenance::table_exists(&c, "attachments")? { return Ok(Vec::new()); }
        let sql = if crate::storage::db::attachments_have_on_site(&c) {
            "SELECT post_id,url,local_path,mime_type,on_site FROM attachments WHERE post_id = ?1 ORDER BY id DESC"
        } else {
            "SELECT post_id,url,local_path,mime_type,1 FROM attachments WHERE post_id = ?1 ORDER BY id DESC"
        };
        let mut st = c.prepare(sql)?;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for a in st.query_map(params![id], crate::storage::db::attachment_from_row)? {
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
    fn research_projects(&self) -> Result<Vec<ResearchProject>> {
        let Some(c) = self.conn()? else { return Ok(Vec::new()) };
        if !crate::maintenance::table_exists(&c, "research_projects")? { return Ok(Vec::new()); }
        list_projects(&c)
    }
    fn research_items(&self, name: &str) -> Result<Vec<ResearchItem>> {
        let Some(c) = self.conn()? else { return Ok(Vec::new()) };
        if !crate::maintenance::table_exists(&c, "research_items")? { return Ok(Vec::new()); }
        match project_id(&c, name)? { Some(id) => project_items(&c, id), None => Ok(Vec::new()) }
    }
    fn site_feed(&self, key: &str) -> Result<Option<SiteFeed>> {
        let Some(c) = self.conn()? else { return Ok(None) };
        load_site_feed(&c, key)
    }
    fn post_versions(&self, post_id: &str) -> Result<Vec<PostVersion>> {
        let Some(c) = self.conn()? else { return Ok(Vec::new()) };
        load_post_versions(&c, post_id)
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
    /// Research projects.
    pub projects: Vec<ResearchProject>,
    /// Research items: (project name, item).
    pub research: Vec<(String, ResearchItem)>,
    /// Saved site reads (tests and headless renders).
    pub feeds: Vec<SiteFeed>,
    /// Stored edit versions (tests and headless renders).
    pub versions: Vec<PostVersion>,
}

impl Store for MemStore {
    fn nicks(&self) -> Result<Vec<NickEntry>> {
        let mut m: Vec<NickEntry> = Vec::new();
        for p in &self.posts {
            match m.iter_mut().find(|e| e.nick.eq_ignore_ascii_case(&p.nick)) {
                Some(e) => {
                    e.post_count += 1;
                    if e.latest.as_deref().is_none_or(|l| p.posted_at.as_str() > l) { e.latest = Some(p.posted_at.clone()); }
                }
                None => m.push(NickEntry { nick: p.nick.clone(), display_name: None, post_count: 1, latest: Some(p.posted_at.clone()) }),
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
        if id.starts_with("research:") {
            return Ok(self.research.iter().filter(|(_, i)| format!("research:{}", i.id) == id).filter_map(|(_, i)| research_attachment(i)).collect());
        }
        Ok(self.atts.iter().filter(|a| a.post_id == id).cloned().collect())
    }
    fn bookmarks(&self) -> Result<HashSet<String>> { Ok(self.marks.iter().cloned().collect()) }
    fn research_projects(&self) -> Result<Vec<ResearchProject>> { Ok(self.projects.clone()) }
    fn research_items(&self, name: &str) -> Result<Vec<ResearchItem>> {
        Ok(self.research.iter().filter(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, i)| i.clone()).collect())
    }
    fn site_feed(&self, key: &str) -> Result<Option<SiteFeed>> {
        Ok(self.feeds.iter().find(|f| f.key == key).cloned())
    }
    fn post_versions(&self, post_id: &str) -> Result<Vec<PostVersion>> {
        let mut v: Vec<PostVersion> = self.versions.iter().filter(|x| x.post_id == post_id).cloned().collect();
        v.sort_by(|a, b| a.edited_at.cmp(&b.edited_at).then(a.version_hash.cmp(&b.version_hash)));
        Ok(v)
    }
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
