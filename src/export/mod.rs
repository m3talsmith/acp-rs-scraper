//! `--format json|html`, `--output`, `--compress --tar-gz`: writing exports.

pub mod archive;
pub mod html;
pub mod json;

use crate::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
/// Export format (`--format`).
pub enum Format {
    /// JSON document (default).
    Json,
    /// Self-contained HTML page.
    Html,
}

/// Archive formats selected by format flags (--tar-gz). Add e.g. `Zip` + a
/// `--zip` flag in the `compress_format` group later.
#[derive(Clone, Copy)]
pub enum Compress {
    /// gzip-compressed tar.
    TarGz,
}

impl Compress {
    /// File extension used for this archive format (e.g. `tar.gz`).
    pub fn ext(self) -> &'static str {
        match self { Compress::TarGz => "tar.gz" }
    }
}

impl Format {
    /// File extension for this export format.
    pub fn ext(self) -> &'static str {
        match self { Format::Json => "json", Format::Html => "html" }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.ext()) }
}

/// Where a `.tar.gz` export is written.
///
/// No path: `<exports_dir>/<name>.<ext>`. A directory, or a path ending in `/`,
/// is `<dir>/<slug>.<ext>` (the export name, made into a file name). A name that
/// already ends in `.tar.gz` or `.tgz` is kept. Any other path gets `.<ext>`
/// appended, so a bare name is a file and not the staging directory.
pub fn archive_output(out: Option<&Path>, name: &str, exports_dir: &Path, ext: &str) -> PathBuf {
    let Some(out) = out.filter(|p| !p.as_os_str().is_empty()) else {
        return exports_dir.join(format!("{name}.{ext}"));
    };
    if is_directory_dest(out) {
        return out.join(format!("{}.{ext}", export_file_stem(name)));
    }
    if has_archive_suffix(out) {
        return out.to_path_buf();
    }
    let mut os = out.as_os_str().to_owned();
    os.push(".");
    os.push(ext);
    PathBuf::from(os)
}

/// A directory the user named, including a path that ends in `/` and does not exist yet.
fn is_directory_dest(path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }
    path.to_str().is_some_and(|s| s.ends_with('/') || s.ends_with('\\'))
}

/// `.tar.gz` or `.tgz`, ignoring case.
fn has_archive_suffix(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else { return false };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".tar.gz") || lower.ends_with(".tgz")
}

/// Export name as a file stem: letters, digits, and dashes. This is [`slug`] without the length cap,
/// so a timestamp in the name survives.
fn export_file_stem(name: &str) -> String {
    let mut o = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            o.push(ch.to_ascii_lowercase());
        } else if !o.ends_with('-') {
            o.push('-');
        }
    }
    let o = o.trim_matches('-').to_string();
    if o.is_empty() { "export".into() } else { o }
}

/// Stem of an archive file name (`resonance-cosmos.tar.gz` → `resonance-cosmos`).
/// The staging directory is this stem beside the archive, never the archive path itself.
fn staging_stem(archive: &Path, fallback: &str) -> String {
    let Some(name) = archive.file_name().and_then(|n| n.to_str()) else {
        return export_file_stem(fallback);
    };
    let lower = name.to_ascii_lowercase();
    let cut = if lower.ends_with(".tar.gz") {
        ".tar.gz".len()
    } else if lower.ends_with(".tgz") {
        ".tgz".len()
    } else {
        0
    };
    if cut == 0 || cut >= name.len() {
        return export_file_stem(fallback);
    }
    let stem = &name[..name.len() - cut];
    if stem.is_empty() { export_file_stem(fallback) } else { stem.to_string() }
}

