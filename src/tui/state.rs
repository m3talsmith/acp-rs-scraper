//! TUI state and its pure transitions (no terminal, no network): nick
//! selection, the Body views (nick posts / search results), the detail stack,
//! menu state, modals and the side effects the event loop must carry out.

use crate::prelude::*;
use crate::tui::commands::{Action, CommandId, Menu, ResearchSource};
use crate::research::{ItemKind, ResearchItem};
use std::collections::HashSet;
use std::path::Path;
use crate::tui::data::{NickEntry, SearchFilter, Store};
use crate::tui::jobs::JobKind;
use crate::tui::modals::*;
use crate::tui::theme::Theme;
use ratatui::layout::Rect;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Whether a status line is an error or a warning, whatever the capitalization.
///
/// The child's runtime prints `Error: ...`. The status bar used to look only
/// for a lowercase prefix, so that line was drawn as ordinary text.
pub(crate) fn status_line_is_problem(line: &str) -> bool {
    let line = line.trim_start();
    let bytes = line.as_bytes();
    bytes.len() >= 5 && bytes[..5].eq_ignore_ascii_case(b"error")
        || bytes.len() >= 7 && bytes[..7].eq_ignore_ascii_case(b"warning")
}

/// Focusable panes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// Left nick bar.
    Nicks,
    /// Posts / details.
    Body,
    /// Menu bar.
    Menu,
    /// Status bar (Enter opens job details).
    Status,
}

/// What a link in a detail points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    /// The post this one answers (`in_reply_to_id`).
    ReplyTo,
    /// The thread root (`parent_post_id`), when different.
    Thread,
    /// A post id mentioned in the body.
    Mentioned,
    /// A direct reply to this post.
    Reply,
    /// A stored attachment file (`id` is its local path); opens with the desktop's opener.
    Attachment,
    /// A `#tag` (`id` is the name without `#`). Opens that tag's feed.
    Tag,
}

/// One cross-link inside a detail.
#[derive(Clone, Debug)]
pub struct Link {
    /// Kind.
    pub kind: LinkKind,
    /// Target post id.
    pub id: String,
    /// Target, when it is in the database.
    pub target: Option<Post>,
}

/// One opened post (a frame of the detail stack).
#[derive(Clone, Debug)]
pub struct DetailFrame {
    /// The post.
    pub post: Post,
    /// Its attachments.
    pub attachments: Vec<Attachment>,
    /// Cross-links (reply-to, thread, mentioned ids, replies).
    pub links: Vec<Link>,
    /// Selected link.
    pub link_cursor: usize,
    /// Reply order. `s` cycles newest → oldest → top.
    pub sort: ThreadSort,
    /// First visible line.
    pub scroll: u16,
    /// Scroll so the selected link is visible on the next render.
    pub follow_link: bool,
    /// Inline notice (e.g. "not in database").
    pub notice: Option<String>,
}

/// A list of posts in the Body, with its own cursor and detail stack.
#[derive(Clone, Debug, Default)]
pub struct BodyView {
    /// Title in the Body's navigation bar.
    pub title: String,
    /// Posts, newest first.
    pub posts: Vec<Post>,
    /// Selected post.
    pub cursor: usize,
    /// First visible post.
    pub offset: usize,
    /// Open details; `[0]` is the post opened from the list.
    pub stack: Vec<DetailFrame>,
    /// The search that produced it (search views only).
    pub filter: Option<SearchFilter>,
    /// The bookmarks view.
    pub bookmarks: bool,
    /// The research project this view shows.
    pub research: Option<String>,
    /// The site read this view shows (`top:week`, …), when it is one.
    pub site_key: Option<String>,
    /// Frames closed with prev / Esc, nearest last: `next` reopens them.
    pub forward: Vec<DetailFrame>,
    /// The full list, before order and limit. Empty means `posts` was filled by a test and is left as it is.
    pub source: Vec<Post>,
    /// Which list this is. Decides which remembered order applies.
    pub kind: crate::tui::order::ListKind,
    /// Keep `source`'s order instead of sorting by time. Site lists do this.
    pub site_order: bool,
}

/// Which animation runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimKind {
    /// The detail slides in and the list slides up.
    Open,
    /// A linked post opens, pushing the previous detail up.
    Push,
}

/// A running animation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anim {
    /// Kind.
    pub kind: AnimKind,
    /// Start.
    pub start: Instant,
}

impl Anim {
    /// 0.0 → 1.0 over [`TUI_ANIM_MS`] (eased).
    pub fn progress(&self, now: Instant) -> f32 {
        let t = (now.saturating_duration_since(self.start).as_millis() as f32 / TUI_ANIM_MS as f32).clamp(0.0, 1.0);
        1.0 - (1.0 - t) * (1.0 - t)
    }
    /// Finished at `now`.
    pub fn done(&self, now: Instant) -> bool { now.saturating_duration_since(self.start) >= Duration::from_millis(TUI_ANIM_MS) }
}

/// How the Body lays out posts (View menu, `v`, or the nav bar's `[View: …]`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    /// Cards; a detail slides in and linked posts nest inside it.
    #[default]
    Nested,
    /// One endless column of full posts; a detail replaces the list (no nesting).
    Stack,
    /// The stack on top and the clicked post in a content pane below.
    Pane,
    /// One post at a time, side to side; wraps around at the ends.
    CarouselInfinite,
    /// One post at a time, side to side; stops at the ends.
    CarouselFixed,
    /// A grid of cards; a post opens in a modal with a Close button.
    Grid,
}

impl ViewMode {
    /// Every mode, in `v` order.
    pub const ALL: [ViewMode; 6] = [ViewMode::Nested, ViewMode::Stack, ViewMode::Pane, ViewMode::CarouselInfinite, ViewMode::CarouselFixed, ViewMode::Grid];
    /// Label.
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Nested => "Nested", ViewMode::Stack => "Stack", ViewMode::Pane => "Stack + pane",
            ViewMode::CarouselInfinite => "Carousel ∞", ViewMode::CarouselFixed => "Carousel", ViewMode::Grid => "Grid",
        }
    }
    /// The next mode (`d` = ±1).
    pub fn cycle(self, d: i32) -> ViewMode {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[cycle(i, Self::ALL.len(), d)]
    }
    /// Posts move with ←/→.
    pub fn horizontal(self) -> bool { matches!(self, ViewMode::CarouselInfinite | ViewMode::CarouselFixed | ViewMode::Grid) }
}

/// Login state as far as the TUI knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    /// No saved session.
    LoggedOut,
    /// A saved session exists (not yet checked against the site).
    Saved,
    /// The site accepted the session.
    Valid,
    /// Being checked.
    Checking,
}

/// The running job, as shown in the UI.
#[derive(Clone, Debug, PartialEq)]
pub struct JobView {
    /// Kind.
    pub kind: JobKind,
    /// Process id.
    pub pid: u32,
    /// Log file.
    pub log: PathBuf,
}

/// Something only the event loop can do (processes, network, quitting).
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Check the session, then run this job (or open Login first).
    CheckSessionThen(JobKind, Vec<String>),
    /// Start a headless job.
    RunJob(JobKind, Vec<String>),
    /// Automatic sync: check the session quietly, then run (skip when offline / logged out).
    AutoSync(Vec<String>),
    /// Write the `[sync]` preferences.
    SaveSyncPrefs(SyncPrefs),
    /// Start a small headless job next to the main one (bookmark writes).
    RunSideJob(JobKind, Vec<String>),
    /// Kill the running job.
    CancelJob,
    /// Kill one side job (research, bookmarks) by pid.
    CancelSide(u32),
    /// Open a file with the desktop's default app (`xdg-open`).
    OpenFile(PathBuf),
    /// Start logging in with this email.
    LoginEmail(String),
    /// Submit the code to the running login.
    LoginCode(String),
    /// Ask for another code on the running login.
    LoginResend,
    /// Abandon the running login.
    LoginCancel,
    /// Forget the session.
    Logout,
    /// Store `[ui] theme` in preferences.toml.
    SaveUiTheme(String),
    /// Write a copy of this theme into the exports folder.
    ExportTheme(crate::themes::ThemeFile),
    /// Download a theme from the repo into the themes folder (`true`: asked for by hand).
    DownloadTheme(String, bool),
    /// Leave; `cancel_job` kills a running job first (else it is detached).
    Quit {
        /// Kill the job before leaving.
        cancel_job: bool,
    },
}

/// Clickable regions recorded while drawing (topmost last).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// n-th nick.
    Nick(usize),
    /// n-th post of the current view.
    Post(usize),
    /// n-th menu item.
    MenuItem(usize),
    /// n-th option of the open dropdown.
    MenuOpt(usize),
    /// Start Scanning / Cancel.
    StartScan,
    /// The Body's `<` back button.
    Back,
    /// n-th link of the top detail frame.
    Link(usize),
    /// Header strip of the frame at this depth.
    Frame(usize),
    /// The nick pane (wheel).
    NickPane,
    /// The post list (wheel).
    ListPane,
    /// The top detail (wheel).
    DetailPane,
    /// The nav bar's `[View: …]` switch.
    ViewMode,
    /// The nav bar's `[Order: …]` switch.
    ListOrder,
    /// The nav bar's `◂ prev`.
    StackPrev,
    /// The nav bar's `next ▸`.
    StackNext,
    /// The Grid modal's Close button.
    CloseDetail,
    /// Inside the modal.
    Modal(ModalHit),
    /// Modal backdrop (swallows clicks).
    Backdrop,
    /// The "★ Bookmarks" row of the nick bar.
    Bookmarks,
    /// The "EB" row of the nick bar.
    Eb,
    /// The "Explore" row of the nick bar.
    Explore,
    /// The nick filter field.
    NickFilter,
    /// The nick filter's `x` (clear).
    NickFilterClear,
    /// The bookmark star of the n-th post card.
    Star(usize),
    /// The bookmark star of the top detail.
    StarTop,
    /// The export-basket button of the n-th post card.
    Basket(usize),
    /// The export-basket button of the top detail.
    BasketTop,
    /// The add-to-research button of the n-th post card.
    Research(usize),
    /// The add-to-research button of the top detail.
    ResearchTop,
    /// The header's `‹theme› ▾` switch.
    ThemeSwitch,
    /// n-th row of the theme editor's list.
    EditorRow(usize),
    /// The header's `⟳ … running` mark.
    Job,
    /// The status bar.
    Status,
    /// The header's red star.
    Logo,
    /// The n-th toast. A click dismisses it. The log keeps the notice.
    Toast(usize),
}

/// Menu bar state.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuState {
    /// Catalog with current values.
    pub menu: Menu,
    /// Highlighted item.
    pub item: usize,
    /// Item whose dropdown is open.
    pub open: Option<usize>,
    /// Bar item that opened a tucked menu (Library, while Export is showing). Esc returns here.
    pub anchor: Option<usize>,
    /// Highlighted option in the dropdown.
    pub opt: usize,
    /// Typing into the highlighted text option.
    pub editing: bool,
    /// Other path completions for the field being typed. Empty when there is one match or none.
    pub path_matches: Vec<String>,
    /// Which [`MenuState::path_matches`] entry is highlighted.
    pub path_at: usize,
}

