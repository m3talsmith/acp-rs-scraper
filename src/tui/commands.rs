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
pub const ARGV0: &str = "acp-red-star";

/// A menu path the child process will open. `~` and `~/…` expand here because the
/// TUI does not pass the line through a shell. Other paths are unchanged.
pub fn expand_menu_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() { return String::new(); }
    let home = crate::config::home().ok();
    crate::site::expand_with_home(std::path::Path::new(trimmed), home.as_deref()).to_string_lossy().into_owned()
}

/// Menu text fields that name a file or directory.
pub fn is_path_field(id: CommandId, key: &str) -> bool {
    matches!((id, key),
        (CommandId::Scrape, "output") | (CommandId::Export, "path") | (CommandId::Import, "path")
        | (CommandId::Research, "file") | (CommandId::Research, "export_path"))
}

/// Menu bar items, left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandId {
    /// Scrape options. Pinned beside Start Scanning, not a left-hand category.
    Scrape,
    /// Offline search into the Body, and a site search of the same query.
    Search,
    /// New post, Reply, and Quote.
    Compose,
    /// Export, Import, Basket, List, and Maintain.
    Library,
    /// People, nodes, and rooms. A stub until the network node.
    Discover,
    /// Show, add, export, and clear the export basket.
    Basket,
    /// Dump the DB / selection to a file.
    Export,
    /// Import an export into the DB.
    Import,
    /// Refresh the nick bar.
    List,
    /// Remove nicks / clear everything.
    Maintain,
    /// Login / logout. The bar label is Account.
    Session,
    /// Bookmarks: show, toggle, sync.
    Bookmarks,
    /// Research projects.
    Research,
    /// Periodic sync with the site.
    Sync,
    /// Body layout.
    View,
    /// Log level for jobs started from the TUI.
    Debug,
    /// Usage modal.
    Help,
    /// Product name, slogan, and a star. The credits stay hidden.
    About,
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
    /// About page, on Credits.
    About,
    /// About, focused on a section: 0 Credits, 1 Why this is necessary, 2 ACP platform.
    AboutAt(u8),
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
    /// Pick or create a research project, then show it.
    ResearchProjects,
    /// Show Research › Project.
    ShowResearch,
    /// Add the current post (or something in its body) to a research project (`r`).
    AddToResearch,
    /// Add every post of the current view / search.
    AddViewToResearch,
    /// Add Research › Local file.
    AddFileToResearch,
    /// Add Research › Web page URL.
    AddUrlToResearch,
    /// A source that is pinned for later (0 = Google Drive, 1 = Telegram).
    ComingLater(u8),
    /// `--research-export` with Research › Export to / Format / Compress.
    ExportResearch,
    /// The theme picker (F5, the header's `‹theme› ▾`).
    ThemePicker,
    /// The theme editor (Shift-F5).
    ThemeEditor,
    /// List the theme files.
    ThemeList,
    /// Read the theme files again.
    ThemeReload,
    /// Export the current theme.
    ThemeExport,
    /// Download the default theme from the repo (kept if it exists).
    ThemeInit,
    /// Open the site-wide Top feed (View › Site Top).
    OpenTop,
    /// Open the Executive Board feed (the EB row, or View › EB).
    OpenEb,
    /// Open Explore (the Explore row, or View › Explore).
    OpenExplore,
    /// Write the minimal starter theme (the palette's `Theme: starter [name]`).
    ThemeStarter,
    /// The brand picker (the header's `«brand» ▾`).
    BrandPicker,
    /// The brand editor.
    BrandEditor,
    /// List built-in brands and brand files.
    BrandList,
    /// Read the selected brand again.
    BrandReload,
    /// Compose › New post.
    NewPost,
    /// Compose › Reply.
    Reply,
    /// Compose › Quote.
    Quote,
    /// Compose › Save draft. Stored locally. Nothing is sent.
    SaveDraft,
    /// Compose › List drafts.
    ShowDrafts,
    /// Compose › Queue the draft id.
    QueueDraft,
    /// Compose › List the outbox.
    ShowOutbox,
    /// Compose › Send one outbox row, after confirm.
    SendOutbox,
    /// Compose › Cancel one outbox row.
    CancelOutbox,
    /// Search the site with the Search menu's query (`--site-search`).
    RunSiteSearch,
    /// Open a menu that is not on the bar (Library › Export, and the rest).
    OpenMenu(CommandId),
    /// Discover has no network yet.
    DiscoverLater,
    /// Sync › Rules.
    OpenRules,
    /// View › Notifications. The saved list. Not an action-history step.
    OpenNotifications,
    /// Search › History. Re-run a saved query.
    SearchHistory,
    /// Search › Clear history. Deletes saved queries only.
    ClearSearchHistory,
    /// Collapse or open the nick bar.
    ToggleNicks,
    /// Collapse or open the action history bar.
    ToggleActions,
}

