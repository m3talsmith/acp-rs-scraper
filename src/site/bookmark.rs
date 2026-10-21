//! Site bookmark. The captured hook is `bookmark {id, state: "1"}`.
//!
//! Clearing a bookmark (`state: "0"`) was not in the capture, so this command
//! only sends state `"1"`. Local `--bookmark` / `--unbookmark` stay offline.

use super::hook::{self, HookCall, HookGate};
use super::write::WriteError;
use crate::prelude::*;

/// `bookmark {id, state:"1"}`, as captured.
pub fn hook(id: &str) -> Value {
    json!({"type": "hook", "event": "bookmark", "value": {"id": id, "state": "1"}})
}

/// Phoenix event frame. `join_ref`, `ref`, and the topic are placeholders.
pub fn frame(id: &str) -> Value {
    json!(["1", "1", "lv:<id>", "event", hook(id)])
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

/// The missing clear frame.
pub const CLEAR_MISSING: &str = "clearing a site bookmark was not captured. The missing frame is a bookmark hook with state \"0\". Nothing was sent.";

/// Preview or send. A confirmed bookmark is [`HookGate::Send`].
pub fn present(id: &str, confirmed: bool, dry_run: bool) -> Result<HookGate, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = hook::clean_id(id, "bookmark")?;
    let payload = hook(&id);
    let text = hook::dry_run(
        &format!("bookmark {id} state 1"),
        &payload,
        "The reply shows bookmarked true and bookmarks_count. Clearing a bookmark (state \"0\") is not sent.",
    );
    if dry_run {
        return Ok(HookGate::Done(text));
    }
    if !confirmed {
        return Ok(HookGate::Done("nothing sent".into()));
    }
    Ok(HookGate::Send(HookCall { id, payload }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/site/bookmark.json")).expect("bookmark fixture")
    }

    #[test]
    fn a_site_bookmark_sends_state_1() {
        let fix = fixture();
        assert_eq!(fix["capture_status"], "captured");
        assert_eq!(hook("POST"), fix["documented_hook"]);
        assert_eq!(hook("POST")["value"]["state"], "1");
        let HookGate::Done(dry) = present("POST", false, true).unwrap() else { panic!("dry-run") };
        assert!(dry.contains("\"event\": \"bookmark\""), "{dry}");
        assert!(dry.contains("state \"0\""), "{dry}");
        assert!(CLEAR_MISSING.contains("state \"0\""));
        assert!(!dry.contains("https://acp.us"), "{dry}");
        let HookGate::Done(cancel) = present("POST", false, false).unwrap() else { panic!("cancel") };
        assert_eq!(cancel, "nothing sent");
        let HookGate::Send(call) = present("POST", true, false).unwrap() else { panic!("send") };
        assert_eq!(call.payload["value"]["state"], "1");
    }
}