/// Everything the TUI shows.
pub struct App {
    /// Data source (read-only).
    pub store: Box<dyn Store>,
    /// Nick bar: the nicks shown (every nick, narrowed by the filter).
    pub nicks: Vec<NickEntry>,
    /// Every nick in the database.
    pub all_nicks: Vec<NickEntry>,
    /// Nick-bar filter text.
    pub nick_filter: crate::tui::widgets::text::TextField,
    /// Typing into the nick-bar filter.
    pub filter_editing: bool,
    /// Highlighted nick.
    pub nick_cursor: usize,
    /// First visible nick.
    pub nick_offset: usize,
    /// Selected nicks (multi-select), in selection order.
    pub selected: Vec<String>,
    /// Nick posts view.
    pub base: BodyView,
    /// Search results replacing the Body (back with `<`).
    pub search: Option<BodyView>,
    /// Focused pane.
    pub focus: Focus,
    /// Menu bar.
    pub menu: MenuState,
    /// Open modal.
    pub modal: Option<Modal>,
    /// Status / log lines (newest last).
    pub status: VecDeque<String>,
    /// Log panel expanded.
    pub show_log: bool,
    /// The next frame should clear the terminal before drawing.
    ///
    /// Set when the menu changes or an error line arrives, so a status message
    /// cannot be left painted over the post list.
    pub needs_redraw: bool,
    /// Login state.
    pub session: SessionState,
    /// Running job.
    pub job: Option<JobView>,
    /// Jobs the details window can show (every running one, plus a short history).
    pub tracked: Vec<crate::tui::modals::job::TrackedJob>,
    /// Job (scan / sync) waiting for a login.
    pub pending_job: Option<(JobKind, Vec<String>)>,
    /// Bookmarked post ids (local changes show at once; a side job stores them).
    pub marks: HashSet<String>,
    /// Body layout.
    pub view_mode: ViewMode,
    /// Grid columns at the last draw (↑/↓ move a row).
    pub grid_cols: usize,
    /// When the next automatic sync is due (None: as soon as Auto sync is on).
    pub sync_next: Option<Instant>,
    /// The running job is an automatic sync (failures go to the status line, not a modal).
    pub auto_running: bool,
    /// Running animation.
    pub anim: Option<Anim>,
    /// The empty-Body animation (None: off).
    pub empty_anim: Option<crate::tui::empty_anim::Frames>,
    /// Draw it in the accent colour instead of muted.
    pub empty_anim_accent: bool,
    /// When it started (frame 0).
    pub empty_anim_start: Instant,
    /// The frame drawn last, while it is on screen (None: not shown or paused).
    pub empty_anim_frame: Option<usize>,
    /// Clickable regions from the last draw.
    pub hits: Vec<(Rect, Hit)>,
    /// Colours.
    pub theme: Theme,
    /// Side effects for the event loop.
    pub effects: Vec<Effect>,
    /// The research project last used (preselected in the modal).
    pub research_current: Option<String>,
    /// Post ids picked for export (`x`), in the order added.
    pub basket: Vec<String>,
    /// Pictures (logo, post images); `None` when off or not on a terminal.
    pub images: Option<crate::tui::images::Images>,
    /// Hyperlinks drawn this frame (x, y, text, target URL), re-emitted as OSC 8 after the draw.
    pub osc_links: Vec<(u16, u16, String, String)>,
    /// Post cards that fit the list (set while drawing).
    pub page_rows: usize,
    /// Login email pre-fill.
    pub email: Option<String>,
    /// Login modal hidden while Help is open (`?` / F1). Restored when Help closes.
    /// The login attempt keeps running.
    pub paused_login: Option<LoginModal>,
    /// `~/.acp/themes` (None: theme files are off, e.g. in tests and previews).
    pub themes_dir: Option<PathBuf>,
    /// The selected theme's name.
    pub theme_name: String,
    /// The selected theme's file, when it loaded.
    pub theme_file: Option<crate::themes::ThemeFile>,
    /// Why the selected theme couldn't be used.
    pub theme_error: Option<String>,
    /// What is wrong with the selected theme's file, for the header: `missing` or `broken`.
    pub theme_fault: Option<&'static str>,
    /// `[ui] theme_mode`.
    pub theme_mode: crate::themes::Mode,
    /// The terminal's background is dark (`None`: unknown).
    pub dark_bg: Option<bool>,
    /// The full-screen theme editor (Shift-F5).
    pub theme_editor: Option<Box<crate::tui::editor::ThemeEditor>>,
    /// The theme being downloaded in the background, if any.
    pub theme_downloading: Option<String>,
    /// Site-feed key to open when the running read job ends.
    pub pending_feed: Option<String>,
    /// Thread whose reply order a running sort job should restore.
    pub pending_sort: Option<(String, ThreadSort)>,
    /// Header chip for this machine's node (`◉ label`). Empty when no node exists.
    pub node_chip: String,
    /// Unix seconds of the last confirmed post or reply in this session.
    /// The capture gate still applies; the stamp only feeds the rate limit.
    pub last_write_unix: Option<i64>,
    /// Newest or oldest for each list.
    pub orders: crate::tui::order::ListOrders,
    /// Newest or oldest for the nick bar, by each nick's latest post.
    pub nick_order: ScrapeOrder,
    /// How many nicks to keep after the filter. `None` keeps all of them.
    pub nick_limit: Option<usize>,
    /// Colour output for this run. `always` ignores NO_COLOR.
    pub color_mode: ColorMode,
    /// NO_COLOR was set when the terminal UI started.
    pub no_color: bool,
    /// Progress through the star-burst sequence. Cleared while a text field has focus.
    pub konami: crate::eggs::Konami,
    /// A full-screen clip, when one is playing.
    pub egg: Option<crate::eggs::EggPlay>,
    /// Header-star presses toward the credits.
    pub logo_taps: u8,
    /// When the header star was last pressed.
    pub logo_tap_at: Option<Instant>,
    /// Toasts, the notice log, and desktop notifications.
    pub notices: crate::notify::NoticeBoard,
}

fn nick_label(q: &str) -> String {
    let q = q.trim();
    if q.is_empty() { "nicks".into() } else { format!("nicks \"{q}\"") }
}

/// Lay `v` out again from its source. An empty source means the posts were set directly.
fn reflow_in(v: &mut BodyView, mode: ViewMode, orders: crate::tui::order::ListOrders) {
    if v.source.is_empty() { return; }
    let keep = v.posts.get(v.cursor).map(|p| p.post_id.clone());
    let (order, limit) = if v.kind == crate::tui::order::ListKind::Search {
        (v.filter.as_ref().map(|f| f.order).unwrap_or(orders.search), v.filter.as_ref().and_then(|f| f.limit))
    } else {
        (orders.get(v.kind), None)
    };
    v.posts = crate::tui::order::arrange_posts(&v.source, order, limit, mode == ViewMode::Nested, v.site_order, v.kind == crate::tui::order::ListKind::Edits);
    if let Some(id) = keep && let Some(i) = v.posts.iter().position(|p| p.post_id == id) {
        v.cursor = i;
    } else if v.posts.is_empty() {
        v.cursor = 0;
    } else {
        v.cursor = v.cursor.min(v.posts.len() - 1);
    }
    if v.posts.is_empty() { v.offset = 0; }
    else if v.offset > v.cursor { v.offset = v.cursor; }
}

impl App {
    /// State over `store`; `args` seeds the menu, `--nick(s)` the selection.
    pub fn new(store: Box<dyn Store>, args: &Args, theme: Theme, session_saved: bool) -> Self {
        let mut app = App {
            store, nicks: Vec::new(), all_nicks: Vec::new(), nick_filter: Default::default(), filter_editing: false, nick_cursor: 0, nick_offset: 0, selected: Vec::new(),
            base: BodyView::default(), search: None, focus: Focus::Nicks,
            menu: MenuState { menu: Menu::new(args), item: 0, open: None, anchor: None, opt: 0, editing: false, path_matches: Vec::new(), path_at: 0 },
            modal: None, status: VecDeque::new(), show_log: false, needs_redraw: false,
            session: if session_saved { SessionState::Saved } else { SessionState::LoggedOut },
            job: None, tracked: Vec::new(), pending_job: None, marks: HashSet::new(), view_mode: ViewMode::Nested, grid_cols: 1, sync_next: None, auto_running: false, anim: None, empty_anim: None, empty_anim_accent: false, empty_anim_start: Instant::now(), empty_anim_frame: None, hits: Vec::new(), theme, effects: Vec::new(), osc_links: Vec::new(), images: None, basket: Vec::new(), research_current: None,
            page_rows: 5, email: None, paused_login: None, themes_dir: None, theme_name: DEFAULT_THEME.into(), theme_file: None, theme_error: None, theme_fault: None,
            theme_mode: crate::themes::Mode::System, dark_bg: None, theme_editor: None, theme_downloading: None,
            pending_feed: None, pending_sort: None, node_chip: String::new(), last_write_unix: None,
            orders: crate::tui::order::ListOrders::default(), nick_order: ScrapeOrder::Newest, nick_limit: None,
            color_mode: ColorMode::Auto, no_color: false,
            konami: crate::eggs::Konami::new(), egg: None, logo_taps: 0, logo_tap_at: None,
            notices: crate::notify::NoticeBoard::new(),
        };
        app.refresh_nicks();
        app.reload_marks();
        let want = args.nick_list().unwrap_or_default();
        if !want.is_empty() {
            app.selected = want.iter().filter_map(|w| app.nicks.iter().find(|n| n.nick.eq_ignore_ascii_case(w)).map(|n| n.nick.clone())).collect();
            if let Some(i) = app.selected.first().and_then(|s| app.nicks.iter().position(|n| &n.nick == s)) { app.nick_cursor = i; }
            app.menu.menu.set_text(CommandId::Scrape, "nicks", &want.join(","));
            app.load_base();
        }
        app
    }

    /// Add a status line (kept to [`TUI_STATUS_KEEP`]).
    /// An error or warning asks for a full redraw so the line stays in the status bar.
    pub fn push_status(&mut self, s: impl Into<String>) {
        let s = s.into();
        for l in s.lines() {
            if status_line_is_problem(l) { self.needs_redraw = true; }
            if let Some(n) = crate::notify::Notice::from_status_line(l) {
                self.notices.post(n, Instant::now());
            } else if l.starts_with("auto sync skipped") {
                self.notices.post(crate::notify::Notice {
                    kind: crate::notify::NoticeKind::Sync,
                    title: "Sync".into(),
                    body: l.to_string(),
                }, Instant::now());
            }
            self.status.push_back(l.to_string());
            while self.status.len() > TUI_STATUS_KEEP { self.status.pop_front(); }
        }
    }
    /// Flip do-not-disturb or one notice kind, and store it when this TUI has a preferences file.
    pub fn toggle_notice_row(&mut self, row: usize) {
        if !self.notices.settings.toggle_row(row) { return; }
        self.persist_notices();
    }
    /// Store the notice switches. Unit tests leave `themes_dir` unset and do not write.
    fn persist_notices(&mut self) {
        if self.themes_dir.is_none() { return; }
        let s = &self.notices.settings;
        let p = crate::auth::prefs::NotifyPrefs {
            dnd: s.dnd,
            error: s.on[crate::notify::NoticeKind::Error.index()],
            warning: s.on[crate::notify::NoticeKind::Warning.index()],
            sync: s.on[crate::notify::NoticeKind::Sync.index()],
            job: s.on[crate::notify::NoticeKind::Job.index()],
        };
        if let Err(e) = crate::auth::prefs::save_notify_prefs(&p) {
            self.push_status(format!("error: could not save notice settings: {e:#}"));
        }
    }
    /// Show an error modal and log it.
    pub fn error(&mut self, title: &str, text: &str) {
        self.push_status(format!("error: {text}"));
        self.modal = Some(Modal::Message(MessageModal::error(title, text)));
    }

    // ------------------------------------------------------------ nicks
    /// Re-read the nick bar; keep the selection that still exists.
    pub fn refresh_nicks(&mut self) {
        match self.store.nicks() {
            Ok(n) => self.all_nicks = n,
            Err(e) => { self.push_status(format!("error: reading nicks: {e:#}")); self.all_nicks.clear(); }
        }
        // Selected nicks hidden by the filter stay selected.
        let keep: Vec<String> = self.selected.iter()
            .filter_map(|s| self.all_nicks.iter().find(|n| n.nick.eq_ignore_ascii_case(s)).map(|n| n.nick.clone())).collect();
        self.selected = keep;
        let cur = self.nicks.get(self.nick_cursor).map(|n| n.nick.clone());
        self.nicks = self.arrange_shown_nicks();
        self.nick_cursor = cur.and_then(|c| self.nicks.iter().position(|n| n.nick == c))
            .unwrap_or(self.nick_cursor).min(self.nicks.len().saturating_sub(1));
    }
    /// Re-apply the filter after it changed: the cursor stays on its nick when still shown, else the first match.
    pub fn apply_nick_filter(&mut self) {
        let cur = self.nicks.get(self.nick_cursor).map(|n| n.nick.clone());
        self.nicks = self.arrange_shown_nicks();
        self.nick_cursor = cur.and_then(|c| self.nicks.iter().position(|n| n.nick == c)).unwrap_or(0);
        self.nick_offset = 0;
    }
    fn arrange_shown_nicks(&self) -> Vec<crate::tui::data::NickEntry> {
        crate::tui::order::arrange_nicks(
            crate::tui::data::filter_nicks(&self.all_nicks, &self.nick_filter.value),
            self.nick_order, self.nick_limit,
        )
    }
    /// Start typing into the nick filter (`/` or a click).
    pub fn focus_nick_filter(&mut self) {
        self.close_menu_dropdown();
        self.focus = Focus::Nicks;
        self.filter_editing = true;
    }
    /// Empty the nick filter and stop editing it (`Esc` or the `x`).
    pub fn clear_nick_filter(&mut self) {
        self.filter_editing = false;
        if self.nick_filter.value.is_empty() { return; }
        self.nick_filter.set("");
        self.apply_nick_filter();
    }
    /// Whether the nick list is narrowed by a filter.
    pub fn nick_filter_active(&self) -> bool { !self.nick_filter.value.trim().is_empty() }
    /// Select only the n-th nick.
    pub fn select_only(&mut self, i: usize) {
        let Some(n) = self.nicks.get(i) else { return };
        self.nick_cursor = i;
        self.selected = vec![n.nick.clone()];
        self.selection_changed();
    }
    /// Add / remove the n-th nick from the selection.
    pub fn toggle_nick(&mut self, i: usize) {
        let Some(n) = self.nicks.get(i).map(|n| n.nick.clone()) else { return };
        self.nick_cursor = i;
        if let Some(p) = self.selected.iter().position(|s| *s == n) { self.selected.remove(p); } else { self.selected.push(n); }
        self.selection_changed();
    }
    /// Move the cursor by `d`, adding the passed nicks to the selection (Shift-↑/↓).
    pub fn extend_selection(&mut self, d: i32) {
        if self.nicks.is_empty() { return; }
        let cur = self.nicks[self.nick_cursor].nick.clone();
        if !self.selected.contains(&cur) { self.selected.push(cur); }
        self.move_nick_cursor(d);
        let n = self.nicks[self.nick_cursor].nick.clone();
        if !self.selected.contains(&n) { self.selected.push(n); }
        self.selection_changed();
    }
    /// Select every shown nick (added to the selection; with no filter that is every nick).
    pub fn select_all(&mut self) {
        for n in &self.nicks { if !self.selected.contains(&n.nick) { self.selected.push(n.nick.clone()); } }
        self.selection_changed();
    }
    /// Move the nick cursor.
    pub fn move_nick_cursor(&mut self, d: i32) {
        if self.nicks.is_empty() { return; }
        self.nick_cursor = (self.nick_cursor as i64 + d as i64).clamp(0, self.nicks.len() as i64 - 1) as usize;
    }
    fn selection_changed(&mut self) {
        self.menu.menu.set_text(CommandId::Scrape, "nicks", &self.selected.join(","));
        self.search = None;
        self.load_base();
    }
    /// Reload the nick posts view for the selection.
    pub fn load_base(&mut self) {
        let posts = match self.store.posts_for(&self.selected) {
            Ok(p) => p,
            Err(e) => { self.push_status(format!("error: reading posts: {e:#}")); Vec::new() }
        };
        let who = self.selected.iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join(", ");
        self.base = self.prepared(BodyView { title: who, posts, kind: crate::tui::order::ListKind::Timeline, ..BodyView::default() });
        self.anim = None;
    }