/// What a research add takes from (one `--research-add` source).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchSource {
    /// Posts / replies by id (`--post-ids`).
    Posts(Vec<String>),
    /// The matches of a keyword search (`--search`, `--nicks`).
    Search {
        /// Keyword.
        query: String,
        /// Nick filter.
        nicks: Vec<String>,
    },
    /// A local file (`--file`).
    File(String),
    /// A web page (`--url`).
    Url(String),
}

/// argv for `--research-add NAME` from `source`; `description` is used when the project is new.
pub fn research_add_argv(name: &str, description: &str, source: &ResearchSource, note: &str) -> std::result::Result<Vec<String>, String> {
    if name.trim().is_empty() { return Err("pick a research project or type a new name".into()); }
    let mut v = vec![ARGV0.to_string(), "--research-add".into(), name.trim().into()];
    if !description.trim().is_empty() { v.extend(["--research-description".into(), description.trim().into()]); }
    match source {
        ResearchSource::Posts(ids) => {
            if ids.is_empty() { return Err("nothing to add: no posts".into()); }
            v.extend(["--post-ids".into(), ids.join(",")]);
        }
        ResearchSource::Search { query, nicks } => {
            v.extend(["--search".into(), query.clone()]);
            if !nicks.is_empty() { v.extend(["--nicks".into(), nicks.join(",")]); }
        }
        ResearchSource::File(p) => {
            if p.trim().is_empty() { return Err("type a path in Research › Local file first".into()); }
            v.extend(["--file".into(), p.trim().into()]);
        }
        ResearchSource::Url(u) => {
            if u.trim().is_empty() { return Err("type a URL in Research › Web page URL first".into()); }
            v.extend(["--url".into(), u.trim().into()]);
        }
    }
    if !note.trim().is_empty() { v.extend(["--note".into(), note.trim().into()]); }
    validate(v)
}

/// argv for `--research-create NAME`.
pub fn research_create_argv(name: &str, description: &str) -> std::result::Result<Vec<String>, String> {
    if name.trim().is_empty() { return Err("a research project needs a name".into()); }
    let mut v = vec![ARGV0.to_string(), "--research-create".into(), name.trim().into()];
    if !description.trim().is_empty() { v.extend(["--research-description".into(), description.trim().into()]); }
    validate(v)
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
    /// Help topic this option deep-links to (`?` while it is highlighted). Empty: the general Help.
    pub help: &'static str,
}

impl Opt {
    /// The dropdown's hint line: the flag text, and `? {topic}` when this option has its own Help topic.
    pub fn hint_line(&self) -> String {
        if self.help.is_empty() { self.flag.to_string() } else { format!("{}  ? {}", self.flag, self.help) }
    }
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
    /// Drawn on the bar. Library's children stay off the bar and open from Library.
    pub on_bar: bool,
    /// Drawn in the right cluster, immediately left of Start Scanning.
    pub pin_right: bool,
}

fn opt(key: &'static str, label: &'static str, flag: &'static str, required: bool, widget: Widget) -> Opt {
    opt_topic(key, label, flag, "", required, widget)
}
fn opt_topic(key: &'static str, label: &'static str, flag: &'static str, help: &'static str, required: bool, widget: Widget) -> Opt {
    Opt { key, label, flag, required, widget, help }
}
fn check(v: bool) -> Widget { Widget::Check(v) }
fn text(v: &str) -> Widget { Widget::Text(TextField::new(v)) }
fn choice(choices: &'static [&'static str]) -> Widget { Widget::Choice { choices, idx: 0 } }
fn button(a: Action) -> Widget { Widget::Button(a) }
fn shown(id: CommandId, label: &'static str, hint: char, opts: Vec<Opt>) -> MenuCommand {
    MenuCommand { id, label, hint, opts, action: None, on_bar: true, pin_right: false }
}
fn shown_action(id: CommandId, label: &'static str, hint: char, action: Action) -> MenuCommand {
    MenuCommand { id, label, hint, opts: vec![], action: Some(action), on_bar: true, pin_right: false }
}
fn tucked(id: CommandId, label: &'static str, hint: char, opts: Vec<Opt>) -> MenuCommand {
    MenuCommand { id, label, hint, opts, action: None, on_bar: false, pin_right: false }
}
fn pinned(id: CommandId, label: &'static str, hint: char, opts: Vec<Opt>) -> MenuCommand {
    MenuCommand { id, label, hint, opts, action: None, on_bar: true, pin_right: true }
}

