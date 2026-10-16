//! `~/.acp/preferences.toml`: the remembered login email and the terminal UI theme.

use crate::prelude::*;

#[derive(Serialize, Deserialize, Default)]
/// Contents of `~/.acp/preferences.toml`.
pub struct Prefs {
    /// Login email.
    pub email: Option<String>,
    /// Terminal UI colours (`[theme]` table); every key is optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemePrefs>,
    /// Terminal UI sync defaults (`[sync]` table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncPrefs>,
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
/// `[theme]` in `~/.acp/preferences.toml`: `#rrggbb` colours for the terminal UI.
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
