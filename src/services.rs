//! Registered services, their toml files, and sealed secrets.
//!
//! Red Star is on when `~/.acp/services/redstar.toml` is missing. Every other
//! service is off until the user enables it. A field marked secret is refused
//! if it appears in the toml file. The value is not logged.

use crate::plugins::{self, ReloadReport};
use crate::prelude::*;
use acp_red_star_node_core::service::{
    DynService, Field, PluginRegistrar, Store, CapabilityId,
};
use acp_red_star_node_core::store::{open_blob, seal_blob};
/// How a combined view lays out rows from more than one service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combine {
    /// One list. The default, and the same list Red Star already shows.
    Mixed,
    /// A section per service.
    Grouped,
}

impl Combine {
    /// Stored in `[ui] combine`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mixed => "mixed",
            Self::Grouped => "grouped",
        }
    }

    /// `grouped` selects [`Combine::Grouped`]. Anything else is mixed.
    pub fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("grouped") { Self::Grouped } else { Self::Mixed }
    }
}

/// `~/.acp/services/<id>.toml`.
pub fn service_path(home: &Path, id: &str) -> PathBuf {
    home.join(".acp/services").join(format!("{id}.toml"))
}

/// Built-in services, Red Star first. Linked in. No `dlopen`.
pub fn builtins() -> Vec<Box<dyn DynService>> {
    let mut reg = BuiltinRegistrar { services: Vec::new() };
    unsafe { (acp_red_star_service_redstar::plugin_declaration.register)(&mut reg) };
    unsafe { (acp_red_star_service_youtube::plugin_declaration.register)(&mut reg) };
    reg.services
}

struct BuiltinRegistrar {
    services: Vec<Box<dyn DynService>>,
}

impl PluginRegistrar for BuiltinRegistrar {
    fn register_service(&mut self, service: Box<dyn DynService>) {
        if self.services.iter().any(|s| s.id() == service.id()) {
            return;
        }
        self.services.push(service);
    }
}

/// Red Star is on when its file is missing. Any other missing file is off.
pub fn is_enabled(home: &Path, id: &str) -> Result<bool> {
    let service = find_builtin(id);
    let schema = service.as_ref().map(|s| s.schema()).unwrap_or(&[]);
    let path = service_path(home, id);
    if !path.is_file() {
        return Ok(id == "redstar");
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let value: toml::Value = toml::from_str(&text).with_context(|| format!("{} is not valid TOML", path.display()))?;
    reject_secret_keys(&value, schema, id)?;
    warn_unknown(&value, schema, id);
    match value.get("enabled").and_then(|v| v.as_bool()) {
        Some(v) => Ok(v),
        None => Ok(id == "redstar"),
    }
}

fn find_builtin(id: &str) -> Option<Box<dyn DynService>> {
    builtins().into_iter().find(|s| s.id() == id)
}

fn reject_secret_keys(value: &toml::Value, schema: &[Field], id: &str) -> Result<()> {
    let Some(table) = value.as_table() else { bail!("{id}.toml is not a table") };
    for key in table.keys() {
        if schema.iter().any(|f| f.key == key && f.store == Store::Secret) {
            bail!("{id}.toml contains secret key {key}");
        }
    }
    Ok(())
}

fn warn_unknown(value: &toml::Value, schema: &[Field], id: &str) {
    let Some(table) = value.as_table() else { return };
    for key in table.keys() {
        if key == "enabled" { continue; }
        if !schema.iter().any(|f| f.key == key) {
            info!("{id}.toml: unknown key {key}");
        }
    }
}

/// Write `enabled` and keep every other key.
pub fn set_enabled(home: &Path, id: &str, enabled: bool) -> Result<()> {
    if find_builtin(id).is_none() && plugins::summaries().iter().all(|s| s.id != id) {
        bail!("no service named {id}");
    }
    let path = service_path(home, id);
    let mut table = read_table(&path)?;
    table.insert("enabled".into(), toml::Value::Boolean(enabled));
    write_table(&path, &table)
}

/// Set one config field. A secret field is refused and the value is not stored or logged.
pub fn set_config(home: &Path, id: &str, key: &str, value: &str) -> Result<()> {
    let service = find_builtin(id).ok_or_else(|| anyhow!("no service named {id}"))?;
    let field = service.schema().iter().find(|f| f.key == key).ok_or_else(|| anyhow!("{id} has no field {key}"))?;
    if field.store == Store::Secret {
        bail!("{key} is a secret. It is not written to the toml file.");
    }
    let encoded = encode_field(field, value)?;
    let path = service_path(home, id);
    let mut table = read_table(&path)?;
    table.insert(key.into(), encoded);
    write_table(&path, &table)
}

fn encode_field(field: &Field, value: &str) -> Result<toml::Value> {
    use acp_red_star_node_core::service::FieldKind;
    match field.kind {
        FieldKind::Bool => {
            let v = match value {
                "true" | "on" | "yes" => true,
                "false" | "off" | "no" => false,
                _ => bail!("{} expects true or false", field.key),
            };
            Ok(toml::Value::Boolean(v))
        }
        FieldKind::Int => {
            let n: i64 = value.parse().with_context(|| format!("{} expects an integer", field.key))?;
            Ok(toml::Value::Integer(n))
        }
        FieldKind::StringList => {
            let items = value.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| toml::Value::String(s.into())).collect();
            Ok(toml::Value::Array(items))
        }
        FieldKind::Text | FieldKind::Enum(_) => Ok(toml::Value::String(value.into())),
        FieldKind::Secret => bail!("{} is a secret", field.key),
    }
}