/// Scrape feed choices for the mentions crawl (`--latest` / `--trending`).
pub const MODE_CHOICES: &[&str] = &["Latest", "Trending"];
/// Profile feed a nick scrape reads (`--profile-feed`).
pub const PROFILE_FEED_CHOICES: &[&str] = &["Newest", "Top"];
/// Which end of that feed `--limit` keeps (`--order`).
pub const ORDER_CHOICES: &[&str] = &["Newest", "Oldest"];
/// View › Colour.
pub const COLOR_CHOICES: &[&str] = &["Auto", "Always", "Never"];
/// Export format choices.
pub const FORMAT_CHOICES: &[&str] = &["json", "html", "json,html"];
/// Export scope choices.
pub const SCOPE_CHOICES: &[&str] = &["Selected nicks", "Current view", "Everything", "Basket"];
/// Site Top periods (`/rs/top?t=`). Week is first: the default.
pub const TOP_PERIODS: &[&str] = &["week", "day", "month", "year", "all"];

fn view_item() -> MenuCommand {
    shown(CommandId::View, "View", 'v', vec![
        opt("nested", "Nested", "v cycles", false, button(Action::SetView(ViewMode::Nested))),
        opt("stack", "Stack", "", false, button(Action::SetView(ViewMode::Stack))),
        opt("pane", "Stack + content pane", "", false, button(Action::SetView(ViewMode::Pane))),
        opt("carousel_inf", "Carousel (infinite)", "", false, button(Action::SetView(ViewMode::CarouselInfinite))),
        opt("carousel", "Carousel (fixed)", "", false, button(Action::SetView(ViewMode::CarouselFixed))),
        opt("grid", "Grid", "", false, button(Action::SetView(ViewMode::Grid))),
        opt_topic("list_order", "Order", "Newest or oldest for this list. o toggles. Nested orders roots; replies stay in thread order.", "order", false, choice(ORDER_CHOICES)),
        opt_topic("nick_order", "Nick order", "Newest or oldest nick by latest post. o toggles while Nicks is focused.", "nick-order", false, choice(ORDER_CHOICES)),
        opt_topic("nick_limit", "Nick limit", "Keep this many nicks at the chosen end. Blank keeps all. The / filter runs first.", "nick-limit", false, text("")),
        opt_topic("color", "Colour", "auto, always, or never. Same as --color", "color", false, choice(COLOR_CHOICES)),
        opt_topic("notifications", "Notifications", "current and past notices", "notifications", false, button(Action::OpenNotifications)),
        opt("top_period", "Top period", "--top", false, choice(TOP_PERIODS)),
        opt("top_replies", "Top includes replies", "--top-replies", false, check(false)),
        opt("top", "Site Top", "--top", false, button(Action::OpenTop)),
        opt("eb_replies", "EB includes replies", "--eb-replies", false, check(false)),
        opt("eb", "Executive Board", "--eb", false, button(Action::OpenEb)),
        opt("explore_latest", "Explore latest", "--explore-latest", false, check(false)),
        opt("explore", "Explore", "--explore", false, button(Action::OpenExplore)),
        opt("prev", "◂ Previous post", ", / Alt-←", false, button(Action::StackPrev)),
        opt("next", "Next post ▸", ". / Alt-→", false, button(Action::StackNext)),
    ])
}

fn search_item() -> MenuCommand {
    shown(CommandId::Search, "Search", 'e', vec![
        opt("query", "Query", "--search", true, text("")),
        opt("nick", "Nick filter", "--nicks", false, text("")),
        opt_topic("order", "Order", "Newest or oldest for these results. Same window as --order", "search-order", false, choice(ORDER_CHOICES)),
        opt_topic("limit", "Limit", "Keep this many matches at the chosen end. Blank keeps all. Same as --limit", "search-limit", false, text("")),
        opt("run", "Search", "", false, button(Action::RunSearch)),
        opt("site", "Search the site", "--site-search", false, button(Action::RunSiteSearch)),
        opt("advanced", "Advanced…", "Shift-S", false, button(Action::OpenSearchModal)),
        opt("clear", "Clear search", "Ctrl-S", false, button(Action::ClearSearch)),
        opt("keep", "History kept", "blank keeps every search. A number keeps that many.", false, text("")),
        opt("history", "History…", "re-run a past query", false, button(Action::SearchHistory)),
        opt("clear_history", "Clear history", "deletes saved queries only", false, button(Action::ClearSearchHistory)),
    ])
}

