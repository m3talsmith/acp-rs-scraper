//! Delete. The hook name is the one in the plan. No live frame has been
//! captured, so a confirmed delete does not open a connection.

use super::write::{WriteError, CAPTURE_BLOCK};
use crate::prelude::*;

const FIXTURE: &str = include_str!("../../tests/fixtures/site/delete.json");

/// `delete-post {id}`, as named in the plan. Not a captured frame.
pub fn hook(id: &str) -> Value {
    json!({"type": "hook", "event": "delete-post", "value": {"id": id}})
}

/// Phoenix event frame. `join_ref`, `ref`, and the topic are placeholders.
pub fn frame(id: &str) -> Value {
    json!(["1", "1", "lv:<id>", "event", hook(id)])
}

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("delete fixture")
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

fn needed() -> String {
    let mut s = String::from("capture still needed (no delete fixture from the site):\n");
    for line in fixture()["needed"].as_array().into_iter().flatten() {
        if let Some(line) = line.as_str() {
            s.push_str("- ");
            s.push_str(line);
            s.push('\n');
        }
    }
    s
}

/// Preview or refuse. `Ok` is a dry-run or a cancel. A confirmed delete is an
/// error, and the message includes the frame that was not sent.
pub fn present(id: &str, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = id.trim();
    if id.is_empty() || id.chars().any(|c| c.is_whitespace()) {
        return Err(err("delete needs a post id"));
    }
    let frame = serde_json::to_string_pretty(&frame(id)).unwrap_or_default();
    let text = format!("dry-run: no connection opened\ndelete {id}\n\n{frame}\n\n{}", needed());
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
    fn the_stub_matches_the_documented_hook_and_blocks_the_send() {
        let fix = fixture();
        assert_eq!(fix["capture_status"], "missing");
        assert_eq!(hook("POST"), fix["documented_hook"]);
        let dry = present("POST", false, true).unwrap();
        assert!(dry.contains("\"event\": \"delete-post\""), "{dry}");
        assert!(!dry.contains("nothing was published"), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        for line in fix["needed"].as_array().unwrap() {
            assert!(dry.contains(line.as_str().unwrap()), "{dry}");
        }
        assert_eq!(present("POST", false, false).unwrap(), "nothing sent");
        let blocked = present("POST", true, false).unwrap_err();
        assert!(blocked.message.contains("nothing was published"), "{}", blocked.message);
        assert!(blocked.message.contains("capture still needed"), "{}", blocked.message);
    }
}
