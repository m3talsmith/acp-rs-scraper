//! Local notices: a toast, a desktop notification, a saved row, and a stderr line.
//!
//! This is not the site Notifications pane. There is no cap. Do-not-disturb and a
//! per-type switch silence the toast and the desktop notification. The saved list
//! still keeps the row. Tests use [`RecordingDesktop`]. Nothing here talks to a
//! notification daemon.

use std::collections::VecDeque;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long a toast stays up.
pub const TOAST_TTL: Duration = Duration::from_secs(6);
/// Rows one toast occupies.
pub const TOAST_ROWS: u16 = 2;
/// Rows the View all button occupies.
pub const VIEW_ALL_ROWS: u16 = 1;

/// A kind of notice. Each one can be turned off on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    /// A failure the user has to see.
    Error = 0,
    /// A warning.
    Warning = 1,
    /// Sync skipped, finished, or failed.
    Sync = 2,
    /// A job finished.
    Job = 3,
}

impl NoticeKind {
    /// The kinds, in toggle order.
    pub const ALL: [NoticeKind; 4] = [NoticeKind::Error, NoticeKind::Warning, NoticeKind::Sync, NoticeKind::Job];

    /// Short label for a toast, the log, and stderr.
    pub fn label(self) -> &'static str {
        match self {
            NoticeKind::Error => "Error",
            NoticeKind::Warning => "Warning",
            NoticeKind::Sync => "Sync",
            NoticeKind::Job => "Job",
        }
    }

    /// Index into [`NoticeSettings::on`].
    pub fn index(self) -> usize { self as usize }

    /// Stored kind. `error`, `warning`, `sync`, or `job`.
    pub fn key(self) -> &'static str {
        match self {
            NoticeKind::Error => "error",
            NoticeKind::Warning => "warning",
            NoticeKind::Sync => "sync",
            NoticeKind::Job => "job",
        }
    }

    /// Parse [`NoticeKind::key`].
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "error" => Some(NoticeKind::Error),
            "warning" => Some(NoticeKind::Warning),
            "sync" => Some(NoticeKind::Sync),
            "job" => Some(NoticeKind::Job),
            _ => None,
        }
    }
}

/// One notice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// Database id. `0` before it is saved.
    pub id: i64,
    /// When it was raised, RFC 3339. Empty before it is saved.
    pub created_at: String,
    /// Which switch it follows.
    pub kind: NoticeKind,
    /// Short heading.
    pub title: String,
    /// The rest of the text.
    pub body: String,
}

impl Notice {
    /// A notice that has not been saved yet.
    pub fn new(kind: NoticeKind, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self { id: 0, created_at: String::new(), kind, title: title.into(), body: body.into() }
    }

    /// A notice from a status line that starts with `error` or `warning`.
    pub fn from_status_line(line: &str) -> Option<Self> {
        let line = line.trim();
        let (kind, rest) = if let Some(rest) = strip_prefix_ignore_case(line, "error") {
            (NoticeKind::Error, rest)
        } else if let Some(rest) = strip_prefix_ignore_case(line, "warning") {
            (NoticeKind::Warning, rest)
        } else if let Some(rest) = strip_prefix_ignore_case(line, "sync") {
            (NoticeKind::Sync, rest)
        } else if let Some(rest) = strip_prefix_ignore_case(line, "job") {
            (NoticeKind::Job, rest)
        } else {
            return None;
        };
        let body = rest.trim_start_matches([':', ' ', '-']).trim();
        let body = if body.is_empty() { line } else { body };
        Some(Self::new(kind, kind.label(), body.to_string()))
    }
}

fn strip_prefix_ignore_case<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let (head, tail) = line.split_at_checked(prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then_some(tail)
}

/// Which notices raise a toast and a desktop notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoticeSettings {
    /// Silences every toast and desktop notification. The log still records them.
    pub dnd: bool,
    /// One flag per [`NoticeKind`], in [`NoticeKind::ALL`] order.
    pub on: [bool; 4],
}

impl Default for NoticeSettings {
    fn default() -> Self { Self { dnd: false, on: [true; 4] } }
}