fn compose_item() -> MenuCommand {
    shown(CommandId::Compose, "Compose", 'c', vec![
        opt("new", "New post", "n", false, button(Action::NewPost)),
        opt("reply", "Reply", "r on an open post", false, button(Action::Reply)),
        opt("quote", "Quote", "Q on an open post", false, button(Action::Quote)),
        opt_topic("draft_id", "Draft id", "--draft-id", "outbox", false, text("")),
        opt_topic("draft_title", "Draft title", "--draft-title", "outbox", false, text("")),
        opt_topic("body", "Draft body", "--body", "outbox", false, text("")),
        opt_topic("thread", "Draft thread", "--draft-thread", "outbox", false, text("")),
        opt_topic("keep", "Keep local", "--keep-local", "outbox", false, check(false)),
        opt_topic("save", "Save draft", "--draft-save", "outbox", false, button(Action::SaveDraft)),
        opt_topic("drafts", "List drafts", "--draft-list", "outbox", false, button(Action::ShowDrafts)),
        opt_topic("queue", "Queue draft", "--outbox-queue", "outbox", false, button(Action::QueueDraft)),
        opt_topic("outbox", "Outbox", "--outbox-list", "outbox", false, button(Action::ShowOutbox)),
        opt_topic("send_id", "Outbox id", "--outbox-send", "outbox", false, text("")),
        opt_topic("send", "Send from outbox", "--outbox-send", "outbox", false, button(Action::SendOutbox)),
        opt_topic("cancel", "Cancel outbox item", "--outbox-cancel", "outbox", false, button(Action::CancelOutbox)),
    ])
}

fn library_item() -> MenuCommand {
    use CommandId::*;
    shown(Library, "Library", 'l', vec![
        opt("export", "Export", "", false, button(Action::OpenMenu(Export))),
        opt("import", "Import", "", false, button(Action::OpenMenu(Import))),
        opt("basket", "Basket", "", false, button(Action::OpenMenu(Basket))),
        opt("list", "List", "", false, button(Action::OpenMenu(List))),
        opt("maintain", "Maintain", "", false, button(Action::OpenMenu(Maintain))),
    ])
}

fn bookmarks_item() -> MenuCommand {
    shown(CommandId::Bookmarks, "Bookmarks", 'b', vec![
        opt("show", "Show bookmarks", "B · --list-bookmarks", false, button(Action::ShowBookmarks)),
        opt("toggle", "Bookmark current post", "b · --bookmark / --unbookmark", false, button(Action::ToggleBookmark)),
        opt("sync", "Sync with acp.us", "--sync-bookmarks", false, button(Action::SyncBookmarks)),
    ])
}

fn research_item() -> MenuCommand {
    shown(CommandId::Research, "Research", 'r', vec![
        opt("project", "Project", "--research-add / --research-show NAME", false, text("")),
        opt("projects", "Projects…", "--research-list · pick or create", false, button(Action::ResearchProjects)),
        opt("show", "Show project", "--research-show", false, button(Action::ShowResearch)),
        opt("add", "Add current post…", "r · --post-ids", false, button(Action::AddToResearch)),
        opt("add_view", "Add current view / search…", "--search / --post-ids", false, button(Action::AddViewToResearch)),
        opt("file", "Local file", "--file  Tab completes", false, text("")),
        opt("add_file", "Add local file…", "--file", false, button(Action::AddFileToResearch)),
        opt("url", "Web page URL", "--url", false, text("")),
        opt("add_url", "Add web page…", "--url", false, button(Action::AddUrlToResearch)),
        opt("gdrive", "Google Drive (coming later)", "pinned for later", false, button(Action::ComingLater(0))),
        opt("telegram", "Telegram (coming later)", "pinned for later", false, button(Action::ComingLater(1))),
        opt("export_path", "Export to", "--export  Tab completes", false, text("")),
        opt("export_format", "Format", "--format", false, choice(FORMAT_CHOICES)),
        opt("export_compress", "Compress tar.gz", "--compress --tar-gz", false, check(false)),
        opt("export", "Export project", "--research-export", false, button(Action::ExportResearch)),
    ])
}

