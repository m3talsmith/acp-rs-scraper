//! Local files a New post or Reply would attach.
//!
//! Any regular file stays on the local copy. The site copy takes images and
//! videos only (`image/*` and `video/*`); see
//! [`SITE_ATTACHMENT_MIME_PREFIXES`](crate::config::SITE_ATTACHMENT_MIME_PREFIXES).
//! This module checks the path. It does not build an `allow_upload` preflight
//! or an `lvu` frame.

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

/// Check one path. `~/…` is expanded before the file is checked. Missing paths and directories are refused. Any other file is kept locally. Nothing is uploaded.
pub fn plan_attachment(path: &Path) -> Result<PlannedAttachment, String> {
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
    let kind = if on_site { AttachmentKind::Media } else { AttachmentKind::File };
    let bytes = std::fs::metadata(&path)
        .map(|m| m.len())
        .map_err(|e| format!("could not read the size of {shown}: {e}. Nothing was sent."))?;
    Ok(PlannedAttachment { path, name, kind, bytes, on_site })
}

/// Which files are the site copy, and which stay on the local and network copies only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteSplit {
    /// 0-based indexes that go to the site, in the order chosen.
    pub site: Vec<usize>,
    /// 0-based indexes left off the site copy.
    pub local_only: Vec<usize>,
    /// No choice was given and the list was longer than the site cap, so the first files were kept.
    pub took_default: bool,
}

impl SiteSplit {
    /// The sentence printed when the first images or videos were kept because no choice was given.
    pub fn notice(&self, files: &[PlannedAttachment]) -> Option<String> {
        if !self.took_default {
            return None;
        }
        let max = crate::config::SITE_MAX_ATTACHMENTS;
        let kinds = crate::config::site_attachment_kinds();
        let eligible = files.iter().filter(|f| crate::config::site_accepts_path(&f.path)).count();
        let kept = self.site.len();
        let left = self.local_only.iter().filter_map(|&i| files.get(i).map(|f| format!("{} {}", i + 1, f.name))).collect::<Vec<_>>().join(", ");
        Some(format!("site copy keeps the first {kept} of {eligible} images or videos (Red Star allows {max}: {kinds}). Left off the site: {left}"))
    }
}

/// Split attachments into the site subset.
///
/// Only images and videos are eligible
/// ([`site_accepts_path`](crate::config::site_accepts_path)). `chosen` is
/// 0-based. `None` keeps every eligible file when there are at most
/// [`SITE_MAX_ATTACHMENTS`](crate::config::SITE_MAX_ATTACHMENTS) of them, and
/// the first ones when there are more. Ineligible files are skipped. `Some`
/// is checked even when the list is short: duplicates, indexes past the list,
/// more than the site cap, and a file that is not an image or a video are
/// refused. The refusal names that file. An empty choice sends no file to the site.
pub fn split_for_site(files: &[PlannedAttachment], chosen: Option<&[usize]>) -> Result<SiteSplit, String> {
    let max = crate::config::SITE_MAX_ATTACHMENTS;
    let count = files.len();
    let eligible: Vec<usize> = files.iter().enumerate().filter(|(_, f)| crate::config::site_accepts_path(&f.path)).map(|(i, _)| i).collect();
    let (site, took_default) = match chosen {
        None if eligible.len() <= max => (eligible, false),
        None => (eligible.into_iter().take(max).collect(), true),
        Some(picked) => {
            if picked.len() > max {
                return Err(format!("Red Star allows {max} attachments on the site copy; {} were chosen", picked.len()));
            }
            let mut seen = std::collections::HashSet::new();
            for &i in picked {
                if i >= count {
                    let shown = i + 1;
                    return Err(if count == 0 {
                        format!("attachment {shown} is not in the list (there are no attachments)")
                    } else {
                        format!("attachment {shown} is not in the list (1 to {count})")
                    });
                }
                if !seen.insert(i) {
                    return Err(format!("attachment {} is listed twice", i + 1));
                }
                if !crate::config::site_accepts_path(&files[i].path) {
                    return Err(crate::config::site_attachment_refusal(&files[i].name));
                }
            }
            (picked.to_vec(), false)
        }
    };
    let local_only = (0..count).filter(|i| !site.contains(i)).collect();
    Ok(SiteSplit { site, local_only, took_default })
}

