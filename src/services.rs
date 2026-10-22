//! Service files, sealed secrets, and the quarantine list.
//!
//! This binary does not link a site service. `redstar` stays reserved: a
//! missing `~/.acp/services/redstar.toml` means that id is on. Every other id
//! is off until its file says so. A secret field is refused. The value is not
//! stored in the toml file and it is not printed. `plugins.reload` is not
//! bound to a key.

use crate::prelude::*;
use acp_rs_plugin::{Field, FieldKind, Store};

/// How a combined view lays out rows from more than one service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combine {
    /// One list.
    Mixed,
    /// The navigation bar names the source. Rows stay one list until a post names its service.
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

fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn file_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.contains('/')
        && !name.contains('\\')
        && name != "."
        && name != ".."
        && !name.ends_with(".reason")
}

fn known(id: &str) -> bool {
    if !id_ok(id) {
        return false;
    }
    id == "redstar" || crate::service::runtime().services().iter().any(|s| s.id() == id)
}

fn schema_of(id: &str) -> &'static [Field] {
    crate::service::runtime()
        .services()
        .iter()
        .find(|s| s.id() == id)
        .map(|s| s.schema())
        .unwrap_or(&[])
}

/// A missing file enables only the reserved `redstar` id.
pub fn is_enabled(home: &Path, id: &str) -> Result<bool> {
    if !id_ok(id) {
        bail!("no service named {id}");
    }
    let schema = schema_of(id);
    let path = service_path(home, id);
    if !path.is_file() {
        return Ok(id == "redstar");
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let value: toml::Value = toml::from_str(&text).with_context(|| format!("{} is not valid TOML", path.display()))?;
    reject_secret_keys(&value, schema, id)?;
    match value.get("enabled").and_then(|v| v.as_bool()) {
        Some(v) => Ok(v),
        None => Ok(id == "redstar"),
    }
}

fn reject_secret_keys(value: &toml::Value, schema: &[Field], id: &str) -> Result<()> {
    let Some(table) = value.as_table() else { bail!("{id}.toml is not a table") };
    for key in table.keys() {
        if schema.iter().any(|f| f.key == key && f.store == Store::Secret) {
            bail!("{id}.toml contains secret key {key}");
        }
        if schema.is_empty() && key != "enabled" {
            bail!("{id}.toml contains {key}");
        }
    }
    Ok(())
}

/// Write `enabled` and keep every other non-secret key.
pub fn set_enabled(home: &Path, id: &str, enabled: bool) -> Result<()> {
    if !known(id) {
        bail!("no service named {id}");
    }
    let path = service_path(home, id);
    let mut table = read_table(&path)?;
    table.insert("enabled".into(), toml::Value::Boolean(enabled));
    write_table(&path, &table)
}

/// Set one config field. A secret field is refused and the value is not stored or logged.
pub fn set_config(home: &Path, id: &str, key: &str, value: &str) -> Result<()> {
    if !known(id) {
        bail!("no service named {id}");
    }
    let schema = schema_of(id);
    let field = schema.iter().find(|f| f.key == key).ok_or_else(|| anyhow!("{id} has no field {key}"))?;
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
            let items = value
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| toml::Value::String(s.into()))
                .collect();
            Ok(toml::Value::Array(items))
        }
        FieldKind::Enum(opts) => {
            if !opts.contains(&value) {
                bail!("{} expects one of {}", field.key, opts.join(", "));
            }
            Ok(toml::Value::String(value.into()))
        }
        FieldKind::Text => Ok(toml::Value::String(value.into())),
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = toml::to_string(table).context("encoding service settings")?;
    crate::themes::atomic_write(path, body.as_bytes()).with_context(|| format!("writing {}", path.display()))
}