fn sync_item() -> MenuCommand {
    shown(CommandId::Sync, "Sync", 'y', vec![
        opt("auto", "Auto sync", "while the TUI is open", false, check(false)),
        opt("interval", "Interval (minutes)", "", false, text(&TUI_SYNC_DEFAULT_MINUTES.to_string())),
        opt("nicks", "Nicks", "--sync --nicks (blank: selected nicks)", false, text("")),
        opt("mentions", "Mentions", "--no-mentions when off", false, check(false)),
        opt("bookmarks", "Bookmarks", "--no-bookmarks when off", false, check(true)),
        opt_topic("whole_site", "Whole site", "--sync --scrape-all", "whole-site", false, check(false)),
        opt("now", "Sync now", "--sync", false, button(Action::SyncNow)),
        opt("save", "Save as default", "~/.acp/preferences.toml [sync]", false, button(Action::SaveSyncPrefs)),
        opt_topic("rules", "Rules…", "--rules-list", "rules", false, button(Action::OpenRules)),
    ])
}

fn discover_item() -> MenuCommand {
    shown(CommandId::Discover, "Discover", 'g', vec![
        opt("later", "People, nodes, and rooms", "with the network node", false, button(Action::DiscoverLater)),
    ])
}

fn account_item() -> MenuCommand {
    shown(CommandId::Session, "Account", 'n', vec![
        opt("login", "Log in", "", false, button(Action::Login)),
        opt("logout", "Log out", "--logout", false, button(Action::Logout)),
    ])
}

fn about_item() -> MenuCommand {
    shown(CommandId::About, "About", 't', vec![
        opt("credits", "Credits", "", false, button(Action::AboutAt(0))),
        opt("why", "Why this is necessary", "", false, button(Action::AboutAt(1))),
        opt("platform", "ACP platform", "", false, button(Action::AboutAt(2))),
    ])
}

fn debug_item(args: &Args) -> MenuCommand {
    shown(CommandId::Debug, "Debug", 'd', vec![
        opt("verbose", "Verbose", "-v", false, check(args.verbose)),
        opt("debug", "Debug wire log", "--debug", false, check(args.debug)),
        opt("logging", "Log to files", "--enable-logging", false, check(args.enable_logging)),
        opt("show_log", "Show log panel", "l", false, button(Action::ToggleLog)),
    ])
}

fn scrape_item() -> MenuCommand {
    pinned(CommandId::Scrape, "Scrape", 's', vec![
        opt("mode", "Mode", "--latest / --trending", false, choice(MODE_CHOICES)),
        opt("nicks", "Nicks", "--nicks (blank: selected nicks)", true, text("")),
        opt("mentions", "Mentions", "--no-mentions when off", false, check(true)),
        opt("said_by", "Said by", "--said-by", false, check(false)),
        opt("profile_feed", "Profile feed", "--profile-feed (default: newest)", false, choice(PROFILE_FEED_CHOICES)),
        opt("order", "Order", "--order (default: newest)", false, choice(ORDER_CHOICES)),
        opt("limit", "Post limit", "--limit (blank: whole profile)", false, text("")),
        opt("crawl_limit", "Crawl limit", "--crawl-limit", false, text(&DEFAULT_CRAWL_LIMIT.to_string())),
        opt("attachments", "Attachments", "--no-attachments when off", false, check(true)),
        opt("full", "Full", "--full", false, check(false)),
        opt("refetch", "Refetch truncated", "--refetch-truncated", false, check(false)),
        opt("output", "Output file", "--output (blank: database only)  Tab completes", false, text("")),
        opt("format", "Format", "--format", false, choice(FORMAT_CHOICES)),
        opt_topic("whole_site", "Whole site", "--scrape-all", "whole-site", false, check(false)),
        opt_topic("budget", "Time budget", "--scrape-budget seconds (blank: none)", "whole-site", false, text("")),
        opt("start", "Start Scanning", "", false, button(Action::StartScan)),
    ])
}

fn export_item() -> MenuCommand {
    tucked(CommandId::Export, "Export", 'x', vec![
        opt("path", "Path", "--export  Tab completes", true, text("")),
        opt("scope", "From", "--nicks / --search", false, choice(SCOPE_CHOICES)),
        opt("format", "Format", "--format", false, choice(FORMAT_CHOICES)),
        opt("name", "Name", "--name", false, text("")),
        opt("compress", "Compress tar.gz", "--compress --tar-gz", false, check(false)),
        opt("run", "Export", "", false, button(Action::RunExport)),
    ])
}

