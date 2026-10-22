//! The command-line program: dispatches to offline modes (list, search,
//! export, import, maintenance) or logs in and scrapes.

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
    if args.show_phrase { return crate::crypto::cmd::show_phrase(); }
    if let Some(cadence) = args.backup_schedule.as_deref() {
        return crate::crypto::cmd::backup_schedule(cadence, args.backup_keep.unwrap_or(1), args.backup_dest.as_deref().unwrap());
    }
    if args.backup { return crate::crypto::cmd::backup_now(args.output.as_deref()); }
    if let Some(path) = args.restore.as_deref() { return crate::crypto::cmd::restore_now(path); }
    if args.encrypt_db { return crate::crypto::cmd::encrypt_db(); }
    if args.change_password { return crate::crypto::cmd::change_password(); }
    if crate::outbox::cli_requested(&args) { return crate::outbox::run_cli(&args).await; }
    if crate::site::cli_requested(&args) { return crate::site::run_cli(&args).await; }
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
        let jar = Jar::default();
        load_session(&jar).ok();
        let removed = logout(&jar)?;
        if removed { info!("Logged out: removed {}.", session_path()?.display()); }
        else { info!("Not logged in: no saved session at {}.", session_path()?.display()); }
        return Ok(());
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
    let jar = Arc::new(Jar::default());
    if let Err(e) = load_session(&jar) { cli_warning(&args, &format!("ignoring saved session: {e:#}")); }
    let http = client(jar.clone())?;
    login(&http, &jar).await?;
    if let Some(period) = args.top {
        let n = read_top(&http, &jar, period, args.top_replies).await?;
        info!("== top {}: {n} posts", period.as_str());
        return Ok(());
    }
    if args.eb {
        let n = read_eb(&http, &jar, args.eb_replies).await?;
        info!("== eb: {n} posts");
        return Ok(());
    }
    if args.explore || args.explore_latest {
        let n = read_explore(&http, &jar, args.explore_latest).await?;
        info!("== explore: {n} posts");
        return Ok(());
    }
    if let Some(q) = &args.site_search {
        let n = read_site_search(&http, &jar, q).await?;
        info!("== site search {q:?}: {n} posts");
        return Ok(());
    }
    if let Some(tag) = &args.tag {
        let n = read_tag(&http, &jar, tag).await?;
        info!("== tag {}: {n} posts", tag_title(tag));
        return Ok(());
    }
    if let Some(id) = &args.reposts {
        let n = read_reposts(&http, &jar, id).await?;
        info!("== reposts {id}: {n} posts");
        return Ok(());
    }
    if let Some(spec) = &args.edits {
        let (thread, post) = resolve_edits_spec(spec)?;
        let n = read_edits(&http, &jar, &thread, &post).await?;
        info!("== edits {thread}/{post}: {n} versions");
        return Ok(());
    }
    let live_wait = std::time::Duration::from_secs(args.live_for.unwrap_or(1));
    if args.live.is_none() && !args.live_feed && !args.site_notifications && args.live_for.is_some() {
        bail!("--live-for needs --live, --live-feed, or --site-notifications");
    }
    if let Some(id) = &args.live {
        let lines = crate::livefeed::watch_thread(&http, &jar, id, live_wait).await?;
        for line in &lines { println!("{line}"); }
        info!("== live {id}: {} events", lines.len());
        return Ok(());
    }
    if args.live_feed {
        let lines = crate::livefeed::watch_feed(&http, &jar, live_wait).await?;
        for line in &lines { println!("{line}"); }
        info!("== live feed: {} events", lines.len());
        return Ok(());
    }
    if args.site_notifications {
        let lines = crate::livefeed::read_site_notifications(&http, &jar, live_wait).await?;
        for line in &lines { println!("{line}"); }
        info!("== site notifications: {} lines", lines.len());
        return Ok(());
    }
    if let Some(id) = &args.sort_thread {
        let order = args.thread_sort.as_deref().unwrap_or("oldest");
        let n = read_sorted_thread(&http, &jar, id, order).await?;
        info!("== sort thread {id} {order}: {n} posts");
        return Ok(());
    }
    if args.backfill_tags {
        let nicks = args.nick_list()?;
        let st = backfill_tags(&http, &jar, &home()?.join(DB_REL_PATH), &nicks).await?;
        info!("== backfill-tags: {} scanned, {} tagged, {} json filled, {} still missing ({} threads)",
            st.scanned, st.tagged, st.json_filled, st.missing, st.threads);
        return Ok(());
    }
    let mut nicks = args.nick_list()?;
    if args.sync_bookmarks || (args.sync && !args.no_bookmarks) {
        let st = sync_bookmarks(&http, &jar, &home()?.join(DB_REL_PATH)).await?;
        info!("== bookmarks: {} on the site, {} new posts stored, {} pulled, {} added and {} removed on the site, {} dropped{}",
            st.site, st.posts_saved, st.pulled, st.pushed.0, st.pushed.1, st.dropped,
            if st.failed > 0 { format!(", {} refused", st.failed) } else { String::new() });
        if nicks.is_empty() && !args.refetch_truncated && !args.scrape_all { return Ok(()); }
    }
    if args.scrape_all {
        let db = home()?.join(DB_REL_PATH);
        let report = match crate::scrape::sitewide::scrape_site(&http, &jar, &db, crate::scrape::sitewide::CrawlOpts {
            origin: base_url().to_string(),
            incremental: !args.full,
            limit: args.limit,
            budget_ms: args.scrape_budget.map(|secs| secs.saturating_mul(1000)),
            attachments: !args.no_attachments,
        }).await {
            Ok(report) => report,
            Err(e) => {
                err!("error: whole site: {e:#}");
                return Err(e);
            }
        };
        let line = match &report.stopped {
            Some(why) => format!("paused at the {why}. {} pages left. The next run resumes.", report.pending),
            None => format!("finished. {} new posts across {} pages.", report.new_posts, report.pages),
        };
        info!("Job: whole site {line}");
        if args.sync {
            if let Err(e) = crate::rules::on_sync(&db, &report.posts) {
                err!("warning: rules: {e:#}");
            }
        }
        if nicks.is_empty() && !args.refetch_truncated { return Ok(()); }
    }
    if args.sync && nicks.is_empty() && !args.refetch_truncated {
        info!("sync: no nicks given, nothing to scrape");
        return Ok(());
    }
    if args.refetch_truncated {
        let st = refetch_truncated(&http, &jar, &home()?.join(DB_REL_PATH), &nicks).await?;
        info!("== refetch-truncated: {} rows checked, {} completed, {} already full, {} not found ({} thread pages)",
            st.candidates, st.fixed, st.already_full, st.missing, st.threads);
        if nicks.is_empty() { return Ok(()); }
    }
    if nicks.is_empty() {
        nicks = parse_nicks(&prompt("Nick(s) to scrape (comma-separated): ")?).map_err(|e| anyhow!(e))?.0;
    }
    let db = home()?.join(DB_REL_PATH);
    let att_dir = home()?.join(ATTACH_REL_PATH);
    let known = db_known_ids(&db)?;
    let reply_counts = db_reply_counts(&db)?;
    let known_infos = list_nicks_db(&db)?;
    info!("== mode: {} (mentions crawl); profile feed: {} ({})", args.feed_mode(), args.profile_feed, args.order);
    if args.said_by { info!("== --said-by: only the nicks' own posts and replies; no thread crawl, no mentions"); }
    info!("== targets");
    for nick in &nicks {
        if let Some(i) = known_infos.iter().find(|i| i.nick.eq_ignore_ascii_case(nick)) {
            let dn = i.display_name.as_deref().unwrap_or("-");
            info!("  @{nick}: known ({dn}, {} posts{})",
                i.post_count,
                if i.profile_scraped { ", profile scraped" } else { "" });
        } else {
            info!("  @{nick}: new");
        }
    }
    let mut posts: Vec<Post> = Vec::new();
    let mut profiles: Vec<Profile> = Vec::new();
    let mut mention_hits: Vec<MentionHit> = Vec::new();
    let mut targets: Vec<MentionTarget> = Vec::new();
    let mut seen = HashSet::new();
    let mut summary = Vec::new();
    let mut ok_nicks = Vec::new();
    for nick in &nicks {
        info!("== scraping @{nick}");
        match scrape(&http, &jar, nick, &ScrapeOptions {
            known: &known,
            reply_counts: &reply_counts,
            full: args.full,
            said_by: args.said_by,
            limit: args.limit,
            profile_feed: args.profile_feed,
            order: args.order,
        }).await {
            Ok((ps, html, st)) => {
                info!("  @{nick}: feed {} items ({} from posts and replies), ~{} new, {} skipped known, threads fetched {} / skipped {}",
                    st.feed_items, st.replies_tab_items, st.new_posts, st.skipped_known, st.threads_fetched, st.threads_skipped);
                if st.truncated_in_feed > 0 {
                    info!("  @{nick}: {} truncated feed bodies, {} completed from thread pages",
                        st.truncated_in_feed, st.truncated_completed);
                }
                let mut profile = Profile {
                    nick: nick.clone(),
                    scraped_at: chrono::Local::now().to_rfc3339(),
                    ..Default::default()
                };
                // The feed's author object carries the real profile photo; read it
                // before the page's og: tags, which fall back to the site banner.
                // Reply-only nicks have an empty Posts feed, so also look at
                // "Posts and replies" when the Posts feed had no author data.
                for url in [profile_url(nick), profile_replies_url(nick)] {
                    if profile.avatar_url.is_some() && profile.display_name.is_some() { break; }
                    match fetch_feed(&http, &jar, &url, Some(10), None, true).await {
                        Ok(feed) => {
                            enrich_from_feed(&mut profile, &feed.items, nick);
                            if profile.avatar_url.is_none() && let Some(it) = feed.items.first() {
                                dbg_log("profile", &format!("@{nick} no photo in {url}; first item: {it}"));
                            }
                        }
                        Err(e) => dbg_log("profile", &format!("@{nick} {url}: {e:#}")),
                    }
                }
                enrich_from_html(&mut profile, &html);
                if profile.avatar_url.as_deref().is_some_and(is_site_image) { profile.avatar_url = None; }
                dbg_log("profile", &format!("@{nick} show-info unused event={PROFILE_SHOW_INFO_EVENT} fields={profile:?}"));
                if !args.no_attachments {
                    if let Some(u) = &profile.avatar_url {
                        match fetch_avatar(&http, u, &att_dir, nick).await {
                            Ok(path) => {
                                info!("  @{nick}: profile image {}", path.display());
                                profile.avatar_path = Some(path.to_string_lossy().into());
                            }
                            Err(e) => cli_warning(&args, &format!("@{nick} profile image: {e:#}")),
                        }
                    }
                    if let Some(u) = &profile.banner_url
                        && let Ok((path, _)) = fetch_one_pub(&http, u, &att_dir).await {
                            profile.banner_path = Some(path.to_string_lossy().into());
                        }
                }
                profile.raw_json = serde_json::to_string(&profile).unwrap_or_default();
                save_profile(&db, &profile)?;
                targets.push(MentionTarget {
                    nick: nick.clone(),
                    user_id: profile.user_id.clone(),
                    display_name: profile.display_name.clone(),
                });
                profiles.push(profile);

                let roots = ps.iter().filter(|p| p.parent_post_id.is_empty()).count();
                let total = ps.len();
                let new: Vec<Post> = ps.into_iter().filter(|p| seen.insert(p.post_id.clone())).collect();
                let inserted = save(&new)?;
                summary.push(format!(
                    "@{nick}: {total} items ({roots} root posts, {} replies), {inserted} new DB rows",
                    total - roots
                ));
                posts.extend(new);
                ok_nicks.push(nick.clone());
            }
            Err(e) => {
                cli_warning(&args, &format!("@{nick} failed: {e:#}"));
                summary.push(format!("@{nick}: FAILED ({e})"));
            }
        }
    }
    if args.want_mentions() && !ok_nicks.is_empty() {
        info!("== mentions crawl (Latest, limit {})", args.crawl_limit);
        match crawl_mentions(&http, &jar, &targets, args.crawl_limit, &mut seen, &known, args.full, args.feed_mode()).await {
            Ok((extra, hits)) => {
                let inserted = save(&extra)?;
                save_mentions(&db, &hits)?;
                info!("mentions: {} posts, {} hits, {inserted} new DB rows", extra.len(), hits.len());
                posts.extend(extra);
                mention_hits.extend(hits);
            }
            Err(e) => cli_warning(&args, &format!("mentions crawl failed: {e:#}")),
        }
    }
    info!("== summary");
    for l in &summary { info!("  {l}"); }
    if ok_nicks.is_empty() { bail!("all nicks failed"); }
    let atts = if args.no_attachments { Vec::new() } else {
        let for_dl: Vec<Post> = if args.full {
            posts.clone()
        } else {
            // Only download media for posts we didn't already have.
            posts.iter().filter(|p| !known.contains(&p.post_id)).cloned().collect()
        };
        info!("attachments: downloading for {} posts ({} skipped as known)",
            for_dl.len(), posts.len().saturating_sub(for_dl.len()));
        let a = download_all(&http, &for_dl, &att_dir).await;
        save_attachments(&db, &a)?;
        a
    };
    let name = clean(args.name.clone().unwrap_or_else(|| nicks_name(&ok_nicks, &stamp)));
    let label = ok_nicks.iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join(", ");
    // A sync only updates the database, unless an output was asked for.
    if args.sync && args.output.is_none() && args.compression().is_none() {
        if let Err(e) = crate::rules::on_sync(&db, &posts) {
            err!("warning: rules: {e:#}");
        }
        info!("sync: {} nicks up to date", ok_nicks.len());
        return Ok(());
    }
    export(&args, &name, &label, &posts, &atts, &profiles, &mention_hits, &home()?.join(EXPORTS_REL_PATH))?;

    Ok(())
}
