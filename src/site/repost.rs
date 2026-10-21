//! Repost. The route is the one in the plan. The form has not been captured,
//! so this build does not invent it and does not send it.

use super::write::{WriteError, CAPTURE_BLOCK};
use crate::prelude::*;

const FIXTURE: &str = include_str!("../../tests/fixtures/site/repost.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("repost fixture")
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

fn needed() -> String {
    let mut s = String::from("capture still needed (no repost fixture from the site):\n");
    for line in fixture()["needed"].as_array().into_iter().flatten() {
        if let Some(line) = line.as_str() {
            s.push_str("- ");
            s.push_str(line);
            s.push('\n');
        }
    }
    s
}

/// The route named in the plan. This is not a captured request.
pub fn route(id: &str) -> String {
    format!("/rs/post/new?repost={id}")
}

/// Preview or refuse. The form fields are not built. A confirmed repost is
/// an error and does not open a connection.
pub fn present(id: &str, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = id.trim();
    if id.is_empty() || id.chars().any(|c| c.is_whitespace()) {
        return Err(err("a repost needs a post id"));
    }
    let text = format!("dry-run: no connection opened\nrepost {id}\nroute: {}\nthe repost form is not built\n\n{}", route(id), needed());
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
    fn the_stub_names_the_route_and_does_not_build_a_form() {
        let fix = fixture();
        assert_eq!(fix["capture_status"], "missing");
        assert!(fix["payload"].is_null());
        assert_eq!(route("POST"), "/rs/post/new?repost=POST");
        let dry = present("POST", false, true).unwrap();
        assert!(dry.contains("dry-run: no connection opened"), "{dry}");
        assert!(dry.contains("the repost form is not built"), "{dry}");
        assert!(dry.contains("/rs/post/new?repost=POST"), "{dry}");
        assert!(!dry.contains("form[body_input]"), "{dry}");
        assert!(!dry.contains("\"quoted_post\""), "{dry}");
        assert!(!dry.contains("nothing was published"), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        for line in fix["needed"].as_array().unwrap() {
            assert!(dry.contains(line.as_str().unwrap()), "{dry}");
        }
        assert_eq!(present("POST", false, false).unwrap(), "nothing sent");
        let blocked = present("POST", true, false).unwrap_err();
        assert!(blocked.message.contains("nothing was published"), "{}", blocked.message);
        assert!(blocked.message.contains("the repost form is not built"), "{}", blocked.message);
        assert!(blocked.message.contains("capture still needed"), "{}", blocked.message);
    }
}
