//! Research projects: named collections of posts, replies, search results,
//! local files and web pages, each with an optional note, exported together
//! (JSON + HTML, or a bundle with the files and page snapshots).
//!
//! Stored in the `research_projects` / `research_items` tables of the main
//! database. Local files are copied to `~/.acp/research/<project>/files/`
//! (unless `--no-copy`); web pages are fetched once and kept as an HTML
//! snapshot in `~/.acp/research/<project>/web/` plus their readable text.

use crate::prelude::*;

pub mod cli;
pub use cli::*;

/// What a research item points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// A thread root post (ref = post id).
    Post,
    /// A reply (ref = post id).
    Reply,
    /// A post or reply added from search results (ref = post id; meta has the query).
    SearchResult,
    /// A file on the local drive (ref = its absolute path).
    LocalFile,
    /// A web page (ref = its URL).
    WebUrl,
}

impl ItemKind {
    /// Name stored in the database.
    pub fn as_str(self) -> &'static str {
        match self { ItemKind::Post => "post", ItemKind::Reply => "reply", ItemKind::SearchResult => "search-result",
            ItemKind::LocalFile => "local-file", ItemKind::WebUrl => "web-url" }
    }
    /// Parse a stored name.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s { "post" => ItemKind::Post, "reply" => ItemKind::Reply, "search-result" => ItemKind::SearchResult,
            "local-file" => ItemKind::LocalFile, "web-url" => ItemKind::WebUrl, _ => return None })
    }
    /// Whether the ref is a post id.
    pub fn is_post(self) -> bool { matches!(self, ItemKind::Post | ItemKind::Reply | ItemKind::SearchResult) }
}

/// A project with its item count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchProject {
    /// Row id.
    pub id: i64,
    /// Unique name (case-insensitive).
    pub name: String,
    /// Free text.
    pub description: String,
    /// RFC 3339.
    pub created_at: String,
    /// Items in it.
    pub items: usize,
}

/// One item of a project.
#[derive(Clone, Debug, PartialEq)]
pub struct ResearchItem {
    /// Row id.
    pub id: i64,
    /// Kind.
    pub kind: ItemKind,
    /// Post id, absolute path or URL.
    pub r#ref: String,
    /// Title (post snippet, file name, page title).
    pub title: String,
    /// The user's note.
    pub note: String,
    /// Extra facts (JSON object): size, mime, query, fetched_at, …
    pub meta: Value,
    /// Readable text (web pages, text files).
    pub content: String,
    /// Copied file / page snapshot ("" for posts).
    pub local_path: String,
    /// RFC 3339.
    pub added_at: String,
}

/// An item to add.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewItem {
    /// Kind.
    pub kind: Option<ItemKind>,
    /// Ref.
    pub r#ref: String,
    /// Title.
    pub title: String,
    /// Note.
    pub note: String,
    /// Meta (an object).
    pub meta: Value,
    /// Readable text.
    pub content: String,
    /// Local copy.
    pub local_path: String,
}

/// Open the database for research writes (creating the tables).
pub fn research_conn(db: &Path) -> Result<Connection> {
    let c = open_db(db)?;
    c.execute_batch("PRAGMA foreign_keys = ON;")?;
    c.execute_batch(RESEARCH_SCHEMA)?;
    Ok(c)
}

fn now() -> String { chrono::Local::now().to_rfc3339() }

/// Check a project name: 1–80 characters, no control characters.
pub fn check_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 80 || n.chars().any(char::is_control) {
        bail!("research project names need 1-80 printable characters (got {name:?})");
    }
    Ok(n.to_string())
}

/// Create a project; an existing name is an error.
pub fn create_project(c: &Connection, name: &str, description: &str) -> Result<i64> {
    let name = check_name(name)?;
    if project_id(c, &name)?.is_some() { bail!("research project {name:?} already exists"); }
    c.execute("INSERT INTO research_projects (name, description, created_at) VALUES (?1, ?2, ?3)", params![name, description.trim(), now()])?;
    Ok(c.last_insert_rowid())
}

/// The id of project `name` (case-insensitive).
pub fn project_id(c: &Connection, name: &str) -> Result<Option<i64>> {
    if !crate::maintenance::table_exists(c, "research_projects")? { return Ok(None); }
    Ok(c.query_row("SELECT id FROM research_projects WHERE name = ?1 COLLATE NOCASE", [name.trim()], |r| r.get(0)).optional()?)
}

/// The project `name`, created (with `description`) when missing; true when created.
pub fn get_or_create(c: &Connection, name: &str, description: Option<&str>) -> Result<(i64, bool)> {
    match project_id(c, name)? {
        Some(id) => {
            if let Some(d) = description.filter(|d| !d.trim().is_empty()) {
                c.execute("UPDATE research_projects SET description = ?1 WHERE id = ?2", params![d.trim(), id])?;
            }
            Ok((id, false))
        }
        None => Ok((create_project(c, name, description.unwrap_or(""))?, true)),
    }
}

