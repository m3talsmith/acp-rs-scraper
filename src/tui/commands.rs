//! The menu bar's command catalog and the menu → command-line bridge.
//!
//! Every menu command maps to CLI flags (see SPECS "Menu bar ↔ CLI flag map").
//! Actions that change data (scrape, export, import, maintenance) are turned into
//! the exact argv the headless CLI takes, validated by clap ([`parse_with_mode`]),
//! and run as a child process, so the TUI and the CLI share one code path.

use crate::prelude::*;
use crate::tui::data::SearchFilter;
use crate::tui::state::ViewMode;
use crate::tui::widgets::text::TextField;

/// Program name used as `argv[0]` for validation.
pub const ARGV0: &str = "acp-rs-scraper";

/// Menu bar items, left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandId {
    /// Scrape options (Start Scanning runs it).
    Scrape,
    /// Offline search into the Body.
    Search,
    /// Dump the DB / selection to a file.
    Export,
    /// Import an export into the DB.
    Import,
    /// Refresh the nick bar.
    List,
    /// Remove nicks / clear everything.
    Maintain,
    /// Login / logout.
    Session,
    /// Bookmarks: show, toggle, sync.
    Bookmarks,
    /// Periodic sync with the site.
    Sync,
    /// Body layout.
    View,
    /// Log level for jobs started from the TUI.
    Debug,
    /// Usage modal.
    Help,
    /// Leave the TUI.
    Quit,
}

/// Something a button, key or palette entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Next view mode (`v`).
    CycleView,
    /// Previous post of the detail stack.
    StackPrev,
    /// Next post of the detail stack.
    StackNext,
    /// Switch to this view mode.
    SetView(crate::tui::state::ViewMode),
    /// Start Scanning (or Cancel while a job runs).
    StartScan,
    /// Kill the running job.
    CancelJob,
    /// Search with the Search menu's fields.
    RunSearch,
    /// Open the Shift-S search modal.
    OpenSearchModal,
    /// Leave the search view.
    ClearSearch,
    /// Export with the Export menu's fields.
    RunExport,
    /// Import with the Import menu's fields.
    RunImport,
    /// Re-read the nick bar from the DB.
    RefreshNicks,
    /// `--remove-nick` (after a confirm modal).
    RemoveNicks,
    /// `--clear-all` (after a confirm modal).
    ClearAll,
    /// Open the Login modal.
    Login,
    /// Forget the saved session, then route to Login.
    Logout,
    /// Help modal.
    Help,
    /// Toggle the log panel.
    ToggleLog,
    /// Quit (detaching a running job).
    Quit,
    /// Show the bookmarks view.
    ShowBookmarks,
    /// Bookmark / unbookmark the current post.
    ToggleBookmark,
    /// `--sync-bookmarks` with the site.
    SyncBookmarks,
    /// Sync › Sync now.
    SyncNow,
    /// Sync › Save as default.
    SaveSyncPrefs,
    /// Add / remove the current post in the export basket (`x`).
    ToggleBasket,
    /// Show the export basket.
    ShowBasket,
    /// Empty the export basket.
    ClearBasket,
    /// Export the basket with the Export menu's Path / Format / Name / Compress.
    ExportBasket,
}

/// The input type shown right of an option's label.
#[derive(Clone, Debug, PartialEq)]
pub enum Widget {
    /// Boolean flag.
    Check(bool),
    /// Known values.
    Choice {
        /// Values.
        choices: &'static [&'static str],
        /// Selected index.
        idx: usize,
    },
    /// Open-ended value.
    Text(TextField),
    /// Direct action.
    Button(Action),
}

/// One option under a menu command.
#[derive(Clone, Debug, PartialEq)]
pub struct Opt {
    /// Stable key used by the argv builders.
    pub key: &'static str,
    /// Label.
    pub label: &'static str,
    /// CLI flag(s) it maps to (shown as a hint).
    pub flag: &'static str,
    /// Marked `*`: must be set for the command's action.
    pub required: bool,
    /// Input type.
    pub widget: Widget,
}