    // ------------------------------------------------------------ bookmarks
    /// Re-read the bookmarks from the store.
    pub fn reload_marks(&mut self) {
        match self.store.bookmarks() {
            Ok(m) => self.marks = m,
            Err(e) => self.push_status(format!("error: reading bookmarks: {e:#}")),
        }
    }
    /// Whether `id` is bookmarked.
    pub fn is_marked(&self, id: &str) -> bool { self.marks.contains(id) }
    /// Show only bookmarked posts and comments (replaces the Body like a search; `<` goes back).
    pub fn open_bookmarks(&mut self) {
        let mut posts: Vec<Post> = self.marks.iter().filter_map(|id| self.store.post(id).ok().flatten()).collect();
        posts.sort_by(|a, b| b.posted_at.cmp(&a.posted_at));
        let missing = self.marks.len() - posts.len();
        if missing > 0 { self.push_status(format!("{missing} bookmarked posts are not in the database (Bookmarks › Sync stores them)")); }
        self.place_search(BodyView { title: "★ Bookmarks".into(), posts, bookmarks: true, kind: crate::tui::order::ListKind::Bookmarks, ..BodyView::default() });
        self.focus = Focus::Body;
    }
    /// Show a saved site read, in the site's order. Returns whether it was stored.
    pub fn show_site_feed(&mut self, key: &str) -> bool {
        match self.store.site_feed(key) {
            Ok(Some(f)) => {
                let posts: Vec<Post> = f.post_ids.iter().filter_map(|id| self.store.post(id).ok().flatten()).collect();
                let missing = f.post_ids.len() - posts.len();
                if missing > 0 { self.push_status(format!("{missing} posts from {} are not in the database", f.title)); }
                self.place_search(BodyView { title: f.title, posts, site_key: Some(key.into()), kind: crate::tui::order::ListKind::Site, site_order: true, ..BodyView::default() });
                self.focus = Focus::Body;
                true
            }
            Ok(None) => false,
            Err(e) => { self.push_status(format!("error: reading {key}: {e:#}")); false }
        }
    }
    /// Show the saved view at once (offline), then refresh it from the site.
    pub fn open_site_read(&mut self, key: &str, argv: Vec<String>) {
        let had = self.show_site_feed(key);
        if self.job.is_some() {
            self.push_status(if had { format!("showing saved {key}; a job is already running") }
                else { "another job is still running; wait for it or cancel it (F6)".into() });
            return;
        }
        self.pending_feed = Some(key.into());
        self.push_status(if had { format!("showing saved {key}; refreshing from the site…") }
            else { format!("fetching {key} from the site…") });
        self.start_online_job(JobKind::Read, argv);
    }
    /// View › Site Top: the period and replies checkbox in the View menu.
    pub fn open_top(&mut self) {
        let replies = self.menu.menu.check(crate::tui::commands::CommandId::View, "top_replies");
        let period = TopPeriod::parse(self.menu.menu.choice(crate::tui::commands::CommandId::View, "top_period"))
            .unwrap_or(TopPeriod::Week);
        match self.menu.menu.top_argv() {
            Ok(argv) => self.open_site_read(&top_key(period, replies), argv),
            Err(e) => self.error("Site Top", &e),
        }
    }
    /// The EB row / View › Executive Board.
    pub fn open_eb(&mut self) {
        let replies = self.menu.menu.check(crate::tui::commands::CommandId::View, "eb_replies");
        match self.menu.menu.eb_argv() {
            Ok(argv) => self.open_site_read(&eb_key(replies), argv),
            Err(e) => self.error("Executive Board", &e),
        }
    }
    /// The Explore row / View › Explore. Latest follows the View checkbox.
    pub fn open_explore(&mut self) {
        let latest = self.menu.menu.check(crate::tui::commands::CommandId::View, "explore_latest");
        match self.menu.menu.explore_argv() {
            Ok(argv) => self.open_site_read(&explore_key(latest), argv),
            Err(e) => self.error("Explore", &e),
        }
    }
    /// `p`: the post menu for the open detail, else the highlighted card.
    pub fn open_post_menu(&mut self) {
        let Some(id) = self.current_post_id() else { self.push_status("no post: open or highlight one first"); return; };
        let thread = self.top().map(|f| f.post.clone())
            .or_else(|| self.view().posts.iter().find(|p| p.post_id == id).cloned());
        let thread_id = thread.map(|p| if p.parent_post_id.is_empty() { p.post_id } else { p.parent_post_id }).unwrap_or_else(|| id.clone());
        self.modal = Some(Modal::Post(PostMenu::new(&id, &thread_id)));
    }
    /// Nicks and tags the compose box can complete without asking the site.
    fn compose_sources(&self) -> (Vec<crate::site::NickHint>, Vec<String>) {
        let nicks = self.nicks.iter().map(|n| crate::site::NickHint { nick: n.nick.clone(), user_id: String::new() }).collect();
        let mut tags = Vec::new();
        for post in &self.view().posts {
            for tag in crate::scrape::extract_tags(&post.body_json, &post.content) {
                if !tags.iter().any(|e: &String| e.eq_ignore_ascii_case(&tag)) { tags.push(tag); }
            }
        }
        (nicks, tags)
    }
    fn writes_are_on(&self) -> bool { crate::site::writes_enabled() }
    fn show_writes_off(&mut self) {
        self.modal = Some(Modal::Message(MessageModal::info(
            "Writes are off",
            "Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins.",
        )));
    }
    /// `n`: compose a new post. Cancel is the default button.
    pub fn open_new_post(&mut self) {
        if !self.writes_are_on() { self.show_writes_off(); return; }
        let (nicks, tags) = self.compose_sources();
        self.modal = Some(Modal::Compose(ComposeModal::new_post(nicks, tags)));
    }
    /// The open or highlighted post, when there is one.
    fn current_post(&self) -> Option<Post> {
        if let Some(f) = self.top() { return Some(f.post.clone()); }
        let v = self.view();
        v.posts.get(v.cursor).cloned()
    }
    /// `r` on an open post: reply. A comment pre-fills `postRef`.
    pub fn open_reply(&mut self) {
        if !self.writes_are_on() { self.show_writes_off(); return; }
        let Some(post) = self.current_post() else { self.push_status("no post: open one to reply"); return; };
        let thread = if post.parent_post_id.is_empty() { post.post_id.clone() } else { post.parent_post_id.clone() };
        let in_reply_to = if post.parent_post_id.is_empty() { None } else { Some(post.post_id.clone()) };
        let (nicks, tags) = self.compose_sources();
        self.modal = Some(Modal::Compose(ComposeModal::reply(&thread, in_reply_to, None, None, nicks, tags)));
    }
    /// `Q` on an open post: reply with the body quoted.
    pub fn open_quote(&mut self) {
        if !self.writes_are_on() { self.show_writes_off(); return; }
        let Some(post) = self.current_post() else { self.push_status("no post: open one to quote"); return; };
        let thread = if post.parent_post_id.is_empty() { post.post_id.clone() } else { post.parent_post_id.clone() };
        let (nicks, tags) = self.compose_sources();
        self.modal = Some(Modal::Compose(ComposeModal::reply(&thread, None, Some(post.content.clone()), Some(post.post_id.clone()), nicks, tags)));
    }
    /// Post menu › View reposts.
    pub fn open_reposts(&mut self, id: &str) {
        match self.menu.menu.reposts_argv(id) {
            Ok(argv) => self.open_site_read(&reposts_key(id), argv),
            Err(e) => self.error("Reposts", &e),
        }
    }
    /// Show stored edit versions of `post_id` as diffs, oldest first. Returns whether any were stored.
    pub fn show_edits(&mut self, post_id: &str, thread_id: &str) -> bool {
        match self.store.post_versions(post_id) {
            Ok(vs) if !vs.is_empty() => {
                let posts = edits_as_posts(&vs, thread_id);
                self.place_search(BodyView {
                    title: format!("Edits · {post_id}"), posts,
                    site_key: Some(edits_key(thread_id, post_id)),
                    kind: crate::tui::order::ListKind::Edits, ..BodyView::default()
                });
                self.focus = Focus::Body;
                true
            }
            Ok(_) => false,
            Err(e) => { self.push_status(format!("error: reading edits of {post_id}: {e:#}")); false }
        }
    }
    /// Post menu › View edits: the saved diffs at once, then a refresh from the site.
    pub fn open_edits(&mut self, post_id: &str, thread_id: &str) {
        let key = edits_key(thread_id, post_id);
        let had = self.show_edits(post_id, thread_id);
        let argv = match self.menu.menu.edits_argv(thread_id, post_id) {
            Ok(v) => v,
            Err(e) => { self.error("Edits", &e); return; }
        };
        if self.job.is_some() {
            self.push_status(if had { format!("showing saved {key}; a job is already running") }
                else { "another job is still running; wait for it or cancel it (F6)".into() });
            return;
        }
        self.pending_feed = Some(key.clone());
        self.push_status(if had { format!("showing saved {key}; refreshing from the site…") }
            else { format!("fetching {key} from the site…") });
        self.start_online_job(JobKind::Read, argv);
    }
    /// `s` in a thread: cycle reply order and, for oldest/top, refresh scores from the site.
    pub fn cycle_thread_sort(&mut self) {
        if self.top().is_none() { self.push_status("open a thread to sort its replies"); return; }
        let next = self.top().map(|f| f.sort.cycle()).unwrap_or_default();
        if let Some(f) = self.top_mut() { f.sort = next; }
        self.apply_reply_sort();
        self.push_status(format!("replies · {}", next.label()));
        let Some(order) = next.hook() else { return };
        let post = self.top().map(|f| f.post.clone());
        let Some(post) = post else { return };
        let thread = if post.parent_post_id.is_empty() { post.post_id.clone() } else { post.parent_post_id.clone() };
        if self.job.is_some() {
            self.push_status("reply order changed locally; a job is already running");
            return;
        }
        match self.menu.menu.sort_thread_argv(&thread, order) {
            Ok(argv) => {
                self.pending_sort = Some((post.post_id.clone(), next));
                self.start_online_job(JobKind::Read, argv);
            }
            Err(e) => self.error("Thread sort", &e),
        }
    }
    /// Put the open detail's reply links in its current order, using stored scores.
    pub fn apply_reply_sort(&mut self) {
        let Some(sort) = self.top().map(|f| f.sort) else { return };
        let id = self.top().unwrap().post.post_id.clone();
        let mut replies = self.store.replies(&id).unwrap_or_default();
        sort_replies(&mut replies, sort);
        let Some(f) = self.top_mut() else { return };
        let mut links: Vec<Link> = f.links.iter().filter(|l| l.kind != LinkKind::Reply).cloned().collect();
        for r in replies {
            links.push(Link { kind: LinkKind::Reply, id: r.post_id.clone(), target: Some(r) });
        }
        if f.link_cursor >= links.len() { f.link_cursor = 0; }
        f.links = links;
    }
    /// After a sort job: reopen `id` if the list refresh closed it, then reapply `sort`.
    fn restore_thread_sort(&mut self, id: &str, sort: ThreadSort) {
        if self.top().is_none() {
            if let Some(i) = self.view().posts.iter().position(|p| p.post_id == id) { self.open_post(i); }
        }
        if self.top().is_some_and(|f| f.post.post_id == id) {
            if let Some(f) = self.top_mut() { f.sort = sort; }
            self.apply_reply_sort();
        }
    }
    /// A `#tag` in a post: the saved tag feed, then a refresh from `/rs/tag/{tag}`.
    pub fn open_tag(&mut self, tag: &str) {
        match self.menu.menu.tag_argv(tag) {
            Ok(argv) => self.open_site_read(&tag_key(tag), argv),
            Err(e) => self.error("Tag", &e),
        }
    }
    /// Search modal › Where: Site. Other fields stay local and are named in the status line.
    pub fn open_site_search(&mut self, s: &crate::tui::modals::SearchModal) {
        let q = s.fields[0].value.trim();
        let local = s.fields.iter().skip(1).any(|f| !f.value.trim().is_empty()) || s.kind != crate::tui::data::KindFilter::Any
            || s.order != ScrapeOrder::Newest;
        if local {
            self.push_status("nicks, dates, post id, kind, order and limit stay local; searching the site for the keyword");
        }
        match self.menu.menu.site_search_argv(q) {
            Ok(argv) => self.open_site_read(&search_key(q), argv),
            Err(e) => self.error("Site search", &e),
        }
    }
    /// The post `b` / the menu toggles: the top detail, else the highlighted card.
    pub fn current_post_id(&self) -> Option<String> {
        if let Some(f) = self.top() { return Some(f.post.post_id.clone()); }
        let v = self.view();
        v.posts.get(v.cursor).map(|p| p.post_id.clone())
    }
    /// Bookmark / unbookmark `id`: shown at once, stored by a side job (works offline).
    pub fn toggle_mark(&mut self, id: &str) {
        let on = !self.marks.contains(id);
        match self.menu.menu.bookmark_argv(id, on) {
            Ok(argv) => {
                if on { self.marks.insert(id.to_string()); } else { self.marks.remove(id); }
                self.push_status(format!("{} {id}", if on { "★ bookmarked" } else { "☆ unbookmarked" }));
                self.effects.push(Effect::RunSideJob(JobKind::Bookmark, argv));
            }
            Err(e) => self.error("Cannot bookmark", &e),
        }
    }
    /// Whether `id` is in the export basket.
    pub fn in_basket(&self, id: &str) -> bool { self.basket.iter().any(|b| b == id) }
    /// Add `id` to the export basket, or take it out.
    pub fn toggle_basket(&mut self, id: &str) {
        if let Some(i) = self.basket.iter().position(|b| b == id) {
            self.basket.remove(i);
            self.push_status(format!("− {id} taken out of the export basket ({} left)", self.basket.len()));
        } else {
            self.basket.push(id.to_string());
            self.push_status(format!("+ {id} added to the export basket ({}) · Export › Export basket writes it", self.basket.len()));
        }
    }
    /// Show the basket's posts (replaces the Body like a search; `<` goes back).
    pub fn open_basket(&mut self) {
        let posts: Vec<Post> = self.basket.iter().filter_map(|id| self.store.post(id).ok().flatten()).collect();
        self.place_search(BodyView { title: format!("+ Export basket ({})", self.basket.len()), posts, kind: crate::tui::order::ListKind::Basket, ..BodyView::default() });
        self.focus = Focus::Body;
    }
    // ------------------------------------------------------------ research
    /// Projects for the modal: (name, items, description).
    fn research_list(&mut self) -> Vec<(String, usize, String)> {
        match self.store.research_projects() {
            Ok(v) => v.into_iter().map(|p| (p.name, p.items, p.description)).collect(),
            Err(e) => { self.push_status(format!("error: reading research projects: {e:#}")); Vec::new() }
        }
    }
    /// The project named in Research › Project, else the last one used.
    pub fn research_name(&self) -> Option<String> {
        let t = self.menu.menu.text(CommandId::Research, "project");
        if !t.trim().is_empty() { return Some(t.trim().to_string()); }
        self.research_current.clone()
    }
    /// What `r` can add from here: the open post (or the highlighted card), the
    /// posts it links, its attachments and web links, and the whole search view.
    pub fn research_candidates(&self) -> Vec<(String, ResearchSource)> {
        let mut v: Vec<(String, ResearchSource)> = Vec::new();
        let kind = |p: &Post| if p.parent_post_id.is_empty() { "post" } else { "reply" };
        let in_search = self.search.as_ref().is_some_and(|s| s.filter.is_some());
        let label = |p: &Post| format!("This {} {} · @{}", if in_search { "search result" } else { kind(p) }, p.post_id, p.nick);
        if let Some(f) = self.top() {
            if !f.post.post_id.starts_with("research:") { v.push((label(&f.post), ResearchSource::Posts(vec![f.post.post_id.clone()]))); }
            let mut seen: HashSet<String> = HashSet::new();
            for l in &f.links {
                match l.kind {
                    LinkKind::Attachment => {
                        let p = Path::new(&l.id);
                        if p.exists() && seen.insert(l.id.clone()) {
                            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                            v.push((format!("Attachment file {name}"), ResearchSource::File(l.id.clone())));
                        }
                    }
                    _ => if let Some(t) = &l.target && seen.insert(t.post_id.clone()) && t.post_id != f.post.post_id {
                        let what = match l.kind { LinkKind::ReplyTo => "Replied-to", LinkKind::Thread => "Thread", LinkKind::Reply => "Reply", _ => "Linked" };
                        v.push((format!("{what} {} {} · @{}", kind(t), t.post_id, t.nick), ResearchSource::Posts(vec![t.post_id.clone()])));
                    }
                }
            }
            for u in web_links(&f.post.content) {
                if seen.insert(u.clone()) { v.push((format!("Web page {u}"), ResearchSource::Url(u))); }
            }
        } else {
            let view = self.view();
            if let Some(p) = view.posts.get(view.cursor).filter(|p| !p.post_id.starts_with("research:")) {
                v.push((label(p), ResearchSource::Posts(vec![p.post_id.clone()])));
            }
        }
        if let Some(all) = self.view_source() { v.push(all); }
        v
    }
    /// Every post of the current search / view as one source.
    fn view_source(&self) -> Option<(String, ResearchSource)> {
        let view = self.view();
        if view.research.is_some() { return None; }
        let ids: Vec<String> = view.posts.iter().map(|p| p.post_id.clone()).filter(|id| !id.starts_with("research:")).collect();
        if ids.is_empty() { return None; }
        match &view.filter {
            Some(f) if !f.keyword.trim().is_empty() && f.post_id.trim().is_empty() && f.from.trim().is_empty() && f.to.trim().is_empty()
                && f.kind == crate::tui::data::KindFilter::Any =>
                Some((format!("All {} results of the search {:?}", ids.len(), f.keyword.trim()),
                    ResearchSource::Search { query: f.keyword.trim().to_string(), nicks: f.nicks.clone() })),
            _ => Some((format!("All {} posts in this view ({})", ids.len(), view.title), ResearchSource::Posts(ids))),
        }
    }
    /// Open the add-to-research modal for `what`.
    pub fn open_research_add(&mut self, what: Vec<(String, ResearchSource)>) {
        self.close_menu();
        if what.is_empty() { self.push_status("nothing to add to research here: open or highlight a post first"); return; }
        let projects = self.research_list();
        let cur = self.research_name();
        self.modal = Some(Modal::Research(ResearchModal::add(what, projects, cur.as_deref())));
    }
    /// Show research project `name` as a Body view (`<` goes back).
    pub fn open_research(&mut self, name: &str) {
        self.open_research_keep(name);
        self.focus = Focus::Body;
        self.anim = None;
    }
    fn open_research_keep(&mut self, name: &str) {
        let items = match self.store.research_items(name) {
            Ok(v) => v,
            Err(e) => { self.push_status(format!("error: reading research {name}: {e:#}")); return; }
        };
        let mut posts = Vec::new();
        for it in &items {
            if it.kind.is_post() && let Ok(Some(p)) = self.store.post(&it.r#ref) { posts.push(p); continue; }
            posts.push(research_post(it));
        }
        let keep = self.search.as_ref().filter(|v| v.research.as_deref() == Some(name))
            .and_then(|v| v.posts.get(v.cursor).map(|p| p.post_id.clone()));
        let stack = self.search.as_mut().filter(|v| v.research.as_deref() == Some(name)).map(|v| std::mem::take(&mut v.stack)).unwrap_or_default();
        self.place_search(BodyView { title: format!("◈ Research: {name} ({} items)", items.len()), posts, stack,
            research: Some(name.to_string()), kind: crate::tui::order::ListKind::Research, ..BodyView::default() });
        if let Some(id) = keep && let Some(v) = self.search.as_mut() && let Some(i) = v.posts.iter().position(|p| p.post_id == id) {
            v.cursor = i;
            if v.offset > i { v.offset = i; }
        }
        self.research_current = Some(name.to_string());
    }
    /// A research modal closed with a choice.
    fn research_chosen(&mut self, m: ResearchModal) {
        let Some((name, desc, new)) = m.chosen.clone() else { return };
        self.research_current = Some(name.clone());
        self.menu.menu.set_text(CommandId::Research, "project", &name);
        match m.mode {
            ResearchMode::Add => {
                let Some(src) = m.source() else { return };
                match crate::tui::commands::research_add_argv(&name, if new { &desc } else { "" }, src, &m.note.value) {
                    Ok(argv) => {
                        let what = m.what.get(m.what_idx).map(|w| w.0.clone()).unwrap_or_default();
                        self.push_status(format!("◈ adding to research {name}: {what}{}", if new { " (new project)" } else { "" }));
                        self.effects.push(Effect::RunSideJob(JobKind::Research, argv));
                    }
                    Err(e) => self.error("Cannot add to research", &e),
                }
            }
            ResearchMode::Browse => {
                if new {
                    match crate::tui::commands::research_create_argv(&name, &desc) {
                        Ok(argv) => { self.push_status(format!("◈ creating research project {name}")); self.effects.push(Effect::RunSideJob(JobKind::Research, argv)); }
                        Err(e) => { self.error("Cannot create the project", &e); return; }
                    }
                }
                self.open_research(&name);
            }
        }
    }

    /// A side job ended; `pending` = side jobs still running.
    pub fn side_job_finished(&mut self, pid: u32, ok: bool, code: Option<i32>, tail: &[String], pending: usize) {
        self.settle_job(pid, ok, code, tail);
        let shown = tail.iter().rev().find(|l| !l.trim().is_empty()).map(|l| crate::tui::modals::job::redact_line(l)).unwrap_or_else(|| "?".into());
        if !ok { self.push_status(format!("error: change not saved: {shown}")); }
        else if let Some(l) = tail.iter().rev().find_map(|l| l.find("research \"").map(|i| l[i..].to_string())) { self.push_status(format!("◈ {}", crate::tui::modals::job::redact_line(&l))); }
        // Re-read only once every write landed, so quick toggles don't flicker back.
        if pending == 0 {
            self.reload_marks();
            if let Some(name) = self.search.as_ref().and_then(|v| v.research.clone()) { self.open_research_keep(&name); }
        }
    }
    /// Run `argv` (a network job) after making sure we're logged in.
    fn start_online_job(&mut self, kind: JobKind, argv: Vec<String>) {
        if self.job.is_some() { self.error("Busy", "another job is still running; wait for it or cancel it (F6)"); return; }
        self.close_menu();
        if self.session == SessionState::Valid { self.effects.push(Effect::RunJob(kind, argv)); }
        else { self.session = SessionState::Checking; self.effects.push(Effect::CheckSessionThen(kind, argv)); }
    }

    // ------------------------------------------------------------ sync
    /// Start an automatic sync when Sync › Auto sync is on and one is due. Skips (and
    /// reschedules) while a job runs or when logged out; never opens a modal.
    pub fn sync_tick(&mut self, now: Instant) {
        if !self.menu.menu.check(CommandId::Sync, "auto") { self.sync_next = None; return; }
        if self.sync_next.is_some_and(|t| now < t) { return; }
        let every = match self.menu.menu.sync_interval() {
            Ok(d) => d,
            Err(e) => { self.menu.menu.set_check(CommandId::Sync, "auto", false); self.push_status(format!("auto sync off: {e}")); return; }
        };
        self.sync_next = Some(now + every);
        if self.job.is_some() { self.push_status("auto sync skipped: a job is running"); return; }
        if self.session == SessionState::LoggedOut { self.push_status("auto sync skipped: logged out (Account › Log in)"); return; }
        match self.menu.menu.sync_argv(&self.selected.clone()) {
            Ok(argv) => { self.push_status("auto sync…"); self.effects.push(Effect::AutoSync(argv)); }
            Err(e) => self.push_status(format!("auto sync skipped: {e}")),
        }
    }
    /// The quiet session check of an automatic sync came back.
    pub fn auto_sync_checked(&mut self, r: std::result::Result<bool, String>, argv: Vec<String>) {
        match r {
            Ok(true) if self.job.is_none() => { self.session = SessionState::Valid; self.auto_running = true; self.effects.push(Effect::RunJob(JobKind::Sync, argv)); }
            Ok(true) => self.push_status("auto sync skipped: a job is running"),
            Ok(false) => { self.session = SessionState::LoggedOut; self.push_status("auto sync skipped: logged out (Account › Log in)"); }
            Err(e) => self.push_status(format!("auto sync skipped: offline ({})", e.lines().next().unwrap_or(""))),
        }
    }

    // ------------------------------------------------------------ view modes
    /// Switch the Body layout (keeps the selection and any open detail).
    pub fn set_view_mode(&mut self, m: ViewMode) {
        self.view_mode = m;
        self.reflow_lists();
        self.anim = None;
        self.push_status(format!("view: {}", m.label()));
    }
    /// Move to the previous / next post in the current layout: carousels and the
    /// grid go side to side (the infinite carousel wraps around).
    pub fn step_post(&mut self, d: i32) {
        let n = self.view().posts.len();
        if n == 0 { return; }
        if self.view_mode == ViewMode::CarouselInfinite {
            let v = self.view_mut();
            v.cursor = cycle(v.cursor, n, d);
            v.offset = v.cursor;
        } else {
            self.move_post_cursor(d);
        }
    }
    /// Stack + pane: show the highlighted post in the pane (replacing what it showed).
    pub fn preview_cursor(&mut self) {
        if self.view_mode == ViewMode::Pane { let i = self.view().cursor; self.open_post(i); self.anim = None; }
    }
    /// Close every open detail (the Grid modal's Close); next reopens them one by one.
    pub fn close_details(&mut self) { self.pop_to_list(); }
    fn pop_to_list(&mut self) {
        let v = self.view_mut();
        while let Some(f) = v.stack.pop() { v.forward.push(f); }
        self.anim = None;
    }

    // ------------------------------------------------------------ body views
    /// The view on screen (search results replace the nick posts in place).
    pub fn view(&self) -> &BodyView { self.search.as_ref().unwrap_or(&self.base) }
    /// Mutable [`App::view`].
    pub fn view_mut(&mut self) -> &mut BodyView { self.search.as_mut().unwrap_or(&mut self.base) }
    /// Replace the Body with the results of `f`.
    ///
    /// The exact cosmos phrase flashes a short starfield first. The results are already in place underneath it.
    pub fn run_search(&mut self, f: SearchFilter) {
        if crate::eggs::is_cosmos_query(&f.keyword) { self.show_egg(crate::eggs::EggKind::Cosmos); }
        self.show_search(f);
    }
    fn show_search(&mut self, f: SearchFilter) {
        if f.is_empty() { self.clear_search(); return; }
        match self.store.search(&f) {
            Ok(posts) => {
                let total = posts.len();
                let order = f.order;
                let limited = f.limit.is_some();
                let label = format!("search {}", f.describe());
                self.orders.set(crate::tui::order::ListKind::Search, order);
                self.sync_search_menu(&f);
                self.place_search(BodyView {
                    title: format!("Search: {}", f.describe()), posts, filter: Some(f),
                    kind: crate::tui::order::ListKind::Search, ..BodyView::default()
                });
                self.focus = Focus::Body;
                let shown = self.view().posts.len();
                self.push_status(crate::tui::order::result_status(&label, shown, total, order, limited));
                self.persist_list_order();
            }
            Err(e) => self.error("Search failed", &format!("{e:#}")),
        }
    }
    /// Back from search results to the nick posts (`<`).
    pub fn search_back(&mut self) -> bool { self.search.take().is_some() }
    /// Clear the search: back, and empty the Search menu fields.
    pub fn clear_search(&mut self) {
        self.search = None;
        self.menu.menu.set_text(CommandId::Search, "query", "");
        self.menu.menu.set_text(CommandId::Search, "nick", "");
        self.menu.menu.set_text(CommandId::Search, "limit", "");
        self.menu.menu.set_choice(CommandId::Search, "order", crate::tui::order::order_choice(self.orders.search));
    }
    /// Current search filter (for pre-filling the modal). Order comes from the search list when no search is open.
    pub fn current_filter(&self) -> SearchFilter {
        self.search.as_ref().and_then(|v| v.filter.clone()).unwrap_or(SearchFilter { order: self.orders.search, ..SearchFilter::default() })
    }

    /// Move `posts` into `source` and lay the view out. A view with an empty source is left unchanged.
    fn prepared(&self, mut view: BodyView) -> BodyView {
        if view.source.is_empty() { view.source = std::mem::take(&mut view.posts); }
        reflow_in(&mut view, self.view_mode, self.orders);
        view
    }
    /// Show `view` in place of the Body.
    fn place_search(&mut self, view: BodyView) {
        self.search = Some(self.prepared(view));
        self.anim = None;
    }
    /// Re-lay every list whose source is set. Nested and flat group posts differently.
    fn reflow_lists(&mut self) {
        let mode = self.view_mode;
        let orders = self.orders;
        reflow_in(&mut self.base, mode, orders);
        if let Some(v) = self.search.as_mut() { reflow_in(v, mode, orders); }
    }
    /// Copy the search filter's order and limit into the Search menu.
    fn sync_search_menu(&mut self, f: &SearchFilter) {
        self.menu.menu.set_choice(CommandId::Search, "order", crate::tui::order::order_choice(f.order));
        self.menu.menu.set_text(CommandId::Search, "limit", &f.limit.map(|n| n.to_string()).unwrap_or_default());
    }
    /// Apply `[ui.list_order]` and re-read the nick bar (and the open nick timeline).
    pub fn apply_list_prefs(&mut self, p: Option<&crate::auth::prefs::ListOrderPrefs>) {
        let Some(p) = p else { return };
        self.orders.apply_prefs(p);
        self.nick_order = p.nicks.as_deref().and_then(|s| ScrapeOrder::parse(s).ok()).unwrap_or_default();
        self.nick_limit = p.nick_limit.and_then(|n| usize::try_from(n).ok());
        self.sync_order_menus();
        self.refresh_nicks();
        if !self.selected.is_empty() { self.load_base(); }
        self.reflow_lists();
    }
    fn sync_order_menus(&mut self) {
        self.menu.menu.set_choice(CommandId::View, "nick_order", crate::tui::order::order_choice(self.nick_order));
        self.menu.menu.set_text(CommandId::View, "nick_limit", &self.nick_limit.map(|n| n.to_string()).unwrap_or_default());
        self.menu.menu.set_choice(CommandId::Search, "order", crate::tui::order::order_choice(self.orders.search));
        self.menu.menu.set_choice(CommandId::View, "color", color_choice(self.color_mode));
        let kind = self.view().kind;
        if kind != crate::tui::order::ListKind::Edits {
            self.menu.menu.set_choice(CommandId::View, "list_order", crate::tui::order::order_choice(self.orders.get(kind)));
        }
    }
    /// Write `[ui.list_order]` when the TUI has a real preferences file (not a unit test).
    fn persist_list_order(&mut self) {
        if self.themes_dir.is_none() { return; }
        let prefs = self.orders.to_prefs(self.nick_order, self.nick_limit.map(|n| n as u64));
        if let Err(e) = crate::auth::prefs::save_list_order(&prefs) {
            self.push_status(format!("error: could not save list order: {e:#}"));
        }
    }
    /// Toggle the nick bar's order.
    pub fn toggle_nick_order(&mut self) {
        self.nick_order = match self.nick_order { ScrapeOrder::Newest => ScrapeOrder::Oldest, ScrapeOrder::Oldest => ScrapeOrder::Newest };
        self.menu.menu.set_choice(CommandId::View, "nick_order", crate::tui::order::order_choice(self.nick_order));
        self.apply_nick_filter();
        self.persist_list_order();
        let shown = self.nicks.len();
        let total = crate::tui::data::filter_nicks(&self.all_nicks, &self.nick_filter.value).len();
        self.push_status(format!("{}. Help: nick-order", crate::tui::order::result_status(&nick_label(&self.nick_filter.value), shown, total, self.nick_order, self.nick_limit.is_some())));
    }
    /// Toggle the list on screen. Edit history stays in version order.
    pub fn toggle_list_order(&mut self) {
        let kind = self.view().kind;
        if kind == crate::tui::order::ListKind::Edits {
            self.push_status("edit history stays in version order. Help: thread-order");
            return;
        }
        let next = match self.orders.get(kind) { ScrapeOrder::Newest => ScrapeOrder::Oldest, ScrapeOrder::Oldest => ScrapeOrder::Newest };
        self.set_list_order(kind, next);
    }
    fn set_list_order(&mut self, kind: crate::tui::order::ListKind, order: ScrapeOrder) {
        if kind == crate::tui::order::ListKind::Edits {
            self.push_status("edit history stays in version order. Help: thread-order");
            return;
        }
        self.orders.set(kind, order);
        if kind == crate::tui::order::ListKind::Search && let Some(f) = self.search.as_mut().and_then(|v| v.filter.as_mut()) { f.order = order; }
        self.menu.menu.set_choice(CommandId::View, "list_order", crate::tui::order::order_choice(order));
        if kind == crate::tui::order::ListKind::Search {
            self.menu.menu.set_choice(CommandId::Search, "order", crate::tui::order::order_choice(order));
        }
        self.reflow_lists();
        self.persist_list_order();
        let site = self.view().site_order;
        let meaning = crate::tui::order::order_meaning(self.view_mode == ViewMode::Nested, site, false);
        let topic = if kind == crate::tui::order::ListKind::Search { "search-order" } else { "order" };
        self.push_status(format!("order: {order} ({meaning}). Help: {topic}"));
    }
    /// Apply the highlighted menu choice when it is an order control.
    pub fn apply_open_choice(&mut self) {
        use CommandId::{Search, View};
        let Some(item) = self.menu.open else { return };
        let id = self.menu.menu.items[item].id;
        let Some(key) = self.menu.menu.items[item].opts.get(self.menu.opt).map(|o| o.key) else { return };
        match (id, key) {
            (View, "list_order") => {
                let order = crate::tui::order::order_from_choice(self.menu.menu.choice(View, "list_order"));
                let kind = self.view().kind;
                self.set_list_order(kind, order);
            }
            (View, "nick_order") => {
                let order = crate::tui::order::order_from_choice(self.menu.menu.choice(View, "nick_order"));
                if self.nick_order != order {
                    self.nick_order = order;
                    self.apply_nick_filter();
                    self.persist_list_order();
                    let shown = self.nicks.len();
                    let total = crate::tui::data::filter_nicks(&self.all_nicks, &self.nick_filter.value).len();
                    self.push_status(format!("{}. Help: nick-order", crate::tui::order::result_status(&nick_label(&self.nick_filter.value), shown, total, self.nick_order, self.nick_limit.is_some())));
                }
            }
            (View, "color") => {
                let mode = color_from_choice(self.menu.menu.choice(View, "color"));
                if self.color_mode != mode {
                    self.color_mode = mode;
                    apply_color_mode(mode);
                    self.persist_color_mode();
                    if self.color_hint().is_some() { self.note_no_color(); }
                    else { self.push_status(format!("colour: {mode}. Help: color")); }
                }
            }
            (Search, "order") => {
                let order = crate::tui::order::order_from_choice(self.menu.menu.choice(Search, "order"));
                if self.view().kind == crate::tui::order::ListKind::Search {
                    self.set_list_order(crate::tui::order::ListKind::Search, order);
                } else {
                    self.orders.set(crate::tui::order::ListKind::Search, order);
                    self.persist_list_order();
                    self.push_status(format!("search order: {order}. Help: search-order"));
                }
            }
            _ => {}
        }
    }
    /// Apply a menu text field when editing ends (nick limit, search limit).
    pub fn finish_menu_text(&mut self) {
        use CommandId::{Search, View};
        let Some(item) = self.menu.open else { return };
        let id = self.menu.menu.items[item].id;
        let Some(key) = self.menu.menu.items[item].opts.get(self.menu.opt).map(|o| o.key) else { return };
        match (id, key) {
            (View, "nick_limit") => self.apply_nick_limit_text(),
            (Search, "limit") => self.apply_search_limit_text(),
            _ => {}
        }
    }
    fn apply_nick_limit_text(&mut self) {
        let text = self.menu.menu.text(CommandId::View, "nick_limit");
        let limit = if text.is_empty() { None } else {
            match parse_post_limit(&text) { Ok(n) => Some(n), Err(e) => { self.error("Nick limit", &e); return; } }
        };
        self.nick_limit = limit;
        self.apply_nick_filter();
        self.persist_list_order();
        let shown = self.nicks.len();
        let total = crate::tui::data::filter_nicks(&self.all_nicks, &self.nick_filter.value).len();
        self.push_status(format!("{}. Help: nick-limit", crate::tui::order::result_status(&nick_label(&self.nick_filter.value), shown, total, self.nick_order, limit.is_some())));
    }
    fn apply_search_limit_text(&mut self) {
        let text = self.menu.menu.text(CommandId::Search, "limit");
        let limit = if text.is_empty() { None } else {
            match parse_post_limit(&text) { Ok(n) => Some(n), Err(e) => { self.error("Limit", &e); return; } }
        };
        let (order, describe) = {
            let Some(f) = self.search.as_mut().and_then(|v| v.filter.as_mut()) else {
                self.push_status(match limit { Some(n) => format!("search limit: {n}. Help: search-limit"), None => "search limit: none. Help: search-limit".into() });
                return;
            };
            f.limit = limit;
            (f.order, f.describe())
        };
        let total = self.search.as_ref().map(|v| v.source.len()).unwrap_or(0);
        self.reflow_lists();
        let shown = self.view().posts.len();
        self.push_status(format!("{}. Help: search-limit", crate::tui::order::result_status(&format!("search {describe}"), shown, total, order, limit.is_some())));
    }
    /// Help scrolled to `topic`, or the general Help.
    pub fn open_help(&mut self, topic: Option<&str>) {
        self.close_menu();
        let h = match topic { Some(t) if !t.is_empty() => crate::tui::modals::HelpModal::topic(t), _ => crate::tui::modals::HelpModal::new() };
        self.modal = Some(Modal::Help(h));
    }
    /// Write `[ui] color` when the TUI has a real preferences file (not a unit test).
    fn persist_color_mode(&mut self) {
        if self.themes_dir.is_none() { return; }
        if let Err(e) = crate::auth::prefs::save_ui_color(self.color_mode.as_str()) {
            self.push_status(format!("error: could not save colour: {e:#}"));
        }
    }
    /// The NO_COLOR hint, when colours are off because of it.
    pub fn color_hint(&self) -> Option<&'static str> { no_color_hint(self.color_mode, self.no_color) }
    /// Say so on the status line. Skips a repeat of the same line.
    pub fn note_no_color(&mut self) {
        let Some(h) = self.color_hint() else { return };
        if self.status.back().is_some_and(|s| s == h) { return; }
        self.push_status(h);
    }

    /// The highlighted menu option's Help topic, when it has one.
    pub fn menu_help_topic(&self) -> Option<&'static str> {
        let i = self.menu.open?;
        let help = self.menu.menu.items.get(i)?.opts.get(self.menu.opt)?.help;
        if help.is_empty() { None } else { Some(help) }
    }

