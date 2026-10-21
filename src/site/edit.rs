//! Edit. The one-hour window is a product constraint. The form itself has
//! not been captured, so this build does not invent it and does not send it.

use super::write::{WriteError, CAPTURE_BLOCK};
use crate::prelude::*;

const FIXTURE: &str = include_str!("../../tests/fixtures/site/edit.json");

/// An edit stays open for one hour after the post.
pub const EDIT_WINDOW_SECS: i64 = 3600;

/// Whether the one-hour window is still open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// `inserted_at` is within the hour.
    Open,
    /// `inserted_at` is older than the hour.
    Closed,
    /// No timestamp was supplied, so the hour was not checked.
    Unknown,
}

/// `Closed` when `now - inserted` is greater than one hour.
pub fn window(inserted_unix: Option<i64>, now_unix: i64) -> Window {
    match inserted_unix {
        Some(at) if now_unix.saturating_sub(at) > EDIT_WINDOW_SECS => Window::Closed,
        Some(_) => Window::Open,
        None => Window::Unknown,
    }
}

/// Parse an RFC 3339 `posted_at` into unix seconds.
pub fn posted_unix(posted_at: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(posted_at.trim()).ok().map(|t| t.timestamp())
}

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("edit fixture")
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

fn needed() -> String {
    let mut s = String::from("capture still needed (no edit fixture from the site):\n");
    for line in fixture()["needed"].as_array().into_iter().flatten() {
        if let Some(line) = line.as_str() {
            s.push_str("- ");
            s.push_str(line);
            s.push('\n');
        }
    }
    s
}

/// Preview or refuse. The form fields are not built. A post older than one
/// hour is refused before that. A confirmed edit inside the hour is an error
/// and does not open a connection.
pub fn present(id: &str, inserted_unix: Option<i64>, now_unix: i64, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = id.trim();
    if id.is_empty() || id.chars().any(|c| c.is_whitespace()) {
        return Err(err("an edit needs a post id"));
    }
    if window(inserted_unix, now_unix) == Window::Closed {
        return Err(err("an edit stays open for one hour after the post. Nothing was sent."));
    }
    let hour = if window(inserted_unix, now_unix) == Window::Open {
        "within one hour"
    } else {
        "not checked (no posted_at)"
    };
    let text = format!(
        "dry-run: no connection opened\nedit {id}\nwindow: {hour}\nthe edit form is not built\n\n{}",
        needed()
    );
    if dry_run {
        return Ok(text);
    }
    if !confirmed {
        return Ok("nothing sent".into());
    }
    Err(err(format!("{CAPTURE_BLOCK}\n\n{text}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hour_closes_and_the_open_window_does_not_build_a_form() {
        let fix = fixture();
        assert_eq!(fix["capture_status"], "missing");
        assert!(fix["payload"].is_null());
        assert_eq!(fix["window_secs"], EDIT_WINDOW_SECS);
        let now = 10_000;
        assert_eq!(window(Some(now - 60), now), Window::Open);
        assert_eq!(window(Some(now - EDIT_WINDOW_SECS), now), Window::Open);
        assert_eq!(window(Some(now - EDIT_WINDOW_SECS - 1), now), Window::Closed);
        assert_eq!(window(None, now), Window::Unknown);
        assert!(posted_unix("2026-10-19T23:38:10Z").is_some());
        assert!(posted_unix("not a time").is_none());
        let closed = present("POST", Some(now - 7_200), now, true, true).unwrap_err();
        assert!(closed.message.contains("one hour"), "{}", closed.message);
        assert!(!closed.message.contains("nothing was published"), "{}", closed.message);
        assert!(!closed.message.contains("https://acp.us"), "{}", closed.message);
        let dry = present("POST", Some(now - 60), now, false, true).unwrap();
        assert!(dry.contains("the edit form is not built"), "{dry}");
        assert!(dry.contains("within one hour"), "{dry}");
        assert!(!dry.contains("form[body_input]"), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        for line in fix["needed"].as_array().unwrap() {
            assert!(dry.contains(line.as_str().unwrap()), "{dry}");
        }
        let unknown = present("POST", None, now, false, true).unwrap();
        assert!(unknown.contains("not checked"), "{unknown}");
        assert_eq!(present("POST", Some(now), now, false, false).unwrap(), "nothing sent");
        let blocked = present("POST", Some(now), now, true, false).unwrap_err();
        assert!(blocked.message.contains("are not sent"), "{}", blocked.message);
        assert!(blocked.message.contains("the edit form is not built"), "{}", blocked.message);
    }
}
