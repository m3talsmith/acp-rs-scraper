//! `~/.acp/preferences.toml`: the remembered login email and terminal UI settings.

use crate::prelude::*;

#[derive(Serialize, Deserialize, Default)]
/// Contents of `~/.acp/preferences.toml`.
pub struct Prefs {
    /// Login email.
    pub email: Option<String>,
    /// The old (before 2.12) terminal UI colours (`[theme]` table). Converted once into
    /// `~/.acp/themes/custom.toml`, then ignored; the keys are left in the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemePrefs>,
    /// Terminal UI sync defaults (`[sync]` table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncPrefs>,
    /// Terminal UI behaviour (`[ui]` table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiPrefs>,
}

/// `[ui]` in `~/.acp/preferences.toml`.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct UiPrefs {
    /// The empty-Body animation: `true` / `"on"` (default: your local file, else the built-in
    /// clip), `"star"` / `"minimal"` (the procedural red star), `false` / `"off"` (none).
    pub empty_animation: Option<toml::Value>,
    /// A frames file to play instead of `~/.acp/animations/empty-body.acpanim(.gz)`.
    pub empty_animation_path: Option<String>,
    /// `"muted"` (default) or `"accent"`.
    pub empty_animation_color: Option<String>,
    /// The theme: a file name in `~/.acp/themes` without `.toml` (default `"default"`).
    pub theme: Option<String>,
    /// The brand: a built-in name, or a file in `~/.acp/brands` without `.toml` (default `"default"`).
    pub brand: Option<String>,
    /// `"dark"`, `"light"` or `"system"` (default: from the terminal's background).
    pub theme_mode: Option<String>,
    /// `false` stops the first run from downloading the default theme into an empty
    /// `~/.acp/themes` (default `true`; `ACP_THEME_DOWNLOAD=off` does the same).
    pub theme_download: Option<bool>,
    /// Writes (3.4.0). Omitted means on. `ACP_WRITES=off` wins over this field.
    pub writes: Option<bool>,
    /// Per-list order and the nick-list limit (`[ui.list_order]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_order: Option<ListOrderPrefs>,
    /// `auto`, `always`, or `never`. Omitted means auto (follow NO_COLOR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Toasts and desktop notifications (`[ui.notify]`). Omitted keeps every kind on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<NotifyPrefs>,
    /// How many search-history rows to keep. Omitted keeps the entire history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_history_limit: Option<u64>,
}

/// `[ui.notify]` in `~/.acp/preferences.toml`.
///
/// `dnd` silences toasts and desktop notifications together. The log still records them.
/// Each kind defaults to on when the key is omitted.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NotifyPrefs {
    /// Do-not-disturb.
    #[serde(default)]
    pub dnd: bool,
    /// Error notices.
    #[serde(default = "default_true")]
    pub error: bool,
    /// Warning notices.
    #[serde(default = "default_true")]
    pub warning: bool,
    /// Sync notices.
    #[serde(default = "default_true")]
    pub sync: bool,
    /// Job notices.
    #[serde(default = "default_true")]
    pub job: bool,
}

fn default_true() -> bool { true }

/// `[ui.list_order]` in `~/.acp/preferences.toml`.
///
/// Each value is `newest` or `oldest`. Omitted means newest. `nick_limit` is how
/// many nicks the nick bar keeps; omitted means all of them.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct ListOrderPrefs {
    /// Nick timeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline: Option<String>,
    /// Bookmarks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookmarks: Option<String>,
    /// Search results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    /// Research items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research: Option<String>,
    /// Site lists (Top, EB, Explore).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// Export basket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basket: Option<String>,
    /// Nick bar, by each nick's latest post.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nicks: Option<String>,
    /// How many nicks to keep after the nick filter. Blank in the UI means all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nick_limit: Option<u64>,
}

