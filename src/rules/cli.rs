//! `--rules-*` flags. Offline. Nothing here talks to the site.

use super::{
    by_name, clone_system, delete_user, export_file, file, import_file, list_rules, open_rules, preview, set_enabled, set_priority,
};
use crate::prelude::*;

/// Run whichever `--rules-*` flag was given.
pub fn run_cli(args: &Args) -> Result<()> {
    let db = home()?.join(DB_REL_PATH);
    let c = open_rules(&db)?;
    let json = match args.rules_format {
        Some(RulesFormat::Json) => Some(true),
        Some(RulesFormat::Toml) => Some(false),
        None => None,
    };
    if args.rules_list {
        let rules = list_rules(&c)?;
        if json == Some(true) {
            println!("{}", file::render(&rules, true)?);
        } else if rules.is_empty() {
            println!("No rules.");
        } else {
            println!("{:<8} {:<5} {:<8} NAME", "PRIORITY", "ON", "ORIGIN");
            for r in &rules {
                let on = if r.enabled { "on" } else { "off" };
                println!("{:<8} {:<5} {:<8} {}", r.priority, on, r.origin.as_str(), r.name);
            }
        }
        return Ok(());
    }
    if let Some(name) = &args.rules_show {
        let rule = by_name(&c, name)?;
        println!("{}", file::render(&[rule], json == Some(true))?);
        return Ok(());
    }
    if let Some(name) = &args.rules_enable {
        set_enabled(&c, name, true)?;
        info!("rule {name:?} enabled");
        return Ok(());
    }
    if let Some(name) = &args.rules_disable {
        set_enabled(&c, name, false)?;
        info!("rule {name:?} disabled");
        return Ok(());
    }
    if let Some(key) = &args.rules_clone {
        let rule = clone_system(&c, key)?;
        info!("cloned {} as {:?}", key, rule.name);
        return Ok(());
    }
    if let Some(name) = &args.rules_delete {
        delete_user(&c, name)?;
        info!("deleted rule {name:?}");
        return Ok(());
    }
    if let Some(name) = &args.rules_priority {
        let rank = args.rules_rank.ok_or_else(|| anyhow!("pass --rules-rank N with --rules-priority"))?;
        set_priority(&c, name, rank)?;
        info!("rule {name:?} priority {rank}");
        return Ok(());
    }
    if let Some(path) = &args.rules_import {
        let n = import_file(&c, path, json)?;
        info!("imported {n} rules from {}", path.display());
        return Ok(());
    }
    if let Some(path) = &args.rules_export {
        let as_json = json.unwrap_or_else(|| path.extension().is_some_and(|e| e == "json"));
        export_file(&c, path, as_json)?;
        info!("exported rules to {}", path.display());
        return Ok(());
    }
    if args.rules_preview {
        let fired = preview(&db)?;
        if fired.is_empty() { println!("No rules matched stored posts."); }
        for f in &fired {
            println!("{}  {}  {}  {}  {}", f.outcome, f.rule_name, f.post_id, f.action, f.detail);
        }
        return Ok(());
    }
    Ok(())
}
