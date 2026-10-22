//! Brands: the logo, the title, and the motto.
//!
//! Branding travels with a theme (`[brand]` in the theme file). The built-in
//! `default` brand is ACP Red Star. The built-in `plain` brand is a neutral title
//! and a star, with no motto. A file `~/.acp/brands/<name>.toml` still overrides a
//! built-in of the same name, and `--brand` still selects one for this run.
//! Saving a theme writes that branding into the theme. Loading, the size limit,
//! and a broken file follow the theme rules: a broken file is reported and never
//! overwritten.

use crate::branding::SLOGAN;
use crate::config::{home, BRANDS_REL_DIR, BRAND_FORMAT_VERSION, BRAND_LOGO_MAX_BYTES, BRAND_MAX_BYTES, DEFAULT_BRAND, PLAIN_BRAND, TUI_TITLE};
use crate::themes::{atomic_write, valid_name};
use anyhow::{Context, Result, anyhow, bail};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

/// How the header logo is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogoKind {
    /// The built-in red-star picture (sextants when the terminal has no graphics).
    Builtin,
    /// [`Brand::logo_text`], one line per row, in the logo's rectangle.
    Text,
    /// [`Brand::logo_file`], a picture inside `~/.acp/brands`.
    File,
}

impl LogoKind {
    /// `builtin`, `text`, or `file`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "builtin" => Some(Self::Builtin),
            "text" => Some(Self::Text),
            "file" => Some(Self::File),
            _ => None,
        }
    }
    /// The TOML value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Text => "text",
            Self::File => "file",
        }
    }
}

/// One brand: logo, title, and motto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brand {
    /// File stem, or the built-in name.
    pub name: String,
    /// One line for the picker and `--list-brands`.
    pub description: String,
    /// Header, window title, About, status, `--help`, `--version`, HTML export.
    pub title: String,
    /// Status bar and About. Empty hides it.
    pub motto: String,
    /// Big letters beside the logo. Empty draws none. The default is `ACP`.
    pub word: String,
    /// Which logo to draw.
    pub logo: LogoKind,
    /// Lines drawn when [`LogoKind::Text`].
    pub logo_text: String,
    /// Path relative to `~/.acp/brands` when [`LogoKind::File`].
    pub logo_file: String,
    /// Absolute path of a logo file that stayed inside the brands folder.
    pub logo_path: Option<PathBuf>,
    /// `true` when this came from a file, including an override of a built-in.
    pub from_file: bool,
}

impl Brand {
    /// This brand keeps the ACP wording: the title, the word ACP, or the slogan.
    ///
    /// A neutral brand has none of those, so the menus, Help, and About that
    /// follow the brand leave the party text out.
    pub fn shows_acp(&self) -> bool {
        self.title.contains("ACP") || self.word == "ACP" || self.motto == SLOGAN
    }
}

/// Why a brand could not be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The name is not allowed.
    BadName(String),
    /// No built-in and no file.
    Missing(PathBuf),
    /// Unreadable, too big, not TOML, a newer format, or a logo path that escapes.
    Broken(PathBuf, String),
}

impl LoadError {
    /// What went wrong and how to fix it, naming the file.
    pub fn notice(&self) -> String {
        match self {
            LoadError::BadName(e) => e.clone(),
            LoadError::Missing(p) => format!(
                "brand file {} is missing, and there is no built-in with that name. Using the default brand. \
                 Pick a theme with F5, or add a file under ~/.acp/brands.",
                p.display()
            ),
            LoadError::Broken(p, e) => format!(
                "brand file {} can't be used: {e}. Using the default brand. Fix the file or pick another brand. It is never overwritten.",
                p.display()
            ),
        }
    }
}

/// One row of `--list-brands` and the picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Name (the file stem, or the built-in name).
    pub name: String,
    /// Its description.
    pub description: String,
    /// The title it would show.
    pub title: String,
    /// The motto. Empty means hidden.
    pub motto: String,
    /// Why it can't be used, if so.
    pub error: Option<String>,
}

/// `~/.acp/brands`.
pub fn brands_dir() -> Result<PathBuf> {
    Ok(home()?.join(BRANDS_REL_DIR))
}

