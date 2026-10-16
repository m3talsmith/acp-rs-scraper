//! Logging levels (`-q`, default, `-v`, `--debug`) and the `info!`, `verbose!`, `err!` macros.
//!
//! With `--enable-logging` (or `--debug`, `--log`, `--error-log`) every message the macros emit
//! is also appended, timestamped, to a log file: progress, verbose detail and debug wire frames
//! go to the success log, errors and warnings to the error log. Stderr still follows `-q`/`-v`.


/// Set when the log level is Debug (fast check for wire logging).
pub static DEBUG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Verbosity of stderr output.
pub enum LogLevel {
    /// `-q`: prompts and errors only.
    Quiet = 0,
    /// Default: progress and summaries.
    Normal = 1,
    /// `-v`: per-page / per-thread detail.
    Verbose = 2,
    /// `--debug` / `ACP_DEBUG=1`: raw LiveView wire log.
    Debug = 3,
}

/// Process-wide log level, stored as `LogLevel as u8`.
pub static LOG_LEVEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1);

/// Current log level.
pub fn log_level() -> LogLevel {
    match LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) {
        0 => LogLevel::Quiet,
        2 => LogLevel::Verbose,
        3 => LogLevel::Debug,
        _ => LogLevel::Normal,
    }
}

/// Set the log level (and the debug flag) for the whole process.
pub fn set_log_level(l: LogLevel) {
    LOG_LEVEL.store(l as u8, std::sync::atomic::Ordering::Relaxed);
    DEBUG.store(l >= LogLevel::Debug, std::sync::atomic::Ordering::Relaxed);
}

/// Progress / summary (hidden by --quiet).
pub fn log_info(args: std::fmt::Arguments<'_>) {
    file_line(false, "info", &args);
    if log_level() >= LogLevel::Normal { eprintln!("{args}"); }
}

/// Extra detail (shown with --verbose / --debug).
pub fn log_verbose(args: std::fmt::Arguments<'_>) {
    file_line(false, "verbose", &args);
    if log_level() >= LogLevel::Verbose { eprintln!("{args}"); }
}

/// Always (errors / warnings users must see).
pub fn log_error(args: std::fmt::Arguments<'_>) {
    file_line(true, "error", &args);
    eprintln!("{args}");
}

macro_rules! info { ($($t:tt)*) => { $crate::logging::log_info(format_args!($($t)*)) }; }

macro_rules! verbose { ($($t:tt)*) => { $crate::logging::log_verbose(format_args!($($t)*)) }; }

macro_rules! err { ($($t:tt)*) => { $crate::logging::log_error(format_args!($($t)*)) }; }

/// `--debug` only: print a wire message, cut at 2000 characters.
pub fn dbg_log(dir: &str, msg: &str) {
    if log_level() >= LogLevel::Debug {
        let cut: String = msg.chars().take(2000).collect();
        let more = if cut.len() < msg.len() { format!(" ...[{} bytes]", msg.len()) } else { String::new() };
        file_line(false, "debug", &format_args!("[{dir}] {cut}{more}"));
        eprintln!("[debug {dir}] {cut}{more}");
    }
}

/// Open append-mode log files (success and error), shared by every thread.
struct FileSinks {
    success: std::sync::Mutex<std::fs::File>,
    error: std::sync::Mutex<std::fs::File>,
}

static SINKS: std::sync::OnceLock<FileSinks> = std::sync::OnceLock::new();

/// Resolve the log paths: `None` when logging is off. Logging is on with `enable`, `debug`,
/// or when either path is given; a missing path falls back to `~/.acp/logs/{success,error}.log`.
pub fn resolve_log_paths(
    enable: bool, debug: bool, log: Option<&std::path::Path>, error_log: Option<&std::path::Path>,
    home: &std::path::Path,
) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    if !(enable || debug || log.is_some() || error_log.is_some()) { return None; }
    let dir = home.join(crate::config::LOGS_REL_DIR);
    Some((
        log.map(|p| p.to_path_buf()).unwrap_or_else(|| dir.join(crate::config::SUCCESS_LOG_NAME)),
        error_log.map(|p| p.to_path_buf()).unwrap_or_else(|| dir.join(crate::config::ERROR_LOG_NAME)),
    ))
}

fn open_append(p: &std::path::Path) -> std::io::Result<std::fs::File> {
    if let Some(d) = p.parent().filter(|d| !d.as_os_str().is_empty()) { std::fs::create_dir_all(d)?; }
    std::fs::OpenOptions::new().create(true).append(true).open(p)
}

/// Start file logging (creating parent directories). Only the first call takes effect.
pub fn init_file_logging(success: &std::path::Path, error: &std::path::Path) -> std::io::Result<()> {
    let s = open_append(success)?;
    let e = open_append(error)?;
    let _ = SINKS.set(FileSinks { success: std::sync::Mutex::new(s), error: std::sync::Mutex::new(e) });
    Ok(())
}

/// Whether file logging is active.
pub fn file_logging_enabled() -> bool { SINKS.get().is_some() }

/// One log line: `2026-10-15T21:34:00.123-04:00 [level] message`.
pub fn format_line(level: &str, args: &std::fmt::Arguments<'_>) -> String {
    format!("{} [{level}] {args}\n", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z"))
}

fn file_line(is_err: bool, level: &str, args: &std::fmt::Arguments<'_>) {
    use std::io::Write;
    let Some(s) = SINKS.get() else { return };
    let m = if is_err { &s.error } else { &s.success };
    if let Ok(mut f) = m.lock() { let _ = f.write_all(format_line(level, args).as_bytes()); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn off_without_flags() {
        assert!(resolve_log_paths(false, false, None, None, Path::new("/h")).is_none());
    }

    #[test]
    fn debug_implies_enable_with_defaults() {
        let (s, e) = resolve_log_paths(false, true, None, None, Path::new("/h")).unwrap();
        assert_eq!(s, Path::new("/h/.acp/logs/success.log"));
        assert_eq!(e, Path::new("/h/.acp/logs/error.log"));
    }

    #[test]
    fn path_overrides_imply_enable() {
        let (s, e) = resolve_log_paths(false, false, Some(Path::new("/x/a.log")), None, Path::new("/h")).unwrap();
        assert_eq!(s, Path::new("/x/a.log"));
        assert_eq!(e, Path::new("/h/.acp/logs/error.log"));
    }

    #[test]
    fn writes_timestamped_lines_to_the_right_file() {
        let dir = std::env::temp_dir().join(format!("acp-log-test-{}", std::process::id()));
        let (s, e) = (dir.join("sub/success.log"), dir.join("sub/error.log"));
        init_file_logging(&s, &e).unwrap();
        log_info(format_args!("hello info"));
        log_error(format_args!("boom"));
        let sv = std::fs::read_to_string(&s).unwrap();
        let ev = std::fs::read_to_string(&e).unwrap();
        assert!(sv.contains("[info] hello info") && !sv.contains("boom"));
        assert!(ev.contains("[error] boom") && !ev.contains("hello info"));
        assert!(sv.starts_with("20"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
