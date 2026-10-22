//! The command-line program: list, search, export, import, and maintenance.
//! Fetching, login, and publishing are not in this binary.

use crate::prelude::*;

/// A warning on stderr, in the notice form and as the existing `warning:` line.
fn cli_warning(args: &Args, body: &str) {
    err!("warning: {body}");
    let color = !colors_suppressed(args.color.unwrap_or_default(), no_color_set());
    crate::notify::print_stderr(
        &crate::notify::Notice::new(crate::notify::NoticeKind::Warning, "Warning", body),
        color,
    );
}

/// `--remove-nick`: preview (rolled-back transaction), confirm, then commit.
pub fn run_remove_nick(args: &Args, nicks: &[String]) -> Result<()> {
    let db = home()?.join(DB_REL_PATH);
    let att = home()?.join(ATTACH_REL_PATH);
    let preview = remove_nicks(&db, &att, nicks, args.keep_replies, false)?;
    let who = nicks.iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join(", ");
    eprintln!("{}", preview.summary(&format!("remove {who} would delete")));
    if args.keep_replies { eprintln!("--keep-replies: other people's replies in their threads are kept"); }
    if args.dry_run {
        for f in &preview.files { eprintln!("  file: {}", f.display()); }
        eprintln!("dry run: nothing changed");
        return Ok(());
    }
    if preview.is_empty() {
        eprintln!("nothing to remove");
        return Ok(());
    }
    if !confirm("Proceed?", false, args.yes)? {
        eprintln!("aborted: nothing changed");
        return Ok(());
    }
    let done = remove_nicks(&db, &att, nicks, args.keep_replies, true)?;
    eprintln!("{}", done.summary("removed"));
    Ok(())
}

/// `--clear-all`: delete the database and attachments dir; prefs and exports stay.
pub fn run_clear_all(args: &Args) -> Result<()> {
    let db = home()?.join(DB_REL_PATH);
    let att = home()?.join(ATTACH_REL_PATH);
    let targets = clear_all_targets(&db, &att);
    let (n, bytes) = dir_usage(&att);
    if targets.is_empty() {
        eprintln!("nothing to clear ({} and {} do not exist)", db.display(), att.display());
        return Ok(());
    }
    eprintln!("clear-all will delete:");
    for t in &targets {
        if t.is_dir() { eprintln!("  {}/ ({n} files, {bytes} bytes)", t.display()); } else { eprintln!("  {}", t.display()); }
    }
    eprintln!("kept: {} and {}", home()?.join(PREFS_REL_PATH).display(), home()?.join(EXPORTS_REL_PATH).display());
    if args.dry_run {
        eprintln!("dry run: nothing changed");
        return Ok(());
    }
    if !confirm("Delete all scraped data?", true, args.yes)? {
        eprintln!("aborted: nothing changed");
        return Ok(());
    }
    let gone = clear_all(&db, &att)?;
    eprintln!("cleared: {} paths removed", gone.len());
    Ok(())
}