/// `dir/<name>.toml`, refusing names that are not [`valid_name`].
pub fn brand_path(dir: &Path, name: &str) -> std::result::Result<PathBuf, String> {
    if !valid_name(name) {
        return Err(format!("invalid brand name {name:?}: use 1–32 of a-z, 0-9, - and _"));
    }
    Ok(dir.join(format!("{name}.toml")))
}

/// The built-in ACP Red Star brand. Unchanged from the header this program shipped with.
pub fn builtin_default() -> Brand {
    Brand {
        name: DEFAULT_BRAND.into(),
        description: "ACP Red Star: the red star, the title, and the slogan.".into(),
        title: TUI_TITLE.into(),
        motto: SLOGAN.into(),
        word: "ACP".into(),
        logo: LogoKind::Builtin,
        logo_text: String::new(),
        logo_file: String::new(),
        logo_path: None,
        from_file: false,
    }
}

/// A neutral brand for a demo: plain title, a star, no motto, no big "ACP".
pub fn builtin_plain() -> Brand {
    Brand {
        name: PLAIN_BRAND.into(),
        description: "Neutral title and a star, with no motto.".into(),
        title: "Red Star".into(),
        motto: String::new(),
        word: String::new(),
        logo: LogoKind::Text,
        logo_text: "★".into(),
        logo_file: String::new(),
        logo_path: None,
        from_file: false,
    }
}

/// The theme's `[brand]` table for this brand.
pub fn theme_brand_of(b: &Brand) -> crate::themes::ThemeBrand {
    crate::themes::ThemeBrand {
        title: b.title.clone(),
        motto: b.motto.clone(),
        word: b.word.clone(),
        logo: b.logo.as_str().into(),
        logo_text: b.logo_text.clone(),
        logo_file: b.logo_file.clone(),
    }
}

/// A brand from a theme's `[brand]` table. A `file` logo is resolved only when `dir` is set.
pub fn from_theme_brand(name: &str, b: &crate::themes::ThemeBrand, dir: Option<&Path>) -> Brand {
    let brand = Brand {
        name: name.into(),
        description: String::new(),
        title: b.title.clone(),
        motto: b.motto.clone(),
        word: b.word.clone(),
        logo: LogoKind::parse(&b.logo).unwrap_or(LogoKind::Builtin),
        logo_text: b.logo_text.clone(),
        logo_file: b.logo_file.clone(),
        logo_path: None,
        from_file: false,
    };
    match (dir, brand.logo) {
        (Some(dir), LogoKind::File) => finalize(&brand, dir).unwrap_or(brand),
        _ => brand,
    }
}

/// A built-in brand by name.
pub fn builtin(name: &str) -> Option<Brand> {
    match name {
        DEFAULT_BRAND => Some(builtin_default()),
        PLAIN_BRAND => Some(builtin_plain()),
        _ => None,
    }
}

/// Built-in names, in picker order.
pub fn builtin_names() -> &'static [&'static str] {
    &[DEFAULT_BRAND, PLAIN_BRAND]
}

fn current_slot() -> &'static Mutex<Brand> {
    static CURRENT: Mutex<Brand> = Mutex::new(Brand {
        name: String::new(),
        description: String::new(),
        title: String::new(),
        motto: String::new(),
        word: String::new(),
        logo: LogoKind::Builtin,
        logo_text: String::new(),
        logo_file: String::new(),
        logo_path: None,
        from_file: false,
    });
    // First read fills the real default. An empty name marks the unset slot.
    {
        let mut g = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
        if g.name.is_empty() {
            *g = builtin_default();
        }
    }
    &CURRENT
}

/// The brand last installed for this process (CLI help, HTML export, notifications).
pub fn current() -> Brand {
    current_slot().lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn gate() -> &'static std::sync::Mutex<()> {
    static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    &GATE
}

/// Remember `brand` for CLI help, HTML export, and desktop notifications.
pub fn install(brand: &Brand) {
    let _gate = gate().lock().unwrap_or_else(|e| e.into_inner());
    *current_slot().lock().unwrap_or_else(|e| e.into_inner()) = brand.clone();
}

