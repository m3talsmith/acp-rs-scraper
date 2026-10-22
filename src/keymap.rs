//! `~/.acp/keymap.toml`, with the shipped default when that file is missing.
//!
//! Widgets ask this module for a chord. They do not embed `plugins.reload`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

/// Shipped default. Copied in spirit: a missing file uses this text and does not write over a home directory.
pub const DEFAULT_TOML: &str = include_str!("../keymap/default.toml");

/// A Ctrl global this binary handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalAction {
    /// Open the command palette.
    Palette,
    /// Reload plugins through the package gate.
    ReloadPlugins,
    /// Which service's rows are on screen.
    CycleSource,
    /// Read the keymap file again.
    ReloadKeymap,
}

/// Parse a keymap file. A bad file is an error and does not produce a partial map.
pub fn map_from(text: &str) -> Result<BTreeMap<String, String>, String> {
    let value: toml::Value = toml::from_str(text).map_err(|e| format!("keymap: {e}"))?;
    let table = value.as_table().ok_or("keymap is not a table")?;
    let global = table.get("global").and_then(|v| v.as_table()).ok_or("keymap needs [global]")?;
    let mut map = BTreeMap::new();
    for (chord, action) in global {
        let action = action.as_str().ok_or_else(|| format!("{chord} is not a string"))?;
        map.insert(chord.clone(), action.to_string());
    }
    Ok(map)
}

fn embedded() -> BTreeMap<String, String> {
    map_from(DEFAULT_TOML).unwrap_or_default()
}

fn cache() -> &'static Mutex<Option<BTreeMap<String, String>>> {
    static MAP: Mutex<Option<BTreeMap<String, String>>> = Mutex::new(None);
    &MAP
}

/// The active map. A missing file uses the shipped default. A bad file keeps the previous map.
pub fn current() -> BTreeMap<String, String> {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(load_file().unwrap_or_else(|_| embedded()));
    }
    guard.clone().unwrap_or_default()
}

fn load_file() -> Result<BTreeMap<String, String>, String> {
    let home = crate::config::home().map_err(|e| e.to_string())?;
    let path = home.join(".acp/keymap.toml");
    if !path.is_file() {
        return Ok(embedded());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    map_from(&text)
}

/// Read `path` and replace the active map. On failure the previous map stays.
pub fn reload_path(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("keymap: {e}"))?;
    let map = map_from(&text)?;
    *cache().lock().unwrap_or_else(|e| e.into_inner()) = Some(map);
    Ok(())
}

/// Read `~/.acp/keymap.toml` again.
pub fn reload() -> Result<(), String> {
    let home = crate::config::home().map_err(|e| e.to_string())?;
    let path = home.join(".acp/keymap.toml");
    if !path.is_file() {
        *cache().lock().unwrap_or_else(|e| e.into_inner()) = Some(embedded());
        return Ok(());
    }
    reload_path(&path)
}

/// Action for a Ctrl chord such as `ctrl-p`, if this map names one we handle.
pub fn lookup(chord: &str) -> Option<GlobalAction> {
    let map = current();
    match map.get(chord).map(String::as_str) {
        Some("palette.open") => Some(GlobalAction::Palette),
        Some("plugins.reload") => Some(GlobalAction::ReloadPlugins),
        Some("view.source") => Some(GlobalAction::CycleSource),
        Some("keymap.reload") => Some(GlobalAction::ReloadKeymap),
        _ => None,
    }
}

/// `ctrl-{c}` when `c` is the character of a Ctrl chord.
pub fn lookup_char(c: char) -> Option<GlobalAction> {
    lookup(&format!("ctrl-{c}"))
}

/// The chord currently bound to `action`, for help. Empty when the map has none.
pub fn chord_for(action: &str) -> String {
    current().into_iter().find(|(_, v)| v == action).map(|(k, _)| k).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_gives_ctrl_p_to_plugins_and_ctrl_space_stays_the_palette() {
        let map = map_from(DEFAULT_TOML).unwrap();
        assert_eq!(map.get("ctrl-p").map(String::as_str), Some("plugins.reload"));
        assert_eq!(map.get("ctrl-]").map(String::as_str), Some("view.source"));
        assert_eq!(map.get("ctrl-space").map(String::as_str), Some("palette.open"));
        assert_eq!(map.get("ctrl-k").map(String::as_str), Some("keymap.reload"));
    }

    #[test]
    fn a_remap_is_what_lookup_reads() {
        let text = "[global]\n\"ctrl-p\" = \"palette.open\"\n\"ctrl-]\" = \"plugins.reload\"\n";
        let map = map_from(text).unwrap();
        assert_eq!(map.get("ctrl-p").map(String::as_str), Some("palette.open"));
        assert!(map_from("nope").is_err());
    }
}