/// A free path beside `path` when `path` already exists.
///
/// The first try is `{stem}-{YYYYMMDD-HHMMSS}.{ext}`. The next tries are
/// `{stem}-{stamp}-2`, then `-3`. A `.tar.gz` name keeps that suffix.
/// A path that does not exist is returned as given.
pub fn unique_if_exists(path: &Path) -> PathBuf {
    if !path.exists() { return path.to_path_buf(); }
    let parent = path.parent().unwrap_or(Path::new(""));
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("export");
    let (stem, ext) = if let Some(stem) = name.strip_suffix(".tar.gz") {
        (stem.to_string(), "tar.gz".to_string())
    } else {
        match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => (stem.to_string(), ext.to_string()),
            _ => (name.to_string(), String::new()),
        }
    };
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    for n in 0..1000 {
        let suffix = if n == 0 { format!("{stem}-{stamp}") } else { format!("{stem}-{stamp}-{}", n + 1) };
        let file = if ext.is_empty() { suffix } else { format!("{suffix}.{ext}") };
        let candidate = parent.join(file);
        if !candidate.exists() { return candidate; }
    }
    path.to_path_buf()
}

/// Where each format goes for a plain (uncompressed) `--output` path.
/// One format: the path as given (a directory gets `<name>.<ext>`).
/// Several: a directory gets `<dir>/<name>.<ext>` each; a file path keeps its
/// stem and gets one `<stem>.<ext>` per format (`out/x.json` -> `out/x.json`, `out/x.html`).
pub fn output_files(out: &Path, name: &str, formats: &[Format]) -> Vec<(Format, PathBuf)> {
    if out.is_dir() {
        return formats.iter().map(|f| (*f, out.join(format!("{name}.{}", f.ext())))).collect();
    }
    if formats.len() == 1 { return vec![(formats[0], out.to_path_buf())]; }
    formats.iter().map(|f| (*f, out.with_extension(f.ext()))).collect()
}

/// Serialize an export document in the requested format.
pub fn render(f: Format, doc: &Value) -> Result<String> {
    Ok(match f {
        Format::Json => serde_json::to_string_pretty(doc)?,
        Format::Html => build_html(doc),
    })
}

#[allow(clippy::too_many_arguments)]
/// Write an export (JSON or HTML) to stdout, `--output`, or a `.tar.gz` archive with an `attachments/` folder, as selected in `args`.
pub fn export(args: &Args, name: &str, nick: &str, posts: &[Post], atts: &[Attachment], profiles: &[Profile], hits: &[MentionHit], exports_dir: &Path) -> Result<()> {
    export_with(args, name, nick, posts, atts, profiles, hits, exports_dir, None)
}

/// A research project to add to an export (its `research` section).
pub type ResearchPart<'a> = Option<(&'a ResearchProject, &'a [ResearchItem])>;