/// One line per linked service. Secret values are not printed.
pub fn print_services() -> Result<()> {
    let home = crate::config::home()?;
    let services = crate::service::runtime().services();
    if services.is_empty() && !service_path(&home, "redstar").is_file() {
        println!("redstar  reserved  enabled");
    }
    for service in services {
        let enabled = is_enabled(&home, service.id())?;
        let state = if enabled { "enabled" } else { "disabled" };
        println!("{}  {}  {state}", service.id(), service.display_name());
    }
    if services.is_empty() && service_path(&home, "redstar").is_file() {
        let enabled = is_enabled(&home, "redstar")?;
        let state = if enabled { "enabled" } else { "disabled" };
        println!("redstar  reserved  {state}");
    }
    Ok(())
}

fn quarantine_dir(home: &Path) -> PathBuf {
    home.join(".acp/quarantined/plugins")
}

fn plugins_dir(home: &Path) -> PathBuf {
    home.join(".acp/plugins")
}

/// Quarantine list. Plugins are the first kind.
pub fn print_quarantine() -> Result<()> {
    let home = crate::config::home()?;
    let items = quarantine_items(&home)?;
    if items.is_empty() {
        println!("(nothing quarantined)");
        return Ok(());
    }
    for (file, reason) in items {
        println!("plugins  {file}  {reason}");
    }
    Ok(())
}

/// One quarantined item and its reason.
pub fn print_quarantine_show(filename: &str) -> Result<()> {
    if !file_ok(filename) {
        bail!("no quarantined file {filename}");
    }
    let home = crate::config::home()?;
    let path = quarantine_dir(&home).join(filename);
    if !path.is_file() {
        bail!("no quarantined file {filename}");
    }
    let reason = std::fs::read_to_string(quarantine_dir(&home).join(format!("{filename}.reason")))
        .unwrap_or_else(|_| "quarantined".into());
    println!("plugins  {filename}  {}", reason.trim());
    Ok(())
}

/// Move a plugin back and run the gate. A second failure quarantines it again.
pub fn quarantine_restore(filename: &str) -> Result<()> {
    if !file_ok(filename) {
        bail!("no quarantined file {filename}");
    }
    let home = crate::config::home()?;
    let src = quarantine_dir(&home).join(filename);
    if !src.is_file() {
        bail!("no quarantined file {filename}");
    }
    std::fs::create_dir_all(plugins_dir(&home))?;
    let dest = plugins_dir(&home).join(filename);
    std::fs::rename(&src, &dest).with_context(|| format!("restoring {filename}"))?;
    let _ = std::fs::remove_file(quarantine_dir(&home).join(format!("{filename}.reason")));
    let report = crate::plugins::reload(&plugins_dir(&home), &quarantine_dir(&home));
    if report.quarantined.iter().any(|n| n == filename) {
        bail!("{filename} failed the package check again and was quarantined");
    }
    println!("restored {filename}");
    Ok(())
}

/// Delete the quarantined copy only.
pub fn quarantine_delete(filename: &str) -> Result<()> {
    delete_quarantine(&crate::config::home()?, filename)
}

fn delete_quarantine(home: &Path, filename: &str) -> Result<()> {
    if !file_ok(filename) {
        bail!("no quarantined file {filename}");
    }
    let path = quarantine_dir(home).join(filename);
    if !path.is_file() {
        bail!("no quarantined file {filename}");
    }
    std::fs::remove_file(&path).with_context(|| format!("deleting {filename}"))?;
    let _ = std::fs::remove_file(quarantine_dir(home).join(format!("{filename}.reason")));
    println!("deleted {filename}");
    Ok(())
}

fn quarantine_items(home: &Path) -> Result<Vec<(String, String)>> {
    let dir = quarantine_dir(home);
    let entries = match std::fs::read_dir(&dir) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !file_ok(&name) {
            continue;
        }
        let reason = std::fs::read_to_string(dir.join(format!("{name}.reason")))
            .unwrap_or_else(|_| "quarantined".into());
        out.push((name, reason.trim().to_string()));
    }
    out.sort();
    Ok(out)
}

/// Reload installed packages. Account › Services calls this. It is not a key.
pub fn reload_plugins() -> Result<crate::plugins::ReloadReport> {
    let home = crate::config::home()?;
    Ok(crate::plugins::reload(&plugins_dir(&home), &quarantine_dir(&home)))
}

