//! Attachments download + JSON/HTML export + optional .tar.gz bundle.

use super::{Args, FIREFOX_UA, Post};
use crate::mentions::{MentionHit, MatchType, Profile};
use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
};

pub const EXPORT_FORMAT: &str = "acp-rs-export";
pub const EXPORT_FORMAT_VERSION: u32 = 2;
/// Id of the machine-readable JSON copy embedded in HTML exports.
const HTML_DATA_ID: &str = "acp-data";

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Json,
    Html,
}

/// Archive formats selected by format flags (--tar-gz). Add e.g. `Zip` + a
/// `--zip` flag in the `compress_format` group later.
#[derive(Clone, Copy)]
pub enum Compress {
    TarGz,
}

impl Compress {
    fn ext(self) -> &'static str {
        match self { Compress::TarGz => "tar.gz" }
    }
}

#[derive(Debug, Clone)]
pub struct Attachment {
    pub post_id: String,
    pub url: String,
    pub kind: String,
    /// Absolute path under ~/.acp/ai/data/attachments.
    pub local_path: PathBuf,
    pub mime_type: String,
}

fn ext_from_mime(m: &str) -> Option<&'static str> {
    Some(match m.split(';').next()?.trim() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/svg+xml" => "svg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mp4" => "m4a",
        "application/pdf" => "pdf",
        _ => return None,
    })
}

fn mime_from_ext(e: &str) -> &'static str {
    match e {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn ext_from_url(u: &str) -> Option<String> {
    let path = url::Url::parse(u).ok()?.path().to_string();
    let e = path.rsplit('/').next()?.rsplit_once('.')?.1.to_ascii_lowercase();
    (e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric())).then_some(e)
}

fn url_key(u: &str) -> String {
    let mut h = DefaultHasher::new();
    u.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Download every media URL once (dedupe by URL), skipping failures.
pub async fn download_all(http: &reqwest::Client, posts: &[Post], dir: &Path) -> Vec<Attachment> {
    let _ = std::fs::create_dir_all(dir);
    let mut done: HashMap<String, Option<(PathBuf, String)>> = HashMap::new();
    let mut out = Vec::new();
    let mut seen_pair = HashSet::new();
    for p in posts {
        for m in &p.media {
            if !seen_pair.insert((p.post_id.clone(), m.url.clone())) { continue; }
            if !done.contains_key(&m.url) {
                let r = fetch_one_pub(http, &m.url, dir).await;
                if let Err(e) = &r { crate::log_error(format_args!("warning: attachment {} skipped: {e:#}", m.url)); }
                done.insert(m.url.clone(), r.ok());
            }
            if let Some(Some((path, mime))) = done.get(&m.url) {
                out.push(Attachment {
                    post_id: p.post_id.clone(), url: m.url.clone(), kind: m.kind.clone(),
                    local_path: path.clone(), mime_type: mime.clone(),
                });
            }
        }
    }
    crate::log_info(format_args!("attachments: {} files for {} references", done.values().flatten().count(), out.len()));
    out
}

pub async fn fetch_one_pub(http: &reqwest::Client, u: &str, dir: &Path) -> Result<(PathBuf, String)> {
    let key = url_key(u);
    // Reuse an existing download.
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with(&key) {
                let ext = n.rsplit_once('.').map(|x| x.1).unwrap_or("");
                return Ok((e.path(), mime_from_ext(ext).to_string()));
            }
        }
    }
    let resp = http.get(u)
        .header("User-Agent", FIREFOX_UA)
        .header("Accept", "image/avif,image/webp,video/*,audio/*,*/*;q=0.8")
        .header("Sec-Fetch-Dest", "image")
        .header("Sec-Fetch-Mode", "no-cors")
        .header("Referer", "https://acp.us/")
        .send().await?.error_for_status()?;
    let ct = resp.headers().get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let ext = ext_from_mime(&ct).map(str::to_string).or_else(|| ext_from_url(u)).unwrap_or_else(|| "bin".into());
    let mime = if ct.is_empty() { mime_from_ext(&ext).to_string() } else { ct.split(';').next().unwrap_or("").trim().to_string() };
    let bytes = resp.bytes().await?;
    let path = dir.join(format!("{key}.{ext}"));
    std::fs::write(&path, &bytes)?;
    Ok((path, mime))
}