    /// Move the post cursor by `d` (detail closed).
    pub fn move_post_cursor(&mut self, d: i32) {
        let rows = self.page_rows.max(1);
        let v = self.view_mut();
        if v.posts.is_empty() { return; }
        v.cursor = (v.cursor as i64 + d as i64).clamp(0, v.posts.len() as i64 - 1) as usize;
        if v.cursor < v.offset { v.offset = v.cursor; }
        if v.cursor >= v.offset + rows { v.offset = v.cursor + 1 - rows; }
    }
    /// Scroll the post list without moving the cursor past the screen.
    pub fn scroll_posts(&mut self, d: i32) {
        let rows = self.page_rows.max(1);
        let v = self.view_mut();
        let max = v.posts.len().saturating_sub(1) as i64;
        v.offset = (v.offset as i64 + d as i64).clamp(0, max) as usize;
        v.cursor = v.cursor.clamp(v.offset, (v.offset + rows - 1).min(max as usize));
    }

    // ------------------------------------------------------------ detail stack
    /// Build a detail frame for `post` (attachments, links resolved against the store).
    pub fn frame_for(&self, post: Post) -> DetailFrame {
        let s = &self.store;
        let mut links = Vec::new();
        let mut seen: HashSet<String> = HashSet::from([post.post_id.clone()]);
        let mut add = |kind, id: &str, links: &mut Vec<Link>| {
            if id.is_empty() || !seen.insert(id.to_string()) { return; }
            links.push(Link { kind, id: id.into(), target: s.post(id).ok().flatten() });
        };
        let direct = if post.in_reply_to_id.is_empty() { post.parent_post_id.clone() } else { post.in_reply_to_id.clone() };
        add(LinkKind::ReplyTo, &direct, &mut links);
        add(LinkKind::Thread, &post.parent_post_id, &mut links);
        for id in crate::tui::data::linked_ids(&post.content) { add(LinkKind::Mentioned, &id, &mut links); }
        let mut reply_posts = s.replies(&post.post_id).unwrap_or_default();
        sort_replies(&mut reply_posts, ThreadSort::Newest);
        for r in reply_posts {
            if seen.insert(r.post_id.clone()) { links.push(Link { kind: LinkKind::Reply, id: r.post_id.clone(), target: Some(r) }); }
        }
        let attachments = s.attachments(&post.post_id).unwrap_or_default();
        for a in &attachments {
            links.push(Link { kind: LinkKind::Attachment, id: a.local_path.display().to_string(), target: None });
        }
        for tag in extract_tags(&post.body_json, &post.content) {
            links.push(Link { kind: LinkKind::Tag, id: tag, target: None });
        }
        DetailFrame { attachments, post, links,
            link_cursor: 0, scroll: 0, follow_link: false, notice: None, sort: ThreadSort::Newest }
    }
    /// Open the n-th post of the view as the root detail (replacing any open stack).
    pub fn open_post(&mut self, i: usize) {
        let Some(p) = self.view().posts.get(i).cloned() else { return };
        let f = self.frame_for(p);
        let v = self.view_mut();
        v.cursor = i;
        v.stack = vec![f];
        v.forward.clear();
        self.focus = Focus::Body;
        self.anim = Some(Anim { kind: AnimKind::Open, start: Instant::now() });
    }
    /// Top frame.
    pub fn top(&self) -> Option<&DetailFrame> { self.view().stack.last() }
    /// Mutable top frame.
    pub fn top_mut(&mut self) -> Option<&mut DetailFrame> { self.view_mut().stack.last_mut() }
    /// Open the n-th link of the top frame on top of the stack.
    pub fn open_link(&mut self, i: usize) {
        let depth = self.view().stack.len();
        let Some(link) = self.top().and_then(|f| f.links.get(i)).cloned() else { return };
        if let Some(f) = self.top_mut() { f.link_cursor = i; }
        if link.kind == LinkKind::Tag {
            let tag = link.id.clone();
            self.open_tag(&tag);
            return;
        }
        if link.kind == LinkKind::Attachment {
            let p = PathBuf::from(&link.id);
            if p.exists() { self.push_status(format!("opening {}", p.display())); self.effects.push(Effect::OpenFile(p)); }
            else {
                if let Some(f) = self.top_mut() { f.notice = Some(format!("{} is missing (re-scrape to download it)", link.id)); }
                self.push_status(format!("{} is missing", link.id));
            }
            return;
        }
        let Some(target) = link.target else {
            if let Some(f) = self.top_mut() { f.notice = Some(format!("{} is not in the database", link.id)); }
            self.push_status(format!("{} is not in the database (scrape its thread to read it)", link.id));
            return;
        };
        let max = if self.view_mode == ViewMode::Nested { MAX_DETAIL_DEPTH } else { MAX_FLAT_DEPTH };
        if depth >= max {
            self.push_status(format!("at most {max} open posts here: press Esc to close one first"));
            let what = if max == MAX_DETAIL_DEPTH { "nesting" } else { "depth" };
            if let Some(f) = self.top_mut() { f.notice = Some(format!("{what} limit ({max}) reached")); }
            return;
        }
        let f = self.frame_for(target);
        let v = self.view_mut();
        v.stack.push(f);
        v.forward.clear();
        self.anim = Some(Anim { kind: AnimKind::Push, start: Instant::now() });
    }
    /// Close the top frame; returns whether one was open.
    pub fn pop_detail(&mut self) -> bool {
        let v = self.view_mut();
        let r = match v.stack.pop() { Some(f) => { v.forward.push(f); true } None => false };
        self.anim = None;
        r
    }
    /// Keep frames `0..=depth` (click on a collapsed header); the closed ones can be reopened with next.
    pub fn pop_to(&mut self, depth: usize) {
        let v = self.view_mut();
        while v.stack.len() > depth + 1 { if let Some(f) = v.stack.pop() { v.forward.push(f); } }
        self.anim = None;
    }
    /// Previous post of the detail stack (like Esc: closes the top one, remembering it).
    pub fn stack_prev(&mut self) -> bool { self.pop_detail() }
    /// Next post of the detail stack: reopen the last one closed with prev.
    pub fn stack_next(&mut self) -> bool {
        let v = self.view_mut();
        let Some(f) = v.forward.pop() else { return false };
        if v.stack.is_empty() && let Some(i) = v.posts.iter().position(|p| p.post_id == f.post.post_id) { v.cursor = i; }
        v.stack.push(f);
        self.focus = Focus::Body;
        self.anim = None;
        true
    }
    /// Position in the stack history: (open frames, open + reopenable).
    pub fn stack_pos(&self) -> (usize, usize) { let v = self.view(); (v.stack.len(), v.stack.len() + v.forward.len()) }
    /// Move the top frame's link cursor.
    pub fn move_link(&mut self, d: i32) {
        if let Some(f) = self.top_mut() && !f.links.is_empty() {
            f.link_cursor = cycle(f.link_cursor, f.links.len(), d);
            f.follow_link = true;
        }
    }
    /// Scroll the top frame.
    pub fn scroll_detail(&mut self, d: i32) {
        if let Some(f) = self.top_mut() { f.scroll = (f.scroll as i32 + d).max(0) as u16; f.follow_link = false; }
    }
    /// Advance / finish the animation.
    pub fn tick(&mut self, now: Instant) {
        if self.anim.is_some_and(|a| a.done(now)) { self.anim = None; }
        if self.egg.as_ref().is_some_and(|e| e.finished(now)) { self.egg = None; }
        self.notices.tick(now);
    }
    /// A text field is taking keys, so the star burst must not start.
    pub fn text_input_focused(&self) -> bool {
        if self.filter_editing || self.menu.editing || self.theme_editor.is_some() { return true; }
        match &self.modal {
            Some(Modal::Login(_)) | Some(Modal::Palette(_)) => true,
            Some(Modal::Search(s)) => SearchModal::text_index(s.focus).is_some(),
            Some(Modal::Compose(c)) => matches!(c.focus, ComposeFocus::Title | ComposeFocus::Body | ComposeFocus::Attach | ComposeFocus::Complete),
            Some(Modal::Research(r)) => matches!(r.focus, RField::Name | RField::Desc | RField::Note),
            Some(Modal::Confirm(c)) if c.strict => true,
            _ => false,
        }
    }
    /// Start a clip. A clip already playing is left as it is.
    pub fn show_egg(&mut self, kind: crate::eggs::EggKind) {
        if self.egg.is_none() {
            let mut play = crate::eggs::EggPlay::start(kind);
            play.plain = colors_suppressed(self.color_mode, self.no_color);
            self.egg = Some(play);
        }
    }
    /// Drop the clip. The key or click that skips it is not used for anything else.
    pub fn dismiss_egg(&mut self) { self.egg = None; }
    /// One press on the header star.
    pub fn tap_logo(&mut self, now: Instant) {
        if crate::eggs::note_tap(&mut self.logo_taps, &mut self.logo_tap_at, now) { self.open_credits(); }
    }
    /// The public About page.
    pub fn open_about(&mut self) {
        self.close_menu();
        self.modal = Some(Modal::About(AboutModal::page()));
    }
    /// The hidden credits.
    pub fn open_credits(&mut self) {
        self.close_menu();
        self.modal = Some(Modal::About(AboutModal::credits()));
    }
    /// Animation progress (1.0 when none runs).
    pub fn anim_progress(&self, now: Instant) -> (Option<AnimKind>, f32) {
        match self.anim { Some(a) => (Some(a.kind), a.progress(now)), None => (None, 1.0) }
    }

