//! Logging levels (`-q`, default, `-v`, `--debug`) and the `info!`, `verbose!`, `err!` macros.
//!
//! With `--enable-logging` (or `--debug`, `--log`, `--error-log`) every message the macros emit
//! is also appended, timestamped, to a log file: progress, verbose detail and debug wire frames
//! go to the success log, errors and warnings to the error log. Stderr still follows `-q`/`-v`.
//! Debug wire frames are passed through [`redact_secrets`] so CSRF tokens and signed sessions
//! never land in stderr or the success log.


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
    if log_level() >= LogLevel::Normal { emit(&args); }
}

/// Extra detail (shown with --verbose / --debug).
pub fn log_verbose(args: std::fmt::Arguments<'_>) {
    file_line(false, "verbose", &args);
    if log_level() >= LogLevel::Verbose { emit(&args); }
}

/// Always (errors / warnings users must see).
pub fn log_error(args: std::fmt::Arguments<'_>) {
    file_line(true, "error", &args);
    emit(&args);
}

/// Where stderr-bound lines go instead while the terminal UI owns the screen.
static SCREEN_SINK: std::sync::Mutex<Option<std::sync::mpsc::Sender<String>>> = std::sync::Mutex::new(None);

/// Route every `info!` / `verbose!` / `err!` / debug line to `tx` instead of stderr
/// (the TUI shows them in its status bar); `None` restores stderr.
pub fn set_log_sink(tx: Option<std::sync::mpsc::Sender<String>>) {
    if let Ok(mut s) = SCREEN_SINK.lock() { *s = tx; }
}

/// Print one line on stderr, or hand it to the sink set by [`set_log_sink`].
fn emit(args: &dyn std::fmt::Display) {
    if let Ok(s) = SCREEN_SINK.lock()
        && let Some(tx) = s.as_ref() {
            let _ = tx.send(args.to_string());
            return;
        }
    eprintln!("{args}");
}

macro_rules! info { ($($t:tt)*) => { $crate::logging::log_info(format_args!($($t)*)) }; }

macro_rules! verbose { ($($t:tt)*) => { $crate::logging::log_verbose(format_args!($($t)*)) }; }

macro_rules! err { ($($t:tt)*) => { $crate::logging::log_error(format_args!($($t)*)) }; }

/// Placeholder shown in place of a redacted secret value.
const REDACTED: &str = "[REDACTED]";

/// JSON object keys whose string values must never appear in debug output.
const SECRET_JSON_KEY_RE: &str = concat!(
    r#"(?i)"("#,
    r"_csrf_token|csrf_token|csrf-token|session|static|cookie|",
    r"authorization|user_token|access_token|refresh_token|token",
    r#")"\s*:\s*"(?:\\.|[^"\\])*""#,
);

