//! Temporary directories and archive helpers for `.tar.gz` import/export.

use crate::prelude::*;

/// A directory that is deleted when dropped.
pub struct TmpDir(pub PathBuf);

impl Drop for TmpDir {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

/// First .json, else .html, in the archive (shallowest first).
pub fn find_doc(dir: &Path) -> Option<PathBuf> {
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
