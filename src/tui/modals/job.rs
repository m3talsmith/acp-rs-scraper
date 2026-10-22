//! Job details: what the terminal UI started, how long it ran, and the recent
//! log. Elapsed ticks only while the job is running. After it finishes, fails,
//! or is cancelled, the line is the recorded end minus the start. A stopped job
//! with no recorded end shows `unknown` rather than the time until now. The
//! password and the database key never appear here.

use crate::tui::jobs::JobKind;
use crate::tui::state::{App, Effect};
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::{cycle, Modal, ModalHit, ModalResult, ModalState};

/// Log lines kept for one job.
const TAIL_CAP: usize = 200;
/// Finished jobs kept after the running ones.
const FINISHED_KEEP: usize = 12;
/// Spinner frames. Same set as the encryption progress screen.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Where a job is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobPhase {
    /// Still running.
    Running,
    /// Exited 0.
    Finished,
    /// Exited non-zero or was killed.
    Failed,
    /// The user cancelled it.
    Cancelled,
}

impl JobPhase {
    /// Short label.
    pub fn label(self) -> &'static str {
        match self {
            JobPhase::Running => "running",
            JobPhase::Finished => "finished",
            JobPhase::Failed => "failed",
            JobPhase::Cancelled => "cancelled",
        }
    }
}

/// One job the details window can show.
#[derive(Clone, Debug)]
pub struct TrackedJob {
    /// Kind.
    pub kind: JobKind,
    /// Process id.
    pub pid: u32,
    /// Log file.
    pub log: PathBuf,
    /// Shell-style command (argv never carries the key).
    pub cmd: String,
    /// What started it (Scrape, Explore, Executive Board, …).
    pub source: String,
    /// When it started.
    pub started: Instant,
    /// Local wall time at the start.
    pub started_at: String,
    /// When it stopped. Absent while it is running, and absent when a stopped
    /// job was kept without an end. Elapsed then stays `unknown`.
    pub ended: Option<Instant>,
    /// State.
    pub phase: JobPhase,
    /// Exit code, once it has one.
    pub code: Option<i32>,
    /// Failure text, already redacted.
    pub error: String,
    /// Recent log lines, oldest first, already redacted.
    pub tail: VecDeque<String>,
    /// A side job (research, bookmarks), not the main one.
    pub side: bool,
}

/// One row of the details window, copied from [`TrackedJob`] at draw time.
#[derive(Clone, Debug)]
pub(crate) struct JobRow {
    pub pid: u32,
    pub name: String,
    pub source: String,
    pub cmd: String,
    pub state: String,
    pub started_at: String,
    pub elapsed: String,
    pub stage: String,
    pub spinner: String,
    pub exit: String,
    pub error: String,
    pub tail: Vec<String>,
    pub running: bool,
}

/// The job-details window.
#[derive(Debug)]
pub struct JobModal {
    /// Selected job.
    pub(crate) cursor: usize,
    /// First visible log line. `usize::MAX` means the end while [`Self::stick`] is set.
    pub(crate) scroll: usize,
    /// Keep the log pinned to the newest line.
    pub(crate) stick: bool,
    /// Jobs at the last refresh.
    pub(crate) rows: Vec<JobRow>,
    /// Cancel this pid; the event loop drains it and the window stays open.
    pub(crate) pending_cancel: Option<u32>,
}

/// Friendly job name.
pub fn job_label(kind: JobKind) -> &'static str {
    match kind {
        JobKind::Scan => "Scrape",
        JobKind::Export => "Export",
        JobKind::Import => "Import",
        JobKind::Maintain => "Maintain",
        JobKind::Bookmark => "Bookmarks",
        JobKind::Sync => "Sync",
        JobKind::Research => "Research",
        JobKind::Read => "Read",
        JobKind::Write => "Write",
    }
}