/// Write `[ui.list_order]`, keeping every other key in `path` (also unknown ones).
pub fn save_list_order_at(path: &Path, prefs: &ListOrderPrefs) -> Result<()> {
    let mut t: toml::Table = match std::fs::read_to_string(path) {
        Ok(s) => toml::from_str(&s).with_context(|| format!("{} is not valid TOML; not changing it", path.display()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => toml::Table::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let ui = t.entry("ui").or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(ui) = ui.as_table_mut() else { bail!("[ui] in {} is not a table; not changing it", path.display()) };
    ui.insert("list_order".into(), toml::Value::try_from(prefs).context("encoding list order")?);
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    crate::themes::atomic_write(path, toml::to_string(&t)?.as_bytes()).with_context(|| format!("writing {}", path.display()))
}

/// Write `[ui.list_order]` in `~/.acp/preferences.toml`, keeping every other key.
pub fn save_list_order(prefs: &ListOrderPrefs) -> Result<()> {
    save_list_order_at(&home()?.join(PREFS_REL_PATH), prefs)
}

/// Set `[ui] color` in `path`, keeping every other key (also unknown ones).
pub fn save_ui_color_at(path: &Path, color: &str) -> Result<()> {
    let mut t: toml::Table = match std::fs::read_to_string(path) {
        Ok(s) => toml::from_str(&s).with_context(|| format!("{} is not valid TOML; not changing it", path.display()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => toml::Table::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let ui = t.entry("ui").or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(ui) = ui.as_table_mut() else { bail!("[ui] in {} is not a table; not changing it", path.display()) };
    ui.insert("color".into(), toml::Value::String(color.into()));
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    crate::themes::atomic_write(path, toml::to_string(&t)?.as_bytes()).with_context(|| format!("writing {}", path.display()))
}

/// Set `[ui] color` in `~/.acp/preferences.toml`, keeping every other key.
pub fn save_ui_color(color: &str) -> Result<()> {
    save_ui_color_at(&home()?.join(PREFS_REL_PATH), color)
}

/// Set `[ui.notify]` in `path`, keeping every other key.
pub fn save_notify_prefs_at(path: &Path, p: &NotifyPrefs) -> Result<()> {
    let mut t: toml::Table = match std::fs::read_to_string(path) {
        Ok(s) => toml::from_str(&s).with_context(|| format!("{} is not valid TOML; not changing it", path.display()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => toml::Table::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let ui = t.entry("ui").or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(ui) = ui.as_table_mut() else { bail!("[ui] in {} is not a table; not changing it", path.display()) };
    let notify = ui.entry("notify").or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(notify) = notify.as_table_mut() else { bail!("[ui.notify] in {} is not a table; not changing it", path.display()) };
    notify.insert("dnd".into(), toml::Value::Boolean(p.dnd));
    notify.insert("error".into(), toml::Value::Boolean(p.error));
    notify.insert("warning".into(), toml::Value::Boolean(p.warning));
    notify.insert("sync".into(), toml::Value::Boolean(p.sync));
    notify.insert("job".into(), toml::Value::Boolean(p.job));
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    crate::themes::atomic_write(path, toml::to_string(&t)?.as_bytes()).with_context(|| format!("writing {}", path.display()))
}

/// Set `[ui.notify]` in `~/.acp/preferences.toml`.
pub fn save_notify_prefs(p: &NotifyPrefs) -> Result<()> {
    save_notify_prefs_at(&home()?.join(PREFS_REL_PATH), p)
}

/// Set `[ui] search_history_limit` in `path`. `None` removes the key, which keeps the entire history.
pub fn save_search_history_limit_at(path: &Path, limit: Option<u64>) -> Result<()> {
    let mut t: toml::Table = match std::fs::read_to_string(path) {
        Ok(s) => toml::from_str(&s).with_context(|| format!("{} is not valid TOML; not changing it", path.display()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => toml::Table::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let ui = t.entry("ui").or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(ui) = ui.as_table_mut() else { bail!("[ui] in {} is not a table; not changing it", path.display()) };
    match limit {
        Some(n) => { ui.insert("search_history_limit".into(), toml::Value::Integer(n as i64)); }
        None => { ui.remove("search_history_limit"); }
    }
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    crate::themes::atomic_write(path, toml::to_string(&t)?.as_bytes()).with_context(|| format!("writing {}", path.display()))
}

/// Set `[ui] search_history_limit` in `~/.acp/preferences.toml`.
pub fn save_search_history_limit(limit: Option<u64>) -> Result<()> {
    save_search_history_limit_at(&home()?.join(PREFS_REL_PATH), limit)
}

/// Set `[ui] theme` in `~/.acp/preferences.toml`, keeping every other key (also unknown
/// ones). Refuses to touch a file that isn't valid TOML. Atomic, mode 0600.
pub fn save_ui_theme(name: &str) -> Result<()> {
    let path = home()?.join(PREFS_REL_PATH);
    let mut t: toml::Table = match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).with_context(|| format!("{} is not valid TOML; not changing it", path.display()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => toml::Table::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let ui = t.entry("ui").or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(ui) = ui.as_table_mut() else { bail!("[ui] in {} is not a table; not changing it", path.display()) };
    ui.insert("theme".into(), toml::Value::String(name.into()));
    crate::themes::atomic_write(&path, toml::to_string(&t)?.as_bytes()).with_context(|| format!("writing {}", path.display()))
}

/// Set `[ui] brand` in `~/.acp/preferences.toml`, keeping every other key (also unknown
/// ones). Refuses to touch a file that isn't valid TOML. Atomic, mode 0600.
pub fn save_ui_brand(name: &str) -> Result<()> {
    let path = home()?.join(PREFS_REL_PATH);
    let mut t: toml::Table = match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).with_context(|| format!("{} is not valid TOML; not changing it", path.display()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => toml::Table::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let ui = t.entry("ui").or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(ui) = ui.as_table_mut() else { bail!("[ui] in {} is not a table; not changing it", path.display()) };
    ui.insert("brand".into(), toml::Value::String(name.into()));
    crate::themes::atomic_write(&path, toml::to_string(&t)?.as_bytes()).with_context(|| format!("writing {}", path.display()))
}


/// `[sync]` in `~/.acp/preferences.toml`: the TUI's Sync menu defaults (Sync › Save as default).
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct SyncPrefs {
    /// Sync automatically while the TUI is open.
    pub auto: Option<bool>,
    /// Minutes between automatic syncs (default 15).
    pub interval_minutes: Option<u64>,
    /// Nicks to re-scrape (blank: the selected nicks).
    pub nicks: Option<String>,
    /// Also scan mentions (off by default: syncs stay quick).
    pub mentions: Option<bool>,
    /// Sync bookmarks (default on).
    pub bookmarks: Option<bool>,
}

/// Store the `[sync]` table, keeping the file's other settings.
pub fn save_sync_prefs(s: &SyncPrefs) -> Result<()> {
    let path = home()?.join(PREFS_REL_PATH);
    let mut p = load_prefs();
    p.sync = Some(s.clone());
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, toml::to_string(&p)?)?;
    Ok(())
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
/// `[theme]` in `~/.acp/preferences.toml` (before 2.12): `#rrggbb` colours for the terminal UI.
pub struct ThemePrefs {
    /// Accent (reply links, selection, Start Scanning). Default `#dc2626` (ACP red).
    pub accent: Option<String>,
    /// Secondary text (dates, ids, hints).
    pub muted: Option<String>,
    /// Borders of unfocused panes.
    pub border: Option<String>,
    /// Text on accent backgrounds.
    pub on_accent: Option<String>,
}

/// Read `~/.acp/preferences.toml` (defaults when missing or unreadable).
pub fn load_prefs() -> Prefs {
    home().ok().map(|h| h.join(PREFS_REL_PATH))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

/// The saved login email, without prompting.
pub fn saved_email() -> Option<String> { load_prefs().email.filter(|e| !e.is_empty()) }

/// `ui.writes` defaults to on. `ACP_WRITES=off` (or `0`, `false`, `no`) wins.
/// `ACP_WRITES=on` (or `1`, `true`, `yes`) turns writes on even when the file says off.
pub fn writes_enabled_from(pref: Option<bool>, env: Option<&str>) -> bool {
    if let Some(v) = env {
        let v = v.trim().to_ascii_lowercase();
        if matches!(v.as_str(), "0" | "off" | "false" | "no") {
            return false;
        }
        if matches!(v.as_str(), "1" | "on" | "true" | "yes") {
            return true;
        }
    }
    pref.unwrap_or(true)
}

/// The switch for this process: the environment, then `~/.acp/preferences.toml`.
pub fn writes_enabled() -> bool {
    let env = std::env::var("ACP_WRITES").ok();
    let pref = load_prefs().ui.and_then(|u| u.writes);
    writes_enabled_from(pref, env.as_deref())
}

/// Remember `email` in `~/.acp/preferences.toml`, keeping the file's other settings.
pub fn save_email(email: &str) -> Result<()> {
    let path = home()?.join(PREFS_REL_PATH);
    let mut p = load_prefs();
    if p.email.as_deref() == Some(email) && path.exists() { return Ok(()); }
    p.email = Some(email.to_string());
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, toml::to_string(&p)?)?;
    Ok(())
}

/// Read `email` from `~/.acp/preferences.toml`, prompting for it and creating the file when missing.
pub fn load_email() -> Result<String> {
    let path = home()?.join(PREFS_REL_PATH);
    if path.exists() {
        let p: Prefs = toml::from_str(&std::fs::read_to_string(&path)?)?;
        if let Some(e) = p.email.filter(|e| !e.is_empty()) {
            return Ok(e);
        }
    }
    let email = prompt("Email: ")?;
    save_email(&email)?;
    Ok(email)
}