    // ------------------------------------------------------------ actions
    /// Run a menu / key / palette action.
    pub fn act(&mut self, a: Action) {
        let sel = self.selected.clone();
        match a {
            Action::StartScan => {
                if self.job.is_some() { self.effects.push(Effect::CancelJob); return; }
                match self.menu.menu.scrape_argv(&sel) {
                    Err(e) => self.error("Cannot start scanning", &e),
                    Ok(argv) => self.start_online_job(JobKind::Scan, argv),
                }
            }
            Action::CancelJob => if self.job.is_some() { self.effects.push(Effect::CancelJob); },
            Action::RunSearch => match self.menu.menu.search_filter() {
                Ok(f) => { self.close_menu(); self.run_search(f); }
                Err(e) => self.error("Search", &e),
            },
            Action::OpenSearchModal => { self.close_menu(); self.modal = Some(Modal::Search(SearchModal::new(&self.current_filter()))); }
            Action::ClearSearch => { self.close_menu(); self.clear_search(); }
            Action::RunExport => {
                let view = self.search.as_ref().and_then(|v| v.filter.clone());
                match self.menu.menu.export_argv_with(&sel, view.as_ref(), &self.basket, None) {
                    Ok(argv) => { self.close_menu(); self.start_job(JobKind::Export, argv); }
                    Err(e) => self.error("Cannot export", &e),
                }
            }
            Action::RunImport => match self.menu.menu.import_argv() {
                Ok(argv) => { self.close_menu(); self.start_job(JobKind::Import, argv); }
                Err(e) => self.error("Cannot import", &e),
            },
            Action::RefreshNicks => { self.close_menu(); self.refresh_nicks(); self.load_base_keep(); self.push_status(format!("{} nicks", self.nicks.len())); }
            Action::RemoveNicks => {
                let m = self.menu.menu.clone();
                match (m.remove_nicks(&sel), m.remove_argv(&sel)) {
                    (Ok(n), Ok(argv)) => {
                        self.close_menu();
                        let skip = m.check(CommandId::Maintain, "yes") || m.check(CommandId::Maintain, "dry_run");
                        if skip { self.start_job(JobKind::Maintain, argv); return; }
                        let who = n.iter().map(|n| format!("@{n}")).collect::<Vec<_>>().join(", ");
                        let mut lines = vec![format!("Delete {who}: their posts, replies in their threads, attachments, mentions and profiles?")];
                        if m.check(CommandId::Maintain, "keep_replies") { lines.push("Other people's replies in their threads are kept.".into()); }
                        lines.push("Tick Maintain › Dry run first to preview the counts.".into());
                        self.modal = Some(Modal::Confirm(ConfirmModal::new("Remove nicks", lines, "Remove", ConfirmAction::Job(JobKind::Maintain, argv))));
                    }
                    (Err(e), _) | (_, Err(e)) => self.error("Cannot remove", &e),
                }
            }
            Action::ClearAll => match self.menu.menu.clear_all_argv() {
                Ok(argv) => {
                    self.close_menu();
                    let m = &self.menu.menu;
                    if m.check(CommandId::Maintain, "dry_run") || m.check(CommandId::Maintain, "yes") { self.start_job(JobKind::Maintain, argv); return; }
                    let mut c = ConfirmModal::new("Clear all", vec![
                        "Delete the whole database and every downloaded attachment?".into(),
                        "preferences.toml, the saved session and exports are kept.".into(),
                        "Type yes to confirm.".into()], "Delete everything", ConfirmAction::Job(JobKind::Maintain, argv));
                    c.strict = true;
                    c.focus = 0;
                    self.modal = Some(Modal::Confirm(c));
                }
                Err(e) => self.error("Cannot clear", &e),
            },
            Action::Login => { self.close_menu(); self.open_login(None); }
            Action::Logout => { self.close_menu(); self.effects.push(Effect::Logout); }
            Action::Help => self.open_help(None),
            Action::About => self.open_about(),
            Action::ToggleLog => { self.show_log = !self.show_log; }
            Action::StackPrev => { if !self.stack_prev() { self.push_status("no previous post: this is the list"); } }
            Action::StackNext => { if !self.stack_next() { self.push_status("no next post: open one, or go back with prev first"); } }
            Action::CycleView => { self.close_menu(); let m = self.view_mode.cycle(1); self.set_view_mode(m); }
            Action::SetView(m) => { self.close_menu(); self.set_view_mode(m); }
            Action::ShowBookmarks => { self.close_menu(); self.open_bookmarks(); }
            Action::OpenTop => self.open_top(),
            Action::OpenEb => self.open_eb(),
            Action::OpenExplore => self.open_explore(),
            Action::ToggleBasket => {
                self.close_menu();
                match self.current_post_id() { Some(id) => self.toggle_basket(&id), None => self.push_status("no post to add: open or highlight one first") }
            }
            Action::ShowBasket => { self.close_menu(); self.open_basket(); }
            Action::ResearchProjects => {
                self.close_menu();
                let projects = self.research_list();
                let cur = self.research_name();
                self.modal = Some(Modal::Research(ResearchModal::browse(projects, cur.as_deref())));
            }
            Action::ShowResearch => match self.research_name() {
                Some(n) => { self.close_menu(); self.open_research(&n); }
                None => self.act(Action::ResearchProjects),
            },
            Action::AddToResearch => { let w = self.research_candidates(); self.open_research_add(w); }
            Action::AddViewToResearch => {
                let w: Vec<_> = self.view_source().into_iter().collect();
                if w.is_empty() { self.close_menu(); self.push_status("no posts in this view to add"); } else { self.open_research_add(w); }
            }
            Action::AddFileToResearch => {
                let f = self.menu.menu.text(CommandId::Research, "file");
                if f.trim().is_empty() { self.error("Add local file", "type the file's path in Research › Local file first (~ works)"); }
                else { self.open_research_add(vec![(format!("Local file {f}"), ResearchSource::File(f))]); }
            }
            Action::AddUrlToResearch => {
                let u = self.menu.menu.text(CommandId::Research, "url");
                if !(u.starts_with("http://") || u.starts_with("https://")) { self.error("Add web page", "type an http:// or https:// address in Research › Web page URL first"); }
                else { self.open_research_add(vec![(format!("Web page {u}"), ResearchSource::Url(u))]); }
            }
            Action::ComingLater(which) => {
                let what = if which == 0 { "Google Drive" } else { "Telegram" };
                self.push_status(format!("{what} as a research source is pinned for later; add files from disk or web pages for now"));
            }
            Action::ThemePicker => self.open_theme_picker(),
            Action::ThemeEditor => self.open_theme_editor(),
            Action::ThemeList => self.list_themes(),
            Action::ThemeReload => self.reload_theme(),
            Action::ThemeExport => self.export_theme(),
            Action::ThemeInit => self.init_theme(),
            Action::ThemeStarter => self.theme_starter(None),
            Action::NewPost => { self.close_menu(); self.open_new_post(); }
            Action::Reply => { self.close_menu(); self.open_reply(); }
            Action::Quote => { self.close_menu(); self.open_quote(); }
            Action::RunSiteSearch => {
                let q = self.menu.menu.text(CommandId::Search, "query");
                let nick = self.menu.menu.text(CommandId::Search, "nick");
                let limit = self.menu.menu.text(CommandId::Search, "limit");
                let order = self.menu.menu.choice(CommandId::Search, "order");
                if !nick.is_empty() || !limit.is_empty() || order != "Newest" {
                    self.push_status("nick, order and limit stay local; searching the site for the keyword");
                }
                match self.menu.menu.site_search_argv(&q) {
                    Ok(argv) => { self.close_menu(); self.open_site_read(&search_key(&q), argv); }
                    Err(e) => self.error("Site search", &e),
                }
            }
            Action::OpenMenu(id) => {
                let Some(i) = self.menu.menu.items.iter().position(|c| c.id == id) else { return };
                if let Some(parent) = self.menu.open
                    && self.menu.menu.items.get(parent).is_some_and(|c| c.on_bar)
                {
                    self.menu.anchor = Some(parent);
                }
                self.open_menu(i);
            }
            Action::DiscoverLater => {
                self.close_menu();
                self.push_status("people, nodes, and rooms arrive with the network node");
            }
            Action::OpenNotices => {
                self.close_menu();
                self.modal = Some(Modal::Notices(crate::tui::modals::NoticesModal::new()));
            }
            Action::ExportResearch => match self.research_name().ok_or_else(|| "pick a research project first (Research › Projects…)".to_string())
                .and_then(|n| self.menu.menu.research_export_argv(&n)) {
                Ok(argv) => { self.close_menu(); self.start_job(JobKind::Export, argv); }
                Err(e) => self.error("Cannot export research", &e),
            },
            Action::ClearBasket => { self.close_menu(); let n = self.basket.len(); self.basket.clear(); self.push_status(format!("export basket emptied ({n} removed)")); }
            Action::ExportBasket => match self.menu.menu.export_argv_with(&sel, None, &self.basket, Some("Basket")) {
                Ok(argv) => { self.close_menu(); self.start_job(JobKind::Export, argv); }
                Err(e) => self.error("Cannot export", &e),
            },
            Action::ToggleBookmark => {
                self.close_menu();
                match self.current_post_id() { Some(id) => self.toggle_mark(&id), None => self.push_status("no post to bookmark: open or highlight one first") }
            }
            Action::SyncNow => match self.menu.menu.sync_argv(&sel) {
                Ok(argv) => { self.sync_next = None; self.start_online_job(JobKind::Sync, argv); }
                Err(e) => self.error("Cannot sync", &e),
            },
            Action::SaveSyncPrefs => { self.close_menu(); self.effects.push(Effect::SaveSyncPrefs(self.menu.menu.sync_prefs())); }
            Action::SyncBookmarks => match self.menu.menu.sync_bookmarks_argv() {
                Ok(argv) => self.start_online_job(JobKind::Sync, argv),
                Err(e) => self.error("Cannot sync", &e),
            },
            Action::Quit => {
                self.close_menu();
                if let Some(j) = &self.job {
                    let mut c = ConfirmModal::new("Quit", vec![
                        format!("A {} is running (pid {}).", j.kind.name(), j.pid),
                        "Keep it running in the background, or cancel it and quit?".into(),
                        format!("Its log: {}", j.log.display())], "Keep running & quit", ConfirmAction::Quit);
                    c.buttons = vec!["Keep running & quit".into(), "Cancel it & quit".into(), "Stay".into()];
                    c.focus = 0;
                    self.modal = Some(Modal::Confirm(c));
                } else {
                    self.effects.push(Effect::Quit { cancel_job: false });
                }
            }
        }
    }
    fn load_base_keep(&mut self) {
        let (c, o) = (self.base.cursor, self.base.offset);
        self.load_base();
        self.base.cursor = c.min(self.base.posts.len().saturating_sub(1));
        self.base.offset = o.min(self.base.cursor);
    }
    fn start_job(&mut self, kind: JobKind, argv: Vec<String>) {
        if self.job.is_some() { self.error("Busy", "another job is still running; wait for it or cancel it (F6)"); return; }
        self.effects.push(Effect::RunJob(kind, argv));
    }
    /// Open the Login modal; `then` runs after a successful login.
    pub fn open_login(&mut self, then: Option<(JobKind, Vec<String>)>) {
        let reason = then.as_ref().map(|(k, _)| format!("Log in to start the {}.", k.name()));
        self.pending_job = then;
        self.paused_login = None;
        self.modal = Some(Modal::Login(LoginModal::new(self.email.clone(), reason)));
    }
    /// The open login modal, including one parked under Help.
    fn with_login<R>(&mut self, f: impl FnOnce(&mut LoginModal) -> R) -> Option<R> {
        if let Some(Modal::Login(m)) = &mut self.modal { return Some(f(m)); }
        self.paused_login.as_mut().map(f)
    }
    /// The session check before a scan / sync came back.
    pub fn session_checked(&mut self, r: std::result::Result<bool, String>, kind: JobKind, argv: Vec<String>) {
        match r {
            Ok(true) => { self.session = SessionState::Valid; self.effects.push(Effect::RunJob(kind, argv)); }
            Ok(false) => { self.session = SessionState::LoggedOut; self.open_login(Some((kind, argv))); }
            Err(e) => { self.session = SessionState::Saved; self.pending_feed = None; self.pending_sort = None; self.error("Cannot reach acp.us", &e); }
        }
    }
    /// The site sent the login code.
    pub fn login_need_code(&mut self) {
        self.with_login(|m| m.need_code());
    }
    /// Verification failed and the attempt is still open. Stay on the code step.
    pub fn login_hold(&mut self, e: &str) {
        let _ = self.with_login(|m| m.held(e));
    }
    /// The running attempt is gone. Back to the email step.
    pub fn login_expired(&mut self) {
        let _ = self.with_login(|m| m.failed("the login expired; send a new code"));
    }
    /// The login finished. A login parked under Help still counts.
    pub fn login_done(&mut self, r: std::result::Result<(), String>) {
        let present = matches!(self.modal, Some(Modal::Login(_))) || self.paused_login.is_some();
        if !present { return; }
        match r {
            Ok(()) => {
                let email = self.with_login(|m| m.email.value.trim().to_string());
                self.session = SessionState::Valid;
                self.email = email.filter(|e| !e.is_empty()).or(self.email.take());
                self.paused_login = None;
                if matches!(self.modal, Some(Modal::Login(_))) {
                    self.close_modal(ModalResult::ClosedSubmitted);
                } else {
                    self.modal = None;
                    self.push_status("logged in");
                    if let Some((k, argv)) = self.pending_job.take() { self.effects.push(Effect::RunJob(k, argv)); }
                }
            }
            Err(e) => { let _ = self.with_login(|m| m.failed(&e)); }
        }
    }
    /// Logged out.
    pub fn logged_out(&mut self, removed: bool) {
        self.session = SessionState::LoggedOut;
        self.push_status(if removed { "logged out: saved session removed" } else { "logged out (no saved session)" });
        self.open_login(None);
    }
    /// A job started.
    pub fn job_started(&mut self, j: JobView, cmd: &str) {
        self.push_status(format!("started {} (pid {}): {cmd}", j.kind.name(), j.pid));
        self.remember_job(j.kind, j.pid, j.log.clone(), cmd, false);
        self.job = Some(j);
    }
    /// The job ended (`ok` = exit 0); `tail` = its last log lines.
    pub fn job_finished(&mut self, ok: bool, code: Option<i32>, tail: &[String]) {
        let Some(j) = self.job.take() else { return };
        self.settle_job(j.pid, ok, code, tail);
        let auto = std::mem::take(&mut self.auto_running);
        let feed = self.pending_feed.take();
        let sort = self.pending_sort.take();
        self.refresh_nicks();
        self.load_base_keep();
        self.reload_marks();
        if let Some(key) = feed.as_deref() {
            let had = if let Some((thread, post)) = parse_edits_key(key) {
                self.show_edits(post, thread)
            } else {
                self.show_site_feed(key)
            };
            if !ok && had { self.push_status(format!("showing the saved {key}; the refresh failed")); }
        }
        if let Some((id, sort)) = sort {
            self.restore_thread_sort(&id, sort);
        }
        if ok {
            let name = j.kind.name();
            self.push_status(format!("{name} finished"));
            self.notices.post(crate::notify::Notice {
                kind: crate::notify::NoticeKind::Job,
                title: "Job".into(),
                body: format!("{name} finished"),
            }, Instant::now());
        } else if auto {
            self.push_status(format!("error: auto sync failed (exit {}); see {}", code.map(|c| c.to_string()).unwrap_or_else(|| "killed".into()), j.log.display()));
        } else {
            // The log line is already in the status bar. A modal would paint it
            // a second time, over the post list.
            let code = code.map(|c| c.to_string()).unwrap_or_else(|| "killed".into());
            let detail = tail.iter().rev().find(|l| !l.trim().is_empty()).map(|l| crate::tui::modals::job::redact_line(l.trim())).unwrap_or_else(|| "no output".into());
            self.push_status(format!("error: {} failed (exit {code}): {detail}", j.kind.name()));
        }
    }
    /// The job was cancelled.
    pub fn job_cancelled(&mut self) {
        self.auto_running = false;
        self.pending_feed = None;
        self.pending_sort = None;
        if let Some(j) = self.job.take() {
            self.mark_job_cancelled(j.pid);
            self.push_status(format!("{} cancelled (pid {})", j.kind.name(), j.pid));
        }
        self.refresh_nicks();
        self.load_base_keep();
    }