/// Install `brand` and keep it in place for `f`.
///
/// The About header reads this process-wide brand. A test that draws About
/// holds the default across that draw so another test's `install` waits.
pub fn hold_installed<R>(brand: &Brand, f: impl FnOnce() -> R) -> R {
    let _gate = gate().lock().unwrap_or_else(|e| e.into_inner());
    *current_slot().lock().unwrap_or_else(|e| e.into_inner()) = brand.clone();
    f()
}

/// The brand for this run: `--brand`, else `[ui] brand`, else `default`.
///
/// A broken or unknown name falls back to the default brand and returns a notice.
/// This does not write the file.
pub fn selected(explicit: Option<&str>) -> (Brand, Option<String>) {
    let name = explicit
        .map(str::to_string)
        .or_else(|| crate::prefs::load_prefs().ui.and_then(|u| u.brand))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_BRAND.into());
    let dir = match brands_dir() {
        Ok(d) => d,
        Err(_) => return (builtin(&name).unwrap_or_else(builtin_default), None),
    };
    match resolve(&dir, &name) {
        Ok(b) => (b, None),
        // An explicit flag is also refused by the caller. A preference still starts.
        Err(e) => (builtin_default(), Some(e.notice())),
    }
}

/// Read brand `name`: a user file wins over a built-in of the same name.
pub fn resolve(dir: &Path, name: &str) -> std::result::Result<Brand, LoadError> {
    let path = brand_path(dir, name).map_err(LoadError::BadName)?;
    if path.is_file() {
        return load_path(&path, dir);
    }
    builtin(name).ok_or(LoadError::Missing(path))
}

/// Read and check one file (size limit, TOML, format version, logo path).
pub fn load_path(path: &Path, dir: &Path) -> std::result::Result<Brand, LoadError> {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if !valid_name(&stem) {
        return Err(LoadError::BadName(format!("invalid brand name {stem:?}: use 1–32 of a-z, 0-9, - and _")));
    }
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(LoadError::Missing(path.into())),
        Err(e) => return Err(LoadError::Broken(path.into(), e.to_string())),
    };
    if meta.len() > BRAND_MAX_BYTES {
        return Err(LoadError::Broken(path.into(), format!("it is {} bytes; brand files are at most {} KiB", meta.len(), BRAND_MAX_BYTES / 1024)));
    }
    let src = std::fs::read_to_string(path).map_err(|e| LoadError::Broken(path.into(), e.to_string()))?;
    let mut brand = parse(&src, &stem, dir).map_err(|e| LoadError::Broken(path.into(), e))?;
    brand.from_file = true;
    Ok(brand)
}

/// Every built-in, plus every valid `*.toml` in `dir`. A file replaces the built-in of that name.
pub fn list(dir: &Path) -> Vec<Entry> {
    let mut names: Vec<String> = builtin_names().iter().map(|s| (*s).to_string()).collect();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let Some(name) = p.file_name().and_then(|s| s.to_str()).and_then(|s| s.strip_suffix(".toml")) else { continue };
            if valid_name(name) && !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    // Keep default first, then plain, then the rest alphabetically. sort() would put default first
    // among builtins only by chance (`default` < `plain`). Custom names sort after a stable pass.
    names.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.cmp(b)));
    names.into_iter().map(|name| match resolve(dir, &name) {
        Ok(b) => Entry { name, description: b.description, title: b.title, motto: b.motto, error: None },
        Err(LoadError::Broken(_, e)) => Entry { name, description: String::new(), title: String::new(), motto: String::new(), error: Some(e) },
        Err(e) => Entry { name, description: String::new(), title: String::new(), motto: String::new(), error: Some(e.notice()) },
    }).collect()
}

fn rank(name: &str) -> u8 {
    match name {
        DEFAULT_BRAND => 0,
        PLAIN_BRAND => 1,
        _ => 2,
    }
}