fn read_table(path: &Path) -> Result<toml::Table> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let value: toml::Value = toml::from_str(&text).with_context(|| format!("{} is not valid TOML", path.display()))?;
            value.as_table().cloned().ok_or_else(|| anyhow!("{} is not a table", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

fn write_table(path: &Path, table: &toml::Table) -> Result<()> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let body = toml::to_string(table).context("encoding service settings")?;
    crate::themes::atomic_write(path, body.as_bytes()).with_context(|| format!("writing {}", path.display()))
}

/// The rustc and core-crate pin. This is the only place that prints it.
pub fn print_doctor() -> Result<()> {
    println!("rustc: {}", acp_red_star_node_core::RUSTC_VERSION);
    println!("core: acp_red_star_node_core {}", acp_red_star_node_core::CORE_VERSION);
    Ok(())
}

/// One line per registered service, then refused plugins. No pin, no secret values.
pub fn print_services() -> Result<()> {
    let home = crate::config::home()?;
    for service in builtins() {
        let enabled = is_enabled(&home, service.id())?;
        let state = if enabled { "enabled" } else { "disabled" };
        println!("{}  {}  {state}", service.id(), service.display_name());
    }
    for plugin in plugins::summaries() {
        if builtins().iter().any(|s| s.id() == plugin.id) { continue; }
        match &plugin.refused {
            Some(reason) => println!("{}  refused  {reason}", plugin.filename),
            None => {
                let enabled = is_enabled(&home, &plugin.id).unwrap_or(false);
                let state = if enabled { "enabled" } else { "disabled" };
                println!("{}  {}  {state}", plugin.id, plugin.display);
            }
        }
    }
    Ok(())
}

/// Quarantine list. Every kind, plugins first.
pub fn print_quarantine() -> Result<()> {
    let home = crate::config::home()?;
    let items = plugins::list(&home);
    if items.is_empty() {
        println!("(nothing quarantined)");
        return Ok(());
    }
    for item in items {
        println!("{}  {}  {}  {}", item.kind, item.filename, item.at, item.reason);
    }
    Ok(())
}

/// One quarantined item and its reason.
pub fn print_quarantine_show(filename: &str) -> Result<()> {
    let home = crate::config::home()?;
    let item = plugins::show(&home, filename).map_err(|e| anyhow!(e))?;
    println!("{}  {}  {}  {}", item.kind, item.filename, item.at, item.reason);
    Ok(())
}

/// Move a plugin back and run the gate. A second failure quarantines it again.
pub fn quarantine_restore(filename: &str) -> Result<()> {
    let home = crate::config::home()?;
    let path = plugins::restore(&home, filename).map_err(|e| anyhow!(e))?;
    let report = plugins::reload(&home);
    if report.quarantined.iter().any(|n| n == filename || path.ends_with(n)) {
        bail!("{filename} failed the package check again and was quarantined");
    }
    println!("restored {filename}");
    Ok(())
}

/// Delete the quarantined copy only.
pub fn quarantine_delete(filename: &str) -> Result<()> {
    let home = crate::config::home()?;
    plugins::delete(&home, filename).map_err(|e| anyhow!(e))?;
    println!("deleted {filename}");
    Ok(())
}

/// Reload plugins. Preferences › Services and `plugins.reload` both call this.
pub fn reload_plugins() -> Result<ReloadReport> {
    let home = crate::config::home()?;
    Ok(plugins::reload(&home))
}

/// A control is shown when any registered service implements `cap`.
pub fn any_implements(cap: CapabilityId) -> bool {
    builtins().iter().any(|s| s.has(cap))
}

/// Seal `plain` under `name` in `source_secrets`. The plaintext is not written anywhere else.
pub fn put_secret(conn: &Connection, key: &[u8; 32], name: &str, plain: &[u8]) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS source_secrets (
            name TEXT PRIMARY KEY,
            body BLOB NOT NULL
        );",
    )?;
    let blob = seal_blob(key, name.as_bytes(), plain).context("sealing the secret")?;
    conn.execute(
        "INSERT INTO source_secrets (name, body) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET body = excluded.body",
        params![name, blob],
    )?;
    Ok(())
}

