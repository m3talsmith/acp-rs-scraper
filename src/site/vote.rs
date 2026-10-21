//! Vote (`+` / `-`). The hook shape is the one in the plan. No live frame
//! has been captured, so a confirmed vote does not open a connection.

use super::write::{WriteError, CAPTURE_BLOCK};
use crate::prelude::*;

const FIXTURE: &str = include_str!("../../tests/fixtures/site/vote.json");

/// `1` upvote, `0` clear, `-1` downvote.
pub fn hook(id: &str, state: &str) -> Value {
    json!({"type": "hook", "event": "vote", "value": {"id": id, "state": state}})
}

/// Phoenix event frame. `join_ref`, `ref`, and the topic are placeholders.
pub fn frame(id: &str, state: &str) -> Value {
    json!(["1", "1", "lv:<id>", "event", hook(id, state)])
}

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("vote fixture")
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

fn needed() -> String {
    let mut s = String::from("capture still needed (no vote fixture from the site):\n");
    for line in fixture()["needed"].as_array().into_iter().flatten() {
        if let Some(line) = line.as_str() {
            s.push_str("- ");
            s.push_str(line);
            s.push('\n');
        }
    }
    s
}

/// Preview or refuse. `Ok` is a dry-run or a cancel. A confirmed vote is an
/// error, and the message includes the frame that was not sent.
pub fn present(id: &str, state: &str, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = id.trim();
    if id.is_empty() || id.chars().any(|c| c.is_whitespace()) {
        return Err(err("a vote needs a post id"));
    }
    if !matches!(state, "1" | "0" | "-1") {
        return Err(err("vote state must be 1, 0, or -1"));
    }
    let frame = serde_json::to_string_pretty(&frame(id, state)).unwrap_or_default();
    let text = format!("dry-run: no connection opened\nvote {id} state {state}\n\n{frame}\n\n{}", needed());
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
        assert_eq!(fix["event"], "vote");
        let states: Vec<_> = fix["states"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert_eq!(states, ["1", "0", "-1"]);
        let documented = &fix["documented_hook"];
        let built = hook("POST", "1");
        assert_eq!(&built, documented);
        assert_eq!(hook("POST", "0")["value"]["state"], "0");
        assert_eq!(hook("POST", "-1")["value"]["state"], "-1");
        assert_eq!(frame("POST", "1")[3], "event");
        assert!(present("POST", "2", true, false).unwrap_err().message.contains("1, 0, or -1"));
        let dry = present("POST", "1", false, true).unwrap();
        assert!(dry.contains("dry-run: no connection opened"), "{dry}");
        assert!(dry.contains("\"event\": \"vote\""), "{dry}");
        assert!(!dry.contains("nothing was published"), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        for line in fix["needed"].as_array().unwrap() {
            assert!(dry.contains(line.as_str().unwrap()), "{dry}");
        }
        assert_eq!(present("POST", "1", false, false).unwrap(), "nothing sent");
        let blocked = present("POST", "-1", true, false).unwrap_err();
        assert!(blocked.message.contains("are not sent"), "{}", blocked.message);
        assert!(blocked.message.contains("state -1"), "{}", blocked.message);
        assert!(blocked.message.contains("capture still needed"), "{}", blocked.message);
    }
}
