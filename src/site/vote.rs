//! Vote (`+` / `-`). The hook is the captured one: `{id, state}` with
//! `"1"`, `"0"`, or `"-1"`. A confirmed vote joins the post and sends it once.

use super::hook::{self, HookCall, HookGate};
use super::write::WriteError;
use crate::prelude::*;

/// `1` upvote, `0` clear, `-1` downvote.
pub fn hook(id: &str, state: &str) -> Value {
    json!({"type": "hook", "event": "vote", "value": {"id": id, "state": state}})
}

/// Phoenix event frame. `join_ref`, `ref`, and the topic are placeholders.
pub fn frame(id: &str, state: &str) -> Value {
    json!(["1", "1", "lv:<id>", "event", hook(id, state)])
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

/// Preview or send. A confirmed vote is [`HookGate::Send`].
pub fn present(id: &str, state: &str, confirmed: bool, dry_run: bool) -> Result<HookGate, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = hook::clean_id(id, "vote")?;
    if !matches!(state, "1" | "0" | "-1") {
        return Err(err("vote state must be 1, 0, or -1"));
    }
    let payload = hook(id.as_str(), state);
    let text = hook::dry_run(
        &format!("vote {id} state {state}"),
        &payload,
        "The reply shows score, vote, and upvote_percentage.",
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
        serde_json::from_str(include_str!("../../tests/fixtures/site/vote.json")).expect("vote fixture")
    }

    #[test]
    fn the_hook_matches_the_capture_and_a_confirm_sends() {
        let fix = fixture();
        assert_eq!(fix["capture_status"], "captured");
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
        let HookGate::Done(dry) = present("POST", "1", false, true).unwrap() else { panic!("dry-run") };
        assert!(dry.contains("dry-run: no connection opened"), "{dry}");
        assert!(dry.contains("\"event\": \"vote\""), "{dry}");
        assert!(dry.contains("upvote_percentage"), "{dry}");
        assert!(!dry.contains("nothing was published"), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        let HookGate::Done(cancel) = present("POST", "1", false, false).unwrap() else { panic!("cancel") };
        assert_eq!(cancel, "nothing sent");
        let HookGate::Send(call) = present("POST", "-1", true, false).unwrap() else { panic!("send") };
        assert_eq!(call.payload["value"]["state"], "-1");
        assert_eq!(call.id, "POST");
    }
}