/// One menu bar item.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuCommand {
    /// Which command.
    pub id: CommandId,
    /// Label in the bar.
    pub label: &'static str,
    /// Hotkey letter (underlined; Alt+letter or the letter while the menu is focused).
    pub hint: char,
    /// Options in its dropdown (empty: activating runs `action`).
    pub opts: Vec<Opt>,
    /// Action of an option-less item.
    pub action: Option<Action>,
}

fn opt(key: &'static str, label: &'static str, flag: &'static str, required: bool, widget: Widget) -> Opt {
    Opt { key, label, flag, required, widget }
}
fn check(v: bool) -> Widget { Widget::Check(v) }
fn text(v: &str) -> Widget { Widget::Text(TextField::new(v)) }
fn choice(choices: &'static [&'static str]) -> Widget { Widget::Choice { choices, idx: 0 } }
fn button(a: Action) -> Widget { Widget::Button(a) }

/// Scrape feed choices.
pub const MODE_CHOICES: &[&str] = &["Latest", "Trending"];
/// Export format choices.
pub const FORMAT_CHOICES: &[&str] = &["json", "html", "json,html"];
/// Export scope choices.
pub const SCOPE_CHOICES: &[&str] = &["Selected nicks", "Current view", "Everything", "Basket"];

/// The whole menu bar, with every option's current value.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    /// Items left to right.
    pub items: Vec<MenuCommand>,
}

