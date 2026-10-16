//! TUI state and its pure transitions (no terminal, no network): nick
//! selection, the Body views (nick posts / search results), the detail stack,
//! menu state, modals and the side effects the event loop must carry out.

use crate::prelude::*;
use crate::tui::commands::{Action, CommandId, Menu};
use crate::tui::data::{NickEntry, SearchFilter, Store};
use crate::tui::jobs::JobKind;
use crate::tui::modals::*;
use crate::tui::theme::Theme;
use ratatui::layout::Rect;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Focusable panes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// Left nick bar.
    Nicks,
    /// Posts / details.
    Body,
    /// Menu bar.
    Menu,
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
    /// Check the session, then scan (or open Login first).
    CheckSessionThenScan(Vec<String>),
    /// Start a headless job.
    RunJob(JobKind, Vec<String>),
    /// Kill the running job.
    CancelJob,
    /// Start logging in with this email.
    LoginEmail(String),
    /// Submit the code to the running login.
    LoginCode(String),
    /// Abandon the running login.
    LoginCancel,
    /// Forget the session.
    Logout,
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
    /// Inside the modal.
    Modal(ModalHit),
    /// Modal backdrop (swallows clicks).
    Backdrop,
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
    /// Highlighted option in the dropdown.
    pub opt: usize,
    /// Typing into the highlighted text option.
    pub editing: bool,
}

/// Everything the TUI shows.
pub struct App {
    /// Data source (read-only).
    pub store: Box<dyn Store>,
    /// Nick bar.
    pub nicks: Vec<NickEntry>,
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
    /// Login state.
    pub session: SessionState,
    /// Running job.
    pub job: Option<JobView>,
    /// Scan waiting for a login.
    pub pending_scan: Option<Vec<String>>,
    /// Running animation.
    pub anim: Option<Anim>,
    /// Clickable regions from the last draw.
    pub hits: Vec<(Rect, Hit)>,
    /// Colours.
    pub theme: Theme,
    /// Side effects for the event loop.
    pub effects: Vec<Effect>,
    /// Post cards that fit the list (set while drawing).
    pub page_rows: usize,
    /// Login email pre-fill.
    pub email: Option<String>,
}