/// Seal `plain` under `name` in `source_secrets`. The plaintext is not written anywhere else.
pub fn put_secret(conn: &Connection, key: &[u8; 32], name: &str, plain: &[u8]) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS source_secrets (
            name TEXT PRIMARY KEY,
            body BLOB NOT NULL
        );",
    )?;
    let blob = acp_red_star_node_core::store::seal_blob(key, name.as_bytes(), plain).context("sealing the secret")?;
    conn.execute(
        "INSERT INTO source_secrets (name, body) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET body = excluded.body",
        params![name, blob],
    )?;
    Ok(())
}

/// Whether `name` is sealed. Does not return the bytes.
pub fn secret_is_set(conn: &Connection, name: &str) -> Result<bool> {
    let has: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='source_secrets'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if has == 0 {
        return Ok(false);
    }
    let n: i64 = conn.query_row("SELECT count(*) FROM source_secrets WHERE name = ?1", params![name], |r| r.get(0))?;
    Ok(n > 0)
}

/// Open one sealed secret.
pub fn open_secret(conn: &Connection, key: &[u8; 32], name: &str) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>> {
    if !secret_is_set(conn, name)? {
        return Ok(None);
    }
    let blob: Vec<u8> = conn.query_row("SELECT body FROM source_secrets WHERE name = ?1", params![name], |r| r.get(0))?;
    Ok(Some(acp_red_star_node_core::store::open_blob(key, name.as_bytes(), &blob).context("opening the secret")?))
}

/// Drop secret rows from a backup snapshot. An absent table leaves the file alone.
pub fn strip_secrets(conn: &Connection) -> Result<()> {
    let has: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='source_secrets'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if has == 0 {
        return Ok(());
    }
    let n: i64 = conn.query_row("SELECT count(*) FROM source_secrets", [], |r| r.get(0))?;
    if n > 0 {
        conn.execute("DELETE FROM source_secrets", [])?;
    }
    Ok(())
}

/// Lines for Account › Services.
pub fn services_rows(home: &Path) -> Vec<ServiceRow> {
    let mut rows = Vec::new();
    let services = crate::service::runtime().services();
    if services.is_empty() {
        let enabled = is_enabled(home, "redstar").unwrap_or(true);
        rows.push(ServiceRow::Service { id: "redstar".into(), label: "reserved".into(), enabled });
    }
    for service in services {
        let enabled = is_enabled(home, service.id()).unwrap_or(false);
        rows.push(ServiceRow::Service {
            id: service.id().into(),
            label: service.display_name().into(),
            enabled,
        });
    }
    rows.push(ServiceRow::Header("Quarantine".into()));
    match quarantine_items(home) {
        Ok(items) if items.is_empty() => rows.push(ServiceRow::Note("nothing quarantined".into())),
        Ok(items) => {
            for (file, reason) in items {
                rows.push(ServiceRow::Quarantine { file, reason });
            }
        }
        Err(e) => rows.push(ServiceRow::Note(format!("{e:#}"))),
    }
    rows.push(ServiceRow::Reload);
    rows
}

/// One row in Account › Services.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceRow {
    /// A registered service, or the reserved `redstar` id.
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
    /// Run the plugin loader. Not a key binding.
    Reload,
}

impl ServiceRow {
    /// One line for the Services window.
    pub fn line(&self) -> String {
        match self {
            Self::Service { label, enabled, .. } => format!("{label}  {}", if *enabled { "on" } else { "off" }),
            Self::Header(text) | Self::Note(text) => text.clone(),
            Self::Quarantine { file, reason } => format!("{file}  {reason}"),
            Self::Reload => "Reload plugins".into(),
        }
    }
}

/// Headless service commands. `--doctor` stays in [`crate::app`].
pub fn cli_requested(args: &Args) -> bool {
    args.services
        || args.service_enable.is_some()
        || args.service_disable.is_some()
        || args.service_set.is_some()
        || args.quarantine
        || args.quarantine_show.is_some()
        || args.quarantine_restore.is_some()
        || args.quarantine_delete.is_some()
}

