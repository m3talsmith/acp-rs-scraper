//! `--research-*` flags.

use crate::prelude::*;

/// Run whichever `--research-*` flag was given.
pub async fn run_research(args: &Args) -> Result<()> {
    let db = home()?.join(DB_REL_PATH);
    let json_out = std::env::args().any(|a| a == "--format" || a.starts_with("--format=")) && args.formats().contains(&Format::Json);
    if args.research_list {
        let c = research_conn(&db)?;
        let ps = list_projects(&c)?;
        if json_out {
            let v: Vec<Value> = ps.iter().map(|p| json!({"name": p.name, "description": p.description, "created_at": p.created_at, "items": p.items})).collect();
            println!("{}", serde_json::to_string_pretty(&v)?);
        } else if ps.is_empty() {
            println!("No research projects yet (--research-create NAME).");
        } else {
            println!("{:<30} {:>6}  {:<10}  DESCRIPTION", "PROJECT", "ITEMS", "CREATED");
            for p in &ps { println!("{:<30} {:>6}  {:<10}  {}", p.name, p.items, p.created_at.get(..10).unwrap_or(""), p.description); }
        }
        return Ok(());
    }
    if let Some(name) = &args.research_create {
        let c = research_conn(&db)?;
        create_project(&c, name, args.research_description.as_deref().unwrap_or(""))?;
        info!("research project {:?} created", name.trim());
        return Ok(());
    }
    if let Some(name) = &args.research_show {
        let c = research_conn(&db)?;
        let p = get_project(&c, name)?.ok_or_else(|| anyhow!("no research project {name:?} (--research-list shows them)"))?;
        let items = project_items(&c, p.id)?;
        if json_out {
            println!("{}", serde_json::to_string_pretty(&research_json(&p, &items, &|i| (!i.local_path.is_empty()).then(|| i.local_path.clone())))?);
        } else {
            println!("{} ({} items){}", p.name, items.len(), if p.description.is_empty() { String::new() } else { format!(" — {}", p.description) });
            for i in &items {
                println!("  [{}] {:<13} {}  {}", i.id, i.kind.as_str(), i.title, i.r#ref);
                if !i.note.is_empty() { println!("        note: {}", i.note); }
            }
        }
        return Ok(());
    }
    if let Some(name) = &args.research_add {
        let note = args.note.clone().unwrap_or_default();
        let mut items = Vec::new();
        if let Some(ids) = &args.post_ids {
            let (v, missing) = post_items(&db, &ids.0, None, json!({}), &note)?;
            for m in &missing { err!("warning: post {m} is not in the database; not added"); }
            items.extend(v);
        }
        if let Some(q) = &args.search {
            let nicks = args.nick_list()?;
            let (posts, _, n) = search_db(&db, Some(q), &nicks)?;
            let ids: Vec<String> = posts.iter().take(n).map(|p| p.post_id.clone()).collect();
            let (v, _) = post_items(&db, &ids, Some(ItemKind::SearchResult), json!({"query": q, "nicks": nicks}), &note)?;
            info!("search {q:?}: {} matches", v.len());
            items.extend(v);
        }
        if let Some(f) = &args.file {
            let p = expand_tilde(f);
            items.push(file_item(name, &p, !args.no_copy, &note)?);
        }
        if let Some(u) = &args.url { items.push(web_item(name, u, &note).await?); }
        if items.is_empty() && args.post_ids.is_none() && args.search.is_none() {
            bail!("--research-add needs something to add: --post-ids, --search, --file or --url");
        }
        let c = research_conn(&db)?;
        let (pid, created) = get_or_create(&c, name, args.research_description.as_deref())?;
        if created { info!("research project {:?} created", name.trim()); }
        let mut added = 0;
        for it in &items { if add_item(&c, pid, it)? { added += 1; } }
        info!("research {:?}: {added} added, {} already there", name.trim(), items.len() - added);
        return Ok(());
    }
    if let Some(name) = &args.research_export {
        let c = research_conn(&db)?;
        let p = get_project(&c, name)?.ok_or_else(|| anyhow!("no research project {name:?} (--research-list shows them)"))?;
        let items = project_items(&c, p.id)?;
        drop(c);
        let ids: Vec<String> = items.iter().filter(|i| i.kind.is_post()).map(|i| i.r#ref.clone()).collect();
        let (posts, mut atts, missing) = posts_by_ids(&db, &ids)?;
        for m in &missing { err!("warning: post {m} of the project is no longer in the database"); }
        atts.extend(item_attachments(&items));
        let mut nicks: Vec<String> = posts.iter().map(|p| p.nick.clone()).collect();
        nicks.sort();
        nicks.dedup();
        let (profiles, hits) = if nicks.is_empty() { (Vec::new(), Vec::new()) } else { load_profiles_mentions(&db, &nicks)? };
        let mut a = args.clone();
        a.output = args.export.clone();
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let ename = args.name.clone().unwrap_or_else(|| format!("research-{}-{stamp}", slug(&p.name))).replace(['/', '\\'], "_");
        info!("research export {:?}: {} items, {} posts, {} files", p.name, items.len(), posts.len(), atts.len());
        return export_with(&a, &ename, &format!("research: {}", p.name), &posts, &atts, &profiles, &hits,
            &home()?.join(EXPORTS_REL_PATH), Some((&p, &items)));
    }
    Ok(())
}

/// `~/x` → `$HOME/x`.
pub fn expand_tilde(p: &Path) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix("~/")) {
        Some(rest) => home().map(|h| h.join(rest)).unwrap_or_else(|_| p.to_path_buf()),
        None => p.to_path_buf(),
    }
}