#[allow(clippy::too_many_arguments)]
/// [`export`] plus an optional research section; research files must be in `atts`
/// (see [`item_attachments`]) so bundles copy them.
pub fn export_with(args: &Args, name: &str, nick: &str, posts: &[Post], atts: &[Attachment], profiles: &[Profile], hits: &[MentionHit],
    exports_dir: &Path, research: ResearchPart) -> Result<()> {
    let formats = args.formats();
    let base = name.to_string();

    // stdout: no bundle; attachment paths are the absolute persisted ones.
    if args.output.is_none() && args.compression().is_none() {
        if formats.len() > 1 {
            bail!("--format {}: several formats need --output, --export or --compress (stdout takes one format)",
                formats.iter().map(|f| f.ext()).collect::<Vec<_>>().join(","));
        }
        let mut doc = build_json(nick, posts, atts, profiles, hits, &|a| a.local_path.to_string_lossy().into_owned());
        doc["title"] = json!(name);
        if let Some(s) = crate::storage::history::export_object(args) { doc["search"] = s; }
        if let Some((p, items)) = research {
            doc["research"] = research_json(p, items, &|i| (!i.local_path.is_empty()).then(|| i.local_path.clone()));
        }
        let mut so = std::io::stdout().lock();
        so.write_all(render(formats[0], &doc)?.as_bytes())?;
        so.write_all(b"\n")?;
        return Ok(());
    }

    // Never write (or overwrite a good export with) an empty file.
    if posts.is_empty() && hits.is_empty() && research.is_none_or(|(_, i)| i.is_empty()) {
        err!("warning: export {name}: 0 posts kept, not writing an export (any existing file is left untouched)");
        return Ok(());
    }

    // Export dir: the file plus attachments/ beside it.
    let (dir, files, tar_path) = if let Some(c) = args.compression() {
        let tp = archive_output(args.output.as_deref(), &base, exports_dir, c.ext());
        let stem = staging_stem(&tp, &base);
        let parent = tp.parent().map(Path::to_path_buf).unwrap_or_default();
        let dir = if parent.as_os_str().is_empty() { PathBuf::from(&stem) } else { parent.join(&stem) };
        let files: Vec<(Format, PathBuf)> = formats.iter().map(|f| (*f, dir.join(format!("{stem}.{}", f.ext())))).collect();
        (dir, files, Some((tp, stem)))
    } else {
        let files = output_files(&args.output.clone().unwrap(), &base, &formats);
        let f = &files[0].1;
        (f.parent().map(Path::to_path_buf).filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| ".".into()), files, None)
    };
    let att_dir = dir.join("attachments");
    std::fs::create_dir_all(&att_dir)?;
    for a in atts {
        if let Some(n) = a.local_path.file_name() {
            let dst = att_dir.join(n);
            if !dst.exists() { std::fs::copy(&a.local_path, &dst).with_context(|| format!("copy {}", a.local_path.display()))?; }
        }
    }
    // Profile images go to attachments/profiles/ with export-relative paths.
    let mut profiles: Vec<Profile> = profiles.to_vec();
    for pr in &mut profiles {
        for slot in [&mut pr.avatar_path, &mut pr.banner_path] {
            let Some(src) = slot.clone().map(PathBuf::from) else { continue };
            let Some(n) = src.file_name() else { continue };
            let dst_dir = att_dir.join(PROFILE_IMAGES_DIR);
            if src.exists() && std::fs::create_dir_all(&dst_dir).is_ok() && std::fs::copy(&src, dst_dir.join(n)).is_ok() {
                *slot = Some(format!("attachments/{PROFILE_IMAGES_DIR}/{}", n.to_string_lossy()));
            } else {
                *slot = None;
            }
        }
    }
    let profiles = &profiles[..];
    let mut doc = build_json(nick, posts, atts, profiles, hits, &|a| {
        format!("attachments/{}", a.local_path.file_name().unwrap_or_default().to_string_lossy())
    });
    doc["title"] = json!(name);
    if let Some(s) = crate::storage::history::export_object(args) { doc["search"] = s; }
    if let Some((p, items)) = research {
        let copied: HashSet<String> = atts.iter().map(|a| a.local_path.display().to_string()).collect();
        doc["research"] = research_json(p, items, &|i| copied.contains(&i.local_path)
            .then(|| format!("attachments/{}", Path::new(&i.local_path).file_name().unwrap_or_default().to_string_lossy())));
    }
    for (f, file) in &files {
        if let Some(p) = file.parent().filter(|p| !p.as_os_str().is_empty()) { std::fs::create_dir_all(p)?; }
        let dest = if args.post_ids.is_some() { unique_if_exists(file) } else { file.clone() };
        std::fs::write(&dest, render(*f, &doc)?)?;
        if dest != *file {
            log_info(format_args!("left {} in place; wrote {}", file.display(), dest.display()));
        } else {
            log_info(format_args!("wrote {}", dest.display()));
        }
    }

    if let Some((tp, stem)) = tar_path {
        let dest = if args.post_ids.is_some() { unique_if_exists(&tp) } else { tp.clone() };
        if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) { std::fs::create_dir_all(parent)?; }
        // Only tar-gz today; match on args.compression() here when adding formats.
        let f = std::fs::File::create(&dest)?;
        let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut t = tar::Builder::new(gz);
        t.append_dir_all(&stem, &dir)?;
        t.into_inner()?.finish()?;
        // Success log (and stderr): the path actually written, after a directory or a bare name is resolved.
        if dest != tp {
            log_info(format_args!("left {} in place; wrote {}", tp.display(), dest.display()));
        } else {
            log_info(format_args!("wrote {}", dest.display()));
        }
        // The archive is complete: remove the uncompressed export folder.
        // (Any error above returns early and leaves the folder in place.)
        remove_export_dir(&dir, &tp)?;
    }
    Ok(())
}