impl App {
    /// State over `store`; `args` seeds the menu, `--nick(s)` the selection.
    pub fn new(store: Box<dyn Store>, args: &Args, theme: Theme, session_saved: bool) -> Self {
        let mut app = App {
            store, nicks: Vec::new(), nick_cursor: 0, nick_offset: 0, selected: Vec::new(),
            base: BodyView::default(), search: None, focus: Focus::Nicks,
            menu: MenuState { menu: Menu::new(args), item: 0, open: None, opt: 0, editing: false },
            modal: None, status: VecDeque::new(), show_log: false,
            session: if session_saved { SessionState::Saved } else { SessionState::LoggedOut },
            job: None, pending_scan: None, anim: None, hits: Vec::new(), theme, effects: Vec::new(),
            page_rows: 5, email: None,
        };
        app.refresh_nicks();
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
    pub fn push_status(&mut self, s: impl Into<String>) {
        let s = s.into();
        for l in s.lines() {
            self.status.push_back(l.to_string());
            while self.status.len() > TUI_STATUS_KEEP { self.status.pop_front(); }
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
            Ok(n) => self.nicks = n,
            Err(e) => { self.push_status(format!("error: reading nicks: {e:#}")); self.nicks.clear(); }
        }
        let keep: Vec<String> = self.selected.iter()
            .filter_map(|s| self.nicks.iter().find(|n| n.nick.eq_ignore_ascii_case(s)).map(|n| n.nick.clone())).collect();
        self.selected = keep;
        self.nick_cursor = self.nick_cursor.min(self.nicks.len().saturating_sub(1));
    }
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
    /// Select every nick.
    pub fn select_all(&mut self) {
        self.selected = self.nicks.iter().map(|n| n.nick.clone()).collect();
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
        self.base = BodyView { title: who, posts, ..BodyView::default() };
        self.anim = None;
    }

    // ------------------------------------------------------------ body views
    /// The view on screen (search results replace the nick posts in place).
    pub fn view(&self) -> &BodyView { self.search.as_ref().unwrap_or(&self.base) }
    /// Mutable [`App::view`].
    pub fn view_mut(&mut self) -> &mut BodyView { self.search.as_mut().unwrap_or(&mut self.base) }
    /// Replace the Body with the results of `f`.
    pub fn run_search(&mut self, f: SearchFilter) {
        if f.is_empty() { self.clear_search(); return; }
        match self.store.search(&f) {
            Ok(posts) => {
                self.push_status(format!("search {}: {} posts", f.describe(), posts.len()));
                self.search = Some(BodyView { title: format!("Search: {}", f.describe()), posts, filter: Some(f), ..BodyView::default() });
                self.focus = Focus::Body;
                self.anim = None;
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
    }
    /// Current search filter (for pre-filling the modal).
    pub fn current_filter(&self) -> SearchFilter { self.search.as_ref().and_then(|v| v.filter.clone()).unwrap_or_default() }

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
        for r in s.replies(&post.post_id).unwrap_or_default() {
            if seen.insert(r.post_id.clone()) { links.push(Link { kind: LinkKind::Reply, id: r.post_id.clone(), target: Some(r) }); }
        }
        DetailFrame { attachments: s.attachments(&post.post_id).unwrap_or_default(), post, links,
            link_cursor: 0, scroll: 0, follow_link: false, notice: None }
    }
    /// Open the n-th post of the view as the root detail (replacing any open stack).
    pub fn open_post(&mut self, i: usize) {
        let Some(p) = self.view().posts.get(i).cloned() else { return };
        let f = self.frame_for(p);
        let v = self.view_mut();
        v.cursor = i;
        v.stack = vec![f];
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
        let Some(target) = link.target else {
            if let Some(f) = self.top_mut() { f.notice = Some(format!("{} is not in the database", link.id)); }
            self.push_status(format!("{} is not in the database (scrape its thread to read it)", link.id));
            return;
        };
        if depth >= MAX_DETAIL_DEPTH {
            self.push_status(format!("at most {MAX_DETAIL_DEPTH} nested posts: press Esc to close one first"));
            if let Some(f) = self.top_mut() { f.notice = Some(format!("nesting limit ({MAX_DETAIL_DEPTH}) reached")); }
            return;
        }
        let f = self.frame_for(target);
        self.view_mut().stack.push(f);
        self.anim = Some(Anim { kind: AnimKind::Push, start: Instant::now() });
    }
    /// Close the top frame; returns whether one was open.
    pub fn pop_detail(&mut self) -> bool {
        let v = self.view_mut();
        let r = v.stack.pop().is_some();
        self.anim = None;
        r
    }
    /// Keep frames `0..=depth` (click on a collapsed header).
    pub fn pop_to(&mut self, depth: usize) {
        self.view_mut().stack.truncate(depth + 1);
        self.anim = None;
    }
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
                    Ok(argv) => {
                        self.close_menu();
                        if self.session == SessionState::Valid { self.effects.push(Effect::RunJob(JobKind::Scan, argv)); }
                        else { self.session = SessionState::Checking; self.effects.push(Effect::CheckSessionThenScan(argv)); }
                    }
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
                match self.menu.menu.export_argv(&sel, view.as_ref()) {
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
            Action::Help => { self.close_menu(); self.modal = Some(Modal::Help(HelpModal::new())); }
            Action::ToggleLog => { self.show_log = !self.show_log; }
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
        if self.job.is_some() { self.error("Busy", "another job is still running; wait for it or cancel it (F5)"); return; }
        self.effects.push(Effect::RunJob(kind, argv));
    }
    /// Open the Login modal; `then_scan` runs after a successful login.
    pub fn open_login(&mut self, then_scan: Option<Vec<String>>) {
        let reason = then_scan.as_ref().map(|_| "Log in to start the scan.".to_string());
        self.pending_scan = then_scan;
        self.modal = Some(Modal::Login(LoginModal::new(self.email.clone(), reason)));
    }
    /// The session check before a scan came back.
    pub fn session_checked(&mut self, r: std::result::Result<bool, String>, argv: Vec<String>) {
        match r {
            Ok(true) => { self.session = SessionState::Valid; self.effects.push(Effect::RunJob(JobKind::Scan, argv)); }
            Ok(false) => { self.session = SessionState::LoggedOut; self.open_login(Some(argv)); }
            Err(e) => { self.session = SessionState::Saved; self.error("Cannot reach acp.us", &e); }
        }
    }
    /// The site sent the login code.
    pub fn login_need_code(&mut self) {
        if let Some(Modal::Login(m)) = &mut self.modal { m.need_code(); }
    }
    /// The login finished.
    pub fn login_done(&mut self, r: std::result::Result<(), String>) {
        let Some(Modal::Login(m)) = &mut self.modal else { return };
        match r {
            Ok(()) => {
                self.session = SessionState::Valid;
                self.email = Some(m.email.value.trim().to_string()).filter(|e| !e.is_empty()).or(self.email.take());
                self.close_modal(ModalResult::ClosedSubmitted);
            }
            Err(e) => m.failed(&e),
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
        self.job = Some(j);
    }
    /// The job ended (`ok` = exit 0); `tail` = its last log lines.
    pub fn job_finished(&mut self, ok: bool, code: Option<i32>, tail: &[String]) {
        let Some(j) = self.job.take() else { return };
        self.refresh_nicks();
        self.load_base_keep();
        if ok {
            self.push_status(format!("{} finished", j.kind.name()));
        } else {
            let code = code.map(|c| c.to_string()).unwrap_or_else(|| "killed".into());
            let mut text = format!("The {} stopped (exit {code}). Last lines of {}:\n", j.kind.name(), j.log.display());
            for l in tail { text.push_str(&format!("{l}\n")); }
            self.error(&format!("{} failed", j.kind.name()), text.trim_end());
        }
    }
    /// The job was cancelled.
    pub fn job_cancelled(&mut self) {
        if let Some(j) = self.job.take() { self.push_status(format!("{} cancelled (pid {})", j.kind.name(), j.pid)); }
        self.refresh_nicks();
        self.load_base_keep();
    }

    // ------------------------------------------------------------ menu / modals
    /// Close the dropdown and leave the menu.
    pub fn close_menu(&mut self) {
        self.menu.open = None;
        self.menu.editing = false;
        if self.focus == Focus::Menu { self.focus = if self.view().posts.is_empty() { Focus::Nicks } else { Focus::Body }; }
    }
    /// Highlight / open the n-th menu item (option-less items run at once).
    pub fn open_menu(&mut self, i: usize) {
        let Some(c) = self.menu.menu.items.get(i) else { return };
        self.focus = Focus::Menu;
        self.menu.item = i;
        self.menu.editing = false;
        if let Some(a) = c.action { self.menu.open = None; self.act(a); return; }
        if self.menu.open == Some(i) { self.menu.open = None; } else { self.menu.open = Some(i); self.menu.opt = 0; }
    }
    /// Activate the highlighted option of the open dropdown (Enter / click).
    pub fn activate_opt(&mut self, i: usize) {
        use crate::tui::commands::Widget;
        let Some(item) = self.menu.open else { return };
        self.menu.opt = i;
        let Some(o) = self.menu.menu.items[item].opts.get_mut(i) else { return };
        match &mut o.widget {
            Widget::Check(v) => *v = !*v,
            Widget::Choice { choices, idx } => *idx = (*idx + 1) % choices.len(),
            Widget::Text(_) => self.menu.editing = !self.menu.editing,
            Widget::Button(a) => { let a = *a; self.act(a); }
        }
    }
    /// Close the modal with `r` and apply it.
    pub fn close_modal(&mut self, r: ModalResult) {
        let Some(m) = self.modal.take() else { return };
        match (m, r) {
            (Modal::Login(_), ModalResult::ClosedSubmitted) => {
                self.push_status("logged in");
                if let Some(argv) = self.pending_scan.take() { self.effects.push(Effect::RunJob(JobKind::Scan, argv)); }
            }
            (Modal::Login(m), ModalResult::ClosedCancelled) => {
                self.pending_scan = None;
                if self.session == SessionState::Checking { self.session = SessionState::LoggedOut; }
                let _ = m;
                self.effects.push(Effect::LoginCancel);
            }
            (Modal::Confirm(c), ModalResult::ClosedSubmitted) => match c.action {
                ConfirmAction::Job(k, argv) => self.start_job(k, argv),
                ConfirmAction::Quit => self.effects.push(Effect::Quit { cancel_job: c.chosen == 1 }),
            },
            (Modal::Search(s), ModalResult::ClosedSubmitted) => {
                if s.cleared { self.clear_search(); }
                else if let Ok(f) = s.filter() { self.run_search(f); }
            }
            (Modal::Palette(p), ModalResult::ClosedSubmitted) => match p.chosen() {
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
            ("Start Scanning (F5)".to_string(), PaletteCmd::Action(Action::StartScan)),
            ("Search… (Shift-S)".into(), PaletteCmd::Action(Action::OpenSearchModal)),
            ("Clear search (Ctrl-S)".into(), PaletteCmd::Action(Action::ClearSearch)),
            ("Toggle log panel (l)".into(), PaletteCmd::Action(Action::ToggleLog)),
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
