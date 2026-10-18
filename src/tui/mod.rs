//! Terminal UI (2.0): the default when no flags are given (or with `--tui`).
//!
//! Browse every nick and post in the local database offline, open post details
//! that cross-load linked posts, search in place, and drive scrape / export /
//! import / maintenance from a menu bar built from the CLI flags. Anything that
//! writes runs the headless CLI as a child process ([`jobs`]), so it can be
//! cancelled, and keeps running in the background when you quit.
//!
//! Layout: `mod.rs` (event loop), [`state`] (pure state + transitions),
//! [`input`] (keys/mouse), [`layout`] (geometry), [`widgets`] (drawing),
//! [`modals`], [`commands`] (menu ↔ flags), [`data`] (read-only DB), [`jobs`],
//! [`theme`] (styles), [`theming`] (theme files in the app), [`editor`] (Shift-F5).

pub mod commands;
pub mod data;
pub mod editor;
pub mod empty_anim;
pub mod images;
pub mod input;
pub mod jobs;
pub mod layout;
pub mod markdown;
pub mod modals;
pub mod sample;
pub mod state;
pub mod theme;
pub mod theming;
pub mod widgets;

use crate::prelude::*;
use state::{App, Effect, JobView};
use std::time::{Duration, Instant};

/// Messages from background tasks to the event loop.
enum Msg {
    SessionChecked(std::result::Result<bool, String>, jobs::JobKind, Vec<String>),
    LoginNeedCode(u64),
    LoginDone(u64, std::result::Result<(), String>),
    AutoSyncChecked(std::result::Result<bool, String>, Vec<String>),
    ThemeDownloaded(String, bool, std::result::Result<crate::themes::download::Fetched, String>),
}

struct Terminal(ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>);

fn restore_terminal() {
    use crossterm::{event::DisableMouseCapture, execute, terminal::{LeaveAlternateScreen, disable_raw_mode}};
    let _ = disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture, crossterm::cursor::Show);
}

impl Drop for Terminal {
    fn drop(&mut self) { restore_terminal(); }
}

/// Open the terminal UI. Needs a terminal on stdin and stdout.
pub async fn run(args: Args) -> Result<()> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("the terminal UI needs a terminal; for headless use pass flags, e.g. --scrape, --nick NICK, --search TEXT, --export FILE (see --help)");
    }
    let home_dir = home()?;
    let db = home_dir.join(DB_REL_PATH);
    let prefs = load_prefs();
    let mut app = App::new(Box::new(data::SqliteStore::new(db)), &args, theme::Theme::reset(),
        session_path().map(|p| p.exists()).unwrap_or(false));
    setup_themes(&mut app, &args, &prefs)?;
    app.email = prefs.email.filter(|e| !e.is_empty());
    if let Some(s) = &prefs.sync { app.menu.menu.apply_sync_prefs(s); }
    (app.empty_anim, app.empty_anim_accent) = empty_anim::from_prefs(prefs.ui.as_ref());
    if let Some(msg) = crate::crypto::cmd::shares_reminder(&home_dir) {
        app.push_status(msg);
    }

    let (log_tx, log_rx) = std::sync::mpsc::channel::<String>();
    set_log_sink(Some(log_tx));
    let r = event_loop(&mut app, log_rx).await;
    set_log_sink(None);
    let detached = r?;
    if let Some((kind, pid, log)) = detached {
        eprintln!("The {kind} keeps running in the background (pid {pid}). Follow it with:\n  tail -f {}", log.display());
    }
    Ok(())
}

/// Theme files at startup: convert an old `[theme]` table once, select `--theme`,
/// `[ui] theme` or the default, and on a first run (empty `~/.acp/themes`) start the
/// background download of the default theme (unless it is turned off).
fn setup_themes(app: &mut App, args: &Args, prefs: &Prefs) -> Result<()> {
    let dir = crate::themes::themes_dir()?;
    let first = !crate::themes::has_themes(&dir);
    let (migrated, notes) = crate::themes::prepare(&dir, prefs);
    for n in &notes { app.push_status(n.clone()); }
    if let Some(m) = &migrated {
        if let Err(e) = save_ui_theme(m) { app.push_status(format!("warning: could not select {m} in preferences.toml: {e:#}")); }
        if let Some(n) = notes.iter().find(|n| n.starts_with("Your [theme]")) {
            app.modal = Some(modals::Modal::Message(modals::MessageModal::info("Theme converted", n)));
        }
    }
    app.themes_dir = Some(dir.clone());
    app.theme_mode = crate::themes::Mode::parse(prefs.ui.as_ref().and_then(|u| u.theme_mode.as_deref()));
    app.dark_bg = crate::themes::colorfgbg_dark(std::env::var("COLORFGBG").ok().as_deref());
    let name = args.theme.clone().or(migrated).or_else(|| prefs.ui.as_ref().and_then(|u| u.theme.clone())).unwrap_or_else(|| DEFAULT_THEME.into());
    let download = first && !dir.join(format!("{DEFAULT_THEME}.toml")).exists();
    let enabled = crate::themes::download::auto_enabled_now(prefs);
    let before = app.modal.take();
    let ok = app.use_theme(&name, false);
    if download && enabled {
        // Not an error yet: the default is on its way (drawn with the terminal's colours until then).
        if !ok { app.modal = before; }
        app.start_theme_download(DEFAULT_THEME, false);
    } else {
        if ok { app.modal = before; }
        if download { app.push_status(format!("warning: {}", crate::themes::download::off_notice(&dir))); }
    }
    Ok(())
}

