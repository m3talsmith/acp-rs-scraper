//! Background jobs: scans, exports, imports and maintenance run as the headless
//! CLI in a child process (same binary, argv built by [`crate::tui::commands`]),
//! in its own process group with output in `~/.acp/logs/tui-<kind>-<stamp>.log`.
//! Those flags call a loaded service when it implements the job. Until one does,
//! those commands are not in this binary.
//! Cancel kills it; quitting the TUI leaves it running (detached).
//!
//! When this process has already unlocked the database, the child receives that
//! 32-byte local key on stdin. `ACP_INHERIT_DB_KEY=1` only marks the pipe; the
//! key is not an argument and not an environment value. A child with nothing to
//! unlock (and the tests that run `/bin/sh`) gets a closed stdin and no flag.

use crate::prelude::*;

/// What a job does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    /// Scrape (Start Scanning).
    Scan,
    /// `--export`.
    Export,
    /// `--import`.
    Import,
    /// `--remove-nick` / `--clear-all`.
    Maintain,
    /// `--bookmark` / `--unbookmark` (a side job: runs next to the main job).
    Bookmark,
    /// `--sync` / `--sync-bookmarks` with the site.
    Sync,
    /// `--research-add` / `--research-create` (a side job).
    Research,
    /// A read-only site view (`--top`, and the later read-parity flags).
    Read,
    /// `--org-create`, `--org-post`, and the other organization flags.
    Org,
}

impl JobKind {
    /// Lower-case name (log file names, status).
    pub fn name(self) -> &'static str {
        match self { JobKind::Scan => "scan", JobKind::Export => "export", JobKind::Import => "import", JobKind::Maintain => "maintain",
            JobKind::Bookmark => "bookmark", JobKind::Sync => "sync", JobKind::Research => "research", JobKind::Read => "read",
            JobKind::Org => "org" }
    }
}

/// A running child process.
pub struct Job {
    /// What it does.
    pub kind: JobKind,
    /// The child.
    pub child: std::process::Child,
    /// Its log file.
    pub log: PathBuf,
    /// Shell-style command line (no secrets: argv never carries credentials).
    pub cmd: String,
    offset: u64,
    partial: String,
}

/// Log path for a new job.
pub fn job_log_path(kind: JobKind) -> Result<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    Ok(home()?.join(LOGS_REL_DIR).join(format!("{TUI_JOB_LOG_PREFIX}{}-{stamp}.log", kind.name())))
}

fn open_log(p: &Path) -> Result<std::fs::File> {
    if let Some(d) = p.parent() { std::fs::create_dir_all(d)?; }
    let mut o = std::fs::OpenOptions::new();
    o.create(true).append(true);
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    Ok(o.open(p)?)
}

/// The local key of the default database, when this process has unlocked it.
fn session_key() -> Option<zeroize::Zeroizing<[u8; 32]>> {
    let home = crate::config::home().ok()?;
    let db = home.join(crate::config::DB_REL_PATH);
    crate::crypto::atrest::cached_key(&db)
}

/// Start `exe`. When `key` is set, stdin is that 32-byte local key and
/// `ACP_INHERIT_DB_KEY` is `1`. Otherwise stdin is closed and the flag is cleared.
pub(crate) fn spawn_child(exe: &Path, kind: JobKind, argv: &[String], log: PathBuf, key: Option<&[u8; 32]>) -> Result<Job> {
    let f = open_log(&log)?;
    let mut c = std::process::Command::new(exe);
    c.args(argv.iter().skip(1))
        .stdout(std::process::Stdio::null())
        .stderr(f);
    if key.is_some() {
        c.stdin(std::process::Stdio::piped()).env("ACP_INHERIT_DB_KEY", "1");
    } else {
        c.stdin(std::process::Stdio::null()).env_remove("ACP_INHERIT_DB_KEY");
    }
    // Own process group: terminal signals (Ctrl-C, closing the TUI) don't reach it.
    #[cfg(unix)]
    { use std::os::unix::process::CommandExt; c.process_group(0); }
    let mut child = c.spawn().with_context(|| format!("starting {}", exe.display()))?;
    if let Some(key) = key {
        let Some(mut stdin) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            bail!("the key pipe was not created");
        };
        if let Err(e) = stdin.write_all(key) {
            drop(stdin);
            let _ = child.kill();
            let _ = child.wait();
            return Err(e).context("passing the unlocked database key");
        }
        drop(stdin);
    }
    Ok(Job { kind, child, log, cmd: crate::tui::commands::shell_line(argv), offset: 0, partial: String::new() })
}

impl Job {
    /// Start `argv` (`argv[0]` is ignored; this binary runs) with `exe`, logging to `log`.
    ///
    /// Passes the unlocked database key when this process has one. argv stays free of credentials.
    pub fn spawn_with(exe: &Path, kind: JobKind, argv: &[String], log: PathBuf) -> Result<Job> {
        let key = session_key();
        spawn_child(exe, kind, argv, log, key.as_deref())
    }
    /// Start `argv` with this binary, logging to a fresh `~/.acp/logs/tui-*.log`.
    pub fn spawn(kind: JobKind, argv: &[String]) -> Result<Job> {
        Job::spawn_with(&std::env::current_exe()?, kind, argv, job_log_path(kind)?)
    }
    /// Process id.
    pub fn pid(&self) -> u32 { self.child.id() }
    /// New complete log lines since the last call, and the exit status once it ended.
    pub fn poll(&mut self) -> (Vec<String>, Option<std::process::ExitStatus>) {
        let status = self.child.try_wait().ok().flatten();
        let mut lines = Vec::new();
        if let Ok(mut f) = std::fs::File::open(&self.log) {
            use std::io::Seek;
            if f.seek(std::io::SeekFrom::Start(self.offset)).is_ok() {
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_ok() {
                    self.offset += buf.len() as u64;
                    self.partial.push_str(&String::from_utf8_lossy(&buf));
                    while let Some(i) = self.partial.find('\n') {
                        let l: String = self.partial.drain(..=i).collect();
                        lines.push(l.trim_end().to_string());
                    }
                }
            }
        }
        if status.is_some() && !self.partial.is_empty() { lines.push(std::mem::take(&mut self.partial)); }
        (lines, status)
    }
    /// Stop it now (SIGKILL) and reap it. SQLite rolls back any half-written transaction.
    pub fn cancel(&mut self) -> Result<()> {
        let _ = self.child.kill();
        self.child.wait()?;
        Ok(())
    }
}

/// Last `n` lines of a log file.
pub fn tail(p: &Path, n: usize) -> Vec<String> {
    let s = std::fs::read_to_string(p).unwrap_or_default();
    let v: Vec<&str> = s.lines().collect();
    v[v.len().saturating_sub(n)..].iter().map(|l| l.to_string()).collect()
}
