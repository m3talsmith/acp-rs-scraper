//! Local files and nick completion. Nothing here opens a connection.

use std::path::{Path, PathBuf};

/// Which documented input the file belongs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    /// `media_attachments`.
    Media,
    /// `file_attachments`. A pdf.
    File,
}

impl AttachmentKind {
    /// The form input name from the plan. Not a captured wire field.
    pub fn input_name(self) -> &'static str {
        match self {
            AttachmentKind::Media => "media_attachments",
            AttachmentKind::File => "file_attachments",
        }
    }

    /// Short label for the preview.
    pub fn label(self) -> &'static str {
        match self {
            AttachmentKind::Media => "media",
            AttachmentKind::File => "file",
        }
    }
}

/// One local file that dry-run can list. The bytes stay on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedAttachment {
    /// Path the user named.
    pub path: PathBuf,
    /// File name, for the preview.
    pub name: String,
    /// Media or pdf.
    pub kind: AttachmentKind,
    /// Size in bytes. The file is not read.
    pub bytes: u64,
    /// This file is in the site subset. The local and network copies still keep it when this is false.
    pub on_site: bool,
}

/// `~` and `~/…` expand with `home`. `~user` is left unchanged. A path that is
/// not Unicode is left unchanged.
pub fn expand_with_home(path: &Path, home: Option<&Path>) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    if text == "~" {
        return home.map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = home {
            return if rest.is_empty() { home.to_path_buf() } else { home.join(rest) };
        }
    }
    path.to_path_buf()
}

/// What Tab should write, and the other names when more than one entry matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCompletion {
    /// The attach line after this Tab.
    pub text: String,
    /// Full typed strings when several entries match. Empty when one entry filled `text`, or when nothing matched.
    pub matches: Vec<String>,
}

/// Complete one real path segment. `~` becomes `~/`. `~/…` stays in that display form and is looked up under `home`. `~user` is not completed. Names that are not Unicode are skipped. A name starting with `.` is skipped unless the typed prefix starts with `.`. A directory match ends with `/`.
pub fn complete_path(input: &str, home: Option<&Path>, cwd: &Path) -> PathCompletion {
    if input == "~" {
        return PathCompletion { text: "~/".into(), matches: Vec::new() };
    }
    if input.starts_with('~') && !input.starts_with("~/") {
        return PathCompletion { text: input.to_string(), matches: Vec::new() };
    }
    let (display_dir, partial) = split_display(input);
    let Some(dir) = lookup_dir(&display_dir, home, cwd) else {
        return PathCompletion { text: input.to_string(), matches: Vec::new() };
    };
    let Some(names) = list_matches(&dir, &partial) else {
        return PathCompletion { text: input.to_string(), matches: Vec::new() };
    };
    if names.is_empty() {
        return PathCompletion { text: input.to_string(), matches: Vec::new() };
    }
    if names.len() == 1 {
        let (name, is_dir) = &names[0];
        return PathCompletion { text: typed(&display_dir, name, *is_dir), matches: Vec::new() };
    }
    let prefix = common_prefix(names.iter().map(|(name, _)| name.as_str()));
    let matches = names.into_iter().map(|(name, is_dir)| typed(&display_dir, &name, is_dir)).collect();
    PathCompletion { text: format!("{display_dir}{prefix}"), matches }
}

fn typed(display_dir: &str, name: &str, is_dir: bool) -> String {
    let mut text = format!("{display_dir}{name}");
    if is_dir {
        text.push('/');
    }
    text
}

fn split_display(input: &str) -> (String, String) {
    match input.rfind('/') {
        Some(i) => (input[..=i].to_string(), input[i + 1..].to_string()),
        None => (String::new(), input.to_string()),
    }
}