async fn event_loop(app: &mut App, log_rx: std::sync::mpsc::Receiver<String>) -> Result<Option<(&'static str, u32, PathBuf)>> {
    use crossterm::{event::EnableMouseCapture, execute, terminal::{EnterAlternateScreen, enable_raw_mode}};
    let jar = Arc::new(Jar::default());
    if let Err(e) = load_session(&jar) { app.push_status(format!("warning: ignoring saved session: {e:#}")); }
    let http = client(jar.clone())?;

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| { restore_terminal(); prev(info); }));
    // `theme_mode = "system"` without COLORFGBG: ask the terminal for its background.
    if app.theme_mode == crate::themes::Mode::System && app.dark_bg.is_none() {
        app.dark_bg = images::query_background();
        if app.dark_bg == Some(false) { app.refresh_theme(); }
    }
    // Ask the terminal for its graphics protocol before the input thread starts reading.
    app.images = images::Images::detect(theme::rgb_bytes(app.theme.on_accent), theme::rgb_bytes(app.theme.accent));
    let osc8 = osc8_supported();
    let mut term = Terminal(ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))?);

    // Input thread → channel.
    let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop2 = stop.clone();
    std::thread::spawn(move || {
        while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
            if crossterm::event::poll(Duration::from_millis(100)).unwrap_or(false) {
                match crossterm::event::read() { Ok(e) => { if ev_tx.send(e).is_err() { break; } } Err(_) => break }
            }
        }
    });
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();
    let mut job: Option<jobs::Job> = None;
    // Small writes (bookmarks) that run next to the main job.
    let mut side: Vec<jobs::Job> = Vec::new();
    let mut code_tx: Option<tokio::sync::oneshot::Sender<String>> = None;
    let mut login_task: Option<std::thread::JoinHandle<()>> = None;
    let mut detached = None;
    // Generation of the current login attempt; replies from abandoned ones are ignored.
    let mut login_gen: u64 = 0;

    'outer: loop {
        let now = Instant::now();
        app.tick(now);
        app.sync_tick(now);
        if let Some(im) = app.images.as_mut() { im.poll(); }
        term.0.draw(|f| widgets::draw(f, app, now))?;
        if osc8 { emit_osc8(&app.osc_links, &app.theme); }
        if let Some(im) = app.images.as_mut()
            && im.flush(&mut std::io::stdout()) == images::Flush::Repaint {
                // Sixel / iTerm2 pictures are burnt into the cells: repaint everything, then place them again.
                term.0.clear()?;
                term.0.draw(|f| widgets::draw(f, app, now))?;
                if osc8 { emit_osc8(&app.osc_links, &app.theme); }
                if let Some(im) = app.images.as_mut() { im.flush(&mut std::io::stdout()); }
            }
        let mut wait = Duration::from_millis(if app.anim.is_some() { TUI_FRAME_MS } else { TUI_TICK_MS });
        // The empty-Body animation: wake exactly when its next frame is due (one draw per frame).
        if app.empty_anim_frame.is_some() && let Some(fr) = &app.empty_anim {
            wait = wait.min(fr.until_next(Instant::now().saturating_duration_since(app.empty_anim_start)));
        }
        tokio::select! {
            Some(ev) = ev_rx.recv() => {
                if matches!(ev, crossterm::event::Event::Resize(..)) && let Some(im) = app.images.as_mut() { im.invalidate(); }
                input::on_event(app, ev);
                while let Ok(ev) = ev_rx.try_recv() { input::on_event(app, ev); }
            }
            Some(m) = msg_rx.recv() => match m {
                Msg::SessionChecked(r, k, argv) => app.session_checked(r, k, argv),
                Msg::LoginNeedCode(g) if g == login_gen => app.login_need_code(),
                Msg::LoginDone(g, r) if g == login_gen => { code_tx = None; login_task = None; app.login_done(r); }
                Msg::AutoSyncChecked(r, argv) => app.auto_sync_checked(r, argv),
                Msg::ThemeDownloaded(name, manual, r) => app.theme_downloaded(&name, manual, r),
                Msg::LoginNeedCode(_) | Msg::LoginDone(..) => {}
            },
            _ = tokio::time::sleep(wait) => {}
        }
        while let Ok(l) = log_rx.try_recv() { app.push_status(l); }
        if let Some(j) = &mut job {
            let (lines, status) = j.poll();
            for l in lines { app.push_status(l); }
            if let Some(st) = status {
                let tail = jobs::tail(&j.log, 12);
                app.job_finished(st.success(), st.code(), &tail);
                job = None;
            }
        }
        let mut k = 0;
        while k < side.len() {
            let (_, status) = side[k].poll();
            if let Some(st) = status {
                let j = side.remove(k);
                app.side_job_finished(st.success(), &jobs::tail(&j.log, 3), side.len());
                if st.success() { let _ = std::fs::remove_file(&j.log); }
            } else { k += 1; }
        }
        for eff in std::mem::take(&mut app.effects) {
            match eff {
                Effect::CheckSessionThen(kind, argv) => {
                    let (http, tx) = (http.clone(), msg_tx.clone());
                    app.push_status("checking the saved session…");
                    tokio::spawn(async move {
                        let r = session_valid(&http).await.map_err(|e| format!("{e:#}"));
                        let _ = tx.send(Msg::SessionChecked(r, kind, argv));
                    });
                }
                Effect::RunJob(kind, argv) => {
                    if job.is_some() { app.error("Busy", "another job is still running"); continue; }
                    match jobs::Job::spawn(kind, &argv) {
                        Ok(j) => { app.job_started(JobView { kind, pid: j.pid(), log: j.log.clone() }, &j.cmd); job = Some(j); }
                        Err(e) => app.error("Cannot start", &format!("{e:#}")),
                    }
                }
                Effect::AutoSync(argv) => {
                    let (http, tx) = (http.clone(), msg_tx.clone());
                    tokio::spawn(async move {
                        let r = session_valid(&http).await.map_err(|e| format!("{e:#}"));
                        let _ = tx.send(Msg::AutoSyncChecked(r, argv));
                    });
                }
                Effect::SaveSyncPrefs(p) => match save_sync_prefs(&p) {
                    Ok(()) => app.push_status("sync defaults saved to ~/.acp/preferences.toml"),
                    Err(e) => app.error("Cannot save", &format!("{e:#}")),
                },
                Effect::SaveUiTheme(name) => match save_ui_theme(&name) {
                    Ok(()) => app.push_status(format!("theme {name} selected (saved to ~/.acp/preferences.toml)")),
                    Err(e) => app.error("Cannot save the theme choice", &format!("{e:#}")),
                },
                Effect::ExportTheme(f) => match export_theme(&f) {
                    Ok(p) => app.modal = Some(modals::Modal::Message(modals::MessageModal::info("Theme exported", &format!("Wrote {}", p.display())))),
                    Err(e) => app.error("Cannot export the theme", &format!("{e:#}")),
                },
                Effect::DownloadTheme(name, manual) => {
                    let Some(dir) = app.themes_dir.clone() else { continue };
                    let tx = msg_tx.clone();
                    tokio::spawn(async move {
                        let r = crate::themes::download::download(&dir, &name).await.map_err(|e| format!("{e:#}"));
                        let _ = tx.send(Msg::ThemeDownloaded(name, manual, r));
                    });
                }
                Effect::RunSideJob(kind, argv) => match jobs::Job::spawn(kind, &argv) {
                    Ok(j) => side.push(j),
                    Err(e) => app.error("Cannot start", &format!("{e:#}")),
                },
                Effect::OpenFile(p) => if let Err(e) = open_file(&p) { app.error("Cannot open", &format!("{e:#}")); },
                Effect::CancelJob => if let Some(mut j) = job.take() {
                    if let Err(e) = j.cancel() { app.push_status(format!("error: cancel: {e:#}")); }
                    app.job_cancelled();
                },
                Effect::LoginEmail(email) => {
                    // The LiveView login future is not Send: run it on its own
                    // single-threaded runtime. Dropping `code_tx` ends it.
                    let (ctx, crx) = tokio::sync::oneshot::channel::<String>();
                    code_tx = Some(ctx);
                    login_gen += 1;
                    let g = login_gen;
                    let (http, jar, tx) = (http.clone(), jar.clone(), msg_tx.clone());
                    login_task = Some(std::thread::spawn(move || {
                        let tx2 = tx.clone();
                        let r = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                            Ok(rt) => rt.block_on(login_flow(&http, &jar, Some(email), move || async move {
                                let _ = tx2.send(Msg::LoginNeedCode(g));
                                crx.await.map_err(|_| anyhow!("login cancelled"))
                            })),
                            Err(e) => Err(e.into()),
                        };
                        let _ = tx.send(Msg::LoginDone(g, r.map_err(|e| format!("{e:#}"))));
                    }));
                }
                Effect::LoginCode(c) => match code_tx.take() {
                    Some(tx) => { let _ = tx.send(c); }
                    None => app.login_done(Err("the login expired; send a new code".into())),
                },
                Effect::LoginCancel => { code_tx = None; login_task = None; login_gen += 1; }
                Effect::Logout => match logout(&jar) {
                    Ok(removed) => app.logged_out(removed),
                    Err(e) => app.error("Logout failed", &format!("{e:#}")),
                },
                Effect::Quit { cancel_job } => {
                    if let Some(mut j) = job.take() {
                        if cancel_job { let _ = j.cancel(); } else { detached = Some((j.kind.name(), j.pid(), j.log.clone())); }
                    }
                    break 'outer;
                }
            }
        }
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Some(im) = app.images.as_ref() { im.clear_screen_images(&mut std::io::stdout()); }
    drop((code_tx, login_task));
    drop(term);
    Ok(detached)
}

