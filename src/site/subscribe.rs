//! Subscribe. The captured hook is `{id, state: "1"}`.
//!
//! State `"0"` (unsubscribe) was not in the capture, so a clear is refused
//! before a connection.

use super::hook::{self, HookCall, HookGate};
use super::write::WriteError;
use crate::prelude::*;

/// `1` subscribe. `0` is not sent.
pub fn hook(id: &str, state: &str) -> Value {
    json!({"type": "hook", "event": "subscribe", "value": {"id": id, "state": state}})
}

/// Phoenix event frame. `join_ref`, `ref`, and the topic are placeholders.
pub fn frame(id: &str, state: &str) -> Value {
    json!(["1", "1", "lv:<id>", "event", hook(id, state)])
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

/// The missing unsubscribe frame. Named so a confirm does not invent it.
pub const UNSUBSCRIBE_MISSING: &str = "unsubscribe was not captured. The missing frame is a subscribe hook with state \"0\". Nothing was sent.";

/// Preview or send. State `0` is refused, including on a dry-run.
pub fn present(id: &str, state: &str, confirmed: bool, dry_run: bool) -> Result<HookGate, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = hook::clean_id(id, "subscribe")?;
    if state == "0" {
        return Err(err(UNSUBSCRIBE_MISSING));
    }
    if state != "1" {
        return Err(err("subscribe state must be 1 or 0"));
    }
    let payload = hook(&id, state);
    let text = hook::dry_run(&format!("subscribe {id} state {state}"), &payload, "The reply shows subscribed true.");
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
        serde_json::from_str(include_str!("../../tests/fixtures/site/subscribe.json")).expect("subscribe fixture")
    }

    #[test]
    fn subscribe_sends_and_unsubscribe_names_the_missing_frame() {
        let fix = fixture();
        assert_eq!(fix["capture_status"], "captured");
        assert_eq!(fix["event"], "subscribe");
        assert_eq!(hook("THREAD", "1"), fix["documented_hook"]);
        assert!(present("THREAD", "2", true, false).unwrap_err().message.contains("1 or 0"));
        let missing = present("THREAD", "0", true, true).unwrap_err();
        assert!(missing.message.contains("state \"0\""), "{}", missing.message);
        assert!(missing.message.contains("Nothing was sent"), "{}", missing.message);
        let HookGate::Done(dry) = present("THREAD", "1", false, true).unwrap() else { panic!("dry-run") };
        assert!(dry.contains("dry-run: no connection opened"), "{dry}");
        assert!(dry.contains("\"event\": \"subscribe\""), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        let HookGate::Done(cancel) = present("THREAD", "1", false, false).unwrap() else { panic!("cancel") };
        assert_eq!(cancel, "nothing sent");
        let HookGate::Send(call) = present("THREAD", "1", true, false).unwrap() else { panic!("send") };
        assert_eq!(call.payload["value"]["state"], "1");
    }
}
