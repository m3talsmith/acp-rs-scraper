//! Theme files: every theme, the default included, is `~/.acp/themes/<name>.toml`
//! ([`format`] describes the file). Themes are read from that folder only: nothing
//! is built into the binary or installed by the packages. The first run with an empty
//! folder downloads `default.toml` from the repo ([`download`]). A missing or broken
//! theme falls back to the terminal's own colours (`Color::Reset`) with a notice;
//! broken files are never overwritten.

pub mod download;
pub mod format;

pub use format::*;
use crate::prelude::*;

/// `~/.acp/themes`.
pub fn themes_dir() -> Result<PathBuf> { Ok(home()?.join(THEMES_REL_DIR)) }

/// `dir/<name>.toml`, refusing names that are not [`valid_name`] (no path tricks).
pub fn theme_path(dir: &Path, name: &str) -> std::result::Result<PathBuf, String> {
    if !valid_name(name) { return Err(format!("invalid theme name {name:?}: use 1–32 of a-z, 0-9, - and _")); }
    Ok(dir.join(format!("{name}.toml")))
}

/// Why a theme could not be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The name is not allowed.
    BadName(String),
    /// No such file.
    Missing(PathBuf),
    /// Unreadable, too big, not TOML, or a newer format.
    Broken(PathBuf, String),
}

impl LoadError {
    /// What went wrong and how to fix it, naming the file.
    pub fn notice(&self) -> String {
        match self {
            LoadError::BadName(e) => e.clone(),
            LoadError::Missing(p) => format!("theme file {} is missing; using the terminal's colours. Pick a theme with F5, \
                create one in the theme editor (Shift-F5), or download the default with `acp-rs-scraper --theme-init` \
                (Theme: init in the command palette).", p.display()),
            LoadError::Broken(p, e) => format!("theme file {} can't be used: {e}. Using the terminal's colours; fix the file \
                (or move it away and run `acp-rs-scraper --theme-init` to download the default again), or pick another theme with F5. It is never overwritten.", p.display()),
        }
    }
}

/// A theme read from disk.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    /// The file.
    pub path: PathBuf,
    /// Its contents.
    pub file: ThemeFile,
    /// Keys that were skipped.
    pub warnings: Vec<String>,
}

/// Read and check one file (size limit, TOML, format version).
pub fn load_path(path: &Path) -> std::result::Result<Loaded, LoadError> {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(LoadError::Missing(path.into())),
        Err(e) => return Err(LoadError::Broken(path.into(), e.to_string())),
    };
    if meta.len() > THEME_MAX_BYTES { return Err(LoadError::Broken(path.into(), format!("it is {} bytes; theme files are at most {} KiB", meta.len(), THEME_MAX_BYTES / 1024))); }
    let src = std::fs::read_to_string(path).map_err(|e| LoadError::Broken(path.into(), e.to_string()))?;
    let (mut file, warnings) = parse(&src, &stem).map_err(|e| LoadError::Broken(path.into(), e))?;
    file.name = stem;
    Ok(Loaded { path: path.into(), file, warnings })
}

/// Read theme `name` from `dir`.
pub fn load(dir: &Path, name: &str) -> std::result::Result<Loaded, LoadError> {
    load_path(&theme_path(dir, name).map_err(LoadError::BadName)?)
}

/// One theme file, for the picker and `--list-themes`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Name (the file stem).
    pub name: String,
    /// Its description.
    pub description: String,
    /// Why it can't be used, if so.
    pub error: Option<String>,
    /// How many keys were skipped.
    pub warnings: usize,
}

/// Every `*.toml` in `dir` with a valid name, sorted by name.
pub fn list(dir: &Path) -> Vec<Entry> {
    let mut v: Vec<Entry> = std::fs::read_dir(dir).into_iter().flatten().flatten()
        .filter_map(|e| {
            let p = e.path();
            let name = p.file_name()?.to_str()?.strip_suffix(".toml")?.to_string();
            valid_name(&name).then(|| match load_path(&p) {
                Ok(l) => Entry { name, description: l.file.description, error: None, warnings: l.warnings.len() },
                Err(LoadError::Broken(_, e)) => Entry { name, description: String::new(), error: Some(e), warnings: 0 },
                Err(e) => Entry { name, description: String::new(), error: Some(e.notice()), warnings: 0 },
            })
        }).collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// Write `bytes` to `path` atomically (a temp file in the same folder, renamed over it), mode 0600.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = write_tmp(path, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| { let _ = std::fs::remove_file(&tmp); })
}

