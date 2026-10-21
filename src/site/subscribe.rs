//! Subscribe. The hook shape is the one in the plan. No live frame has been
//! captured, so a confirmed subscribe does not open a connection.

use super::write::{WriteError, CAPTURE_BLOCK};
use crate::prelude::*;

const FIXTURE: &str = include_str!("../../tests/fixtures/site/subscribe.json");

/// `1` subscribe, `0` clear.
pub fn hook(id: &str, state: &str) -> Value {
    json!({"type": "hook", "event": "subscribe", "value": {"id": id, "state": state}})
}

/// Phoenix event frame. `join_ref`, `ref`, and the topic are placeholders.
pub fn frame(id: &str, state: &str) -> Value {
    json!(["1", "1", "lv:<id>", "event", hook(id, state)])
}

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("subscribe fixture")
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

fn needed() -> String {
    let mut s = String::from("capture still needed (no subscribe fixture from the site):\n");
    for line in fixture()["needed"].as_array().into_iter().flatten() {
        if let Some(line) = line.as_str() {
            s.push_str("- ");
            s.push_str(line);
            s.push('\n');
        }
    }
    s
}

/// Preview or refuse. `Ok` is a dry-run or a cancel. A confirmed subscribe is
/// an error, and the message includes the frame that was not sent.
pub fn present(id: &str, state: &str, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = id.trim();
    if id.is_empty() || id.chars().any(|c| c.is_whitespace()) {
        return Err(err("subscribe needs a thread id"));
    }
    if !matches!(state, "1" | "0") {
        return Err(err("subscribe state must be 1 or 0"));
    }
    let frame = serde_json::to_string_pretty(&frame(id, state)).unwrap_or_default();
    let text = format!("dry-run: no connection opened\nsubscribe {id} state {state}\n\n{frame}\n\n{}", needed());
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
        assert_eq!(fix["event"], "subscribe");
        assert_eq!(hook("THREAD", "1"), fix["documented_hook"]);
        assert_eq!(hook("THREAD", "0")["value"]["state"], "0");
        assert!(present("THREAD", "2", true, false).unwrap_err().message.contains("1 or 0"));
        let dry = present("THREAD", "1", false, true).unwrap();
        assert!(dry.contains("dry-run: no connection opened"), "{dry}");
        assert!(dry.contains("\"event\": \"subscribe\""), "{dry}");
        assert!(!dry.contains("nothing was published"), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        for line in fix["needed"].as_array().unwrap() {
            assert!(dry.contains(line.as_str().unwrap()), "{dry}");
        }
        assert_eq!(present("THREAD", "0", false, false).unwrap(), "nothing sent");
        let blocked = present("THREAD", "0", true, false).unwrap_err();
        assert!(blocked.message.contains("are not sent"), "{}", blocked.message);
        assert!(blocked.message.contains("state 0"), "{}", blocked.message);
        assert!(blocked.message.contains("capture still needed"), "{}", blocked.message);
    }
}