fn import_item() -> MenuCommand {
    tucked(CommandId::Import, "Import", 'i', vec![
        opt("path", "Path", "--import  Tab completes", true, text("")),
        opt("compress", "Compressed tar.gz", "--compress --tar-gz", false, check(false)),
        opt("run", "Import", "", false, button(Action::RunImport)),
    ])
}

fn basket_item() -> MenuCommand {
    tucked(CommandId::Basket, "Basket", 'k', vec![
        opt("basket_add", "Add current post to basket", "x", false, button(Action::ToggleBasket)),
        opt("basket_show", "Show basket", "", false, button(Action::ShowBasket)),
        opt("basket_run", "Export basket", "--post-ids", false, button(Action::ExportBasket)),
        opt("basket_clear", "Clear basket", "", false, button(Action::ClearBasket)),
    ])
}

fn list_item() -> MenuCommand {
    tucked(CommandId::List, "List", 'j', vec![
        opt("refresh", "Refresh nicks", "--list-nicks", false, button(Action::RefreshNicks)),
    ])
}

fn maintain_item() -> MenuCommand {
    tucked(CommandId::Maintain, "Maintain", 'a', vec![
        opt("nicks", "Remove nicks", "--remove-nick (blank: selected nicks)", true, text("")),
        opt("keep_replies", "Keep replies", "--keep-replies", false, check(false)),
        opt("dry_run", "Dry run", "--dry-run", false, check(false)),
        opt("yes", "Skip confirmation", "--yes (dangerous)", false, check(false)),
        opt("remove", "Remove", "", false, button(Action::RemoveNicks)),
        opt("clear", "Clear all", "--clear-all", false, button(Action::ClearAll)),
    ])
}

/// Help topic for a menu command.
pub fn command_topic(id: CommandId) -> &'static str {
    match id {
        CommandId::Scrape => "scrape",
        CommandId::Search => "search",
        CommandId::Compose => "compose",
        CommandId::Library => "library",
        CommandId::Discover => "discover",
        CommandId::Basket => "basket",
        CommandId::Export => "export",
        CommandId::Import => "import",
        CommandId::List => "list",
        CommandId::Maintain => "maintain",
        CommandId::Session => "account",
        CommandId::Bookmarks => "bookmark",
        CommandId::Research => "research",
        CommandId::Sync => "sync",
        CommandId::View => "view",
        CommandId::Debug => "debug",
        CommandId::Help => "keys",
        CommandId::About => "about-credits",
        CommandId::Quit => "quit",
    }
}

/// A more specific Help topic for one option. `None` uses [`command_topic`].
pub fn option_topic(id: CommandId, key: &str) -> Option<&'static str> {
    match (id, key) {
        (CommandId::About, "credits") => Some("about-credits"),
        (CommandId::About, "why") => Some("about-why"),
        (CommandId::About, "platform") => Some("about-platform"),
        (CommandId::Compose, _) => Some("compose"),
        (CommandId::Bookmarks, _) => Some("bookmark"),
        (CommandId::Basket, _) => Some("basket"),
        (CommandId::Session, "login") => Some("login"),
        (CommandId::Session, "logout") => Some("account"),
        (CommandId::Search, "site") | (CommandId::Search, "run") | (CommandId::Search, "clear") => Some("search"),
        (CommandId::Discover, _) => Some("discover"),
        (CommandId::View, "notifications") => Some("notifications"),
        (CommandId::Scrape, "whole_site") | (CommandId::Scrape, "budget") | (CommandId::Sync, "whole_site") => Some("whole-site"),
        _ => None,
    }
}

/// The whole menu bar, with every option's current value.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    /// Items in catalog order. [`Menu::bar_indices`] is what the bar draws.
    pub items: Vec<MenuCommand>,
}

