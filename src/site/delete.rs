//! Delete. The captured hook is `delete-post {id}`.
//!
//! Cancel sends nothing: the capture has one `delete-post` and no frame for
//! the cancelled dialog. Success is `live_redirect` to `/rs`. The local copy
//! stays and is shown with a tombstone.

use super::hook::{self, HookCall, HookGate};
use super::write::WriteError;
use crate::prelude::*;

/// `delete-post {id}`, as captured.
pub fn hook(id: &str) -> Value {
    json!({"type": "hook", "event": "delete-post", "value": {"id": id}})
}

/// Phoenix event frame. `join_ref`, `ref`, and the topic are placeholders.
pub fn frame(id: &str) -> Value {
    json!(["1", "1", "lv:<id>", "event", hook(id)])
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

/// Preview or send. A confirmed delete is [`HookGate::Send`].
pub fn present(id: &str, confirmed: bool, dry_run: bool) -> Result<HookGate, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = hook::clean_id(id, "delete")?;
    let payload = hook(&id);
    let text = hook::dry_run(
        &format!("delete {id}"),
        &payload,
        "Cancel sends nothing. A confirmed delete redirects to /rs. The local copy stays, with a tombstone.",
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
        serde_json::from_str(include_str!("../../tests/fixtures/site/delete.json")).expect("delete fixture")
    }

    #[test]
    fn the_hook_matches_the_capture_and_a_confirm_sends() {
        let fix = fixture();
        assert_eq!(fix["capture_status"], "captured");
        assert_eq!(hook("POST"), fix["documented_hook"]);
        let HookGate::Done(dry) = present("POST", false, true).unwrap() else { panic!("dry-run") };
        assert!(dry.contains("\"event\": \"delete-post\""), "{dry}");
        assert!(dry.contains("tombstone"), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        let HookGate::Done(cancel) = present("POST", false, false).unwrap() else { panic!("cancel") };
        assert_eq!(cancel, "nothing sent");
        let HookGate::Send(call) = present("POST", true, false).unwrap() else { panic!("send") };
        assert_eq!(call.payload["event"], "delete-post");
        assert_eq!(call.id, "POST");
    }
}