impl Menu {
    /// Default catalog. `args` seeds the Debug options (from `-v`/`--debug`/`--enable-logging`).
    pub fn new(args: &Args) -> Self {
        use CommandId::*;
        let items = vec![
            MenuCommand { id: Scrape, label: "Scrape", hint: 's', action: None, opts: vec![
                opt("mode", "Mode", "--latest / --trending", false, choice(MODE_CHOICES)),
                opt("nicks", "Nicks", "--nicks (blank: selected nicks)", true, text("")),
                opt("mentions", "Mentions", "--no-mentions when off", false, check(true)),
                opt("said_by", "Said by", "--said-by", false, check(false)),
                opt("crawl_limit", "Crawl limit", "--crawl-limit", false, text(&DEFAULT_CRAWL_LIMIT.to_string())),
                opt("attachments", "Attachments", "--no-attachments when off", false, check(true)),
                opt("full", "Full", "--full", false, check(false)),
                opt("refetch", "Refetch truncated", "--refetch-truncated", false, check(false)),
                opt("output", "Output file", "--output (blank: database only)", false, text("")),
                opt("format", "Format", "--format", false, choice(FORMAT_CHOICES)),
                opt("start", "Start Scanning", "", false, button(Action::StartScan)),
            ]},
            MenuCommand { id: Search, label: "Search", hint: 'e', action: None, opts: vec![
                opt("query", "Query", "--search", true, text("")),
                opt("nick", "Nick filter", "--nicks", false, text("")),
                opt("run", "Search", "", false, button(Action::RunSearch)),
                opt("advanced", "Advanced…", "Shift-S", false, button(Action::OpenSearchModal)),
                opt("clear", "Clear search", "Ctrl-S", false, button(Action::ClearSearch)),
            ]},
            MenuCommand { id: Export, label: "Export", hint: 'x', action: None, opts: vec![
                opt("path", "Path", "--export", true, text("")),
                opt("scope", "From", "--nicks / --search", false, choice(SCOPE_CHOICES)),
                opt("format", "Format", "--format", false, choice(FORMAT_CHOICES)),
                opt("name", "Name", "--name", false, text("")),
                opt("compress", "Compress tar.gz", "--compress --tar-gz", false, check(false)),
                opt("run", "Export", "", false, button(Action::RunExport)),
                opt("basket_add", "Add current post to basket", "x", false, button(Action::ToggleBasket)),
                opt("basket_show", "Show basket", "", false, button(Action::ShowBasket)),
                opt("basket_run", "Export basket", "--post-ids", false, button(Action::ExportBasket)),
                opt("basket_clear", "Clear basket", "", false, button(Action::ClearBasket)),
            ]},
            MenuCommand { id: Import, label: "Import", hint: 'i', action: None, opts: vec![
                opt("path", "Path", "--import", true, text("")),
                opt("compress", "Compressed tar.gz", "--compress --tar-gz", false, check(false)),
                opt("run", "Import", "", false, button(Action::RunImport)),
            ]},
            MenuCommand { id: List, label: "List", hint: 'l', action: None, opts: vec![
                opt("refresh", "Refresh nicks", "--list-nicks", false, button(Action::RefreshNicks)),
            ]},
            MenuCommand { id: Maintain, label: "Maintain", hint: 'a', action: None, opts: vec![
                opt("nicks", "Remove nicks", "--remove-nick (blank: selected nicks)", true, text("")),
                opt("keep_replies", "Keep replies", "--keep-replies", false, check(false)),
                opt("dry_run", "Dry run", "--dry-run", false, check(false)),
                opt("yes", "Skip confirmation", "--yes (dangerous)", false, check(false)),
                opt("remove", "Remove", "", false, button(Action::RemoveNicks)),
                opt("clear", "Clear all", "--clear-all", false, button(Action::ClearAll)),
            ]},
            MenuCommand { id: Session, label: "Session", hint: 'n', action: None, opts: vec![
                opt("login", "Log in", "", false, button(Action::Login)),
                opt("logout", "Log out", "--logout", false, button(Action::Logout)),
            ]},
            MenuCommand { id: Bookmarks, label: "Bookmarks", hint: 'b', action: None, opts: vec![
                opt("show", "Show bookmarks", "B · --list-bookmarks", false, button(Action::ShowBookmarks)),
                opt("toggle", "Bookmark current post", "b · --bookmark / --unbookmark", false, button(Action::ToggleBookmark)),
                opt("sync", "Sync with acp.us", "--sync-bookmarks", false, button(Action::SyncBookmarks)),
            ]},
            MenuCommand { id: Sync, label: "Sync", hint: 'y', action: None, opts: vec![
                opt("auto", "Auto sync", "while the TUI is open", false, check(false)),
                opt("interval", "Interval (minutes)", "", false, text(&TUI_SYNC_DEFAULT_MINUTES.to_string())),
                opt("nicks", "Nicks", "--sync --nicks (blank: selected nicks)", false, text("")),
                opt("mentions", "Mentions", "--no-mentions when off", false, check(false)),
                opt("bookmarks", "Bookmarks", "--no-bookmarks when off", false, check(true)),
                opt("now", "Sync now", "--sync", false, button(Action::SyncNow)),
                opt("save", "Save as default", "~/.acp/preferences.toml [sync]", false, button(Action::SaveSyncPrefs)),
            ]},
            MenuCommand { id: View, label: "View", hint: 'v', action: None, opts: vec![
                opt("nested", "Nested", "v cycles", false, button(Action::SetView(ViewMode::Nested))),
                opt("stack", "Stack", "", false, button(Action::SetView(ViewMode::Stack))),
                opt("pane", "Stack + content pane", "", false, button(Action::SetView(ViewMode::Pane))),
                opt("carousel_inf", "Carousel (infinite)", "", false, button(Action::SetView(ViewMode::CarouselInfinite))),
                opt("carousel", "Carousel (fixed)", "", false, button(Action::SetView(ViewMode::CarouselFixed))),
                opt("grid", "Grid", "", false, button(Action::SetView(ViewMode::Grid))),
                opt("prev", "◂ Previous post", ", / Alt-←", false, button(Action::StackPrev)),
                opt("next", "Next post ▸", ". / Alt-→", false, button(Action::StackNext)),
            ]},
            MenuCommand { id: Debug, label: "Debug", hint: 'd', action: None, opts: vec![
                opt("verbose", "Verbose", "-v", false, check(args.verbose)),
                opt("debug", "Debug wire log", "--debug", false, check(args.debug)),
                opt("logging", "Log to files", "--enable-logging", false, check(args.enable_logging)),
                opt("show_log", "Show log panel", "l", false, button(Action::ToggleLog)),
            ]},
            MenuCommand { id: Help, label: "Help", hint: 'h', opts: vec![], action: Some(Action::Help) },
            MenuCommand { id: Quit, label: "Quit", hint: 'q', opts: vec![], action: Some(Action::Quit) },
        ];
        Menu { items }
    }
    /// Item by id.
    pub fn get(&self, id: CommandId) -> &MenuCommand { self.items.iter().find(|c| c.id == id).expect("menu id") }
    /// Option by command and key.
    pub fn opt_mut(&mut self, id: CommandId, key: &str) -> &mut Opt {
        self.items.iter_mut().find(|c| c.id == id).expect("menu id").opts.iter_mut().find(|o| o.key == key).expect("opt key")
    }
    fn opt(&self, id: CommandId, key: &str) -> &Opt { self.get(id).opts.iter().find(|o| o.key == key).expect("opt key") }
    /// Checkbox value.
    pub fn check(&self, id: CommandId, key: &str) -> bool { matches!(self.opt(id, key).widget, Widget::Check(true)) }
    /// Text value, trimmed.
    pub fn text(&self, id: CommandId, key: &str) -> String {
        match &self.opt(id, key).widget { Widget::Text(t) => t.value.trim().to_string(), _ => String::new() }
    }
    /// Selected choice.
    pub fn choice(&self, id: CommandId, key: &str) -> &'static str {
        match &self.opt(id, key).widget { Widget::Choice { choices, idx } => choices[*idx], _ => "" }
    }
    /// Set a checkbox.
    pub fn set_check(&mut self, id: CommandId, key: &str, v: bool) { self.opt_mut(id, key).widget = Widget::Check(v); }
    /// Set a text field.
    pub fn set_text(&mut self, id: CommandId, key: &str, v: &str) {
        if let Widget::Text(t) = &mut self.opt_mut(id, key).widget { t.set(v); }
    }
    /// Select a choice by value (ignored when unknown).
    pub fn set_choice(&mut self, id: CommandId, key: &str, v: &str) {
        if let Widget::Choice { choices, idx } = &mut self.opt_mut(id, key).widget
            && let Some(i) = choices.iter().position(|c| *c == v) { *idx = i; }
    }

    /// `-v` / `--debug` / `--enable-logging` from the Debug menu, for every job.
    pub fn debug_args(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.check(CommandId::Debug, "debug") { v.push("--debug".into()); }
        else if self.check(CommandId::Debug, "verbose") { v.push("-v".into()); }
        if self.check(CommandId::Debug, "logging") { v.push("--enable-logging".into()); }
        v
    }

    /// Nicks to scrape: the Nicks field, else the selected nicks.
    pub fn scrape_nicks(&self, selected: &[String]) -> Vec<String> {
        let t = self.text(CommandId::Scrape, "nicks");
        if t.is_empty() { selected.to_vec() } else { parse_nicks(&t).map(|n| n.0).unwrap_or_default() }
    }

    /// argv for Start Scanning (headless scrape of the chosen nicks).
    pub fn scrape_argv(&self, selected: &[String]) -> std::result::Result<Vec<String>, String> {
        use CommandId::Scrape as S;
        let t = self.text(S, "nicks");
        let nicks = if t.is_empty() { selected.to_vec() } else { parse_nicks(&t)?.0 };
        if nicks.is_empty() { return Err("Scrape needs at least one nick: type them in Scrape › Nicks or select nicks on the left".into()); }
        let mut v = vec![ARGV0.to_string(), "--nicks".into(), nicks.join(",")];
        if self.choice(S, "mode") == "Trending" { v.push("--trending".into()); } else { v.push("--latest".into()); }
        if !self.check(S, "mentions") { v.push("--no-mentions".into()); }
        if self.check(S, "said_by") { v.push("--said-by".into()); }
        let cl = self.text(S, "crawl_limit");
        if !cl.is_empty() {
            let n: usize = cl.parse().map_err(|_| format!("Crawl limit must be a number, not {cl:?}"))?;
            if n != DEFAULT_CRAWL_LIMIT { v.extend(["--crawl-limit".into(), n.to_string()]); }
        }
        if !self.check(S, "attachments") { v.push("--no-attachments".into()); }
        if self.check(S, "full") { v.push("--full".into()); }
        if self.check(S, "refetch") { v.push("--refetch-truncated".into()); }
        let out = self.text(S, "output");
        if !out.is_empty() { v.extend(["--format".into(), self.choice(S, "format").into(), "--output".into(), out]); }
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for Export. `selected` = selected nicks; `view` = the current search, if any.
    pub fn export_argv(&self, selected: &[String], view: Option<&SearchFilter>) -> std::result::Result<Vec<String>, String> {
        self.export_argv_with(selected, view, &[], None)
    }

    /// argv for Export with the export `basket`; `scope` overrides Export › From.
    pub fn export_argv_with(&self, selected: &[String], view: Option<&SearchFilter>, basket: &[String], scope: Option<&str>)
        -> std::result::Result<Vec<String>, String> {
        use CommandId::Export as E;
        let path = self.text(E, "path");
        if path.is_empty() { return Err("Export needs a Path".into()); }
        let mut v = vec![ARGV0.to_string(), "--export".into(), path];
        let nicks_arg = |v: &mut Vec<String>, n: &[String]| if !n.is_empty() { v.extend(["--nicks".into(), n.join(",")]); };
        match scope.unwrap_or(self.choice(E, "scope")) {
            "Basket" => {
                if basket.is_empty() { return Err("the export basket is empty: press x on a post (or its + button) to add it".into()); }
                v.extend(["--post-ids".into(), basket.join(",")]);
            }
            "Selected nicks" => {
                if selected.is_empty() { return Err("Export › From is \"Selected nicks\" but no nick is selected".into()); }
                nicks_arg(&mut v, selected);
            }
            "Current view" => match view {
                Some(f) => {
                    if !f.keyword.trim().is_empty() { v.extend(["--search".into(), f.keyword.trim().into()]); }
                    nicks_arg(&mut v, &f.nicks);
                }
                None => nicks_arg(&mut v, selected),
            },
            _ => {}
        }
        v.extend(["--format".into(), self.choice(E, "format").into()]);
        let name = self.text(E, "name");
        if !name.is_empty() { v.extend(["--name".into(), name]); }
        if self.check(E, "compress") { v.extend(["--compress".into(), "--tar-gz".into()]); }
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for Import.
    pub fn import_argv(&self) -> std::result::Result<Vec<String>, String> {
        use CommandId::Import as I;
        let path = self.text(I, "path");
        if path.is_empty() { return Err("Import needs a Path".into()); }
        let mut v = vec![ARGV0.to_string(), "--import".into(), path];
        if self.check(I, "compress") { v.extend(["--compress".into(), "--tar-gz".into()]); }
        v.extend(self.debug_args());
        validate(v)
    }

    /// Nicks Maintain › Remove would delete: the field, else the selection.
    pub fn remove_nicks(&self, selected: &[String]) -> std::result::Result<Vec<String>, String> {
        let t = self.text(CommandId::Maintain, "nicks");
        let n = if t.is_empty() { selected.to_vec() } else { parse_nicks(&t)?.0 };
        if n.is_empty() { return Err("Remove needs nicks: type them in Maintain › Remove nicks or select nicks on the left".into()); }
        Ok(n)
    }

    /// argv for `--remove-nick` (always `--yes`: the TUI confirms first, or Skip confirmation was ticked).
    pub fn remove_argv(&self, selected: &[String]) -> std::result::Result<Vec<String>, String> {
        use CommandId::Maintain as M;
        let n = self.remove_nicks(selected)?;
        let mut v = vec![ARGV0.to_string(), "--remove-nick".into(), n.join(","), "--yes".into()];
        if self.check(M, "keep_replies") { v.push("--keep-replies".into()); }
        if self.check(M, "dry_run") { v.push("--dry-run".into()); }
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for `--clear-all` (always `--yes`, see [`Menu::remove_argv`]).
    pub fn clear_all_argv(&self) -> std::result::Result<Vec<String>, String> {
        let mut v = vec![ARGV0.to_string(), "--clear-all".into(), "--yes".into()];
        if self.check(CommandId::Maintain, "dry_run") { v.push("--dry-run".into()); }
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for one local bookmark change.
    pub fn bookmark_argv(&self, id: &str, on: bool) -> std::result::Result<Vec<String>, String> {
        validate(vec![ARGV0.to_string(), if on { "--bookmark" } else { "--unbookmark" }.into(), id.to_string()])
    }

    /// argv for Bookmarks › Sync.
    pub fn sync_bookmarks_argv(&self) -> std::result::Result<Vec<String>, String> {
        let mut v = vec![ARGV0.to_string(), "--sync-bookmarks".into()];
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for a sync (Sync now / auto sync): `--sync` with the nicks (field, else `selected`).
    pub fn sync_argv(&self, selected: &[String]) -> std::result::Result<Vec<String>, String> {
        use CommandId::Sync as Y;
        let t = self.text(Y, "nicks");
        let nicks = if t.is_empty() { selected.to_vec() } else { parse_nicks(&t)?.0 };
        let bm = self.check(Y, "bookmarks");
        if nicks.is_empty() && !bm { return Err("nothing to sync: tick Sync › Bookmarks, or type nicks in Sync › Nicks / select nicks".into()); }
        let mut v = vec![ARGV0.to_string(), "--sync".into()];
        if !nicks.is_empty() { v.extend(["--nicks".into(), nicks.join(",")]); if !self.check(Y, "mentions") { v.push("--no-mentions".into()); } }
        if !bm { v.push("--no-bookmarks".into()); }
        v.extend(self.debug_args());
        validate(v)
    }
    /// Sync › Interval as a duration (at least a minute).
    pub fn sync_interval(&self) -> std::result::Result<std::time::Duration, String> {
        let t = self.text(CommandId::Sync, "interval");
        if t.is_empty() { return Ok(std::time::Duration::from_secs(TUI_SYNC_DEFAULT_MINUTES * 60)); }
        match t.parse::<u64>() {
            Ok(m) if m >= 1 => Ok(std::time::Duration::from_secs(m * 60)),
            _ => Err(format!("Sync › Interval must be a whole number of minutes (1 or more), not {t:?}")),
        }
    }
    /// The Sync menu as `[sync]` preferences.
    pub fn sync_prefs(&self) -> SyncPrefs {
        use CommandId::Sync as Y;
        SyncPrefs { auto: Some(self.check(Y, "auto")), interval_minutes: self.sync_interval().ok().map(|d| d.as_secs() / 60),
            nicks: Some(self.text(Y, "nicks")), mentions: Some(self.check(Y, "mentions")), bookmarks: Some(self.check(Y, "bookmarks")) }
    }
    /// Fill the Sync menu from `[sync]` preferences.
    pub fn apply_sync_prefs(&mut self, p: &SyncPrefs) {
        use CommandId::Sync as Y;
        if let Some(v) = p.auto { self.set_check(Y, "auto", v); }
        if let Some(v) = p.interval_minutes { self.set_text(Y, "interval", &v.to_string()); }
        if let Some(v) = &p.nicks { self.set_text(Y, "nicks", v); }
        if let Some(v) = p.mentions { self.set_check(Y, "mentions", v); }
        if let Some(v) = p.bookmarks { self.set_check(Y, "bookmarks", v); }
    }

    /// Search menu fields as a filter.
    pub fn search_filter(&self) -> std::result::Result<SearchFilter, String> {
        let nick = self.text(CommandId::Search, "nick");
        Ok(SearchFilter {
            keyword: self.text(CommandId::Search, "query"),
            nicks: if nick.is_empty() { Vec::new() } else { parse_nicks(&nick)?.0 },
            ..SearchFilter::default()
        })
    }
}

/// Check `argv` with clap exactly as the CLI would and make sure it runs headless.
pub fn validate(argv: Vec<String>) -> std::result::Result<Vec<String>, String> {
    match parse_with_mode(&argv) {
        Ok((_, false)) => Ok(argv),
        Ok((_, true)) => Err("these options would open the terminal UI instead of running".into()),
        Err(e) => Err(e.to_string().lines().next().unwrap_or("invalid options").trim_start_matches("error: ").to_string()),
    }
}

/// Shell-style rendering of an argv (for the status line and Help).
pub fn shell_line(argv: &[String]) -> String {
    argv.iter().map(|a| if a.is_empty() || a.contains(|c: char| c.is_whitespace() || "\"'$\\".contains(c)) {
        format!("'{}'", a.replace('\'', "'\\''"))
    } else { a.clone() }).collect::<Vec<_>>().join(" ")
}