    // ------------------------------------------------------------ menu / modals
    /// Close the dropdown and leave the menu.
    pub fn close_menu(&mut self) {
        self.menu.open = None;
        self.menu.anchor = None;
        self.menu.editing = false;
        self.needs_redraw = true;
        if self.focus == Focus::Menu { self.focus = if self.view().posts.is_empty() { Focus::Nicks } else { Focus::Body }; }
    }
    /// Highlight / open the n-th menu item (option-less items run at once).
    ///
    /// A bar item clears [`MenuState::anchor`]. A tucked item keeps the anchor set by
    /// [`Action::OpenMenu`], so Esc can return to Library.
    pub fn open_menu(&mut self, i: usize) {
        let Some(c) = self.menu.menu.items.get(i) else { return };
        let id = c.id;
        let action = c.action;
        let on_bar = c.on_bar;
        self.needs_redraw = true;
        self.focus = Focus::Menu;
        self.menu.editing = false;
        if let Some(a) = action {
            self.menu.item = i;
            self.menu.open = None;
            self.menu.anchor = None;
            self.act(a);
            return;
        }
        if self.menu.open == Some(i) {
            self.menu.open = None;
            self.menu.anchor = None;
            if on_bar { self.menu.item = i; }
            return;
        }
        if on_bar {
            self.menu.anchor = None;
            self.menu.item = i;
        } else if let Some(parent) = self.menu.anchor {
            self.menu.item = parent;
        }
        self.menu.open = Some(i);
        self.menu.opt = 0;
        if id == CommandId::View || id == CommandId::Search { self.sync_order_menus(); }
    }
    /// Activate the highlighted option of the open dropdown (Enter / click).
    pub fn activate_opt(&mut self, i: usize) {
        use crate::tui::commands::Widget;
        let Some(item) = self.menu.open else { return };
        self.menu.opt = i;
        let Some(o) = self.menu.menu.items[item].opts.get_mut(i) else { return };
        let choice = matches!(o.widget, Widget::Choice { .. });
        match &mut o.widget {
            Widget::Check(v) => *v = !*v,
            Widget::Choice { choices, idx } => *idx = (*idx + 1) % choices.len(),
            Widget::Text(_) => self.menu.editing = !self.menu.editing,
            Widget::Button(a) => { let a = *a; self.act(a); }
        }
        if choice { self.apply_open_choice(); }
    }
    /// Close the modal with `r` and apply it.
    pub fn close_modal(&mut self, r: ModalResult) {
        let Some(m) = self.modal.take() else { return };
        match (m, r) {
            (Modal::Login(_), ModalResult::ClosedSubmitted) => {
                self.push_status("logged in");
                if let Some((k, argv)) = self.pending_job.take() { self.effects.push(Effect::RunJob(k, argv)); }
            }
            (Modal::Login(m), ModalResult::ClosedCancelled) => {
                self.pending_job = None;
                self.pending_feed = None;
                self.pending_sort = None;
                self.paused_login = None;
                if self.session == SessionState::Checking { self.session = SessionState::LoggedOut; }
                let _ = m;
                self.effects.push(Effect::LoginCancel);
            }
            (Modal::Login(m), ModalResult::Help(topic)) => {
                self.paused_login = Some(m);
                self.open_help(Some(topic));
            }
            (Modal::Help(_), _) => {
                if let Some(login) = self.paused_login.take() { self.modal = Some(Modal::Login(login)); }
            }
            (Modal::Confirm(c), ModalResult::ClosedSubmitted) => match c.action {
                ConfirmAction::Job(k, argv) => self.start_job(k, argv),
                ConfirmAction::Quit => self.effects.push(Effect::Quit { cancel_job: c.chosen == 1 }),
            },
            (Modal::Post(m), ModalResult::ClosedSubmitted) => match m.action() {
                PostAction::Reposts => self.open_reposts(&m.post_id),
                PostAction::Edits => self.open_edits(&m.post_id, &m.thread_id),
            },
            (Modal::Compose(c), ModalResult::ClosedCancelled) => {
                self.push_status("nothing sent");
                let _ = c;
            }
            (Modal::Compose(c), ModalResult::ClosedSubmitted) => {
                let now = chrono::Utc::now().timestamp();
                let draft = c.draft(now, self.last_write_unix, true);
                match crate::site::decide(&draft, true, false) {
                    Ok(text) => self.push_status(text),
                    Err(e) => {
                        if e.message.contains("nothing was published") { self.last_write_unix = Some(now); }
                        self.error("Write", &e.message);
                    }
                }
            }
            (Modal::Search(s), ModalResult::ClosedSubmitted) => {
                if s.cleared { self.clear_search(); }
                else if s.site { self.open_site_search(&s); }
                else if let Ok(f) = s.filter() { self.run_search(f); }
            }
            (_, ModalResult::Help(topic)) => self.open_help(Some(topic)),
            (Modal::Research(m), ModalResult::ClosedSubmitted) => self.research_chosen(m),
            (Modal::ThemePicker(p), r) => self.theme_picked(*p, r),
            (Modal::Palette(p), ModalResult::ClosedSubmitted) => match p.chosen() {
                Some(PaletteCmd::Action(Action::ThemeStarter)) => self.theme_starter(p.argument()),
                Some(PaletteCmd::Action(a)) => self.act(a),
                Some(PaletteCmd::OpenMenu(i)) => self.open_menu(i),
                None => {}
            },
            _ => {}
        }
    }
    /// Every command for the palette.
    pub fn palette_entries(&self) -> Vec<(String, PaletteCmd)> {
        use crate::tui::commands::Widget;
        let mut v = vec![
            ("Start Scanning (F6)".to_string(), PaletteCmd::Action(Action::StartScan)),
            ("Theme: use… (F5)".into(), PaletteCmd::Action(Action::ThemePicker)),
            ("Theme: list".into(), PaletteCmd::Action(Action::ThemeList)),
            ("Theme: edit… (Shift-F5)".into(), PaletteCmd::Action(Action::ThemeEditor)),
            ("Theme: reload".into(), PaletteCmd::Action(Action::ThemeReload)),
            ("Theme: export".into(), PaletteCmd::Action(Action::ThemeExport)),
            ("Theme: init (download the default)".into(), PaletteCmd::Action(Action::ThemeInit)),
            ("Theme: starter [name]".into(), PaletteCmd::Action(Action::ThemeStarter)),
            ("Search… (Shift-S)".into(), PaletteCmd::Action(Action::OpenSearchModal)),
            ("Clear search (Ctrl-S)".into(), PaletteCmd::Action(Action::ClearSearch)),
            ("Toggle log panel (l)".into(), PaletteCmd::Action(Action::ToggleLog)),
            ("Previous post in the stack (,)".into(), PaletteCmd::Action(Action::StackPrev)),
            ("Next post in the stack (.)".into(), PaletteCmd::Action(Action::StackNext)),
            ("Next view mode (v)".into(), PaletteCmd::Action(Action::CycleView)),
            ("Show bookmarks (B)".into(), PaletteCmd::Action(Action::ShowBookmarks)),
            ("Bookmark current post (b)".into(), PaletteCmd::Action(Action::ToggleBookmark)),
            ("Add current post to the export basket (x)".into(), PaletteCmd::Action(Action::ToggleBasket)),
            ("Add to a research project (r)".into(), PaletteCmd::Action(Action::AddToResearch)),
        ];
        for (i, c) in self.menu.menu.items.iter().enumerate() {
            if c.opts.is_empty() { v.push((c.label.to_string(), PaletteCmd::OpenMenu(i))); continue; }
            v.push((format!("{} › options", c.label), PaletteCmd::OpenMenu(i)));
            for o in &c.opts {
                if let Widget::Button(a) = o.widget { v.push((format!("{} › {}", c.label, o.label), PaletteCmd::Action(a))); }
            }
        }
        v
    }
}