impl Menu {
    /// Default catalog. `args` seeds the Debug options (from `-v`/`--debug`/`--enable-logging`).
    pub fn new(args: &Args) -> Self {
        use CommandId::*;
        let items = vec![
            view_item(),
            search_item(),
            compose_item(),
            library_item(),
            bookmarks_item(),
            research_item(),
            sync_item(),
            discover_item(),
            account_item(),
            debug_item(args),
            shown_action(Help, "Help", 'h', Action::Help),
            about_item(),
            shown_action(Quit, "Quit", 'q', Action::Quit),
            scrape_item(),
            export_item(),
            import_item(),
            basket_item(),
            list_item(),
            maintain_item(),
        ];
        let mut menu = Menu { items };
        menu.fill_help_topics();
        menu
    }
    /// Give every option a Help topic. A topic already set on the option stays.
    fn fill_help_topics(&mut self) {
        for c in &mut self.items {
            let fallback = command_topic(c.id);
            for o in &mut c.opts {
                if o.help.is_empty() { o.help = option_topic(c.id, o.key).unwrap_or(fallback); }
            }
        }
    }
    /// Item by id.
    pub fn get(&self, id: CommandId) -> &MenuCommand { self.items.iter().find(|c| c.id == id).expect("menu id") }
    /// Bar items left to right: categories, then anything pinned beside Start Scanning.
    pub fn bar_indices(&self) -> Vec<usize> {
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for (i, c) in self.items.iter().enumerate() {
            if !c.on_bar { continue; }
            if c.pin_right { right.push(i); } else { left.push(i); }
        }
        left.extend(right);
        left
    }
    /// On-bar item with this hint letter. Tucked menus are not reached from the bar.
    pub fn bar_by_hint(&self, hint: char) -> Option<usize> {
        let h = hint.to_ascii_lowercase();
        self.bar_indices().into_iter().find(|&i| self.items[i].hint == h)
    }
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
        let whole = self.check(S, "whole_site");
        let t = self.text(S, "nicks");
        let nicks = if t.is_empty() { selected.to_vec() } else { parse_nicks(&t)?.0 };
        if nicks.is_empty() && !whole { return Err("Scrape needs at least one nick, or Whole site: type them in Scrape › Nicks or select nicks on the left".into()); }
        let mut v = vec![ARGV0.to_string()];
        if whole { v.push("--scrape-all".into()); }
        if !nicks.is_empty() {
            v.extend(["--nicks".into(), nicks.join(",")]);
            if self.choice(S, "mode") == "Trending" { v.push("--trending".into()); } else { v.push("--latest".into()); }
            if !self.check(S, "mentions") { v.push("--no-mentions".into()); }
            if self.check(S, "said_by") { v.push("--said-by".into()); }
            if self.choice(S, "profile_feed") == "Top" { v.extend(["--profile-feed".into(), "top".into()]); }
            if self.choice(S, "order") == "Oldest" { v.extend(["--order".into(), "oldest".into()]); }
        }
        let lim = self.text(S, "limit");
        if !lim.is_empty() {
            let n: usize = lim.parse().map_err(|_| format!("Post limit must be a number, not {lim:?}"))?;
            if n == 0 { return Err("Post limit must be at least 1".into()); }
            v.extend(["--limit".into(), n.to_string()]);
        }
        let cl = self.text(S, "crawl_limit");
        if !cl.is_empty() {
            let n: usize = cl.parse().map_err(|_| format!("Crawl limit must be a number, not {cl:?}"))?;
            if n != DEFAULT_CRAWL_LIMIT { v.extend(["--crawl-limit".into(), n.to_string()]); }
        }
        if whole {
            let budget = self.text(S, "budget");
            if !budget.is_empty() {
                let n: u64 = budget.parse().map_err(|_| format!("Time budget must be a number of seconds, not {budget:?}"))?;
                if n == 0 { return Err("Time budget must be at least 1 second".into()); }
                v.extend(["--scrape-budget".into(), n.to_string()]);
            }
        } else if !self.text(S, "budget").is_empty() {
            return Err("Time budget is for Whole site".into());
        }
        if !self.check(S, "attachments") { v.push("--no-attachments".into()); }
        if self.check(S, "full") { v.push("--full".into()); }
        if self.check(S, "refetch") { v.push("--refetch-truncated".into()); }
        let out = expand_menu_path(&self.text(S, "output"));
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
        let path = expand_menu_path(&self.text(E, "path"));
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
                    if f.order != crate::prelude::ScrapeOrder::Newest { v.extend(["--order".into(), f.order.to_string()]); }
                    if let Some(n) = f.limit { v.extend(["--limit".into(), n.to_string()]); }
                    v.extend(["--search-json".into(), crate::tui::data::saved_query(f, false).to_json().to_string()]);
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
        let path = expand_menu_path(&self.text(I, "path"));
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

