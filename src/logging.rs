//! Logging levels (`-q`, default, `-v`, `--debug`) and the `info!`, `verbose!`, `err!` macros.


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
    if log_level() >= LogLevel::Normal { eprintln!("{args}"); }
}

/// Extra detail (shown with --verbose / --debug).
pub fn log_verbose(args: std::fmt::Arguments<'_>) {
    if log_level() >= LogLevel::Verbose { eprintln!("{args}"); }
}

/// Always (errors / warnings users must see).
pub fn log_error(args: std::fmt::Arguments<'_>) {
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
        eprintln!("[debug {dir}] {cut}{more}");
    }
}