/// Check `draft` and resolve a file logo. `dir` is `~/.acp/brands`.
pub fn finalize(draft: &Brand, dir: &Path) -> std::result::Result<Brand, String> {
    if !valid_name(&draft.name) {
        return Err(format!("invalid brand name {:?}: use 1–32 of a-z, 0-9, - and _", draft.name));
    }
    let mut b = draft.clone();
    b.title = one_line(&b.title, "title", 80)?;
    if b.title.is_empty() { return Err("title is empty".into()); }
    b.motto = one_line(&b.motto, "motto", 80)?;
    b.word = one_line(&b.word, "word", 12)?;
    b.description = one_line(&b.description, "description", 200)?;
    if b.description.is_empty() {
        b.description = b.title.clone();
    }
    match b.logo {
        LogoKind::Builtin => {
            b.logo_text.clear();
            b.logo_file.clear();
            b.logo_path = None;
        }
        LogoKind::Text => {
            let text = b.logo_text.replace("\r\n", "\n");
            if text.trim().is_empty() { return Err("logo_text is empty".into()); }
            if text.len() > 4096 { return Err("logo_text is longer than 4 KiB".into()); }
            let lines: Vec<&str> = text.split('\n').collect();
            if lines.len() > 16 { return Err("logo_text has more than 16 lines".into()); }
            if lines.iter().any(|l| l.chars().count() > 32) { return Err("a logo_text line is longer than 32 characters".into()); }
            b.logo_text = text;
            b.logo_file.clear();
            b.logo_path = None;
        }
        LogoKind::File => {
            b.logo_path = Some(logo_file_path(dir, &b.logo_file)?);
            b.logo_text.clear();
        }
    }
    Ok(b)
}

fn one_line(s: &str, field: &str, max: usize) -> std::result::Result<String, String> {
    if s.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
        return Err(format!("{field} must be one line"));
    }
    if s.chars().count() > max {
        return Err(format!("{field} is longer than {max} characters"));
    }
    Ok(s.trim().to_string())
}

/// Resolve `rel` to a file inside `dir`. Absolute paths, `..`, and NUL are refused.
/// A symlink that leaves the folder is refused. A missing file is an error.
pub fn logo_file_path(dir: &Path, rel: &str) -> std::result::Result<PathBuf, String> {
    if rel.is_empty() { return Err("logo_file is empty".into()); }
    if rel.contains('\0') { return Err("logo path contains a NUL byte".into()); }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err("logo path must stay inside ~/.acp/brands (no absolute path)".into());
    }
    if rel_path.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_))) {
        return Err("logo path must stay inside ~/.acp/brands (no ..)".into());
    }
    let joined = dir.join(rel_path);
    let mut normal = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => return Err("logo path must stay inside ~/.acp/brands".into()),
            other => normal.push(other.as_os_str()),
        }
    }
    let dir_canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    if !normal.starts_with(dir) && !normal.starts_with(&dir_canon) {
        return Err("logo path must stay inside ~/.acp/brands".into());
    }
    if !normal.is_file() {
        return Err(format!("logo file {} is missing", normal.display()));
    }
    let canon = normal.canonicalize().map_err(|e| format!("logo file {}: {e}", normal.display()))?;
    if !canon.starts_with(&dir_canon) {
        return Err("logo path must stay inside ~/.acp/brands".into());
    }
    let len = std::fs::metadata(&canon).map(|m| m.len()).unwrap_or(0);
    if len > BRAND_LOGO_MAX_BYTES {
        return Err(format!("logo file is {len} bytes; logo files are at most {} MiB", BRAND_LOGO_MAX_BYTES / 1024 / 1024));
    }
    Ok(canon)
}