/// What started a job, from its command line.
///
/// `--encrypt-db`, `--backup` and `--restore` are recognized when they show up.
/// Those commands still run on the progress screen, not as terminal-UI jobs.
pub fn job_source(kind: JobKind, cmd: &str) -> String {
    let has = |flag: &str| cmd.split_whitespace().any(|a| a == flag);
    if has("--encrypt-db") { return "Encrypt database".into(); }
    if has("--backup") { return "Backup".into(); }
    if has("--restore") { return "Restore".into(); }
    if has("--eb") { return "View › Executive Board".into(); }
    if has("--explore") || has("--explore-latest") { return "Explore".into(); }
    if has("--top") { return "Site Top".into(); }
    if has("--live") { return "View › Live".into(); }
    if has("--live-feed") { return "View › Live feed".into(); }
    if has("--site-notifications") { return "View › Site notifications".into(); }
    if has("--show-profile") { return "Account › Profile".into(); }
    if has("--site-settings") || has("--save-settings") { return "Account › Site settings".into(); }
    if has("--edit-profile") { return "Account › Edit profile".into(); }
    if has("--report-post") || has("--report-user") { return "Report".into(); }
    if has("--spaces") { return "View › Spaces".into(); }
    if has("--sync") || has("--sync-bookmarks") { return "Sync".into(); }
    if has("--research") || has("--research-add") || has("--research-create") { return "Research".into(); }
    if has("--bookmark") || has("--unbookmark") { return "Bookmarks".into(); }
    if has("--export") { return "Export".into(); }
    if has("--import") { return "Import".into(); }
    if has("--remove-nick") || has("--clear-all") { return "Maintain".into(); }
    job_label(kind).into()
}

fn redactors() -> &'static [regex::Regex] {
    static RES: std::sync::LazyLock<Vec<regex::Regex>> = std::sync::LazyLock::new(|| {
        vec![
            regex::Regex::new(r"x'[0-9a-fA-F]+'").expect("hex literal pattern"),
            regex::Regex::new(r"(?i)\b(?:ACP_PASSWORD|password|passwd|secret)\s*[:=]\s*\S+").expect("secret assignment pattern"),
            regex::Regex::new(r"\b[0-9a-fA-F]{64,}\b").expect("hex run pattern"),
        ]
    });
    RES.as_slice()
}

/// Hide a database key, a long hex run, or a password assignment.
///
/// `ACP_INHERIT_DB_KEY=1` is only a signal that a pipe is open. It is not the key.
pub fn redact_line(line: &str) -> String {
    let mut s = line.to_string();
    for re in redactors() {
        s = re.replace_all(&s, "[redacted]").into_owned();
    }
    s
}

fn fmt_elapsed(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// Remember the end the first time the job leaves the running state.
fn stamp_end(job: &mut TrackedJob) {
    if job.ended.is_none() { job.ended = Some(Instant::now()); }
}

impl TrackedJob {
    fn begin(kind: JobKind, pid: u32, log: PathBuf, cmd: &str, side: bool) -> Self {
        Self {
            kind, pid, log, cmd: cmd.to_string(), source: job_source(kind, cmd),
            started: Instant::now(),
            started_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            ended: None,
            phase: JobPhase::Running, code: None, error: String::new(), tail: VecDeque::new(), side,
        }
    }

    /// How long it has run. `None` when it has stopped and the end was not recorded.
    fn run_so_far(&self, now: Instant) -> Option<Duration> {
        match self.phase {
            JobPhase::Running => Some(now.saturating_duration_since(self.started)),
            JobPhase::Finished | JobPhase::Failed | JobPhase::Cancelled => {
                self.ended.map(|end| end.saturating_duration_since(self.started))
            }
        }
    }

    fn row(&self, now: Instant) -> JobRow {
        let stage = self.tail.iter().rev().find(|l| !l.trim().is_empty()).cloned().unwrap_or_else(|| "Starting".into());
        let run = self.run_so_far(now);
        let frame = (run.unwrap_or_default().as_millis() / 80) as usize % SPINNER.len();
        let exit = match (self.phase, self.code) {
            (JobPhase::Running, _) => String::new(),
            (_, Some(c)) => c.to_string(),
            (JobPhase::Cancelled | JobPhase::Failed, None) => "killed".into(),
            _ => "0".into(),
        };
        JobRow {
            pid: self.pid,
            name: job_label(self.kind).into(),
            source: self.source.clone(),
            cmd: redact_line(&self.cmd),
            state: self.phase.label().into(),
            started_at: self.started_at.clone(),
            elapsed: run.map(fmt_elapsed).unwrap_or_else(|| "unknown".into()),
            stage,
            spinner: SPINNER[frame].into(),
            exit,
            error: self.error.clone(),
            tail: self.tail.iter().cloned().collect(),
            running: self.phase == JobPhase::Running,
        }
    }
}

impl JobModal {
    fn selected(&self) -> Option<&JobRow> { self.rows.get(self.cursor) }

    fn move_cursor(&mut self, d: i32) {
        let n = self.rows.len();
        if n == 0 { return; }
        self.cursor = cycle(self.cursor, n, d);
        self.scroll = usize::MAX;
        self.stick = true;
    }

    fn bump_scroll(&mut self, d: i32) {
        let n = self.selected().map(|r| r.tail.len()).unwrap_or(0);
        let cur = if self.stick { n } else { self.scroll.min(n) };
        let next = (cur as i32 + d).clamp(0, n as i32) as usize;
        self.scroll = next;
        self.stick = next >= n;
    }

    fn request_cancel(&mut self) {
        if let Some(row) = self.selected()
            && row.running
        {
            self.pending_cancel = Some(row.pid);
        }
    }

    fn press(&mut self, i: usize) -> Option<ModalResult> {
        let running = self.selected().is_some_and(|r| r.running);
        let n = if running { 2 } else { 1 };
        if i >= n { return None; }
        if running && i == 0 { self.request_cancel(); None } else { Some(ModalResult::ClosedCancelled) }
    }
}

impl ModalState for JobModal {
    fn title(&self) -> String { if self.rows.len() > 1 { "Jobs".into() } else { "Job".into() } }

    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc => Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => None,
            KeyCode::Char('c') | KeyCode::Char('C') => { self.request_cancel(); None }
            KeyCode::Up => { if self.rows.len() > 1 { self.move_cursor(-1); } else { self.bump_scroll(-1); } None }
            KeyCode::Down => { if self.rows.len() > 1 { self.move_cursor(1); } else { self.bump_scroll(1); } None }
            KeyCode::PageUp => { self.bump_scroll(-8); None }
            KeyCode::PageDown => { self.bump_scroll(8); None }
            KeyCode::Char('k') => { self.bump_scroll(-1); None }
            KeyCode::Char('j') => { self.bump_scroll(1); None }
            _ => None,
        }
    }

    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Item(i) if i < self.rows.len() => { self.cursor = i; self.scroll = usize::MAX; self.stick = true; None }
            ModalHit::Button(i) => self.press(i),
            _ => None,
        }
    }

    fn on_scroll(&mut self, delta: i32) { self.bump_scroll(delta * 3); }
}