impl NoticeSettings {
    /// Whether a toast and a desktop notification should fire.
    pub fn allows(&self, kind: NoticeKind) -> bool {
        !self.dnd && self.on[kind.index()]
    }

    /// Flip do-not-disturb or one kind. Row 0 is do-not-disturb. The next rows are the kinds.
    pub fn toggle_row(&mut self, row: usize) -> bool {
        if row == 0 {
            self.dnd = !self.dnd;
            return true;
        }
        let Some(kind) = NoticeKind::ALL.get(row - 1) else { return false };
        let i = kind.index();
        self.on[i] = !self.on[i];
        true
    }
}

/// The notifications page. Opening it is not an action-history step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoticePage {
    /// Search text. Blank shows every saved notice that passes the kind filter.
    pub query: String,
    /// The search box has the keyboard.
    pub editing: bool,
    /// Newest first. `false` is oldest first.
    pub newest_first: bool,
    /// `None` is every kind.
    pub kind: Option<NoticeKind>,
    /// Highlighted row in the filtered list.
    pub cursor: usize,
    /// First visible row.
    pub scroll: usize,
}

impl NoticePage {
    /// Newest first, every kind, an empty search.
    pub fn new() -> Self {
        Self { query: String::new(), editing: false, newest_first: true, kind: None, cursor: 0, scroll: 0 }
    }

    /// Switch between newest first and oldest first.
    pub fn toggle_order(&mut self) {
        self.newest_first = !self.newest_first;
        self.cursor = 0;
        self.scroll = 0;
    }

    /// Step the kind filter: all, then each kind, then all again.
    pub fn cycle_kind(&mut self) {
        self.kind = match self.kind {
            None => Some(NoticeKind::Error),
            Some(NoticeKind::Error) => Some(NoticeKind::Warning),
            Some(NoticeKind::Warning) => Some(NoticeKind::Sync),
            Some(NoticeKind::Sync) => Some(NoticeKind::Job),
            Some(NoticeKind::Job) => None,
        };
        self.cursor = 0;
        self.scroll = 0;
    }

    /// Label of the kind filter.
    pub fn kind_label(&self) -> &'static str {
        match self.kind { Some(k) => k.label(), None => "all" }
    }
}

impl Default for NoticePage {
    fn default() -> Self { Self::new() }
}

/// How many toasts fit in a quarter of `height`, and whether View all is shown.
///
/// The stack is not capped. When `count * `[`TOAST_ROWS`] would be taller than
/// `height / 4`, the button takes one row of that quarter and the rest of the
/// quarter shows as many toasts as fit.
pub fn toast_window(count: usize, height: u16) -> (usize, bool) {
    if count == 0 || height == 0 { return (0, false); }
    let quarter = height / 4;
    let all = (count as u16).saturating_mul(TOAST_ROWS);
    if all <= quarter { return (count, false); }
    let room = quarter.saturating_sub(VIEW_ALL_ROWS);
    let shown = (room / TOAST_ROWS) as usize;
    (shown.min(count), true)
}

/// A toast that dismisses itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    /// What it says.
    pub notice: Notice,
    /// When it leaves the stack.
    pub until: Instant,
}

/// Sends a desktop notification. The real one shells out. Tests use [`RecordingDesktop`].
pub trait DesktopNotify: Send {
    /// Show `title` and `body`. Failures are ignored by the caller.
    fn push(&mut self, title: &str, body: &str);
}

/// Drops every desktop notification.
#[derive(Clone, Debug, Default)]
pub struct NullDesktop;

impl DesktopNotify for NullDesktop {
    fn push(&mut self, _title: &str, _body: &str) {}
}

/// Records desktop notifications for tests.
#[derive(Clone, Debug, Default)]
pub struct RecordingDesktop {
    /// Title and body, in send order.
    pub sent: Vec<(String, String)>,
}

impl DesktopNotify for RecordingDesktop {
    fn push(&mut self, title: &str, body: &str) {
        self.sent.push((title.to_string(), body.to_string()));
    }
}

