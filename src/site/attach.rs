//! Local files a New post or Reply would attach.
//!
//! The site form names are `media_attachments` (avif, gif, jpg, png, tiff,
//! webp, mkv, mov, mp4, webm) and `file_attachments` (pdf). `jpg` includes
//! `.jpeg`, and `tiff` includes `.tif`. This module only checks the path.
//! It does not build an `allow_upload` preflight or an `lvu` frame.

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
}

/// Extensions the plan lists for `media_attachments`, plus the usual aliases.
const MEDIA_EXT: &[&str] = &[
    "avif", "gif", "jpg", "jpeg", "png", "tif", "tiff", "webp", "mkv", "mov", "mp4", "webm",
];

/// Check one path. Missing files and other types are refused. Nothing is uploaded.
pub fn plan_attachment(path: &Path) -> Result<PlannedAttachment, String> {
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
    let kind = if ext == "pdf" {
        AttachmentKind::File
    } else if MEDIA_EXT.contains(&ext.as_str()) {
        AttachmentKind::Media
    } else {
        return Err(format!(
            "{shown} is not an allowed attachment. Media is avif, gif, jpg, jpeg, png, tif, tiff, webp, mkv, mov, mp4, or webm. A file attachment is pdf. Nothing was sent."
        ));
    };
    let bytes = std::fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| format!("could not read the size of {shown}: {e}. Nothing was sent."))?;
    Ok(PlannedAttachment {
        path: path.to_path_buf(),
        name,
        kind,
        bytes,
    })
}

/// Classify every path, in order.
pub fn plan_attachments(paths: &[PathBuf]) -> Result<Vec<PlannedAttachment>, String> {
    paths.iter().map(|p| plan_attachment(p)).collect()
}