/// Delete the uncompressed export folder `dir` after `archive` was written.
/// Refuses when the archive is missing/empty or would live inside `dir`.
pub fn remove_export_dir(dir: &Path, archive: &Path) -> Result<()> {
    let ok = std::fs::metadata(archive).map(|m| m.len() > 0).unwrap_or(false);
    if !ok { bail!("archive {} missing; keeping {}", archive.display(), dir.display()); }
    if archive.starts_with(dir) { return Ok(()); }
    std::fs::remove_dir_all(dir).with_context(|| format!("remove {}", dir.display()))?;
    verbose!("removed {}", dir.display());
    Ok(())
}

/// Lowercase, dash-separated, length-limited form of `s` for default file names.
pub fn slug(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() { o.push(ch.to_ascii_lowercase()); }
        else if !o.ends_with('-') { o.push('-'); }
    }
    let o = o.trim_matches('-').chars().take(40).collect::<String>();
    if o.is_empty() { "query".into() } else { o }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn empty_run_leaves_existing_export_untouched() {
        let dir = std::env::temp_dir().join(format!("acp-empty-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("keep.html");
        std::fs::write(&f, "good export").unwrap();
        let args = Args::try_parse_from(["x", "--nick", "hazaldin", "--format", "html", "--output", f.to_str().unwrap()]).unwrap();
        export(&args, "keep", "hazaldin", &[], &[], &[], &[], &dir).unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "good export");
        assert!(!dir.join("attachments").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("acp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn post() -> Post {
        Post { post_id: "p1".into(), parent_post_id: String::new(), in_reply_to_id: String::new(), nick: "hazaldin".into(),
            title: "t".into(), content: "hello multi".into(), posted_at: "2026-10-15T00:00:00Z".into(), media: vec![], truncated: false, body_json: String::new(), score: None }
    }

    #[test]
    fn json_marks_posts_and_replies_and_lists_posts_first() {
        let root = post();
        let mut own_reply = post(); own_reply.post_id = "p2".into(); own_reply.parent_post_id = "p1".into();
        // A newer reply in someone else's thread must not push the post down.
        let mut other = post(); other.post_id = "c1".into(); other.parent_post_id = "t9".into(); other.posted_at = "2026-10-16T00:00:00Z".into();
        let doc = build_json("@hazaldin", &[root, own_reply, other], &[], &[], &[], &|_| String::new());
        assert_eq!(doc["counts"], json!({"posts": 1, "replies": 2}));
        let top = doc["posts"].as_array().unwrap();
        assert_eq!(top[0]["post_id"], "p1");
        assert_eq!(top[0]["kind"], "post");
        assert_eq!(top[0]["children"][0]["kind"], "reply");
        assert_eq!(top[1]["post_id"], "c1");
        assert_eq!(top[1]["kind"], "reply");
    }

    #[test]
    fn output_paths_for_several_formats() {
        let both = [Format::Json, Format::Html];
        let d = tmp("outpaths");
        assert_eq!(output_files(&d.join("x.json"), "n", &[Format::Html]), vec![(Format::Html, d.join("x.json"))]);
        assert_eq!(output_files(&d.join("x.json"), "n", &both),
            vec![(Format::Json, d.join("x.json")), (Format::Html, d.join("x.html"))]);
        assert_eq!(output_files(&d.join("x"), "n", &both),
            vec![(Format::Json, d.join("x.json")), (Format::Html, d.join("x.html"))]);
        assert_eq!(output_files(&d, "n", &both),
            vec![(Format::Json, d.join("n.json")), (Format::Html, d.join("n.html"))]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_second_basket_export_leaves_the_first_file() {
        let d = tmp("basket-unique");
        let out = d.join("basket.html");
        std::fs::write(&out, "earlier").unwrap();
        let args = Args::try_parse_from(["x", "--post-ids", "p1", "--format", "html", "--output", out.to_str().unwrap()]).unwrap();
        export(&args, "basket", "chosen", &[post()], &[], &[], &[], &d).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "earlier", "the first file stays");
        let written: Vec<_> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("html") && p != &out).collect();
        assert_eq!(written.len(), 1, "{written:?}");
        let name = written[0].file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("basket-") && name.ends_with(".html"), "{name}");
        assert!(std::fs::read_to_string(&written[0]).unwrap().contains("hello multi"));
        let nick = d.join("nick.html");
        std::fs::write(&nick, "keep").unwrap();
        let args = Args::try_parse_from(["x", "--nick", "hazaldin", "--format", "html", "--output", nick.to_str().unwrap()]).unwrap();
        export(&args, "nick", "@hazaldin", &[post()], &[], &[], &[], &d).unwrap();
        assert!(std::fs::read_to_string(&nick).unwrap().contains("hello multi"), "a nick export still replaces its file");
        let tar = d.join("basket.tar.gz");
        std::fs::write(&tar, b"tar").unwrap();
        let next = unique_if_exists(&tar);
        let tar_name = next.file_name().unwrap().to_str().unwrap();
        assert!(tar_name.starts_with("basket-") && tar_name.ends_with(".tar.gz"), "{tar_name}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn writes_json_and_html_to_same_stem() {
        let d = tmp("multi-export");
        let out = d.join("haz.html");
        let args = Args::try_parse_from(["x", "--nick", "hazaldin", "--format", "json,html", "--output", out.to_str().unwrap()]).unwrap();
        export(&args, "haz", "@hazaldin", &[post()], &[], &[], &[], &d).unwrap();
        let j: Value = serde_json::from_str(&std::fs::read_to_string(d.join("haz.json")).unwrap()).unwrap();
        assert_eq!(j["format"], "acp-rs-export");
        assert!(std::fs::read_to_string(d.join("haz.html")).unwrap().contains("hello multi"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn writes_both_formats_into_one_archive() {
        let d = tmp("multi-tgz");
        let tp = d.join("bundle.tar.gz");
        let args = Args::try_parse_from(["x", "--nick", "hazaldin", "--format", "html", "--format", "json",
            "--compress", "--tar-gz", "--output", tp.to_str().unwrap()]).unwrap();
        export(&args, "bundle", "@hazaldin", &[post()], &[], &[], &[], &d).unwrap();
        let gz = flate2::read::GzDecoder::new(std::fs::File::open(&tp).unwrap());
        let names: Vec<String> = tar::Archive::new(gz).entries().unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned()).collect();
        assert!(names.iter().any(|n| n == "bundle/bundle.json"), "{names:?}");
        assert!(names.iter().any(|n| n == "bundle/bundle.html"), "{names:?}");
        assert!(!d.join("bundle").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn empty_run_skips_every_format() {
        let d = tmp("multi-empty");
        std::fs::write(d.join("k.json"), "old json").unwrap();
        std::fs::write(d.join("k.html"), "old html").unwrap();
        let args = Args::try_parse_from(["x", "--nick", "h", "--format", "json,html", "--output", d.join("k.json").to_str().unwrap()]).unwrap();
        export(&args, "k", "h", &[], &[], &[], &[], &d).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("k.json")).unwrap(), "old json");
        assert_eq!(std::fs::read_to_string(d.join("k.html")).unwrap(), "old html");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn several_formats_to_stdout_is_an_error() {
        let args = Args::try_parse_from(["x", "--nick", "h", "--format", "json,html"]).unwrap();
        assert!(export(&args, "k", "h", &[post()], &[], &[], &[], Path::new(".")).is_err());
    }

    #[test]
    fn archive_output_appends_tar_gz_or_writes_inside_a_directory() {
        let d = tmp("arch-rules");
        let ext = "tar.gz";
        assert_eq!(archive_output(None, "resonance-cosmos", &d, ext), d.join("resonance-cosmos.tar.gz"));
        assert_eq!(archive_output(Some(&d.join("resonance-cosmos")), "ignored", &d, ext), d.join("resonance-cosmos.tar.gz"));
        assert_eq!(archive_output(Some(&d.join("resonance-cosmos.tar.gz")), "ignored", &d, ext), d.join("resonance-cosmos.tar.gz"));
        assert_eq!(archive_output(Some(&d.join("resonance-cosmos.TGZ")), "ignored", &d, ext), d.join("resonance-cosmos.TGZ"));
        assert_eq!(archive_output(Some(&d), "Resonance Cosmos", &d, ext), d.join("resonance-cosmos.tar.gz"));
        let fresh = PathBuf::from(format!("{}/fresh/", d.display()));
        assert_eq!(archive_output(Some(&fresh), "Resonance Cosmos", &d, ext), d.join("fresh/resonance-cosmos.tar.gz"));
        assert_ne!(staging_stem(Path::new("resonance-cosmos.tar.gz"), "x"), "resonance-cosmos.tar.gz");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn compressed_bare_name_writes_a_tar_gz_and_logs_that_path() {
        let d = tmp("bare-tgz");
        let bare = d.join("resonance-cosmos");
        let args = Args::try_parse_from(["x", "--nick", "hazaldin", "--format", "json",
            "--compress", "--tar-gz", "--output", bare.to_str().unwrap()]).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        crate::logging::set_log_sink(Some(tx));
        struct Restore;
        impl Drop for Restore { fn drop(&mut self) { crate::logging::set_log_sink(None); } }
        let _restore = Restore;
        let previous = crate::logging::log_level();
        crate::logging::set_log_level(crate::logging::LogLevel::Normal);
        export(&args, "resonance-cosmos", "@hazaldin", &[post()], &[], &[], &[], &d).unwrap();
        crate::logging::set_log_level(previous);
        let final_path = d.join("resonance-cosmos.tar.gz");
        assert!(final_path.is_file(), "missing {}", final_path.display());
        assert!(!bare.exists(), "the bare name must not be left as the staging directory");
        let lines: Vec<String> = rx.try_iter().collect();
        let shown = final_path.display().to_string();
        assert!(lines.iter().any(|l| l == &format!("wrote {shown}")), "success log lines: {lines:?}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn compressed_directory_writes_the_project_slug_inside_it() {
        let d = tmp("dir-tgz");
        let args = Args::try_parse_from(["x", "--nick", "hazaldin", "--format", "json",
            "--compress", "--tar-gz", "--output", d.to_str().unwrap()]).unwrap();
        export(&args, "Resonance Cosmos", "@hazaldin", &[post()], &[], &[], &[], &d).unwrap();
        let final_path = d.join("resonance-cosmos.tar.gz");
        assert!(final_path.is_file(), "missing {}", final_path.display());
        assert!(!d.join("resonance-cosmos").exists(), "staging directory is removed after the archive");
        let gz = flate2::read::GzDecoder::new(std::fs::File::open(&final_path).unwrap());
        let names: Vec<String> = tar::Archive::new(gz).entries().unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned()).collect();
        assert!(names.iter().any(|n| n.ends_with("resonance-cosmos.json")), "{names:?}");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