    /// argv for `--research-export` of `name` with Research › Export to / Format / Compress.
    pub fn research_export_argv(&self, name: &str) -> std::result::Result<Vec<String>, String> {
        use CommandId::Research as R;
        if name.trim().is_empty() { return Err("pick a research project first (Research › Projects…)".into()); }
        let path = expand_menu_path(&self.text(R, "export_path"));
        if path.is_empty() { return Err("Research › Export to needs a path".into()); }
        let mut v = vec![ARGV0.to_string(), "--research-export".into(), name.trim().into(), "--export".into(), path,
            "--format".into(), self.choice(R, "export_format").into()];
        if self.check(R, "export_compress") { v.extend(["--compress".into(), "--tar-gz".into()]); }
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

    /// argv for Explore (Latest when that checkbox is on).
    pub fn explore_argv(&self) -> std::result::Result<Vec<String>, String> {
        let flag = if self.check(CommandId::View, "explore_latest") { "--explore-latest" } else { "--explore" };
        let mut v = vec![ARGV0.to_string(), flag.into()];
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for `--reposts`.
    pub fn reposts_argv(&self, id: &str) -> std::result::Result<Vec<String>, String> {
        let id = id.trim();
        if id.is_empty() { return Err("reposts need a post id".into()); }
        let mut v = vec![ARGV0.to_string(), "--reposts".into(), id.into()];
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for `--edits THREAD/POST`. A root post uses its own id as the thread.
    pub fn edits_argv(&self, thread_id: &str, post_id: &str) -> std::result::Result<Vec<String>, String> {
        let post = post_id.trim();
        if post.is_empty() { return Err("edits need a post id".into()); }
        let thread = { let t = thread_id.trim(); if t.is_empty() { post } else { t } };
        let mut v = vec![ARGV0.to_string(), "--edits".into(), format!("{thread}/{post}")];
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for `--sort-thread` (`oldest` or `top`).
    pub fn sort_thread_argv(&self, thread_id: &str, order: &str) -> std::result::Result<Vec<String>, String> {
        let id = thread_id.trim();
        if id.is_empty() { return Err("a thread needs an id".into()); }
        if !matches!(order, "oldest" | "top") { return Err("thread sort must be oldest or top".into()); }
        let mut v = vec![ARGV0.to_string(), "--sort-thread".into(), id.into(), "--thread-sort".into(), order.into()];
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for a tag feed (`--tag`).
    pub fn tag_argv(&self, tag: &str) -> std::result::Result<Vec<String>, String> {
        let t = tag.trim().trim_start_matches('#').trim();
        if t.is_empty() { return Err("a tag needs a name".into()); }
        let mut v = vec![ARGV0.to_string(), "--tag".into(), t.into()];
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for a site search (Search modal › Where: Site).
    pub fn site_search_argv(&self, query: &str) -> std::result::Result<Vec<String>, String> {
        let q = query.trim();
        if q.is_empty() { return Err("site search needs a keyword".into()); }
        let mut v = vec![ARGV0.to_string(), "--site-search".into(), q.into()];
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for the Executive Board feed.
    pub fn eb_argv(&self) -> std::result::Result<Vec<String>, String> {
        let mut v = vec![ARGV0.to_string(), "--eb".into()];
        if self.check(CommandId::View, "eb_replies") { v.push("--eb-replies".into()); }
        v.extend(self.debug_args());
        validate(v)
    }

    /// argv for View › Site Top.
    pub fn top_argv(&self) -> std::result::Result<Vec<String>, String> {
        use CommandId::View as V;
        let mut v = vec![ARGV0.to_string(), "--top".into(), self.choice(V, "top_period").into()];
        if self.check(V, "top_replies") { v.push("--top-replies".into()); }
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
        let whole = self.check(Y, "whole_site");
        if nicks.is_empty() && !bm && !whole { return Err("nothing to sync: tick Sync › Bookmarks or Whole site, or type nicks in Sync › Nicks / select nicks".into()); }
        let mut v = vec![ARGV0.to_string(), "--sync".into()];
        if !nicks.is_empty() { v.extend(["--nicks".into(), nicks.join(",")]); if !self.check(Y, "mentions") { v.push("--no-mentions".into()); } }
        if !bm { v.push("--no-bookmarks".into()); }
        if whole { v.push("--scrape-all".into()); }
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

    /// Search menu fields as a filter. Order and limit use the same window as `--order` and `--limit`.
    pub fn search_filter(&self) -> std::result::Result<SearchFilter, String> {
        let nick = self.text(CommandId::Search, "nick");
        let limit_text = self.text(CommandId::Search, "limit");
        let limit = if limit_text.is_empty() { None } else { Some(parse_post_limit(&limit_text)?) };
        Ok(SearchFilter {
            keyword: self.text(CommandId::Search, "query"),
            nicks: if nick.is_empty() { Vec::new() } else { parse_nicks(&nick)?.0 },
            order: crate::tui::order::order_from_choice(self.choice(CommandId::Search, "order")),
            limit,
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
