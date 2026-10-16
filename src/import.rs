//! `--import`: read a JSON/HTML export or `.tar.gz` archive back into memory.

use crate::prelude::*;

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
        let irt = n["in_reply_to_id"].as_str().filter(|x| !x.is_empty()).map(str::to_string).unwrap_or_else(|| pp.clone());
        posts.push(Post { post_id: id.clone(), parent_post_id: pp, in_reply_to_id: irt, nick: g("nick"), title: g("title"),
            content: g("content"), posted_at: g("posted_at"), media: Vec::new(), truncated: n["truncated"].as_bool().unwrap_or(false) });
        for a in n["attachments"].as_array().into_iter().flatten() {
            let url = a["url"].as_str().unwrap_or_default().to_string();
            if url.is_empty() || !seen.insert((id.clone(), url.clone())) { continue; }
            let lp = PathBuf::from(a["local_path"].as_str().unwrap_or_default());
            let src = if lp.is_absolute() { lp } else { base.join(lp) };
            let Some(fname) = src.file_name() else { continue };
            let dst = att_dir.join(fname);
            if !dst.exists() && let Err(e) = std::fs::copy(&src, &dst) {
                log_error(format_args!("warning: attachment {} not imported: {e}", src.display()));
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