fn parse(src: &str, stem: &str, dir: &Path) -> std::result::Result<Brand, String> {
    let table: toml::Table = toml::from_str(src).map_err(|e| format!("not valid TOML ({e})"))?;
    let version = table.get("version").and_then(|v| v.as_integer()).ok_or("version = 1 is required")?;
    if version > BRAND_FORMAT_VERSION {
        return Err(format!("format version {version} is newer than this program ({BRAND_FORMAT_VERSION})"));
    }
    if version < 1 { return Err(format!("format version {version} is not supported")); }
    let mut base = builtin(stem).unwrap_or_else(|| Brand {
        name: stem.into(),
        description: String::new(),
        title: String::new(),
        motto: String::new(),
        word: String::new(),
        logo: LogoKind::Text,
        logo_text: "★".into(),
        logo_file: String::new(),
        logo_path: None,
        from_file: true,
    });
    base.name = stem.into();
    // A user file must state its own title. Inheriting a built-in's title would hide a typo.
    if builtin(stem).is_none() {
        base.title.clear();
        base.motto.clear();
        base.word.clear();
        base.description.clear();
    }
    for (k, v) in &table {
        match k.as_str() {
            "version" | "name" => {}
            "description" => base.description = v.as_str().ok_or("description must be a string")?.to_string(),
            "title" => base.title = v.as_str().ok_or("title must be a string")?.to_string(),
            "motto" => base.motto = v.as_str().ok_or("motto must be a string")?.to_string(),
            "word" => base.word = v.as_str().ok_or("word must be a string")?.to_string(),
            "logo" => {
                let s = v.as_str().ok_or("logo must be builtin, text, or file")?;
                base.logo = LogoKind::parse(s).ok_or_else(|| format!("logo {s:?} must be builtin, text, or file"))?;
            }
            "logo_text" => base.logo_text = v.as_str().ok_or("logo_text must be a string")?.to_string(),
            "logo_file" => base.logo_file = v.as_str().ok_or("logo_file must be a string")?.to_string(),
            _ => {}
        }
    }
    if !table.contains_key("title") && builtin(stem).is_none() {
        return Err("title is required".into());
    }
    finalize(&base, dir)
}

/// TOML for `brand`, format version 1.
pub fn to_toml(brand: &Brand) -> String {
    let mut s = String::new();
    s.push_str(&format!("version = {BRAND_FORMAT_VERSION}\n"));
    s.push_str(&format!("name = {}\n", toml_str(&brand.name)));
    s.push_str(&format!("description = {}\n", toml_str(&brand.description)));
    s.push_str(&format!("title = {}\n", toml_str(&brand.title)));
    s.push_str(&format!("motto = {}\n", toml_str(&brand.motto)));
    s.push_str(&format!("word = {}\n", toml_str(&brand.word)));
    s.push_str(&format!("logo = {:?}\n", brand.logo.as_str()));
    if brand.logo == LogoKind::Text {
        s.push_str(&format!("logo_text = {}\n", toml_str(&brand.logo_text)));
    }
    if brand.logo == LogoKind::File {
        s.push_str(&format!("logo_file = {}\n", toml_str(&brand.logo_file)));
    }
    s
}

fn toml_str(s: &str) -> String {
    // toml::Value quotes and escapes.
    toml::Value::String(s.into()).to_string()
}

/// Save `brand` as `dir/<name>.toml` (atomic, mode 0600).
pub fn save(dir: &Path, brand: &Brand) -> Result<PathBuf> {
    let brand = finalize(brand, dir).map_err(|e| anyhow!(e))?;
    let p = brand_path(dir, &brand.name).map_err(|e| anyhow!(e))?;
    atomic_write(&p, to_toml(&brand).as_bytes()).with_context(|| format!("writing {}", p.display()))?;
    Ok(p)
}

/// Refuse `name` when it is not a built-in and not a usable file.
pub fn check(name: &str) -> Result<()> {
    let dir = brands_dir()?;
    match resolve(&dir, name) {
        Ok(_) => Ok(()),
        Err(LoadError::Missing(_)) => {
            let have: Vec<String> = list(&dir).into_iter().filter(|e| e.error.is_none()).map(|e| e.name).collect();
            bail!("no brand {name} (brands: {})", if have.is_empty() { "none".into() } else { have.join(", ") })
        }
        Err(LoadError::BadName(e)) => bail!("{e}"),
        Err(e) => bail!("{}", e.notice()),
    }
}

/// Delete the user file for `name`. A built-in with no file is left as it is.
pub fn delete(dir: &Path, name: &str) -> Result<()> {
    let p = brand_path(dir, name).map_err(|e| anyhow!(e))?;
    if !p.exists() {
        if builtin(name).is_some() {
            bail!("{name} is a built-in brand; there is no file to delete");
        }
        bail!("no brand file {}", p.display());
    }
    std::fs::remove_file(&p).with_context(|| format!("deleting {}", p.display()))
}

