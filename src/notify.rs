//! Local notices: a toast, a desktop notification, a log row, and a stderr line.
//!
//! This is not the site Notifications pane. Do-not-disturb and a per-type switch
//! silence the toast and the desktop notification. The log still keeps the row.
//! Tests use [`RecordingDesktop`]. Nothing here talks to a notification daemon.

use std::collections::VecDeque;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long a toast stays up.
pub const TOAST_TTL: Duration = Duration::from_secs(6);
/// Toasts on screen at once. Older ones leave the stack and stay in the log.
pub const TOAST_STACK: usize = 3;
/// Notice-log rows kept, newest first.
pub const LOG_CAP: usize = 100;

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
}

/// One notice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// Which switch it follows.
    pub kind: NoticeKind,
    /// Short heading.
    pub title: String,
    /// The rest of the text.
    pub body: String,
}

impl Notice {
    /// A notice from a status line that starts with `error` or `warning`.
    pub fn from_status_line(line: &str) -> Option<Self> {
        let line = line.trim();
        let (kind, rest) = if let Some(rest) = strip_prefix_ignore_case(line, "error") {
            (NoticeKind::Error, rest)
        } else if let Some(rest) = strip_prefix_ignore_case(line, "warning") {
            (NoticeKind::Warning, rest)
        } else {
            return None;
        };
        let body = rest.trim_start_matches([':', ' ', '-']).trim();
        let body = if body.is_empty() { line } else { body };
        Some(Self { kind, title: kind.label().into(), body: body.to_string() })
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

/// How many control rows the notice window has (do-not-disturb, then one per kind).
pub fn control_rows() -> usize { 1 + NoticeKind::ALL.len() }

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
        let _ = Command::new("notify-send")
            .args(["--app-name", "ACP Red Star", title, body])
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
             [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('ACP Red Star').Show($toast)"
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

    /// Record `notice`. A silenced kind still lands in the log.
    pub fn post(&mut self, notice: Notice, now: Instant) {
        self.log.push_front(notice.clone());
        while self.log.len() > LOG_CAP { self.log.pop_back(); }
        if !self.settings.allows(notice.kind) { return; }
        self.toasts.push(Toast { notice: notice.clone(), until: now + TOAST_TTL });
        if self.toasts.len() > TOAST_STACK { self.toasts.drain(0..self.toasts.len() - TOAST_STACK); }
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
        Notice { kind, title: kind.label().into(), body: body.into() }
    }

    #[test]
    fn toasts_stack_and_the_oldest_leaves_the_screen() {
        let mut b = NoticeBoard::new();
        let now = Instant::now();
        b.post(note(NoticeKind::Error, "one"), now);
        b.post(note(NoticeKind::Warning, "two"), now);
        b.post(note(NoticeKind::Sync, "three"), now);
        b.post(note(NoticeKind::Job, "four"), now);
        assert_eq!(b.toasts.len(), TOAST_STACK);
        assert_eq!(b.toasts[0].notice.body, "two");
        assert_eq!(b.toasts[2].notice.body, "four");
        assert_eq!(b.log.len(), 4);
        assert_eq!(b.log[0].body, "four");
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
    }
}