/// Classify every path, in order.
pub fn plan_attachments(paths: &[PathBuf]) -> Result<Vec<PlannedAttachment>, String> {
    paths.iter().map(|p| plan_attachment(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SCRATCH: AtomicU64 = AtomicU64::new(0);

    fn scratch(label: &str) -> PathBuf {
        let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("acp-path-{label}-{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tilde_alone_becomes_a_home_prefix_and_tilde_user_is_left_alone() {
        let home = scratch("home");
        std::fs::write(home.join("photo.jpg"), b"j").unwrap();
        assert_eq!(complete_path("~", Some(&home), &home).text, "~/");
        assert!(complete_path("~", Some(&home), &home).matches.is_empty());
        let user = complete_path("~nobody/photo", Some(&home), &home);
        assert_eq!(user.text, "~nobody/photo");
        assert!(user.matches.is_empty());
        let listed = complete_path("~/", Some(&home), &home);
        assert_eq!(listed.text, "~/photo.jpg");
        assert!(listed.matches.is_empty());
        assert_eq!(expand_with_home(Path::new("~/photo.jpg"), Some(&home)), home.join("photo.jpg"));
        assert_eq!(expand_with_home(Path::new("~nobody"), Some(&home)), PathBuf::from("~nobody"));
        let planned = plan_attachment(&home.join("photo.jpg")).unwrap();
        assert_eq!(planned.path, home.join("photo.jpg"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_unique_file_fills_and_a_unique_directory_gains_a_slash() {
        let cwd = scratch("cwd");
        std::fs::write(cwd.join("photo.jpg"), b"j").unwrap();
        std::fs::create_dir(cwd.join("pics")).unwrap();
        std::fs::write(cwd.join("pics").join("a.jpg"), b"a").unwrap();
        let file = complete_path("ph", None, &cwd);
        assert_eq!(file.text, "photo.jpg");
        assert!(file.matches.is_empty());
        let again = complete_path("photo.jpg", None, &cwd);
        assert_eq!(again.text, "photo.jpg");
        assert!(again.matches.is_empty());
        let dir = complete_path("pi", None, &cwd);
        assert_eq!(dir.text, "pics/");
        assert!(dir.matches.is_empty());
        let kids = complete_path("pics/", None, &cwd);
        assert_eq!(kids.text, "pics/a.jpg");
        assert!(kids.matches.is_empty());
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn several_matches_share_a_prefix_and_keep_the_typed_form() {
        let cwd = scratch("many");
        std::fs::write(cwd.join("car.jpg"), b"c").unwrap();
        std::fs::write(cwd.join("cat.jpg"), b"t").unwrap();
        std::fs::create_dir(cwd.join("cards")).unwrap();
        let done = complete_path("ca", None, &cwd);
        assert_eq!(done.text, "ca");
        assert_eq!(done.matches, vec!["car.jpg".to_string(), "cards/".to_string(), "cat.jpg".to_string()]);
        let home = scratch("many-home");
        std::fs::create_dir(home.join("shots")).unwrap();
        std::fs::write(home.join("shots").join("car.jpg"), b"c").unwrap();
        std::fs::write(home.join("shots").join("cat.jpg"), b"t").unwrap();
        let nested = complete_path("~/shots/ca", Some(&home), &cwd);
        assert_eq!(nested.text, "~/shots/ca");
        assert_eq!(nested.matches, vec!["~/shots/car.jpg".to_string(), "~/shots/cat.jpg".to_string()]);
        let _ = std::fs::remove_dir_all(&cwd);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn dotfiles_case_missing_dirs_and_non_utf8_names_are_skipped() {
        let cwd = scratch("skip");
        std::fs::write(cwd.join(".hidden.jpg"), b"h").unwrap();
        std::fs::write(cwd.join("Photo.jpg"), b"P").unwrap();
        std::fs::write(cwd.join("photo.jpg"), b"p").unwrap();
        let plain = complete_path("", None, &cwd);
        assert!(!plain.matches.iter().any(|m| m.starts_with('.')) && plain.text != ".hidden.jpg");
        assert!(plain.matches.contains(&"Photo.jpg".to_string()));
        assert!(plain.matches.contains(&"photo.jpg".to_string()));
        let dot = complete_path(".", None, &cwd);
        assert_eq!(dot.text, ".hidden.jpg");
        let upper = complete_path("P", None, &cwd);
        assert_eq!(upper.text, "Photo.jpg");
        assert!(upper.matches.is_empty());
        let missing = complete_path("no-such-dir/ph", None, &cwd);
        assert_eq!(missing.text, "no-such-dir/ph");
        assert!(missing.matches.is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let odd = std::ffi::OsString::from_vec(vec![0xff, 0xfe]);
            std::fs::write(cwd.join(&odd), b"x").unwrap();
            let listed = complete_path("", None, &cwd);
            assert_eq!(listed.matches, vec!["Photo.jpg".to_string(), "photo.jpg".to_string()]);
            std::fs::create_dir(cwd.join("pics-target")).unwrap();
            std::os::unix::fs::symlink("pics-target", cwd.join("linkdir")).unwrap();
            let link = complete_path("linkd", None, &cwd);
            assert_eq!(link.text, "linkdir/");
        }
        let _ = std::fs::remove_dir_all(&cwd);
    }

    fn planned(name: &str) -> PlannedAttachment {
        PlannedAttachment { path: PathBuf::from(name), name: name.into(), kind: AttachmentKind::File, bytes: 1, on_site: false }
    }

    #[test]
    fn only_images_and_videos_are_eligible_for_the_site() {
        assert_eq!(crate::config::SITE_ATTACHMENT_MIME_PREFIXES, &["image/", "video/"]);
        assert!(crate::config::site_accepts_path(Path::new("photo.jpg")));
        assert!(crate::config::site_accepts_path(Path::new("clip.MP4")));
        assert!(crate::config::site_accepts_path(Path::new("film.mkv")));
        assert!(crate::config::site_accepts_path(Path::new("scan.tif")));
        assert!(crate::config::site_accepts_mime("image/png; charset=binary"));
        assert!(!crate::config::site_accepts_path(Path::new("talk.mp3")));
        assert!(!crate::config::site_accepts_path(Path::new("notes.pdf")));
        assert!(!crate::config::site_accepts_path(Path::new("notes.txt")));
        assert!(!crate::config::site_accepts_mime("audio/mpeg"));
        assert!(crate::config::site_attachment_kinds().contains("image/*"));
        assert!(crate::config::site_attachment_kinds().contains("video/*"));
    }

    #[test]
    fn the_site_split_keeps_four_images_or_videos_and_skips_the_rest() {
        let max = crate::config::SITE_MAX_ATTACHMENTS;
        assert_eq!(max, 4);
        let four: Vec<_> = (1..=4).map(|i| planned(&format!("{i}.jpg"))).collect();
        let all = split_for_site(&four, None).unwrap();
        assert_eq!(all.site, vec![0, 1, 2, 3]);
        assert!(all.local_only.is_empty());
        assert!(!all.took_default);
        let short: Vec<_> = (1..=2).map(|i| planned(&format!("{i}.png"))).collect();
        let short = split_for_site(&short, None).unwrap();
        assert_eq!(short.site, vec![0, 1]);
        assert!(!short.took_default);
        let six: Vec<_> = (1..=6).map(|i| planned(&format!("{i}.jpg"))).collect();
        let over = split_for_site(&six, None).unwrap();
        assert_eq!(over.site, vec![0, 1, 2, 3]);
        assert_eq!(over.local_only, vec![4, 5]);
        assert!(over.took_default);
        let picked = split_for_site(&six, Some(&[0, 2, 4, 5])).unwrap();
        assert_eq!(picked.site, vec![0, 2, 4, 5]);
        assert_eq!(picked.local_only, vec![1, 3]);
        assert!(!picked.took_default);
        let none = split_for_site(&six, Some(&[])).unwrap();
        assert!(none.site.is_empty());
        assert_eq!(none.local_only, vec![0, 1, 2, 3, 4, 5]);
        assert!(split_for_site(&six, Some(&[0, 1, 2, 3, 4])).unwrap_err().contains("allows 4"));
        assert!(split_for_site(&six, Some(&[1, 1])).unwrap_err().contains("listed twice"));
        assert!(split_for_site(&six, Some(&[6])).unwrap_err().contains("not in the list"));
        let three: Vec<_> = (1..=3).map(|i| planned(&format!("{i}.webm"))).collect();
        assert!(split_for_site(&three, Some(&[0, 2])).is_ok());

        let mut mix = vec![planned("talk.mp3")];
        mix.extend((1..=5).map(|i| planned(&format!("clip{i}.mp4"))));
        let skipped = split_for_site(&mix, None).unwrap();
        assert_eq!(skipped.site, vec![1, 2, 3, 4], "auto-selection skips audio and keeps the first 4 videos");
        assert_eq!(skipped.local_only, vec![0, 5]);
        assert!(skipped.took_default);
        let err = split_for_site(&mix, Some(&[0])).unwrap_err();
        assert!(err.contains("talk.mp3"), "{err}");
        assert!(err.contains("image/*"), "{err}");
        assert!(err.contains("video/*"), "{err}");
        let images_around_audio = vec![planned("one.jpg"), planned("talk.mp3"), planned("two.png")];
        let under = split_for_site(&images_around_audio, None).unwrap();
        assert_eq!(under.site, vec![0, 2]);
        assert_eq!(under.local_only, vec![1]);
        assert!(!under.took_default);
    }
}