/// Mask auth tokens in a debug / wire payload before it hits stderr or the success log.
///
/// Replaces JSON string values for known secret keys (`_csrf_token`, `session`, `static`,
/// cookies, bearer-style tokens), `_csrf_token=` query params, Phoenix signed blobs
/// (`SFMyNTY....`), one-time codes in `user[otp]` / `user[code]` form fields, and long
/// hidden-input `value="..."` attributes that carry CSRF tokens in LiveView HTML diffs.
/// Event names, topics, URLs, and post structure stay visible. Cookie values are not logged.
pub fn redact_secrets(s: &str) -> String {
    use std::sync::LazyLock;
    static JSON_KEYS: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(SECRET_JSON_KEY_RE).expect("secret json key re"));
    static CSRF_QUERY: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"_csrf_token=[^&\s"\\]+"#).expect("csrf query re")
    });
    static PHOENIX_SIGNED: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"SFMyNTY\.[A-Za-z0-9+/=_.-]+").expect("phoenix signed re")
    });
    // LiveView diffs often split `<input name="_csrf_token" ... value="TOKEN">` across JSON
    // strings; catch long token-shaped value="..." / value=\"..." attributes.
    static HTML_VALUE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"((?:^|[\s])value=)(\\?")([A-Za-z0-9_-]{32,})(\\?")"#)
            .expect("html value re")
    });

    let mut out = JSON_KEYS
        .replace_all(s, |caps: &regex::Captures| format!(r#""{}":"{REDACTED}""#, &caps[1]))
        .into_owned();
    out = CSRF_QUERY.replace_all(&out, format!("_csrf_token={REDACTED}")).into_owned();
    // One-time codes travel in the form body (`user[otp]` / `user[code]`), not as a JSON key.
    static OTP_FIELD: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"(?i)(user(?:%5[Bb]|\[)(?:otp|code)(?:%5[Dd]|])=)[^&\s"\\]+"#)
            .expect("otp field re")
    });
    out = OTP_FIELD
        .replace_all(&out, |caps: &regex::Captures| format!("{}{REDACTED}", &caps[1]))
        .into_owned();
    out = PHOENIX_SIGNED.replace_all(&out, format!("SFMyNTY.{REDACTED}")).into_owned();
    out = HTML_VALUE
        .replace_all(&out, |caps: &regex::Captures| {
            format!("{}{}{}{}", &caps[1], &caps[2], REDACTED, &caps[4])
        })
        .into_owned();
    static JWT: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}").expect("jwt re")
    });
    static LIVEKIT: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"(?i)"(token|livekit_token)"\s*:\s*"(?:\\.|[^"\\])*""#).expect("livekit re")
    });
    out = JWT.replace_all(&out, REDACTED).into_owned();
    out = LIVEKIT
        .replace_all(&out, |caps: &regex::Captures| format!(r#""{}":"{REDACTED}""#, &caps[1]))
        .into_owned();
    out
}

/// `--debug` only: print a wire message (secrets redacted), cut at 2000 characters.
pub fn dbg_log(dir: &str, msg: &str) {
    if log_level() >= LogLevel::Debug {
        let safe = redact_secrets(msg);
        let cut: String = safe.chars().take(2000).collect();
        let more = if cut.len() < safe.len() { format!(" ...[{} bytes]", safe.len()) } else { String::new() };
        file_line(false, "debug", &format_args!("[{dir}] {cut}{more}"));
        emit(&format_args!("[debug {dir}] {cut}{more}"));
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

    #[test]
    fn redacts_liveview_join_csrf_session_and_static() {
        let raw = concat!(
            r#"["1","1","lv:phx-GN7feKAwRQZGuY5j","phx_join","#,
            r#"{"params":{"_csrf_token":"HCcyDg18Q2NxEBRVAXIODCFaBWUKCxYVnmfegKtS6wC9N0QmGlG5_ELF","_mounts":0},"#,
            r#""session":"SFMyNTY.g2gDaAJhBnQAAAAIdwJpZG0AAAAUcGh4LUdON2ZlS0F3UlFaR3VZNWp3B3Nlc3Npb24.sig","#,
            r#""static":"SFMyNTY.g2gDaAJhBnQAAAADdwJpZG0.othersig","#,
            r#""url":"https://acp.us/rs/@hazaldin"}]"#,
        );
        let out = redact_secrets(raw);
        assert!(!out.contains("HCcyDg18Q2Nx"), "csrf leaked: {out}");
        assert!(!out.contains("SFMyNTY.g2g"), "phoenix blob leaked: {out}");
        assert!(out.contains(r#""_csrf_token":"[REDACTED]""#), "{out}");
        assert!(out.contains(r#""session":"[REDACTED]""#), "{out}");
        assert!(out.contains(r#""static":"[REDACTED]""#), "{out}");
        assert!(out.contains("phx_join") && out.contains("@hazaldin") && out.contains(r#""_mounts":0"#));
    }

    #[test]
    fn redacts_csrf_query_param_and_html_value_attr() {
        let raw = concat!(
            r#"https://acp.us/live/websocket?_csrf_token=HCcyDg18Q2NxEBRVAXIODCFaBWUKCxYV&vsn=2.0.0 "#,
            r#"{"0":" value=\"XHZqBQAaKRRKGShlSA92OWsHBkUFNzR7jD0HcjPZ9qKVq7DQ1WH6BbVM\"","#,
            r#""s":["<input name=\"_csrf_token\" type=\"hidden\" hidden",">"]}"#,
        );
        let out = redact_secrets(raw);
        assert!(!out.contains("HCcyDg18Q2Nx"), "{out}");
        assert!(!out.contains("XHZqBQAaKRRKGShl"), "{out}");
        assert!(out.contains("_csrf_token=[REDACTED]"), "{out}");
        assert!(
            out.contains("value=\\\"[REDACTED]\\\"") || out.contains(r#"value=\"[REDACTED]\""#),
            "{out}"
        );
        assert!(out.contains(r#"name=\"_csrf_token\""#));
    }

    #[test]
    fn redacts_otp_in_a_form_body() {
        let raw = "value=user%5Bemail%5D=ada%40example.test&user%5Botp%5D=KWMRUV&user%5Bremember_me%5D=true";
        let out = redact_secrets(raw);
        assert!(!out.contains("KWMRUV"), "{out}");
        assert!(out.contains("user%5Botp%5D=[REDACTED]"), "{out}");
        assert!(out.contains("ada%40example.test"), "{out}");
        let raw = r#"user[code]=SECRET12&user[email]=ada@example.test"#;
        let out = redact_secrets(raw);
        assert!(!out.contains("SECRET12"), "{out}");
        assert!(out.contains("user[code]=[REDACTED]"), "{out}");
    }

    #[test]
    fn redacts_jwt_and_livekit_token() {
        // Pieces, so the source does not contain a JWT-shaped string.
        let raw = format!(
            r#"{{"token":"{}.{}.{}","event":"space-join"}}"#,
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
            "eyJzdWIiOiIxMjM0NTY3ODkwIn0",
            "dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
        );
        let out = redact_secrets(&raw);
        assert!(!out.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"), "{out}");
        assert!(out.contains("space-join"), "{out}");
        assert!(out.contains("[REDACTED]"), "{out}");
    }
}