fn absorb_tail(job: &mut TrackedJob, tail: &[String]) {
    let incoming: Vec<String> = tail.iter().map(|l| redact_line(l)).filter(|l| !l.trim().is_empty()).collect();
    if incoming.is_empty() { return; }
    let have: Vec<&str> = job.tail.iter().map(String::as_str).collect();
    let incoming_ref: Vec<&str> = incoming.iter().map(String::as_str).collect();
    let n = incoming_ref.len();
    let same = have.len() >= n && have[have.len() - n..] == incoming_ref[..];
    if same { return; }
    for l in incoming {
        job.tail.push_back(l);
        while job.tail.len() > TAIL_CAP { job.tail.pop_front(); }
    }
}

impl App {
    fn snapshot(&self, now: Instant) -> Vec<JobRow> {
        let mut v = self.tracked.clone();
        v.sort_by(|a, b| (a.phase != JobPhase::Running).cmp(&(b.phase != JobPhase::Running)).then(a.started.cmp(&b.started)));
        v.iter().map(|j| j.row(now)).collect()
    }

    fn trim_tracked(&mut self) {
        let mut n = self.tracked.iter().filter(|j| j.phase != JobPhase::Running).count();
        while n > FINISHED_KEEP {
            let Some(i) = self.tracked.iter().position(|j| j.phase != JobPhase::Running) else { break };
            self.tracked.remove(i);
            n -= 1;
        }
    }

    /// Remember a job so the details window can show it.
    pub(crate) fn remember_job(&mut self, kind: JobKind, pid: u32, log: PathBuf, cmd: &str, side: bool) {
        self.tracked.retain(|j| j.pid != pid);
        self.tracked.push(TrackedJob::begin(kind, pid, log, cmd, side));
        self.trim_tracked();
        self.refresh_job_modal();
    }

    /// A side job started (research, bookmarks).
    pub(crate) fn track_side(&mut self, kind: JobKind, pid: u32, log: PathBuf, cmd: &str) {
        self.remember_job(kind, pid, log, cmd, true);
        self.push_status(format!("started {} (pid {pid}): {cmd}", kind.name()));
    }