/// Set clap's about text and the long version line from `brand`.
///
/// The default title keeps the existing about text. `--version` adds the title
/// on the next line. The short `-V` line stays `acp-red-star` and the version.
pub fn style_command(mut cmd: clap::Command, brand: &Brand) -> clap::Command {
    cmd = cmd.long_version(format!("{}\n{}", env!("CARGO_PKG_VERSION"), brand.title));
    if brand.title != TUI_TITLE {
        let about = format!("{}: a local client for the Red Star forum (acp.us/rs)", brand.title);
        let long = cmd.get_long_about()
            .map(|s| s.to_string().replacen("ACP Red Star.", &format!("{}.", brand.title), 1))
            .unwrap_or_else(|| about.clone());
        cmd = cmd.about(about).long_about(long);
    }
    cmd
}

/// `--brand` or `--brand=NAME` from `argv` (argv[0] is the program).
pub fn peek_flag(argv: &[std::ffi::OsString], flag: &str) -> Option<String> {
    let mut it = argv.iter().skip(1);
    let eq = format!("--{flag}=");
    while let Some(a) = it.next() {
        let s = a.to_string_lossy();
        if s == format!("--{flag}") {
            return it.next().map(|v| v.to_string_lossy().into_owned());
        }
        if let Some(rest) = s.strip_prefix(&eq) {
            return Some(rest.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("acp-brands-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn default_brand_is_the_shipped_header() {
        let b = builtin_default();
        assert_eq!(b.title, "ACP Red Star");
        assert_eq!(b.motto, SLOGAN);
        assert_eq!(b.word, "ACP");
        assert_eq!(b.logo, LogoKind::Builtin);
    }

    #[test]
    fn plain_brand_has_no_acp_and_no_motto() {
        let b = builtin_plain();
        assert_eq!(b.title, "Red Star");
        assert!(b.motto.is_empty());
        assert!(b.word.is_empty());
        assert!(!b.shows_acp());
        assert!(builtin_default().shows_acp());
        assert_eq!(b.logo, LogoKind::Text);
        assert!(b.logo_text.contains('★'));
        assert!(!b.title.contains("ACP"));
    }

    #[test]
    fn a_user_file_overrides_a_builtin_and_a_broken_file_is_not_the_builtin() {
        let d = dir("override");
        let src = "version = 1\ntitle = \"Local\"\nmotto = \"\"\nword = \"\"\nlogo = \"text\"\nlogo_text = \"*\"\n";
        std::fs::write(d.join("default.toml"), src).unwrap();
        let b = resolve(&d, "default").unwrap();
        assert_eq!(b.title, "Local");
        assert!(b.from_file);
        std::fs::write(d.join("plain.toml"), "version = [oops").unwrap();
        assert!(matches!(resolve(&d, "plain"), Err(LoadError::Broken(_, _))));
    }

    #[test]
    fn logo_paths_cannot_escape_the_brands_folder() {
        let d = dir("paths");
        std::fs::write(d.join("star.txt"), "★").unwrap();
        assert!(logo_file_path(&d, "star.txt").is_ok());
        assert!(logo_file_path(&d, "../star.txt").is_err());
        assert!(logo_file_path(&d, "/etc/passwd").is_err());
        assert!(logo_file_path(&d, "sub/../../star.txt").is_err());
        assert!(logo_file_path(&d, "nope.txt").unwrap_err().contains("missing"));
        let src = "version = 1\ntitle = \"X\"\nlogo = \"file\"\nlogo_file = \"../star.txt\"\n";
        std::fs::write(d.join("bad.toml"), src).unwrap();
        let err = resolve(&d, "bad").unwrap_err();
        let LoadError::Broken(_, msg) = err else { panic!("broken") };
        assert!(msg.contains("stay inside"), "{msg}");
    }

    #[test]
    fn an_empty_motto_round_trips() {
        let d = dir("round");
        let mut b = builtin_plain();
        b.name = "demo".into();
        b.description = "Demo".into();
        save(&d, &b).unwrap();
        let loaded = resolve(&d, "demo").unwrap();
        assert_eq!(loaded.title, "Red Star");
        assert!(loaded.motto.is_empty());
        assert!(loaded.word.is_empty());
    }

    #[test]
    fn list_includes_builtins_when_the_folder_is_missing() {
        let d = dir("missing");
        let _ = std::fs::remove_dir_all(&d);
        let names: Vec<_> = list(&d).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["default", "plain"]);
    }
}