/// Write a copy of theme `f` to `~/.acp/ai/data/exports/themes/<name>-<stamp>.toml`.
pub fn export_theme(f: &crate::themes::ThemeFile) -> Result<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let p = home()?.join(EXPORTS_REL_PATH).join("themes").join(format!("{}-{stamp}.toml", f.name));
    crate::themes::atomic_write(&p, crate::themes::to_toml(f).as_bytes()).with_context(|| format!("writing {}", p.display()))?;
    Ok(p)
}

/// Whether to re-emit attachment links as OSC 8 hyperlinks (off for the Linux console,
/// `TERM=dumb`, or with `ACP_NO_OSC8` set).
pub fn osc8_supported() -> bool {
    if std::env::var_os("ACP_NO_OSC8").is_some() { return false; }
    !matches!(std::env::var("TERM").as_deref(), Ok("linux") | Ok("dumb"))
}

/// The OSC 8 sequence for `text` linking to `url` (no control characters pass through).
pub fn osc8(text: &str, url: &str) -> String {
    let clean = |s: &str| s.chars().filter(|c| !c.is_control()).collect::<String>();
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", clean(url), clean(text))
}

/// Re-print link text as OSC 8 hyperlinks over the drawn frame, in the theme's link colour
/// on its background so the cells look the same.
fn emit_osc8(links: &[(u16, u16, String, String)], t: &theme::Theme) {
    use crossterm::{cursor::MoveTo, queue, style::{Attribute, Color as CColor, Print, SetAttribute, SetBackgroundColor, SetForegroundColor}};
    if links.is_empty() { return; }
    let mut out = std::io::stdout();
    let conv = |c: ratatui::style::Color| crate::themes::rgb(c).map(CColor::from);
    let fg = ratatui::style::Style::from(&t.link()).fg.and_then(conv);
    let bg = conv(t.bg);
    for (x, y, text, url) in links {
        let _ = queue!(out, MoveTo(*x, *y), SetAttribute(Attribute::Underlined));
        if let Some(c) = fg { let _ = queue!(out, SetForegroundColor(c)); }
        if let Some(c) = bg { let _ = queue!(out, SetBackgroundColor(c)); }
        let _ = queue!(out, Print(osc8(text, url)), SetAttribute(Attribute::Reset));
    }
    let _ = out.flush();
}

/// Open `p` with the desktop's default application, detached (`xdg-open`; `open` on macOS;
/// `ACP_OPENER` overrides).
pub fn open_file(p: &Path) -> Result<()> {
    let opener = std::env::var("ACP_OPENER").ok().filter(|s| !s.is_empty())
        .unwrap_or_else(|| if cfg!(target_os = "macos") { "open".into() } else { "xdg-open".into() });
    let mut c = std::process::Command::new(&opener);
    c.arg(p).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)]
    { use std::os::unix::process::CommandExt; c.process_group(0); }
    c.spawn().with_context(|| format!("starting {opener} {}", p.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests;