fn lookup_dir(display_dir: &str, home: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    if display_dir.is_empty() {
        return Some(cwd.to_path_buf());
    }
    if let Some(rest) = display_dir.strip_prefix("~/") {
        let home = home?;
        let rest = rest.trim_end_matches('/');
        return Some(if rest.is_empty() { home.to_path_buf() } else { home.join(rest) });
    }
    if display_dir.starts_with('/') {
        let rest = display_dir.trim_end_matches('/');
        return Some(if rest.is_empty() { PathBuf::from("/") } else { PathBuf::from(rest) });
    }
    let rest = display_dir.trim_end_matches('/');
    Some(cwd.join(rest))
}

/// `None` when the directory cannot be read. An empty vec means nothing matched.
fn list_matches(dir: &Path, partial: &str) -> Option<Vec<(String, bool)>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).ok()? {
        let Ok(entry) = entry else { continue };
        let Some(name) = entry.file_name().to_str().map(str::to_string) else { continue };
        if name.starts_with('.') && !partial.starts_with('.') {
            continue;
        }
        if !name.starts_with(partial) {
            continue;
        }
        names.push((name, entry.path().is_dir()));
    }
    names.sort_by(|a, b| a.0.cmp(&b.0));
    Some(names)
}

fn common_prefix<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let mut names = names.into_iter();
    let Some(first) = names.next() else { return String::new() };
    let mut prefix = first.to_string();
    for name in names {
        while !name.starts_with(&prefix) {
            prefix.pop();
            if prefix.is_empty() {
                return prefix;
            }
        }
    }
    prefix
}

/// Check one path. `~/…` is expanded before the file is checked. Missing paths and directories are refused. Any other file is kept locally. Images, videos, and PDFs can go to the site copy.
pub fn plan_attachment(path: &Path) -> std::result::Result<PlannedAttachment, String> {
    let path = expand_with_home(path, crate::config::home().ok().as_deref());
    let shown = path.display().to_string();
    if !path.exists() {
        return Err(format!("attachment not found: {shown}. Nothing was sent."));
    }
    if !path.is_file() {
        return Err(format!(
            "attachment is not a file: {shown}. Nothing was sent."
        ));
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| format!("attachment has no file name: {shown}. Nothing was sent."))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = crate::config::mime_for_attachment_ext(&ext);
    let on_site = crate::config::site_accepts_mime(mime);
    let kind = if mime.starts_with("image/") || mime.starts_with("video/") {
        AttachmentKind::Media
    } else {
        AttachmentKind::File
    };
    let bytes = std::fs::metadata(&path)
        .map(|m| m.len())
        .map_err(|e| format!("could not read the size of {shown}: {e}. Nothing was sent."))?;
    Ok(PlannedAttachment { path, name, kind, bytes, on_site })
}


/// One local nick the compose box can complete. `user_id` is empty until
/// `mention-query` has been captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NickHint {
    /// Nick without `@`.
    pub nick: String,
    /// Site user id, when this install already stored one.
    pub user_id: String,
}

/// A completion row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// What is inserted (`@nick` or `#tag`).
    pub label: String,
    /// Site id. Empty when the query reply has not been captured.
    pub id: String,
}

/// Local completion. The site event `mention-query` / `tag-query` is not sent.
pub fn completions(query: &str, nicks: &[NickHint], tags: &[String]) -> Vec<Completion> {
    let q = query.trim();
    let (sig, rest) = if let Some(r) = q.strip_prefix('@') {
        ('@', r)
    } else if let Some(r) = q.strip_prefix('#') {
        ('#', r)
    } else {
        return Vec::new();
    };
    let rest = rest.to_ascii_lowercase();
    let mut out = Vec::new();
    if sig == '@' {
        for n in nicks {
            if rest.is_empty() || n.nick.to_ascii_lowercase().contains(&rest) {
                out.push(Completion { label: format!("@{}", n.nick), id: n.user_id.clone() });
            }
        }
    } else {
        for tag in tags {
            let low = tag.to_ascii_lowercase();
            if low.starts_with("post-") {
                continue;
            }
            if rest.is_empty() || low.contains(&rest) {
                out.push(Completion { label: format!("#{tag}"), id: String::new() });
            }
        }
    }
    out.truncate(8);
    out
}