/// Whether `name` is sealed. Does not return the bytes.
pub fn secret_is_set(conn: &Connection, name: &str) -> Result<bool> {
    let has: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='source_secrets'",
        [],
        |r| r.get(0),
    ).unwrap_or(0);
    if has == 0 { return Ok(false); }
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM source_secrets WHERE name = ?1",
        params![name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Open one sealed secret.
pub fn open_secret(conn: &Connection, key: &[u8; 32], name: &str) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>> {
    if !secret_is_set(conn, name)? { return Ok(None); }
    let blob: Vec<u8> = conn.query_row(
        "SELECT body FROM source_secrets WHERE name = ?1",
        params![name],
        |r| r.get(0),
    )?;
    Ok(Some(open_blob(key, name.as_bytes(), &blob).context("opening the secret")?))
}

/// Drop secret rows from a backup snapshot. An absent table leaves the file alone.
pub fn strip_secrets(conn: &Connection) -> Result<()> {
    let has: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='source_secrets'",
        [],
        |r| r.get(0),
    ).unwrap_or(0);
    if has == 0 { return Ok(()); }
    let n: i64 = conn.query_row("SELECT count(*) FROM source_secrets", [], |r| r.get(0))?;
    if n > 0 {
        conn.execute("DELETE FROM source_secrets", [])?;
    }
    Ok(())
}

/// A prepared YouTube read. Nothing is fetched and yt-dlp is not spawned.
pub fn youtube_read(target: &str) -> Result<()> {
    acp_red_star_service_youtube::prepare_read(target).map_err(|e| anyhow!("{e}"))
}

/// Lines for the Services window.
pub fn services_rows(home: &Path) -> Vec<ServiceRow> {
    let mut rows = Vec::new();
    for service in builtins() {
        let enabled = is_enabled(home, service.id()).unwrap_or(true);
        rows.push(ServiceRow::Service {
            id: service.id().into(),
            label: service.display_name().into(),
            enabled,
        });
    }
    for plugin in plugins::summaries() {
        if rows.iter().any(|r| matches!(r, ServiceRow::Service { id, .. } if id == &plugin.id)) {
            continue;
        }
        if let Some(reason) = plugin.refused {
            rows.push(ServiceRow::Note(format!("{} refused: {reason}", plugin.filename)));
        } else {
            let enabled = is_enabled(home, &plugin.id).unwrap_or(false);
            rows.push(ServiceRow::Service { id: plugin.id, label: plugin.display, enabled });
        }
    }
    rows.push(ServiceRow::Header("Quarantine".into()));
    let items = plugins::list(home);
    if items.is_empty() {
        rows.push(ServiceRow::Note("nothing quarantined".into()));
    }
    for item in items {
        rows.push(ServiceRow::Quarantine { file: item.filename, reason: item.reason });
    }
    rows.push(ServiceRow::Reload);
    rows
}