/// The platform notification. Linux uses `notify-send` (freedesktop over D-Bus).
/// macOS uses Notification Center through `osascript`. Windows uses the toast API.
#[derive(Clone, Debug, Default)]
pub struct PlatformDesktop;

impl DesktopNotify for PlatformDesktop {
    fn push(&mut self, title: &str, body: &str) { desktop_send(title, body); }
}

/// Ask the operating system to show a notification. A missing helper is ignored.
pub fn desktop_send(title: &str, body: &str) {
    #[cfg(target_os = "linux")]
    {
        let app_name = crate::brands::current().title;
        let _ = Command::new("notify-send")
            .arg("--app-name").arg(&app_name).arg(title).arg(body)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            body.replace('"', "'"),
            title.replace('"', "'"),
        );
        let _ = Command::new("osascript")
            .args(["-e", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let xml = format!(
            "<toast><visual><binding template=\"ToastText02\"><text id=\"1\">{}</text><text id=\"2\">{}</text></binding></visual></toast>",
            xml_escape(title),
            xml_escape(body),
        );
        let script = format!(
            "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
             $xml = New-Object Windows.Data.Xml.Dom.XmlDocument; \
             $xml.LoadXml('{xml}'); \
             $toast = [Windows.UI.Notifications.ToastNotification]::new($xml); \
             [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{app}').Show($toast)",
            app = crate::brands::current().title.replace('\'', "")
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (title, body);
    }
}

/// Escape a string for the Windows toast XML.
#[cfg(target_os = "windows")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('\'', "&apos;")
}

/// The log, the toast stack, and the desktop backend.
pub struct NoticeBoard {
    /// Newest first.
    pub log: VecDeque<Notice>,
    /// Visible toasts, oldest first.
    pub toasts: Vec<Toast>,
    /// Do-not-disturb and the per-type switches.
    pub settings: NoticeSettings,
    desktop: Box<dyn DesktopNotify>,
}

impl NoticeBoard {
    /// An empty board. Desktop notifications go nowhere until [`NoticeBoard::enable_desktop`].
    pub fn new() -> Self {
        Self {
            log: VecDeque::new(),
            toasts: Vec::new(),
            settings: NoticeSettings::default(),
            desktop: Box::new(NullDesktop),
        }
    }

    /// A board that records desktop notifications.
    pub fn recording() -> (Self, std::sync::Arc<std::sync::Mutex<RecordingDesktop>>) {
        let rec = std::sync::Arc::new(std::sync::Mutex::new(RecordingDesktop::default()));
        let board = Self {
            log: VecDeque::new(),
            toasts: Vec::new(),
            settings: NoticeSettings::default(),
            desktop: Box::new(SharedDesktop(rec.clone())),
        };
        (board, rec)
    }

    /// Send later notices to the operating system.
    pub fn enable_desktop(&mut self) { self.desktop = Box::new(PlatformDesktop); }

    /// The next id that is not already in the log.
    pub fn fresh_id(&self) -> i64 {
        self.log.iter().map(|n| n.id).max().unwrap_or(0) + 1
    }

    /// Record `notice`. A silenced kind still lands in the log. Nothing is dropped.
    pub fn post(&mut self, notice: Notice, now: Instant) {
        self.log.push_front(notice.clone());
        if !self.settings.allows(notice.kind) { return; }
        self.toasts.push(Toast { notice: notice.clone(), until: now + TOAST_TTL });
        self.desktop.push(&notice.title, &notice.body);
    }

    /// Drop toasts whose time is up.
    pub fn tick(&mut self, now: Instant) {
        self.toasts.retain(|t| t.until > now);
    }

    /// Remove one toast. The log keeps it.
    pub fn dismiss(&mut self, index: usize) {
        if index < self.toasts.len() { self.toasts.remove(index); }
    }
}

impl Default for NoticeBoard {
    fn default() -> Self { Self::new() }
}

struct SharedDesktop(std::sync::Arc<std::sync::Mutex<RecordingDesktop>>);

impl DesktopNotify for SharedDesktop {
    fn push(&mut self, title: &str, body: &str) {
        if let Ok(mut g) = self.0.lock() { g.push(title, body); }
    }
}

/// One stderr line. `color` false leaves it plain, including under `NO_COLOR` and `--color never`.
pub fn stderr_line(n: &Notice, color: bool) -> String {
    let kind = n.kind.label();
    let title = &n.title;
    let body = &n.body;
    if color {
        format!("\u{1b}[31m{kind}\u{1b}[0m: {title} — {body}")
    } else {
        format!("{kind}: {title} — {body}")
    }
}

/// Print a notice on stderr.
pub fn print_stderr(n: &Notice, color: bool) {
    eprintln!("{}", stderr_line(n, color));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(kind: NoticeKind, body: &str) -> Notice {
        Notice::new(kind, kind.label(), body)
    }

    #[test]
    fn toasts_keep_every_notice() {
        let mut b = NoticeBoard::new();
        let now = Instant::now();
        for (i, kind) in [NoticeKind::Error, NoticeKind::Warning, NoticeKind::Sync, NoticeKind::Job, NoticeKind::Error, NoticeKind::Job].into_iter().enumerate() {
            b.post(note(kind, &format!("n{i}")), now);
        }
        assert_eq!(b.toasts.len(), 6);
        assert_eq!(b.log.len(), 6);
        assert_eq!(b.toasts[0].notice.body, "n0");
        assert_eq!(b.toasts[5].notice.body, "n5");
        assert_eq!(b.log[0].body, "n5");
    }

    #[test]
    fn a_quarter_of_the_screen_shows_view_all() {
        assert_eq!(toast_window(2, 40), (2, false));
        assert_eq!(toast_window(5, 40), (5, false));
        assert_eq!(toast_window(6, 40), (4, true));
        assert_eq!(toast_window(3, 20), (2, true));
        assert_eq!(toast_window(0, 40), (0, false));
    }

    #[test]
    fn do_not_disturb_and_a_type_switch_keep_the_log() {
        let (mut b, rec) = NoticeBoard::recording();
        let now = Instant::now();
        b.settings.dnd = true;
        b.post(note(NoticeKind::Error, "quiet"), now);
        assert!(b.toasts.is_empty());
        assert_eq!(b.log[0].body, "quiet");
        assert!(rec.lock().unwrap().sent.is_empty());
        b.settings.dnd = false;
        b.settings.on[NoticeKind::Sync.index()] = false;
        b.post(note(NoticeKind::Sync, "held"), now);
        assert!(b.toasts.iter().all(|t| t.notice.body != "held"));
        assert_eq!(b.log[0].body, "held");
        b.post(note(NoticeKind::Job, "shown"), now);
        assert_eq!(b.toasts.last().unwrap().notice.body, "shown");
        assert_eq!(rec.lock().unwrap().sent, vec![("Job".into(), "shown".into())]);
        b.tick(now + TOAST_TTL);
        assert!(b.toasts.is_empty());
        assert_eq!(b.log[0].body, "shown");
    }

    #[test]
    fn stderr_is_plain_when_colour_is_off() {
        let n = note(NoticeKind::Error, "disk full");
        let plain = stderr_line(&n, false);
        let coloured = stderr_line(&n, true);
        assert_eq!(plain, "Error: Error — disk full");
        assert!(!plain.contains('\u{1b}'));
        assert!(coloured.contains('\u{1b}'));
        assert!(coloured.contains("disk full"));
        assert!(coloured.contains("Error"));
    }

    #[test]
    fn a_status_error_becomes_an_error_notice() {
        let n = Notice::from_status_line("error: import failed").unwrap();
        assert_eq!(n.kind, NoticeKind::Error);
        assert_eq!(n.body, "import failed");
        assert!(Notice::from_status_line("started scan").is_none());
        let w = Notice::from_status_line("Warning: kept").unwrap();
        assert_eq!(w.kind, NoticeKind::Warning);
        assert_eq!(w.body, "kept");
        let job = Notice::from_status_line("Job: whole site finished. 2 new posts across 3 pages.").unwrap();
        assert_eq!(job.kind, NoticeKind::Job);
        assert!(job.body.contains("finished"));
    }
}