/// http(s) links in a body, except links to posts on the site itself.
pub fn web_links(text: &str) -> Vec<String> {
    use std::sync::LazyLock;
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"https?://[^\s<>()\[\]\x22']+").expect("url re"));
    let mut seen = HashSet::new();
    RE.find_iter(text).map(|m| m.as_str().trim_end_matches(['.', ',', ';', ':', '!', '?']).to_string())
        .filter(|u| !u.contains("/rs/post/"))
        .filter(|u| seen.insert(u.clone())).collect()
}

/// A research file or web page shown as a card: its note, where it came from, and its text.
pub fn research_post(it: &ResearchItem) -> Post {
    let mut body = String::new();
    if !it.note.trim().is_empty() { body.push_str(&format!("> {}\n\n", it.note.trim())); }
    body.push_str(&it.r#ref);
    if let Some(s) = it.meta["size"].as_u64() { body.push_str(&format!(" · {s} bytes")); }
    if let Some(m) = it.meta["mime_type"].as_str().or(it.meta["content_type"].as_str()) { body.push_str(&format!(" · {m}")); }
    if !it.content.trim().is_empty() {
        let text: String = it.content.chars().take(6000).collect();
        body.push_str("\n\n");
        body.push_str(&text);
        if it.content.chars().count() > 6000 { body.push_str("\n\n…"); }
    }
    let nick = match it.kind { ItemKind::WebUrl => "web", ItemKind::LocalFile => "file", _ => "missing post" };
    let content = if it.kind.is_post() { format!("{} is not in the database any more.\n\n{body}", it.r#ref) } else { body };
    Post { post_id: format!("research:{}", it.id), parent_post_id: String::new(), in_reply_to_id: String::new(), nick: nick.into(),
        title: it.title.clone(), content, posted_at: it.added_at.clone(), media: Vec::new(), truncated: false, body_json: String::new(), score: None }
}