/// One row in Preferences › Services.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceRow {
    /// A registered service.
    Service {
        /// Service id.
        id: String,
        /// Display name.
        label: String,
        /// The user has turned it on.
        enabled: bool,
    },
    /// A section title.
    Header(String),
    /// A note that is not a control.
    Note(String),
    /// A quarantined archive.
    Quarantine {
        /// Filename.
        file: String,
        /// Reason.
        reason: String,
    },
    /// Run the plugin loader.
    Reload,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_enables_only_red_star() {
        let home = std::env::temp_dir().join(format!("acp-svc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(".acp")).unwrap();
        assert!(is_enabled(&home, "redstar").unwrap());
        assert!(!is_enabled(&home, "x").unwrap());
        set_enabled(&home, "redstar", false).unwrap();
        assert!(!is_enabled(&home, "redstar").unwrap());
        let text = std::fs::read_to_string(service_path(&home, "redstar")).unwrap();
        assert!(text.contains("enabled = false"), "{text}");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_secret_in_toml_fails_without_the_value() {
        let home = std::env::temp_dir().join(format!("acp-sec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let path = service_path(&home, "redstar");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let schema = [Field {
            key: "session",
            label: "Session",
            help: "",
            kind: acp_red_star_node_core::service::FieldKind::Secret,
            store: Store::Secret,
        }];
        std::fs::write(&path, "session = \"super-secret-token\"\n").unwrap();
        let value: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let err = reject_secret_keys(&value, &schema, "redstar").unwrap_err().to_string();
        assert!(err.contains("session"), "{err}");
        assert!(!err.contains("super-secret-token"), "{err}");
        let err = set_config(&home, "redstar", "session", "super-secret-token").unwrap_err().to_string();
        assert!(err.contains("no field") || err.contains("secret"), "{err}");
        assert!(!err.contains("super-secret-token"), "{err}");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_youtube_api_key_in_toml_is_refused_without_the_value() {
        let home = std::env::temp_dir().join(format!("acp-ytsec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let path = service_path(&home, "youtube");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "enabled = true\napi_key = \"not-a-key\"\n").unwrap();
        let err = is_enabled(&home, "youtube").unwrap_err().to_string();
        assert!(err.contains("api_key"), "{err}");
        assert!(!err.contains("not-a-key"), "{err}");
        let ids: Vec<_> = builtins().iter().map(|s| s.id()).collect();
        assert_eq!(ids, vec!["redstar", "youtube"]);
        let err = youtube_read("alice").unwrap_err().to_string();
        assert!(err.contains("Nothing was fetched"), "{err}");
        assert!(!err.contains("youtube.com"), "{err}");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn secrets_round_trip_and_a_snapshot_drops_them() {
        let conn = Connection::open_in_memory().unwrap();
        let key = [7u8; 32];
        assert!(!secret_is_set(&conn, "redstar.session").unwrap());
        put_secret(&conn, &key, "redstar.session", b"cookie").unwrap();
        assert!(secret_is_set(&conn, "redstar.session").unwrap());
        let opened = open_secret(&conn, &key, "redstar.session").unwrap().unwrap();
        assert_eq!(&opened[..], b"cookie");
        strip_secrets(&conn).unwrap();
        assert!(!secret_is_set(&conn, "redstar.session").unwrap());
        strip_secrets(&conn).unwrap();
    }

    #[test]
    fn live_and_feeds_are_not_offered() {
        assert!(any_implements(CapabilityId::Notification));
        assert!(any_implements(CapabilityId::Post));
        assert!(!any_implements(CapabilityId::Live));
        assert!(!any_implements(CapabilityId::Feeds));
        assert!(any_implements(CapabilityId::Vote));
    }

    #[test]
    fn doctor_lines_are_the_pin() {
        assert!(acp_red_star_node_core::RUSTC_VERSION.starts_with("rustc "));
    }
}
