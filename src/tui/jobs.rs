//! Background jobs: scans, exports, imports and maintenance run as the headless
//! CLI in a child process (same binary, argv built by [`crate::tui::commands`]),
//! in its own process group with output in `~/.acp/logs/tui-<kind>-<stamp>.log`.
//! Cancel kills it; quitting the TUI leaves it running (detached).

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
}

impl JobKind {
    /// Lower-case name (log file names, status).
    pub fn name(self) -> &'static str {
        match self { JobKind::Scan => "scan", JobKind::Export => "export", JobKind::Import => "import", JobKind::Maintain => "maintain" }
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

impl Job {
    /// Start `argv` (`argv[0]` is ignored; this binary runs) with `exe`, logging to `log`.
    pub fn spawn_with(exe: &Path, kind: JobKind, argv: &[String], log: PathBuf) -> Result<Job> {
        let f = open_log(&log)?;
        let mut c = std::process::Command::new(exe);
        c.args(argv.iter().skip(1))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(f);
        // Own process group: terminal signals (Ctrl-C, closing the TUI) don't reach it.
        #[cfg(unix)]
        { use std::os::unix::process::CommandExt; c.process_group(0); }
        let child = c.spawn().with_context(|| format!("starting {}", exe.display()))?;
        Ok(Job { kind, child, log, cmd: crate::tui::commands::shell_line(argv), offset: 0, partial: String::new() })
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