/// Like [`atomic_write`] but never replaces an existing file; `Ok(false)` when one is there.
pub fn atomic_write_new(path: &Path, bytes: &[u8]) -> io::Result<bool> {
    if path.exists() { return Ok(false); }
    let tmp = write_tmp(path, bytes)?;
    // A hard link fails when the target exists, so a file that appeared meanwhile is kept.
    let r = match std::fs::hard_link(&tmp, path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) if !path.exists() => std::fs::rename(&tmp, path).map(|_| true),
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_file(&tmp);
    r
}

fn write_tmp(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let dir = path.parent().ok_or_else(|| io::Error::other("no parent folder"))?;
    create_dir_private(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    let mut f = o.open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?; }
    Ok(tmp)
}

fn create_dir_private(dir: &Path) -> io::Result<()> {
    if dir.is_dir() { return Ok(()); }
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?; }
    Ok(())
}

/// Save `f` as `dir/<f.name>.toml` (atomic, 0600).
pub fn save(dir: &Path, f: &ThemeFile) -> Result<PathBuf> {
    let p = theme_path(dir, &f.name).map_err(|e| anyhow!(e))?;
    atomic_write(&p, to_toml(f).as_bytes()).with_context(|| format!("writing {}", p.display()))?;
    Ok(p)
}

/// Delete theme `name`.
pub fn delete(dir: &Path, name: &str) -> Result<()> {
    let p = theme_path(dir, name).map_err(|e| anyhow!(e))?;
    std::fs::remove_file(&p).with_context(|| format!("deleting {}", p.display()))
}

fn toml_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml") && p.file_stem().and_then(|s| s.to_str()).is_some_and(valid_name))
        .collect();
    v.sort();
    v
}

/// The warnings of one file as a single notice (`None` when there are none).
pub fn warning_notice(path: &Path, warnings: &[String]) -> Option<String> {
    match warnings {
        [] => None,
        [w] => Some(format!("warning: {}: {w}", path.display())),
        ws => Some(format!("warning: {}: {} keys skipped: {}", path.display(), ws.len(), ws.join("; "))),
    }
}

/// Whether `dir` holds any theme file (an empty or missing folder means a first run).
pub fn has_themes(dir: &Path) -> bool { !toml_files(dir).is_empty() }

/// Whether the old `[theme]` table of preferences.toml sets anything.
pub fn legacy_set(p: &ThemePrefs) -> bool { p.accent.is_some() || p.muted.is_some() || p.border.is_some() || p.on_accent.is_some() }

/// The old `[theme]` colours as a theme: `base` (normally the default theme) with
/// them applied to both variants. Invalid colours are reported and skipped.
pub fn from_legacy(name: &str, legacy: &ThemePrefs, base: Option<&ThemeFile>) -> (ThemeFile, Vec<String>) {
    let mut f = base.cloned().unwrap_or_else(|| ThemeFile::empty(name));
    f.name = name.into();
    f.description = "Converted from the [theme] colours in ~/.acp/preferences.toml".into();
    let mut warn = Vec::new();
    for (k, v) in [("accent", &legacy.accent), ("muted", &legacy.muted), ("border", &legacy.border), ("on_accent", &legacy.on_accent)] {
        let Some(v) = v else { continue };
        // The old table took `#rrggbb` or bare `rrggbb`.
        let bare = v.len() == 6 && v.chars().all(|c| c.is_ascii_hexdigit());
        match parse_color_str(&if bare { format!("#{v}") } else { v.clone() }) {
            Ok(ColorSpec::Value(c)) => {
                f.dark.set(k, ColorSpec::Value(c));
                if let Some(l) = f.light.as_mut() { l.set(k, ColorSpec::Value(c)); }
            }
            _ => warn.push(format!("[theme] {k} = {v:?} is not a colour (skipped)")),
        }
    }
    (f, warn)
}