/// Run a headless service command.
pub fn run_cli(args: &Args) -> Result<()> {
    if args.services {
        return print_services();
    }
    if let Some(id) = args.service_enable.as_deref() {
        set_enabled(&crate::config::home()?, id, true)?;
        println!("{id} enabled");
        return Ok(());
    }
    if let Some(id) = args.service_disable.as_deref() {
        set_enabled(&crate::config::home()?, id, false)?;
        println!("{id} disabled");
        return Ok(());
    }
    if let Some(parts) = &args.service_set {
        let (id, key, value) = (&parts[0], &parts[1], &parts[2]);
        set_config(&crate::config::home()?, id, key, value)?;
        println!("{id} {key} updated");
        return Ok(());
    }
    if args.quarantine {
        return print_quarantine();
    }
    if let Some(name) = args.quarantine_show.as_deref() {
        return print_quarantine_show(name);
    }
    if let Some(name) = args.quarantine_restore.as_deref() {
        return quarantine_restore(name);
    }
    if let Some(name) = args.quarantine_delete.as_deref() {
        return quarantine_delete(name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(tag: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!("acp-svc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(".acp")).unwrap();
        home
    }

    #[test]
    fn a_missing_file_enables_only_the_reserved_id() {
        let home = home("miss");
        assert!(is_enabled(&home, "redstar").unwrap());
        assert!(!is_enabled(&home, "x").unwrap());
        set_enabled(&home, "redstar", false).unwrap();
        assert!(!is_enabled(&home, "redstar").unwrap());
        let text = std::fs::read_to_string(service_path(&home, "redstar")).unwrap();
        assert!(text.contains("enabled = false"), "{text}");
        let err = set_enabled(&home, "../x", true).unwrap_err().to_string();
        assert!(err.contains("no service"), "{err}");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_secret_in_toml_fails_without_the_value() {
        let home = home("sec");
        let path = service_path(&home, "redstar");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "session = \"super-secret-token\"\n").unwrap();
        let err = is_enabled(&home, "redstar").unwrap_err().to_string();
        assert!(err.contains("session"), "{err}");
        assert!(!err.contains("super-secret-token"), "{err}");
        let err = set_config(&home, "redstar", "session", "super-secret-token").unwrap_err().to_string();
        assert!(err.contains("no field") || err.contains("secret"), "{err}");
        assert!(!err.contains("super-secret-token"), "{err}");
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
    fn quarantine_lists_and_deletes_without_opening_the_archive() {
        let home = home("q");
        let dir = home.join(".acp/quarantined/plugins");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bad.tar.gz"), b"not an archive").unwrap();
        std::fs::write(dir.join("bad.tar.gz.reason"), b"potential security threat\n").unwrap();
        let rows = services_rows(&home);
        assert!(rows.iter().any(|r| matches!(r, ServiceRow::Quarantine { file, reason } if file == "bad.tar.gz" && reason.contains("potential security threat"))));
        assert!(!rows.iter().any(|r| r.line().contains("not an archive")));
        delete_quarantine(&home, "bad.tar.gz").unwrap();
        assert!(!dir.join("bad.tar.gz").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn service_flags_stay_headless() {
        let (_, tui) = parse_with_mode(["x", "--services"]).unwrap();
        assert!(!tui);
        assert!(parse_with_mode(["x", "--services", "--nick", "ada"]).is_err());
        let (args, tui) = parse_with_mode(["x", "--service-set", "redstar", "enabled", "true"]).unwrap();
        assert!(!tui);
        assert_eq!(args.service_set.unwrap(), vec!["redstar", "enabled", "true"]);
    }

    #[test]
    fn combine_parses_grouped() {
        assert_eq!(Combine::parse("grouped"), Combine::Grouped);
        assert_eq!(Combine::parse("mixed"), Combine::Mixed);
        assert_eq!(Combine::Grouped.as_str(), "grouped");
    }
}
