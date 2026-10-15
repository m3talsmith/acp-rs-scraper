//! `--format json|html`, `--output`, `--compress --tar-gz`: writing exports.

pub mod archive;
pub mod html;
pub mod json;

use crate::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
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
    log_info(format_args!("wrote {}", file.display()));

    if let Some((tp, stem)) = tar_path {
        if let Some(parent) = tp.parent().filter(|p| !p.as_os_str().is_empty()) { std::fs::create_dir_all(parent)?; }
        // Only tar-gz today; match on args.compression() here when adding formats.
        let f = std::fs::File::create(&tp)?;
        let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut t = tar::Builder::new(gz);
        t.append_dir_all(&stem, &dir)?;
        t.into_inner()?.finish()?;
        log_info(format_args!("bundle {}", tp.display()));
    }
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
