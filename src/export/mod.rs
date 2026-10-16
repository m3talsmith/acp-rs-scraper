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
        let mut so = std::io::stdout().lock();
        so.write_all(render(formats[0], &doc)?.as_bytes())?;
        so.write_all(b"\n")?;
        return Ok(());
    }

    // Never write (or overwrite a good export with) an empty file.
    if posts.is_empty() && hits.is_empty() {
        err!("warning: export {name}: 0 posts kept, not writing an export (any existing file is left untouched)");
        return Ok(());
    }

    // Export dir: the file plus attachments/ beside it.
    let (dir, files, tar_path) = if let Some(c) = args.compression() {
        let tp = args.output.clone().unwrap_or_else(|| exports_dir.join(format!("{base}.{}", c.ext())));
        let stem = tp.file_name().map(|n| n.to_string_lossy().trim_end_matches(".tar.gz").trim_end_matches(".tgz").to_string())
            .unwrap_or(base.clone());
        let dir = tp.parent().map(Path::to_path_buf).unwrap_or_default().join(&stem);
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
    for (f, file) in &files {
        if let Some(p) = file.parent().filter(|p| !p.as_os_str().is_empty()) { std::fs::create_dir_all(p)?; }
        std::fs::write(file, render(*f, &doc)?)?;
        log_info(format_args!("wrote {}", file.display()));
    }

    if let Some((tp, stem)) = tar_path {
        if let Some(parent) = tp.parent().filter(|p| !p.as_os_str().is_empty()) { std::fs::create_dir_all(parent)?; }
        // Only tar-gz today; match on args.compression() here when adding formats.
        let f = std::fs::File::create(&tp)?;
        let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut t = tar::Builder::new(gz);
        t.append_dir_all(&stem, &dir)?;
        t.into_inner()?.finish()?;
        log_info(format_args!("bundle {}", tp.display()));
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
            title: "t".into(), content: "hello multi".into(), posted_at: "2026-10-15T00:00:00Z".into(), media: vec![] }
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
}
