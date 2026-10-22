//! `~/.acp/quarantined/<kind>/`. Plugins are the first kind.

use super::gate::GateFailure;
use std::fs;
use std::path::{Path, PathBuf};

/// One quarantined item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Directory name under `quarantined`, such as `plugins`.
    pub kind: String,
    /// Original filename.
    pub filename: String,
    /// When it was quarantined.
    pub at: String,
    /// Why. Never the file bytes.
    pub reason: String,
    /// Path of the quarantined archive.
    pub path: PathBuf,
}

/// `~/.acp/quarantined`.
pub fn root(home: &Path) -> PathBuf {
    home.join(".acp/quarantined")
}

/// Move `archive` into `~/.acp/quarantined/<kind>/` and write a sidecar reason.
pub fn quarantine(home: &Path, kind: &str, archive: &Path, failure: &GateFailure) -> Result<Item, String> {
    let filename = archive.file_name().and_then(|s| s.to_str()).unwrap_or("plugin.tar.gz").to_string();
    let dir = root(home).join(kind);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = unique(&dir, &filename);
    fs::rename(archive, &dest).or_else(|_| {
        fs::copy(archive, &dest).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(archive);
        Ok::<(), String>(())
    })?;
    let at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let reason = format!("potential security threat: {}", failure.reason);
    let side = sidecar(&dest);
    let body = format!("kind = {kind:?}\nfilename = {filename:?}\nat = {at:?}\nreason = {reason:?}\n");
    fs::write(&side, body).map_err(|e| e.to_string())?;
    eprintln!("potential security threat: plugin package {} at {}: {}", failure.name, archive.display(), failure.reason);
    Ok(Item { kind: kind.into(), filename, at, reason, path: dest })
}

fn unique(dir: &Path, filename: &str) -> PathBuf {
    let plain = dir.join(filename);
    if !plain.exists() { return plain; }
    let stamp = chrono::Local::now().format("%H%M%S");
    dir.join(format!("{stamp}-{filename}"))
}

fn sidecar(archive: &Path) -> PathBuf {
    let mut name = archive.file_name().unwrap_or_default().to_os_string();
    name.push(".reason");
    archive.with_file_name(name)
}

/// Every quarantined item, every kind.
pub fn list(home: &Path) -> Vec<Item> {
    let mut out = Vec::new();
    let root = root(home);
    let Ok(kinds) = fs::read_dir(&root) else { return out };
    for kind in kinds.flatten() {
        if !kind.path().is_dir() { continue; }
        let kind_name = kind.file_name().to_string_lossy().to_string();
        let Ok(files) = fs::read_dir(kind.path()) else { continue };
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().to_string();
            if name.ends_with(".reason") { continue; }
            if let Some(item) = read_item(&kind_name, &file.path()) {
                out.push(item);
            }
        }
    }
    out.sort_by(|a, b| a.filename.cmp(&b.filename));
    out
}

fn read_item(kind: &str, path: &Path) -> Option<Item> {
    let filename = path.file_name()?.to_str()?.to_string();
    let side = fs::read_to_string(sidecar(path)).unwrap_or_default();
    let value: toml::Value = toml::from_str(&side).unwrap_or(toml::Value::Table(toml::Table::new()));
    let at = value.get("at").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let reason = value.get("reason").and_then(|v| v.as_str()).unwrap_or("quarantined").to_string();
    let filename = value.get("filename").and_then(|v| v.as_str()).unwrap_or(&filename).to_string();
    Some(Item { kind: kind.into(), filename, at, reason, path: path.to_path_buf() })
}

/// One item, by filename.
pub fn show(home: &Path, filename: &str) -> Result<Item, String> {
    list(home).into_iter().find(|i| i.filename == filename).ok_or_else(|| format!("no quarantined item named {filename}"))
}

/// Delete the quarantined copy and its reason. The plugins directory is left alone.
pub fn delete(home: &Path, filename: &str) -> Result<(), String> {
    let item = show(home, filename)?;
    fs::remove_file(&item.path).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(sidecar(&item.path));
    Ok(())
}

/// Move a plugin archive back to `~/.acp/plugins/`. The caller runs the gate again.
pub fn restore(home: &Path, filename: &str) -> Result<PathBuf, String> {
    let item = show(home, filename)?;
    if item.kind != "plugins" {
        return Err(format!("{filename} is a {} item, not a plugin", item.kind));
    }
    let dir = home.join(".acp/plugins");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&item.filename);
    fs::rename(&item.path, &dest).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(sidecar(&item.path));
    Ok(dest)
}
