//! TOML and JSON import and export of rules.
//!
//! Import always creates user rules. A file that names a `system_key` becomes a
//! clone (`cloned_from`) and does not insert a second system row. Export includes
//! system rules and user rules.

use super::*;

/// One rule in an import or export file. `trigger` is the name `post_synced`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FileRule {
    name: String,
    #[serde(default = "enabled_default")]
    enabled: bool,
    #[serde(default = "priority_default")]
    priority: i64,
    trigger: String,
    #[serde(default)]
    conditions: Vec<Condition>,
    actions: Vec<RuleAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    system_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cloned_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    template_key: Option<String>,
}

fn enabled_default() -> bool { true }
fn priority_default() -> i64 { 100 }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FileDoc {
    rules: Vec<FileRule>,
}

fn trigger_name(t: &Trigger) -> &'static str {
    match t {
        Trigger::PostSynced => "post_synced",
    }
}

fn parse_trigger(s: &str) -> Result<Trigger> {
    match s.trim() {
        "post_synced" => Ok(Trigger::PostSynced),
        other => bail!("unknown trigger {other:?}"),
    }
}

fn to_file(rule: &Rule) -> FileRule {
    FileRule {
        name: rule.name.clone(),
        enabled: rule.enabled,
        priority: rule.priority,
        trigger: trigger_name(&rule.trigger).into(),
        conditions: rule.conditions.clone(),
        actions: rule.actions.clone(),
        origin: Some(rule.origin.as_str().into()),
        system_key: rule.system_key.clone(),
        cloned_from: rule.cloned_from.clone(),
        template_key: rule.template_key.clone(),
    }
}

/// Render `rules` as a TOML or JSON document.
pub(crate) fn render(rules: &[Rule], json: bool) -> Result<String> {
    let doc = FileDoc { rules: rules.iter().map(to_file).collect() };
    if json { Ok(serde_json::to_string_pretty(&doc)?) } else { Ok(toml::to_string_pretty(&doc)?) }
}

/// Write every rule to `path`. `json` selects JSON; otherwise TOML.
pub fn export_file(c: &Connection, path: &Path, json: bool) -> Result<()> {
    let text = render(&list_rules(c)?, json)?;
    if let Some(dir) = path.parent() { if !dir.as_os_str().is_empty() { std::fs::create_dir_all(dir)?; } }
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Read rules from `path` and insert them as user rules. Returns how many were inserted.
/// `json` forces the format. `None` follows the extension (`.json`, otherwise TOML).
pub fn import_file(c: &Connection, path: &Path, json: Option<bool>) -> Result<usize> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let as_json = json.unwrap_or_else(|| path.extension().is_some_and(|e| e == "json"));
    let doc: FileDoc = if as_json {
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?
    } else {
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?
    };
    if doc.rules.is_empty() { bail!("{} has no rules", path.display()); }
    let mut n = 0;
    for file in doc.rules {
        let cloned_from = file.cloned_from.clone().or_else(|| file.system_key.clone());
        insert_user(c, &NewRule {
            name: file.name,
            enabled: file.enabled,
            priority: file.priority,
            cloned_from,
            template_key: file.template_key,
            trigger: parse_trigger(&file.trigger)?,
            conditions: file.conditions,
            actions: file.actions,
        })?;
        n += 1;
    }
    Ok(n)
}