pub fn save_attachments(db: &Path, atts: &[Attachment]) -> Result<()> {
    let mut c = Connection::open(db)?;
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL,
            url TEXT NOT NULL,
            local_path TEXT NOT NULL,
            mime_type TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS attachments_post_url_uq ON attachments(post_id, url);",
    )?;
    let tx = c.transaction()?;
    {
        let mut st = tx.prepare(
            "INSERT INTO attachments (post_id,url,local_path,mime_type) VALUES (?1,?2,?3,?4)
             ON CONFLICT(post_id,url) DO UPDATE SET local_path=excluded.local_path, mime_type=excluded.mime_type")?;
        for a in atts {
            st.execute(params![a.post_id, a.url, a.local_path.to_string_lossy(), a.mime_type])?;
        }
    }
    tx.commit()?;
    Ok(())
}

// ------------------------------------------------------------ export

/// `rel`: map attachment url -> path written into the export.
fn build_json(nick: &str, posts: &[Post], atts: &[Attachment], profiles: &[Profile], hits: &[MentionHit], rel: &dyn Fn(&Attachment) -> String) -> Value {
    let mut by_post: HashMap<&str, Vec<Value>> = HashMap::new();
    for a in atts {
        by_post.entry(a.post_id.as_str()).or_default().push(json!({
            "url": a.url, "kind": a.kind, "mime_type": a.mime_type, "local_path": rel(a),
        }));
    }
    let ids: HashSet<&str> = posts.iter().map(|p| p.post_id.as_str()).collect();
    let mut kids: HashMap<&str, Vec<&Post>> = HashMap::new();
    let mut roots = Vec::new();
    for p in posts {
        if p.parent_post_id.is_empty() || !ids.contains(p.parent_post_id.as_str()) {
            roots.push(p);
        } else {
            kids.entry(p.parent_post_id.as_str()).or_default().push(p);
        }
    }
    fn node(p: &Post, kids: &HashMap<&str, Vec<&Post>>, by: &HashMap<&str, Vec<Value>>) -> Value {
        let mut ch: Vec<&Post> = kids.get(p.post_id.as_str()).cloned().unwrap_or_default();
        ch.sort_by(|a, b| a.posted_at.cmp(&b.posted_at));
        json!({
            "post_id": p.post_id, "parent_post_id": p.parent_post_id, "nick": p.nick,
            "title": p.title, "content": p.content, "posted_at": p.posted_at,
            "attachments": by.get(p.post_id.as_str()).cloned().unwrap_or_default(),
            "children": ch.iter().map(|c| node(c, kids, by)).collect::<Vec<_>>(),
        })
    }
    roots.sort_by(|a, b| b.posted_at.cmp(&a.posted_at));
    let profiles_v: Vec<Value> = profiles.iter().map(|p| json!({
        "nick": p.nick, "user_id": p.user_id, "display_name": p.display_name, "bio": p.bio,
        "avatar_url": p.avatar_url, "avatar_path": p.avatar_path,
        "banner_url": p.banner_url, "banner_path": p.banner_path,
        "location": p.location, "role": p.role, "joined_at": p.joined_at,
        "followers": p.followers, "following": p.following, "posts_count": p.posts_count,
        "scraped_at": p.scraped_at,
    })).collect();
    let mentions_v: Vec<Value> = hits.iter().map(|h| json!({
        "post_id": h.post_id, "mentioned_nick": h.mentioned_nick, "match_type": h.match_type.as_str(),
    })).collect();
    json!({
        "nick": nick,
        "format": EXPORT_FORMAT,
        "format_version": EXPORT_FORMAT_VERSION,
        "exported_at": chrono::Local::now().to_rfc3339(),
        "profiles": profiles_v,
        "mentions": mentions_v,
        "posts": roots.iter().map(|p| node(p, &kids, &by_post)).collect::<Vec<_>>(),
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn html_post(v: &Value, out: &mut String, depth: usize) {
    let g = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    out.push_str(&format!("<article class=\"post d{}\" id=\"p-{}\">", depth.min(6), esc(&g("post_id"))));
    if !g("title").is_empty() { out.push_str(&format!("<h2>{}</h2>", esc(&g("title")))); }
    out.push_str(&format!("<div class=\"meta\"><b>@{}</b> · <time>{}</time></div>", esc(&g("nick")), esc(&g("posted_at"))));
    out.push_str(&format!("<div class=\"body\">{}</div>", esc(&g("content")).replace('\n', "<br>")));
    for a in v["attachments"].as_array().into_iter().flatten() {
        let p = esc(a["local_path"].as_str().unwrap_or_default());
        let m = a["mime_type"].as_str().unwrap_or_default();
        if a["kind"] == "thumbnail" { continue; }
        out.push_str("<div class=\"att\">");
        if m.starts_with("image/") { out.push_str(&format!("<img src=\"{p}\" loading=\"lazy\">")); }
        else if m.starts_with("video/") { out.push_str(&format!("<video src=\"{p}\" controls preload=\"metadata\"></video>")); }
        else if m.starts_with("audio/") { out.push_str(&format!("<audio src=\"{p}\" controls></audio>")); }
        else { out.push_str(&format!("<a href=\"{p}\">{p}</a>")); }
        out.push_str("</div>");
    }
    for c in v["children"].as_array().into_iter().flatten() { html_post(c, out, depth + 1); }
    out.push_str("</article>");
}

fn build_html(doc: &Value) -> String {
    let title = esc(doc["title"].as_str().or(doc["nick"].as_str()).unwrap_or_default());
    let nick = title.clone();
    let mut body = String::new();
    for pr in doc["profiles"].as_array().into_iter().flatten() {
        let dn = pr["display_name"].as_str().unwrap_or("");
        let un = pr["nick"].as_str().unwrap_or("");
        let bio = pr["bio"].as_str().unwrap_or("");
        let av = pr["avatar_path"].as_str().or(pr["avatar_url"].as_str()).unwrap_or("");
        body.push_str("<header class=\"profile\">");
        if !av.is_empty() { body.push_str(&format!("<img class=\"avatar\" src=\"{}\" alt=\"\">", esc(av))); }
        body.push_str(&format!("<div><h2>{} <span class=\"meta\">@{}</span></h2>", esc(dn), esc(un)));
        if !bio.is_empty() { body.push_str(&format!("<p>{}</p>", esc(bio))); }
        let bits: Vec<String> = ["location","role","joined_at"].iter().filter_map(|k| pr[*k].as_str().filter(|s| !s.is_empty()).map(|s| format!("{k}: {}", esc(s)))).collect();
        if !bits.is_empty() { body.push_str(&format!("<p class=\"meta\">{}</p>", bits.join(" · "))); }
        body.push_str("</div></header>");
    }
    body.push_str("<h2>Posts</h2>");
    for p in doc["posts"].as_array().into_iter().flatten() { html_post(p, &mut body, 0); }
    let mentions = doc["mentions"].as_array().cloned().unwrap_or_default();
    if !mentions.is_empty() {
        body.push_str("<h2>Mentions</h2><ul class=\"mentions\">");
        for h in &mentions {
            body.push_str(&format!(
                "<li><a href=\"#p-{}\">{}</a> → @{} <span class=\"meta\">({})</span></li>",
                esc(h["post_id"].as_str().unwrap_or("")),
                esc(h["post_id"].as_str().unwrap_or("")),
                esc(h["mentioned_nick"].as_str().unwrap_or("")),
                esc(h["match_type"].as_str().unwrap_or("")),
            ));
        }
        body.push_str("</ul>");
    }
    format!(r#"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>{nick}</title>
<style>
body{{font:16px/1.5 system-ui,sans-serif;max-width:800px;margin:2em auto;padding:0 1em;color:#18181b;background:#fafafa}}
.post{{background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:.8em 1em;margin:.8em 0}}
.post .post{{margin-left:1.2em;border-left:3px solid #dc2626;border-radius:0 8px 8px 0}}
h2{{margin:.2em 0;font-size:1.25em}} .meta{{color:#71717a;font-size:.85em}} .body{{margin:.5em 0;white-space:normal}}
.att img,.att video{{max-width:100%;border-radius:6px}} .att{{margin:.4em 0}}
.profile{{display:flex;gap:1em;align-items:flex-start;background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:1em;margin:1em 0}}
.profile .avatar{{width:72px;height:72px;border-radius:50%;object-fit:cover}}
.mentions{{padding-left:1.2em}}
</style><script type="application/json" id="{data_id}">{data}</script></head><body><h1>{nick}</h1><p class="meta">Exported {}</p>{body}</body></html>"#,
        esc(doc["exported_at"].as_str().unwrap_or_default()),
        data_id = HTML_DATA_ID,
        data = doc.to_string().replace('<', "\\u003c"))
}

fn render(f: Format, doc: &Value) -> Result<String> {
    Ok(match f {
        Format::Json => serde_json::to_string_pretty(doc)?,
        Format::Html => build_html(doc),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn export(args: &Args, name: &str, nick: &str, posts: &[Post], atts: &[Attachment], profiles: &[Profile], hits: &[MentionHit], exports_dir: &Path) -> Result<()> {
    let ext = match args.format { Format::Json => "json", Format::Html => "html" };
    let base = name.to_string();

    // stdout: no bundle; attachment paths are the absolute persisted ones.
    if args.output.is_none() && args.compression().is_none() {
        let mut doc = build_json(nick, posts, atts, profiles, hits, &|a| a.local_path.to_string_lossy().into_owned());
        doc["title"] = json!(name);
        let mut so = std::io::stdout().lock();
        so.write_all(render(args.format, &doc)?.as_bytes())?;
        so.write_all(b"\n")?;
        return Ok(());
    }

    // Export dir: the file plus attachments/ beside it.
    let (dir, file, tar_path) = if let Some(c) = args.compression() {
        let tp = args.output.clone().unwrap_or_else(|| exports_dir.join(format!("{base}.{}", c.ext())));
        let stem = tp.file_name().map(|n| n.to_string_lossy().trim_end_matches(".tar.gz").trim_end_matches(".tgz").to_string())
            .unwrap_or(base.clone());
        let dir = tp.parent().map(Path::to_path_buf).unwrap_or_default().join(&stem);
        let file = dir.join(format!("{stem}.{ext}"));
        (dir, file, Some((tp, stem)))
    } else {
        let f = args.output.clone().unwrap();
        // A directory as --output: write <name>.<ext> inside it.
        let f = if f.is_dir() { f.join(format!("{base}.{ext}")) } else { f };
        (f.parent().map(Path::to_path_buf).filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| ".".into()), f, None)
    };
    let att_dir = dir.join("attachments");
    std::fs::create_dir_all(&att_dir)?;
    for a in atts {
        if let Some(n) = a.local_path.file_name() {
            let dst = att_dir.join(n);
            if !dst.exists() { std::fs::copy(&a.local_path, &dst).with_context(|| format!("copy {}", a.local_path.display()))?; }
        }
    }
    let mut doc = build_json(nick, posts, atts, profiles, hits, &|a| {
        format!("attachments/{}", a.local_path.file_name().unwrap_or_default().to_string_lossy())
    });
    doc["title"] = json!(name);
    std::fs::write(&file, render(args.format, &doc)?)?;
    crate::log_info(format_args!("wrote {}", file.display()));

    if let Some((tp, stem)) = tar_path {
        if let Some(parent) = tp.parent().filter(|p| !p.as_os_str().is_empty()) { std::fs::create_dir_all(parent)?; }
        // Only tar-gz today; match on args.compression() here when adding formats.
        let f = std::fs::File::create(&tp)?;
        let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut t = tar::Builder::new(gz);
        t.append_dir_all(&stem, &dir)?;
        t.into_inner()?.finish()?;
        crate::log_info(format_args!("bundle {}", tp.display()));
    }
    Ok(())
}

/// Load posts matching `query` (case-insensitive LIKE on title + content),
/// optionally restricted to `nick`, plus context: each match's parent and
/// its direct children. Attachments come from the attachments table.
pub fn search_db(db: &Path, query: Option<&str>, nicks: &[String]) -> Result<(Vec<Post>, Vec<Attachment>, usize)> {
    let c = Connection::open(db).with_context(|| format!("open {}", db.display()))?;
    let like = query.map(|q| format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let cols = "post_id,parent_post_id,nick,title,content,posted_at";
    let row = |r: &rusqlite::Row| -> rusqlite::Result<Post> {
        Ok(Post { post_id: r.get(0)?, parent_post_id: r.get(1)?, nick: r.get(2)?, title: r.get(3)?,
                  content: r.get(4)?, posted_at: r.get(5)?, media: Vec::new() })
    };
    let mut st = c.prepare(&format!(
        "SELECT {cols} FROM posts WHERE (?1 IS NULL OR title LIKE ?1 ESCAPE '\\' OR content LIKE ?1 ESCAPE '\\')
         ORDER BY posted_at"))?;
    let want: HashSet<String> = nicks.iter().map(|n| n.to_lowercase()).collect();
    let matches: Vec<Post> = st.query_map(params![like], row)?
        .filter(|p| p.as_ref().map_or(true, |p| want.is_empty() || want.contains(&p.nick.to_lowercase())))
        .collect::<rusqlite::Result<_>>()?;
    let n = matches.len();
    let mut seen: HashSet<String> = matches.iter().map(|p| p.post_id.clone()).collect();
    let mut out = matches;
    let mut by_id = c.prepare(&format!("SELECT {cols} FROM posts WHERE post_id = ?1"))?;
    let mut kids = c.prepare(&format!("SELECT {cols} FROM posts WHERE parent_post_id = ?1 ORDER BY posted_at"))?;
    // Context (parent + children) only for text searches; a full/nick dump has it already.
    let base: Vec<(String, String)> = if query.is_none() { Vec::new() } else { out.iter().map(|p| (p.post_id.clone(), p.parent_post_id.clone())).collect() };
    for (id, parent) in base {
        if !parent.is_empty() {
            for p in by_id.query_map(params![parent], row)? { let p = p?; if seen.insert(p.post_id.clone()) { out.push(p); } }
        }
        for p in kids.query_map(params![id], row)? { let p = p?; if seen.insert(p.post_id.clone()) { out.push(p); } }
    }
    let mut atts = Vec::new();
    let has_att: bool = c.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='attachments'", [], |r| r.get::<_, i64>(0))? > 0;
    if has_att {
        let mut st = c.prepare("SELECT post_id,url,local_path,mime_type FROM attachments WHERE post_id = ?1")?;
        for p in &out {
            for a in st.query_map(params![p.post_id], |r| Ok(Attachment {
                post_id: r.get(0)?, url: r.get(1)?, kind: "file".into(),
                local_path: PathBuf::from(r.get::<_, String>(2)?), mime_type: r.get(3)?,
            }))? {
                let a = a?;
                if a.local_path.exists() { atts.push(a); } else { crate::log_error(format_args!("warning: missing attachment file {}", a.local_path.display())); }
            }
        }
    }
    Ok((out, atts, n))
}

pub fn slug(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() { o.push(ch.to_ascii_lowercase()); }
        else if !o.ends_with('-') { o.push('-'); }
    }
    let o = o.trim_matches('-').chars().take(40).collect::<String>();
    if o.is_empty() { "query".into() } else { o }
}

// ------------------------------------------------------------ import

/// Import an export (.json, .html with embedded #acp-data, or .tar.gz/.tgz).
/// Returns (posts seen, attachments seen) for reporting.
#[allow(clippy::type_complexity)]
pub fn import(file: &Path, compress: Option<Compress>, att_dir: &Path, tmp_root: &Path) -> Result<(Vec<Post>, Vec<Attachment>, Vec<Profile>, Vec<MentionHit>)> {
    // Compression comes from the flags; magic bytes only produce a helpful error.
    let mut magic = [0u8; 2];
    {
        use std::io::Read;
        let mut f = std::fs::File::open(file).with_context(|| format!("open {}", file.display()))?;
        let _ = f.read(&mut magic)?;
    }
    let gz = magic == [0x1f, 0x8b];
    match (compress, gz) {
        (None, true) => anyhow::bail!(
            "{} looks gzip-compressed; add --compress --tar-gz to import a .tar.gz archive", file.display()),
        (Some(Compress::TarGz), false) => anyhow::bail!(
            "{} is not gzip-compressed; drop --compress --tar-gz to import a plain .json/.html export", file.display()),
        _ => {}
    }
    let mut _tmp_guard = None;
    let doc_path = if let Some(Compress::TarGz) = compress {
        let tmp = tmp_root.join(format!("import-{}", chrono::Local::now().format("%Y%m%d-%H%M%S%f")));
        std::fs::create_dir_all(&tmp)?;
        let f = std::fs::File::open(file).with_context(|| format!("open {}", file.display()))?;
        tar::Archive::new(flate2::read::GzDecoder::new(f)).unpack(&tmp)?;
        _tmp_guard = Some(TmpDir(tmp.clone()));
        find_doc(&tmp).with_context(|| "no .json/.html export inside archive")?
    } else {
        file.to_path_buf()
    };
    let text = std::fs::read_to_string(&doc_path).with_context(|| format!("read {}", doc_path.display()))?;
    // JSON vs HTML: decided by content, not file extension.
    let doc: Value = if text.trim_start().starts_with('<') {
        let h = scraper::Html::parse_document(&text);
        let sel = scraper::Selector::parse(&format!("script#{HTML_DATA_ID}")).unwrap();
        let el = h.select(&sel).next().context("HTML has no embedded acp-data JSON (not an acp-rs export?)")?;
        serde_json::from_str(&el.text().collect::<String>())?
    } else {
        serde_json::from_str(&text)?
    };
    if doc["format"].as_str().is_some_and(|f| f != EXPORT_FORMAT) {
        anyhow::bail!("unknown export format {}", doc["format"]);
    }
    let v = doc["format_version"].as_u64().unwrap_or(1);
    if v > EXPORT_FORMAT_VERSION as u64 {
        anyhow::bail!("export format_version {v} is newer than supported {EXPORT_FORMAT_VERSION}");
    }
    // v1 (no profiles/mentions) and v2 both accepted.
    let base = doc_path.parent().map(Path::to_path_buf).unwrap_or_default();
    std::fs::create_dir_all(att_dir)?;
    let mut posts = Vec::new();
    let mut atts = Vec::new();
    let mut seen = HashSet::new();
    fn walk(n: &Value, parent: &str, base: &Path, att_dir: &Path, posts: &mut Vec<Post>, atts: &mut Vec<Attachment>, seen: &mut HashSet<(String, String)>) {
        let g = |k: &str| n[k].as_str().unwrap_or_default().to_string();
        let id = g("post_id");
        if id.is_empty() { return; }
        let pp = n["parent_post_id"].as_str().map(str::to_string).unwrap_or_else(|| parent.to_string());
        posts.push(Post { post_id: id.clone(), parent_post_id: pp, nick: g("nick"), title: g("title"),
            content: g("content"), posted_at: g("posted_at"), media: Vec::new() });
        for a in n["attachments"].as_array().into_iter().flatten() {
            let url = a["url"].as_str().unwrap_or_default().to_string();
            if url.is_empty() || !seen.insert((id.clone(), url.clone())) { continue; }
            let lp = PathBuf::from(a["local_path"].as_str().unwrap_or_default());
            let src = if lp.is_absolute() { lp } else { base.join(lp) };
            let Some(fname) = src.file_name() else { continue };
            let dst = att_dir.join(fname);
            if !dst.exists() && let Err(e) = std::fs::copy(&src, &dst) {
                crate::log_error(format_args!("warning: attachment {} not imported: {e}", src.display()));
                continue;
            }
            atts.push(Attachment { post_id: id.clone(), url, kind: a["kind"].as_str().unwrap_or("file").into(),
                local_path: dst, mime_type: a["mime_type"].as_str().unwrap_or_default().into() });
        }
        for c in n["children"].as_array().into_iter().flatten() { walk(c, &id, base, att_dir, posts, atts, seen); }
    }
    for p in doc["posts"].as_array().into_iter().flatten() {
        walk(p, "", &base, att_dir, &mut posts, &mut atts, &mut seen);
    }
        let mut profiles = Vec::new();
    for pr in doc["profiles"].as_array().into_iter().flatten() {
        let g = |k: &str| pr[k].as_str().map(str::to_string);
        profiles.push(Profile {
            nick: g("nick").unwrap_or_default(),
            user_id: g("user_id"), display_name: g("display_name"), bio: g("bio"),
            avatar_url: g("avatar_url"), avatar_path: g("avatar_path"),
            banner_url: g("banner_url"), banner_path: g("banner_path"),
            location: g("location"), role: g("role"), joined_at: g("joined_at"),
            followers: pr["followers"].as_i64(), following: pr["following"].as_i64(),
            posts_count: pr["posts_count"].as_i64(),
            raw_json: pr.to_string(), scraped_at: g("scraped_at").unwrap_or_default(),
        });
    }
    let mut hits = Vec::new();
    for h in doc["mentions"].as_array().into_iter().flatten() {
        let mt = h["match_type"].as_str().and_then(MatchType::parse);
        if let Some(match_type) = mt {
            hits.push(MentionHit {
                post_id: h["post_id"].as_str().unwrap_or("").into(),
                mentioned_nick: h["mentioned_nick"].as_str().unwrap_or("").into(),
                match_type,
            });
        }
    }
    Ok((posts, atts, profiles, hits))
}

struct TmpDir(PathBuf);
impl Drop for TmpDir {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

/// First .json, else .html, in the archive (shallowest first).
fn find_doc(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<(usize, u8, PathBuf)> = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    while let Some((d, depth)) = stack.pop() {
        for e in std::fs::read_dir(&d).ok()?.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != "attachments") { stack.push((p, depth + 1)); }
            } else if let Some(x) = p.extension().and_then(|x| x.to_str()) {
                match x.to_ascii_lowercase().as_str() {
                    "json" => found.push((depth, 0, p)),
                    "html" | "htm" => found.push((depth, 1, p)),
                    _ => {}
                }
            }
        }
    }
    found.sort();
    found.into_iter().next().map(|x| x.2)
}