    /// Append one log line. Secrets are removed before it is stored.
    pub(crate) fn note_job_line(&mut self, pid: u32, line: &str) {
        let line = redact_line(line);
        if line.trim().is_empty() { return; }
        {
            let Some(j) = self.tracked.iter_mut().find(|j| j.pid == pid && j.phase == JobPhase::Running) else { return };
            j.tail.push_back(line);
            while j.tail.len() > TAIL_CAP { j.tail.pop_front(); }
        }
        self.refresh_job_modal();
    }

    /// The job ended. `tail` is its last log lines.
    pub(crate) fn settle_job(&mut self, pid: u32, ok: bool, code: Option<i32>, tail: &[String]) {
        if let Some(j) = self.tracked.iter_mut().find(|j| j.pid == pid) {
            absorb_tail(j, tail);
            stamp_end(j);
            j.phase = if ok { JobPhase::Finished } else { JobPhase::Failed };
            j.code = code;
            if !ok {
                let detail = tail.iter().rev().find(|l| !l.trim().is_empty()).map(|l| l.as_str()).unwrap_or("no output");
                j.error = redact_line(detail);
            }
        }
        self.needs_redraw = true;
        self.trim_tracked();
        self.refresh_job_modal();
    }

    /// The user stopped this job.
    pub(crate) fn mark_job_cancelled(&mut self, pid: u32) {
        if let Some(j) = self.tracked.iter_mut().find(|j| j.pid == pid) {
            stamp_end(j);
            j.phase = JobPhase::Cancelled;
        }
        self.needs_redraw = true;
        self.refresh_job_modal();
    }

    /// Header text for the running job, when there is one.
    pub(crate) fn header_running(&self) -> Option<String> {
        if let Some(j) = &self.job {
            return Some(format!("⟳ {} running (pid {})", j.kind.name(), j.pid));
        }
        self.tracked.iter().find(|t| t.phase == JobPhase::Running)
            .map(|j| format!("⟳ {} running (pid {})", j.kind.name(), j.pid))
    }

    /// A job is still running (main or side).
    pub(crate) fn has_running_job(&self) -> bool {
        self.job.is_some() || self.tracked.iter().any(|j| j.phase == JobPhase::Running)
    }

    /// Open the details window. With nothing to show, say so in the status bar.
    pub(crate) fn open_jobs(&mut self) {
        if self.tracked.is_empty() {
            self.push_status("no job to show");
            self.needs_redraw = true;
            return;
        }
        let rows = self.snapshot(Instant::now());
        let cursor = rows.iter().position(|r| r.running).unwrap_or(0);
        self.modal = Some(Modal::Job(JobModal { cursor, scroll: usize::MAX, stick: true, rows, pending_cancel: None }));
        self.needs_redraw = true;
    }

    /// Copy the live jobs into an open details window, measured at `Instant::now`.
    pub(crate) fn refresh_job_modal(&mut self) {
        self.refresh_job_modal_at(Instant::now());
    }

    /// Copy the live jobs into an open details window, measured at `now`.
    ///
    /// A running job uses `now`. A stopped job uses its recorded end, so a later
    /// `now` does not change the elapsed line.
    pub(crate) fn refresh_job_modal_at(&mut self, now: Instant) {
        let pid = match &self.modal {
            Some(Modal::Job(m)) => m.rows.get(m.cursor).map(|r| r.pid),
            _ => return,
        };
        let rows = self.snapshot(now);
        let Some(Modal::Job(m)) = &mut self.modal else { return };
        m.rows = rows;
        if let Some(pid) = pid {
            if let Some(i) = m.rows.iter().position(|r| r.pid == pid) { m.cursor = i; }
            else if !m.rows.is_empty() { m.cursor = m.cursor.min(m.rows.len() - 1); }
        }
        if m.stick { m.scroll = usize::MAX; }
    }

    /// A cancel asked for by the details window, if any.
    pub(crate) fn take_job_cancel(&mut self) -> Option<u32> {
        let Some(Modal::Job(m)) = &mut self.modal else { return None };
        m.pending_cancel.take()
    }

    /// Cancel a running job. The window stays open and follows the new state.
    pub(crate) fn cancel_tracked(&mut self, pid: u32) {
        let Some(j) = self.tracked.iter().find(|j| j.pid == pid && j.phase == JobPhase::Running) else { return };
        if j.side { self.effects.push(Effect::CancelSide(pid)); }
        else if self.job.as_ref().is_some_and(|jv| jv.pid == pid) { self.effects.push(Effect::CancelJob); }
    }
}