/// Every project, newest first, with item counts.
pub fn list_projects(c: &Connection) -> Result<Vec<ResearchProject>> {
    if !crate::maintenance::table_exists(c, "research_projects")? { return Ok(Vec::new()); }
    let mut st = c.prepare("SELECT p.id, p.name, p.description, p.created_at,
            (SELECT count(*) FROM research_items i WHERE i.project_id = p.id)
        FROM research_projects p ORDER BY p.created_at DESC, p.id DESC")?;
    let v = st.query_map([], |r| Ok(ResearchProject { id: r.get(0)?, name: r.get(1)?, description: r.get(2)?, created_at: r.get(3)?,
        items: r.get::<_, i64>(4)? as usize }))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Project `name` (case-insensitive).
pub fn get_project(c: &Connection, name: &str) -> Result<Option<ResearchProject>> {
    Ok(list_projects(c)?.into_iter().find(|p| p.name.eq_ignore_ascii_case(name.trim())))
}

/// Add an item; false when the project already has it (its note is updated if a new one is given).
pub fn add_item(c: &Connection, project: i64, it: &NewItem) -> Result<bool> {
    let kind = it.kind.ok_or_else(|| anyhow!("research item without a kind"))?;
    let meta = if it.meta.is_object() { it.meta.to_string() } else { "{}".into() };
    let n = c.execute("INSERT OR IGNORE INTO research_items (project_id, kind, ref, title, note, meta, content, local_path, added_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![project, kind.as_str(), it.r#ref, it.title, it.note, meta, it.content, it.local_path, now()])?;
    if n == 0 && !it.note.trim().is_empty() {
        c.execute("UPDATE research_items SET note = ?1 WHERE project_id = ?2 AND kind = ?3 AND ref = ?4",
            params![it.note, project, kind.as_str(), it.r#ref])?;
    }
    Ok(n > 0)
}

/// A project's items, in the order added.
pub fn project_items(c: &Connection, project: i64) -> Result<Vec<ResearchItem>> {
    if !crate::maintenance::table_exists(c, "research_items")? { return Ok(Vec::new()); }
    let mut st = c.prepare("SELECT id, kind, ref, title, note, meta, content, local_path, added_at
        FROM research_items WHERE project_id = ?1 ORDER BY added_at, id")?;
    let v = st.query_map([project], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
        r.get::<_, String>(5)?, r.get(6)?, r.get(7)?, r.get(8)?)))?
        .filter_map(|x| x.ok())
        .filter_map(|(id, k, r#ref, title, note, meta, content, local_path, added_at)| Some(ResearchItem {
            id, kind: ItemKind::parse(&k)?, r#ref, title, note, meta: serde_json::from_str(&meta).unwrap_or_else(|_| json!({})),
            content, local_path, added_at }))
        .collect();
    Ok(v)
}

/// `~/.acp/research/<slug>/`.
pub fn project_dir(name: &str) -> Result<PathBuf> { Ok(home()?.join(RESEARCH_REL_DIR).join(slug(name))) }

/// Short single-line title for a post.
pub fn post_title(p: &Post) -> String {
    let t = if p.title.trim().is_empty() { p.content.as_str() } else { p.title.as_str() };
    let one: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s: String = one.chars().take(90).collect();
    if one.chars().count() > 90 { s.push('…'); }
    format!("@{}: {s}", p.nick)
}

/// Items for posts already in the database (`kind` None = post/reply by the post itself).
pub fn post_items(db: &Path, ids: &[String], kind: Option<ItemKind>, meta: Value, note: &str) -> Result<(Vec<NewItem>, Vec<String>)> {
    let (posts, _, missing) = posts_by_ids(db, ids)?;
    let v = posts.iter().map(|p| NewItem {
        kind: Some(kind.unwrap_or(if p.parent_post_id.is_empty() { ItemKind::Post } else { ItemKind::Reply })),
        r#ref: p.post_id.clone(), title: post_title(p), note: note.into(), meta: meta.clone(), ..NewItem::default()
    }).collect();
    Ok((v, missing))
}

/// MIME type from a file extension (enough for exports and opening).
pub fn mime_for(p: &Path) -> &'static str {
    match p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).as_deref() {
        Some("txt" | "log") => "text/plain", Some("md" | "markdown") => "text/markdown", Some("csv") => "text/csv",
        Some("json") => "application/json", Some("html" | "htm") => "text/html", Some("xml") => "application/xml",
        Some("pdf") => "application/pdf", Some("png") => "image/png", Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif", Some("webp") => "image/webp", Some("avif") => "image/avif", Some("svg") => "image/svg+xml",
        Some("mp4") => "video/mp4", Some("webm") => "video/webm", Some("mp3") => "audio/mpeg", Some("ogg") => "audio/ogg",
        Some("doc") => "application/msword", Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("odt") => "application/vnd.oasis.opendocument.text", Some("epub") => "application/epub+zip",
        Some("zip") => "application/zip", Some("gz" | "tgz") => "application/gzip",
        _ => "application/octet-stream",
    }
}

/// A path as shown on screen.
///
/// A path under the home directory is `~/…`. A path under the working
/// directory is relative. A value that already starts with `~/` stays as typed.
/// The stored path is not changed.
pub fn display_path(path: &str) -> String {
    let raw = path.trim();
    if raw == "~" || raw.starts_with("~/") { return raw.to_string(); }
    let p = Path::new(raw);
    if let Some(home) = std::env::var_os("HOME") {
        let home = Path::new(&home);
        if let Ok(rest) = p.strip_prefix(home) {
            let rest = rest.display().to_string();
            return if rest.is_empty() { "~".into() } else { format!("~/{rest}") };
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Ok(rest) = p.strip_prefix(&cwd) {
            let rest = rest.display().to_string();
            if !rest.is_empty() { return rest; }
        }
    }
    raw.to_string()
}

/// Text files small enough to keep their content searchable in the export.
const TEXT_MAX: u64 = 2 * 1024 * 1024;

/// An item for a local file: metadata, its text (for text files), and a copy
/// under the project's `files/` unless `copy` is false.
pub fn file_item(project: &str, path: &Path, copy: bool, note: &str) -> Result<NewItem> {
    let abs = std::fs::canonicalize(path).with_context(|| format!("{}", path.display()))?;
    let md = std::fs::metadata(&abs)?;
    if !md.is_file() { bail!("{} is not a file", abs.display()); }
    let name = abs.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mime = mime_for(&abs);
    let modified = md.modified().ok().map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339());
    let content = if (mime.starts_with("text/") || mime == "application/json" || mime == "application/xml") && md.len() <= TEXT_MAX {
        let raw = String::from_utf8_lossy(&std::fs::read(&abs)?).into_owned();
        if mime == "text/html" { readable(&raw).1 } else { raw }
    } else { String::new() };
    let local = if copy {
        let mut h = DefaultHasher::new();
        abs.hash(&mut h);
        let dir = project_dir(project)?.join("files");
        std::fs::create_dir_all(&dir)?;
        let dst = dir.join(format!("{:08x}-{name}", h.finish() as u32));
        std::fs::copy(&abs, &dst).with_context(|| format!("copy {} to {}", abs.display(), dst.display()))?;
        dst
    } else { abs.clone() };
    Ok(NewItem { kind: Some(ItemKind::LocalFile), r#ref: abs.display().to_string(), title: name, note: note.into(),
        meta: json!({ "size": md.len(), "mime_type": mime, "modified": modified, "copied": copy, "original_path": abs.display().to_string() }),
        content, local_path: local.display().to_string() })
}

/// Largest web page or file fetched for a research item.
pub const WEB_MAX: usize = 20 * 1024 * 1024;

/// Fetch `url` (no acp.us cookies are sent) into an item: the page title, its
/// readable text and an HTML snapshot (other content types are saved as a file).
pub async fn web_item(project: &str, url: &str, note: &str) -> Result<NewItem> {
    let u = url::Url::parse(url.trim()).with_context(|| format!("not a URL: {url}"))?;
    if !matches!(u.scheme(), "http" | "https") { bail!("only http and https URLs can be added (got {})", u.scheme()); }
    let http = client(Arc::new(Jar::default()))?;
    let resp = http.get(u.clone()).send().await.with_context(|| format!("fetching {u}"))?;
    let status = resp.status();
    let final_url = resp.url().to_string();
    let ctype = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    if !status.is_success() { bail!("{u}: HTTP {status}"); }
    let bytes = resp.bytes().await?;
    if bytes.len() > WEB_MAX { bail!("{u}: {} bytes is more than the {WEB_MAX}-byte limit", bytes.len()); }
    let mut h = DefaultHasher::new();
    u.as_str().hash(&mut h);
    let dir = project_dir(project)?.join("web");
    std::fs::create_dir_all(&dir)?;
    let is_html = ctype.contains("html") || (ctype.is_empty() && bytes.starts_with(b"<"));
    let (title, content, file) = if is_html {
        let raw = String::from_utf8_lossy(&bytes).into_owned();
        let (t, text) = readable(&raw);
        let f = dir.join(format!("{:016x}.html", h.finish()));
        std::fs::write(&f, raw.as_bytes())?;
        (t, text, f)
    } else {
        let name = u.path_segments().and_then(|mut s| s.next_back()).filter(|s| !s.is_empty()).unwrap_or("download").to_string();
        let f = dir.join(format!("{:08x}-{name}", h.finish() as u32));
        std::fs::write(&f, &bytes)?;
        let text = if ctype.starts_with("text/") { String::from_utf8_lossy(&bytes).into_owned() } else { String::new() };
        (name, text, f)
    };
    let title = if title.trim().is_empty() { u.host_str().unwrap_or("web page").to_string() } else { title };
    Ok(NewItem { kind: Some(ItemKind::WebUrl), r#ref: u.to_string(), title, note: note.into(),
        meta: json!({ "status": status.as_u16(), "content_type": ctype, "final_url": final_url, "bytes": bytes.len(), "fetched_at": now() }),
        content, local_path: file.display().to_string() })
}

/// Title and readable text of an HTML page: headings (as `#` Markdown),
/// paragraphs, list items, quotes and code from `<article>`, else `<main>`,
/// else `<body>`, skipping navigation, headers, footers, scripts and forms.
pub fn readable(html: &str) -> (String, String) {
    let doc = Html::parse_document(html);
    let sel = |s: &str| Selector::parse(s).expect("selector");
    // Inline text runs join as written (`in <b>X</b>.` stays `in X.`), then whitespace collapses.
    let text_of = |e: scraper::ElementRef| e.text().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    let title = doc.select(&sel("meta[property='og:title']")).next().and_then(|m| m.value().attr("content").map(str::to_string))
        .or_else(|| doc.select(&sel("title")).next().map(text_of))
        .or_else(|| doc.select(&sel("h1")).next().map(text_of))
        .unwrap_or_default();
    let root = doc.select(&sel("article")).next().or_else(|| doc.select(&sel("main")).next()).or_else(|| doc.select(&sel("body")).next());
    let Some(root) = root else { return (title, String::new()) };
    let blocks = ["h1", "h2", "h3", "h4", "h5", "h6", "p", "li", "blockquote", "pre"];
    let skip = ["nav", "header", "footer", "aside", "script", "style", "form", "noscript", "template"];
    let mut out: Vec<String> = Vec::new();
    for n in root.descendants() {
        let Some(e) = scraper::ElementRef::wrap(n) else { continue };
        let name = e.value().name();
        if !blocks.contains(&name) { continue; }
        let mut skipped = false;
        let mut inside_block = false;
        for a in e.ancestors() {
            let Some(ae) = scraper::ElementRef::wrap(a) else { continue };
            if a.id() == root.id() { break; }
            let an = ae.value().name();
            if skip.contains(&an) { skipped = true; break; }
            if blocks.contains(&an) { inside_block = true; }
        }
        if skipped || inside_block { continue; }
        let t = if name == "pre" { e.text().collect::<String>() } else { text_of(e) };
        if t.trim().is_empty() { continue; }
        out.push(match name {
            h if h.starts_with('h') => format!("{} {}", "#".repeat(h[1..].parse().unwrap_or(2)), t),
            "li" => format!("- {t}"),
            "blockquote" => format!("> {t}"),
            "pre" => format!("```\n{}\n```", t.trim_end()),
            _ => t,
        });
    }
    (title.trim().to_string(), out.join("\n\n"))
}

/// The export document's `research` section.
pub fn research_json(p: &ResearchProject, items: &[ResearchItem], rel: &dyn Fn(&ResearchItem) -> Option<String>) -> Value {
    json!({
        "name": p.name, "description": p.description, "created_at": p.created_at,
        "items": items.iter().map(|i| json!({
            "id": i.id, "kind": i.kind.as_str(), "ref": i.r#ref, "title": i.title, "note": i.note,
            "meta": i.meta, "content": i.content, "file": rel(i), "added_at": i.added_at,
        })).collect::<Vec<_>>(),
    })
}

/// Attachment entries (`post_id` `research:<id>`) for items with a stored file, so exports copy them.
pub fn item_attachments(items: &[ResearchItem]) -> Vec<Attachment> {
    items.iter().filter(|i| !i.local_path.is_empty() && Path::new(&i.local_path).exists()).map(|i| Attachment {
        post_id: format!("research:{}", i.id),
        url: if i.kind == ItemKind::WebUrl { i.r#ref.clone() } else { format!("file://{}", i.r#ref) },
        kind: "file".into(), local_path: PathBuf::from(&i.local_path),
        mime_type: i.meta["mime_type"].as_str().or(i.meta["content_type"].as_str()).unwrap_or("application/octet-stream").to_string(),
        on_site: false,
    }).collect()
}

#[cfg(test)]
mod tests;
