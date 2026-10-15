//! Attachment discovery in post JSON, content-addressed download, MIME/extension mapping.

use crate::prelude::*;

#[derive(Debug, Clone)]
/// Media URL referenced by a post, before download.
pub struct MediaRef {
    /// Page / media URL.
    pub url: String,
    /// Media kind (`image`, `video`, `thumbnail`, ...).
    pub kind: String,
}

/// Verified (PostMedia-*.js): `post.media` = [{id, type, url, thumb_url, width, height}],
/// `type == "video"` renders `<video src=url poster=thumb_url>`; other types render `<img>`.
/// Body (ProseMirror) has no image node in the editor bundle; nodes found: embed
/// {provider,id,kind} (third-party iframes, not downloaded), postRef, mention.
/// GUESSED: any other node attr named src/url/href pointing at the site's media hosts.
pub fn collect_media(v: &Value, out: &mut Vec<MediaRef>) {
    if let Some(ms) = v.get("media").and_then(Value::as_array) {
        for m in ms {
            let kind = m.get("type").and_then(Value::as_str).unwrap_or("image").to_string();
            if let Some(u) = m.get("url").and_then(Value::as_str) {
                out.push(MediaRef { url: u.into(), kind: kind.clone() });
            }
            if let Some(u) = m.get("thumb_url").and_then(Value::as_str) {
                out.push(MediaRef { url: u.into(), kind: "thumbnail".into() });
            }
        }
    }
    for k in ["attachments", "files", "images"] {
        if let Some(ms) = v.get(k).and_then(Value::as_array) {
            for m in ms {
                if let Some(u) = m.get("url").or(m.get("src")).and_then(Value::as_str) {
                    let kind = m.get("type").and_then(Value::as_str).unwrap_or("file").to_string();
                    out.push(MediaRef { url: u.into(), kind });
                }
            }
        }
    }
    fn walk(n: &Value, out: &mut Vec<MediaRef>) {
        if let Some(a) = n.get("attrs").and_then(Value::as_object) {
            for k in ["src", "url", "href"] {
                if let Some(u) = a.get(k).and_then(Value::as_str)
                    && MEDIA_HOSTS.iter().any(|h| u.contains(h))
                {
                    let kind = n.get("type").and_then(Value::as_str).unwrap_or("file").to_string();
                    out.push(MediaRef { url: u.into(), kind });
                }
            }
        }
        for key in ["content", "marks"] {
            if let Some(kids) = n.get(key).and_then(Value::as_array) { kids.iter().for_each(|k| walk(k, out)); }
        }
    }
    if let Some(b) = v.get("body") {
        let b = match b.as_str() { Some(r) => serde_json::from_str(r).unwrap_or(Value::Null), None => b.clone() };
        walk(&b, out);
    }
}

#[derive(Debug, Clone)]
/// A downloaded attachment of a post.
pub struct Attachment {
    /// Site post id (22-character base64url).
    pub post_id: String,
    /// Page / media URL.
    pub url: String,
    /// Media kind (`image`, `video`, `thumbnail`, ...).
    pub kind: String,
    /// Absolute path under ~/.acp/ai/data/attachments.
    pub local_path: PathBuf,
    /// MIME type of the file.
    pub mime_type: String,
}

/// File extension for a known media MIME type.
pub fn ext_from_mime(m: &str) -> Option<&'static str> {
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

/// MIME type for a file extension (fallback `application/octet-stream`).
pub fn mime_from_ext(e: &str) -> &'static str {
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

/// File extension from a URL path, if it has a short alphanumeric one.
pub fn ext_from_url(u: &str) -> Option<String> {
    let path = url::Url::parse(u).ok()?.path().to_string();
    let e = path.rsplit('/').next()?.rsplit_once('.')?.1.to_ascii_lowercase();
    (e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric())).then_some(e)
}

/// Stable hex hash of a URL without its query string, used to name downloaded files.
pub fn url_key(u: &str) -> String {
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
                if let Err(e) = &r { log_error(format_args!("warning: attachment {} skipped: {e:#}", m.url)); }
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
    log_info(format_args!("attachments: {} files for {} references", done.values().flatten().count(), out.len()));
    out
}

/// Download one URL into `dir` (named by [`url_key`] + extension), reusing an existing file; returns the path and MIME type.
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

/// Download a nick's profile image to `{attachments}/profiles/{nick}.{ext}`
/// (replacing any older copy) and return its path.
pub async fn fetch_avatar(http: &reqwest::Client, u: &str, att_dir: &Path, nick: &str) -> Result<PathBuf> {
    let dir = att_dir.join(PROFILE_IMAGES_DIR);
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join(format!(".{nick}.download"));
    std::fs::create_dir_all(&tmp)?;
    let res = fetch_one_pub(http, u, &tmp).await;
    let out = match res {
        Ok((p, _)) => {
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("bin").to_string();
            for e in std::fs::read_dir(&dir)?.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.rsplit_once('.').is_some_and(|(stem, _)| stem == nick) { let _ = std::fs::remove_file(e.path()); }
            }
            let dst = dir.join(format!("{nick}.{ext}"));
            std::fs::rename(&p, &dst)?;
            Ok(dst)
        }
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_dir_all(&tmp);
    out
}