/// `--bookmark` / `--unbookmark` / `--list-bookmarks` (offline).
pub fn run_bookmarks(args: &Args) -> Result<()> {
    let db = home()?.join(DB_REL_PATH);
    let c = bookmarks_conn(&db)?;
    let now = chrono::Local::now().to_rfc3339();
    if let Some(ids) = &args.bookmark { info!("bookmarked {} of {} posts", add_bookmarks(&c, ids, &now)?, ids.len()); }
    if let Some(ids) = &args.unbookmark { info!("removed {} of {} bookmarks", remove_bookmarks(&c, ids)?, ids.len()); }
    if args.list_bookmarks {
        let v = load_bookmarks(&c)?;
        let argv: Vec<String> = std::env::args().collect();
        let json_out = argv.iter().any(|a| a == "--format" || a.starts_with("--format=")) && args.formats().contains(&Format::Json);
        let mut st = if crate::maintenance::table_exists(&c, "posts")? {
            Some(c.prepare("SELECT nick, posted_at, substr(replace(content, char(10), ' '), 1, 60) FROM posts WHERE post_id = ?1")?)
        } else { None };
        let mut rows = Vec::new();
        for b in v.iter().filter(|b| !b.deleted) {
            let p: Option<(String, String, String)> = match st.as_mut() {
                Some(st) => st.query_row(params![b.post_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?,
                None => None,
            };
            rows.push((b, p));
        }
        if json_out {
            let j: Vec<_> = rows.iter().map(|(b, p)| json!({"post_id": b.post_id, "created_at": b.created_at, "on_site": b.remote,
                "nick": p.as_ref().map(|p| p.0.clone()), "posted_at": p.as_ref().map(|p| p.1.clone())})).collect();
            println!("{}", serde_json::to_string_pretty(&j)?);
        } else {
            for (b, p) in &rows {
                let (n, at, t) = p.clone().unwrap_or_else(|| ("?".into(), "-".into(), "(not in database)".into()));
                println!("{}  {:<24} @{n:<20} {at:<26} {t}", if b.remote { "★" } else { "☆" }, b.post_id);
            }
            let pending = v.iter().filter(|b| b.deleted).count();
            eprintln!("{} bookmarks (★ also on the site){}", rows.len(), if pending > 0 { format!("; {pending} removals waiting for --sync-bookmarks") } else { String::new() });
        }
    }
    Ok(())
}

/// `--brand NAME`: a built-in, or a usable file in `~/.acp/brands`.
fn check_brand(name: &str) -> Result<()> {
    crate::brands::check(name)
}

/// `--list-brands`: built-ins and brand files. ● marks the one in use.
fn run_list_brands(args: &Args) -> Result<()> {
    let dir = crate::brands::brands_dir()?;
    let prefs = load_prefs();
    let current = args.brand.clone().or_else(|| prefs.ui.as_ref().and_then(|u| u.brand.clone())).unwrap_or_else(|| DEFAULT_BRAND.into());
    let entries = crate::brands::list(&dir);
    println!("Brands (built-in, and files in {}):", dir.display());
    for e in &entries {
        let mark = if e.name == current { "●" } else { " " };
        match &e.error {
            Some(err) => println!("{mark} {:<20} BROKEN: {err}", e.name),
            None => {
                let acp_row = e.title.contains("ACP") || e.name == DEFAULT_BRAND;
                let motto = if e.motto.is_empty() || (e.motto == crate::branding::SLOGAN && !acp_row) {
                    "(no motto)"
                } else {
                    e.motto.as_str()
                };
                println!("{mark} {:<20} {}  — {motto}", e.name, e.title);
            }
        }
    }
    if !entries.iter().any(|e| e.name == current && e.error.is_none()) {
        println!("(selected brand {current} is missing or broken: the default brand is used)");
    }
    Ok(())
}

/// `--theme NAME`: the name must be valid and the file usable.
async fn check_theme(name: &str) -> Result<()> {
    let dir = crate::themes::themes_dir()?;
    first_run_download(&dir).await;
    crate::themes::prepare(&dir, &load_prefs());
    match crate::themes::load(&dir, name) {
        Ok(_) => Ok(()),
        Err(crate::themes::LoadError::Missing(p)) => {
            let have: Vec<String> = crate::themes::list(&dir).into_iter().map(|e| e.name).collect();
            bail!("no theme {name}: {} does not exist (themes: {})", p.display(), if have.is_empty() { "none".into() } else { have.join(", ") })
        }
        Err(e) => bail!("{}", e.notice()),
    }
}

/// The first run of a headless theme command (empty `~/.acp/themes`): download the
/// default theme unless that is turned off, reporting what happened on stderr.
async fn first_run_download(dir: &Path) {
    use crate::themes::download::*;
    if crate::themes::has_themes(dir) { return; }
    if !auto_enabled_now(&load_prefs()) { info!("{}", off_notice(dir)); return; }
    match download(dir, DEFAULT_THEME).await {
        Ok(f) => info!("{}", done_notice(DEFAULT_THEME, &f)),
        Err(e) => info!("warning: {}", failure_notice(dir, DEFAULT_THEME, &format!("{e:#}"))),
    }
}

/// `--theme-init [NAME|all]`: download themes from the repo into `~/.acp/themes`.
async fn run_theme_init(what: &str) -> Result<()> {
    use crate::themes::download::*;
    let dir = crate::themes::themes_dir()?;
    let names: Vec<String> = if what == "all" { SHIPPED_THEMES.iter().map(|s| s.to_string()).collect() } else { vec![what.to_string()] };
    let mut failed = Vec::new();
    for n in &names {
        match download(&dir, n).await {
            Ok(f) => println!("{}", done_notice(n, &f)),
            Err(e) => { eprintln!("error: {}", failure_notice(&dir, n, &format!("{e:#}"))); failed.push(n.clone()); }
        }
    }
    if !failed.is_empty() { bail!("could not download {}", failed.join(", ")); }
    Ok(())
}

/// `--theme-starter NAME`: write the minimal starter theme (no download, no overwrite).
fn run_theme_starter(name: &str) -> Result<()> {
    let dir = crate::themes::themes_dir()?;
    let f = crate::themes::download::write_starter(&dir, name)?;
    let n = crate::themes::download::starter_notice(name, &f);
    if matches!(f, crate::themes::download::Fetched::Kept(_)) { bail!("{n}; pick another name"); }
    println!("{n}");
    Ok(())
}

/// `--list-themes`: every theme file, the one in use marked.
async fn run_list_themes(args: &Args) -> Result<()> {
    let dir = crate::themes::themes_dir()?;
    first_run_download(&dir).await;
    let prefs = load_prefs();
    let (migrated, notes) = crate::themes::prepare(&dir, &prefs);
    if let Some(m) = &migrated { save_ui_theme(m)?; }
    for n in notes { info!("{n}"); }
    let current = args.theme.clone().or(migrated).or_else(|| prefs.ui.as_ref().and_then(|u| u.theme.clone())).unwrap_or_else(|| DEFAULT_THEME.into());
    let entries = crate::themes::list(&dir);
    println!("Themes in {}:", dir.display());
    if entries.is_empty() { println!("  (none)"); }
    for e in &entries {
        let mark = if e.name == current { "●" } else { " " };
        match &e.error {
            Some(err) => println!("{mark} {:<20} BROKEN: {err}", e.name),
            None => println!("{mark} {:<20} {}{}", e.name, e.description, if e.warnings > 0 { format!(" ({} warnings)", e.warnings) } else { String::new() }),
        }
    }
    if !entries.iter().any(|e| e.name == current) { println!("(selected theme {current} is missing: the terminal UI uses the terminal's colours)"); }
    Ok(())
}

/// Run the program for the current process arguments.
pub async fn run() -> Result<()> {
    // The cache lives for the process. Dropping this wipes every key, including
    // on the early returns below. A job child installs its copy before this runs.
    let _wipe = crate::crypto::atrest::KeyWipe;
    crate::crypto::atrest::accept_inherited_key()?;
    let (args, tui) = match parse_with_mode(std::env::args_os()) {
        Ok(v) => v,
        Err(e) => e.exit(),
    };
    // Before brand files, the home directory, and every other command.
    if args.doctor {
        for line in crate::service::doctor_lines() {
            println!("{line}");
        }
        return Ok(());
    }
    if args.plugins_reload {
        return crate::plugins::run_reload();
    }
    let (brand, brand_warn) = crate::brands::selected(args.brand.as_deref());
    crate::brands::install(&brand);
    if let Some(w) = brand_warn {
        if args.brand.is_none() { eprintln!("warning: {w}"); }
    }
    apply_color_mode(resolve_color_mode(args.color, load_prefs().ui.as_ref().and_then(|u| u.color.as_deref())));
    // Before the database, the home directory, and the terminal UI. `--roar --tui` still exits here.
    if args.roar {
        println!("{}", crate::eggs::roar_text(crate::eggs::roar_index(), crate::eggs::colors_off(), crate::eggs::columns()));
        return Ok(());
    }
    if let Some(what) = args.generate {
        return generate(what, &mut io::stdout().lock());
    }
    let level = if args.quiet {
        LogLevel::Quiet
    } else if args.debug || std::env::var("ACP_DEBUG").is_ok_and(|v| v == "1") {
        LogLevel::Debug
    } else if args.verbose {
        LogLevel::Verbose
    } else {
        LogLevel::Normal
    };
    set_log_level(level);
    if let Some((ok, bad)) = resolve_log_paths(args.enable_logging, level == LogLevel::Debug,
        args.log.as_deref(), args.error_log.as_deref(), &home()?) {
        init_file_logging(&ok, &bad)
            .map_err(|e| anyhow!("cannot open log files {} / {}: {e}", ok.display(), bad.display()))?;
        verbose!("logging to {} and {}", ok.display(), bad.display());
    }
    if args.identity_init { return crate::crypto::cmd::identity_init(); }
    if args.acp_id { return crate::crypto::cmd::print_acp_id(); }
    if args.node_init { return crate::crypto::cmd::node_init(args.node_label.as_deref()); }
    if args.node_id { return crate::crypto::cmd::print_node_id(); }
    if args.shares_print { return crate::crypto::cmd::shares_print(args.shares_file.as_deref()); }
    if args.shares_verify { return crate::crypto::cmd::shares_verify(args.shares_file.as_deref()); }
    if args.show_phrase || args.show_recovery { return crate::crypto::cmd::show_recovery(); }
    if let Some(cadence) = args.backup_schedule.as_deref() {
        return crate::crypto::cmd::backup_schedule(cadence, args.backup_keep.unwrap_or(1), args.backup_dest.as_deref().unwrap());
    }
    if args.backup { return crate::crypto::cmd::backup_now(args.output.as_deref()); }
    if let Some(path) = args.restore.as_deref() { return crate::crypto::cmd::restore_now(path); }
    if args.encrypt_db { return crate::crypto::cmd::encrypt_db(); }
    if args.change_password { return crate::crypto::cmd::change_password(); }
    if crate::outbox::cli_requested(&args) { return crate::outbox::run_cli(&args).await; }
    if let Some(t) = &args.theme_init { return run_theme_init(t).await; }
    if let Some(n) = &args.theme_starter { return run_theme_starter(n); }
    if args.list_brands { return run_list_brands(&args); }
    if args.list_themes { return run_list_themes(&args).await; }
    if let Some(t) = &args.brand { check_brand(t)?; }
    if let Some(t) = &args.theme { check_theme(t).await?; }
    if tui {
        return crate::tui::run(args).await;
    }
    if let Some(list) = &args.remove_nick {
        crate::crypto::cmd::prepare_db()?;
        return run_remove_nick(&args, &list.0);
    }
    if args.clear_all {
        crate::crypto::cmd::prepare_db()?;
        return run_clear_all(&args);
    }
    if args.logout {
        bail!("{}", crate::service::SERVICE_ABSENT);
    }
    crate::crypto::cmd::prepare_db()?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let clean = |n: String| n.replace(['/', '\\'], "_");
    if crate::rules::cli_requested(&args) {
        return crate::rules::run_cli(&args);
    }
    if args.research_list || args.research_create.is_some() || args.research_add.is_some() || args.research_show.is_some() || args.research_export.is_some() {
        return run_research(&args).await;
    }
    if args.bookmark.is_some() || args.unbookmark.is_some() || args.list_bookmarks {
        return run_bookmarks(&args);
    }
    if args.list_nicks {
        let infos = list_nicks_db(&home()?.join(DB_REL_PATH))?;
        let argv: Vec<String> = std::env::args().collect();
        let explicit = argv.iter().any(|a| a == "--format" || a.starts_with("--format="));
        let want_json = explicit && args.formats().contains(&Format::Json);
        if want_json {
            let v: Vec<_> = infos.iter().map(|i| json!({
                "nick": i.nick, "display_name": i.display_name, "post_count": i.post_count,
                "latest": i.latest, "profile_scraped": i.profile_scraped, "scraped_at": i.scraped_at,
            })).collect();
            println!("{}", serde_json::to_string_pretty(&v)?);
        } else {
            print_nicks_table(&infos);
        }
        return Ok(());
    }
    if let Some(file) = &args.import {
        let (posts, atts, profiles, hits) = import(file, args.compression(), &home()?.join(ATTACH_REL_PATH), &home()?.join(EXPORTS_REL_PATH))?;
        let n = save(&posts)?;
        save_attachments(&home()?.join(DB_REL_PATH), &atts)?;
        for p in &profiles { save_profile(&home()?.join(DB_REL_PATH), p)?; }
        save_mentions(&home()?.join(DB_REL_PATH), &hits)?;
        info!("import: {} posts read, {n} new; {} attachments; {} profiles; {} mentions",
            posts.len(), atts.len(), profiles.len(), hits.len());
        return Ok(());
    }
    // Offline export of chosen posts (the TUI's export basket).
    if let Some(ids) = &args.post_ids {
        let mut a = args.clone();
        if let Some(f) = &args.export { a.output = Some(f.clone()); }
        let db = home()?.join(DB_REL_PATH);
        let (posts, atts, missing) = posts_by_ids(&db, &ids.0)?;
        for m in &missing { cli_warning(&args, &format!("post {m} is not in the database")); }
        if posts.is_empty() { bail!("none of the {} posts is in the database", ids.0.len()); }
        info!("db export: {} chosen posts, {} attachments", posts.len(), atts.len());
        let name = clean(args.name.clone().unwrap_or_else(|| format!("basket-{stamp}")));
        let mut nicks: Vec<String> = posts.iter().map(|p| p.nick.clone()).collect();
        nicks.sort();
        nicks.dedup();
        let (profiles, hits) = load_profiles_mentions(&db, &nicks)?;
        let label = format!("{} chosen posts", posts.len());
        return export(&a, &name, &label, &posts, &atts, &profiles, &hits, &home()?.join(EXPORTS_REL_PATH));
    }
    // Offline DB export: --export FILE, or --search (to stdout/--output).
    if args.export.is_some() || args.search.is_some() {
        let mut a = args.clone();
        if let Some(f) = &args.export { a.output = Some(f.clone()); }
        let q = args.search.as_deref();
        let nicks = args.nick_list()?;
        let (mut posts, atts, n) = search_db(&home()?.join(DB_REL_PATH), q, &nicks)?;
        if let Some(raw) = &args.search_json {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
                if let Some(query) = crate::storage::history::SavedQuery::from_value(&v) {
                    posts = query.apply(posts);
                }
            }
        }
        info!("db export: {n} matches, {} posts total, {} attachments", posts.len(), atts.len());
        let name = clean(args.name.clone().unwrap_or_else(|| match q {
            Some(q) => format!("search-{}-{stamp}", slug(q)),
            None if !nicks.is_empty() => nicks_name(&nicks, &stamp),
            None => format!("db-{stamp}"),
        }));
        let who = nicks.iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join(", ");
        let label = match (q, who.is_empty()) {
            (Some(q), true) => format!("search: {q}"),
            (Some(q), false) => format!("search: {q} ({who})"),
            (None, false) => who,
            (None, true) => "all posts".into(),
        };
        let (profiles, hits) = load_profiles_mentions(&home()?.join(DB_REL_PATH), &nicks)?;
        return export(&a, &name, &label, &posts, &atts, &profiles, &hits, &home()?.join(EXPORTS_REL_PATH));
    }
    if crate::drive::handles(acp_rs_plugin::CapabilityId::Reader) {
        if crate::drive::handles(acp_rs_plugin::CapabilityId::Authenticator) {
            crate::drive::login(crate::host_api::CoreHost::shared().as_ref()).await?;
        }
        return crate::drive::read_flags(&args).await;
    }
    if crate::drive::handles(acp_rs_plugin::CapabilityId::Authenticator) {
        crate::drive::login(crate::host_api::CoreHost::shared().as_ref()).await?;
    }
    if let Some(text) = crate::drive::compose(&args).await? {
        if !text.is_empty() {
            println!("{text}");
        }
        return Ok(());
    }
    bail!("{}", crate::service::SERVICE_ABSENT);
}