/// Save the old `[theme]` colours as a new theme file in `dir`: [`MIGRATED_THEME`],
/// or `custom-2`, `custom-3`… when that name is taken. Returns the name and warnings.
pub fn migrate_legacy(dir: &Path, legacy: &ThemePrefs) -> Result<(String, Vec<String>)> {
    let base = load(dir, DEFAULT_THEME).ok().map(|l| l.file);
    for i in 1..100 {
        let name = if i == 1 { MIGRATED_THEME.to_string() } else { format!("{MIGRATED_THEME}-{i}") };
        let (f, warn) = from_legacy(&name, legacy, base.as_ref());
        let p = theme_path(dir, &name).map_err(|e| anyhow!(e))?;
        if atomic_write_new(&p, to_toml(&f).as_bytes()).with_context(|| format!("writing {}", p.display()))? { return Ok((name, warn)); }
    }
    bail!("no free name for the migrated theme in {}", dir.display())
}

/// Theme mode from `[ui] theme_mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Always the dark variant.
    Dark,
    /// Always the light variant.
    Light,
    /// From the terminal's background (`COLORFGBG`, or asking the terminal); dark if unknown.
    #[default]
    System,
}

impl Mode {
    /// `dark` / `light` / `system` (anything else: system).
    pub fn parse(s: Option<&str>) -> Mode {
        match s.map(|s| s.trim().to_ascii_lowercase()).as_deref() { Some("dark") => Mode::Dark, Some("light") => Mode::Light, _ => Mode::System }
    }
    /// The variant, given what the terminal's background looks like (`Some(true)`: dark).
    pub fn variant(self, dark_bg: Option<bool>) -> Variant {
        match self { Mode::Dark => Variant::Dark, Mode::Light => Variant::Light, Mode::System => if dark_bg == Some(false) { Variant::Light } else { Variant::Dark } }
    }
}

/// `COLORFGBG` ("fg;bg", set by rxvt, Konsole and others): `Some(true)` for a dark background.
pub fn colorfgbg_dark(v: Option<&str>) -> Option<bool> {
    let bg: u8 = v?.rsplit(';').next()?.trim().parse().ok()?;
    Some(!matches!(bg, 7 | 9..=15))
}

/// An OSC 11 reply (`ESC ] 11 ; rgb:rrrr/gggg/bbbb`): `Some(true)` for a dark background.
pub fn osc11_dark(reply: &str) -> Option<bool> {
    let i = reply.find("]11;rgb:")?;
    let rest = &reply[i + 8..];
    let mut it = rest.split(['/', '\x07', '\x1b']);
    let mut ch = || -> Option<f64> {
        let h = it.next()?;
        let h = &h[..h.len().min(4)];
        let v = u32::from_str_radix(h, 16).ok()? as f64;
        Some(v / ((1u32 << (4 * h.len())) - 1) as f64)
    };
    let (r, g, b) = (ch()?, ch()?, ch()?);
    Some(0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5)
}

/// The startup work: turn an old `[theme]` table into a theme file once (when
/// `[ui] theme` is unset). Returns the migrated theme's name (to select and save) and
/// notices for the user. Downloading the default is separate ([`download`]).
pub fn prepare(dir: &Path, prefs: &Prefs) -> (Option<String>, Vec<String>) {
    let mut notes = Vec::new();
    let ui_theme = prefs.ui.as_ref().and_then(|u| u.theme.clone());
    let mut migrated = None;
    if ui_theme.is_none() && let Some(old) = prefs.theme.as_ref().filter(|t| legacy_set(t)) {
        match migrate_legacy(dir, old) {
            Ok((name, warn)) => {
                notes.push(format!("Your [theme] colours in ~/.acp/preferences.toml were converted into {} and selected; \
                    the old [theme] keys are kept but no longer used (edit the theme with Shift-F5 instead).", dir.join(format!("{name}.toml")).display()));
                if let Some(n) = warning_notice(&dir.join(format!("{name}.toml")), &warn) { notes.push(n); }
                migrated = Some(name);
            }
            Err(e) => notes.push(format!("warning: could not convert the [theme] colours: {e:#}")),
        }
    }
    (migrated, notes)
}

#[cfg(test)]
mod tests;
