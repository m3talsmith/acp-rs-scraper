//! Reply edit. The captured page is a redirect join to `/rs/post/{id}/edit`.
//!
//! A confirmed reply sends validate, then save, once. The only field is
//! `form[body_input]`. A top-level post edit and an attachment replace stay
//! gated. The one-hour window is a product constraint.

use super::write::{
    captured_form_value, collapse_blank_lines, reply_event, WriteError, CAPTURE_BLOCK,
};
use crate::prelude::*;

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

/// What the caller does after the safety checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditGate {
    /// Dry-run text, or a cancel (`nothing sent`). No socket.
    Done(String),
    /// Confirmed reply edit. Open the editor, then validate, then save, once.
    Send(ReplyEdit),
}

/// A reply edit reduced to the captured form field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyEdit {
    /// Reply id. The edit URL is `/rs/post/{id}/edit`.
    pub id: String,
    /// `phx-target` on the edit form. The capture used 1.
    pub cid: u64,
    /// `form[body_input]=…`, percent-encoded the way the capture was.
    pub value: String,
    /// Plain text of the document, for the preview.
    pub preview: String,
    /// Pieces this build does not invent.
    pub omitted: Vec<String>,
    /// How the hour was described.
    pub window: String,
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

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

fn clean_id(id: &str) -> Result<&str, WriteError> {
    let id = id.trim();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
        return Err(err("an edit needs a post id"));
    }
    Ok(id)
}

fn hour_line(inserted_unix: Option<i64>, now_unix: i64) -> Result<String, WriteError> {
    match window(inserted_unix, now_unix) {
        Window::Closed => Err(err("an edit stays open for one hour after the post. Nothing was sent.")),
        Window::Open => Ok("within one hour".into()),
        Window::Unknown => Ok("not checked (no posted_at)".into()),
    }
}

/// Build the reply-edit plan. This does not open a socket.
pub fn prepare(id: &str, body: &str, inserted_unix: Option<i64>, now_unix: i64, writes_on: bool) -> Result<ReplyEdit, WriteError> {
    if !writes_on {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = clean_id(id)?.to_string();
    let window = hour_line(inserted_unix, now_unix)?;
    if body.trim().is_empty() {
        return Err(err("a reply edit needs a body"));
    }
    let mut doc = super::assemble(body, None, None)?;
    collapse_blank_lines(&mut doc);
    let body_json = super::prosemirror::editor_json(&doc);
    let value = captured_form_value(&[("form[body_input]".into(), body_json)]);
    let mut preview = String::new();
    crate::scrape::prosemirror_text(&doc, &mut preview);
    Ok(ReplyEdit {
        id,
        cid: 1,
        value,
        preview: crate::scrape::tidy_text(&preview),
        omitted: vec![
            "the body replaces the document. A postRef already on the reply is not copied.".into(),
            "_track_static is not sent on the redirect join.".into(),
            "keystroke validates are not sent. One validate, then one save.".into(),
            "no validate is sent after save. The captured browser sent one, and the server answered unmatched topic.".into(),
            "mention-query, view-post, refresh-posts, and lv:clear-flash are not sent.".into(),
            "join_ref, ref, and the LiveView topic are assigned on join. The printed frames use placeholders.".into(),
            "a top-level post edit is refused after the edit page opens. These frames are not sent for it.".into(),
            "attachment replace is not captured.".into(),
            "cid is the form's phx-target, else 1.".into(),
        ],
        window,
    })
}

/// Validate, then save. The printed payloads are the ones a confirmed reply sends.
pub fn frames(plan: &ReplyEdit) -> Vec<Value> {
    vec![reply_event("1", "validate", plan.cid, &plan.value, true), reply_event("2", "save", plan.cid, &plan.value, false)]
}

/// The event payload of each printed frame (index 4).
pub fn payloads(plan: &ReplyEdit) -> Vec<Value> {
    frames(plan).into_iter().filter_map(|frame| frame.get(4).cloned()).collect()
}

/// Preview plus the two frames. Building this string does not open a socket.
pub fn dry_run_text(plan: &ReplyEdit) -> String {
    let frames = serde_json::to_string_pretty(&frames(plan)).unwrap_or_default();
    let mut s = String::new();
    s.push_str("dry-run: no connection opened\n");
    s.push_str("edit ");
    s.push_str(&plan.id);
    s.push_str("\nwindow: ");
    s.push_str(&plan.window);
    s.push_str("\n\n");
    s.push_str(plan.preview.trim_end());
    s.push_str("\n\nopen (sent after confirm): phx_leave, then phx_join redirect /rs/post/");
    s.push_str(&plan.id);
    s.push_str("/edit\nsticky false, _mounts 0. Session and static come from the /rs page. _track_static is not sent.\n");
    s.push_str("These frames are sent only when that page is a reply (data-mode reply, method put, body field only).\n\n");
    s.push_str(&crate::logging::redact_secrets(&frames));
    s.push_str("\n\nnot sent:\n");
    for line in &plan.omitted {
        s.push_str("- ");
        s.push_str(line);
        s.push('\n');
    }
    s
}

/// Apply the safety checks. A confirmed reply edit is [`EditGate::Send`].
pub fn gate(id: &str, body: &str, inserted_unix: Option<i64>, now_unix: i64, confirmed: bool, dry_run: bool, writes_on: bool) -> Result<EditGate, WriteError> {
    let plan = prepare(id, body, inserted_unix, now_unix, writes_on)?;
    if dry_run {
        return Ok(EditGate::Done(dry_run_text(&plan)));
    }
    if !confirmed {
        return Ok(EditGate::Done("nothing sent".into()));
    }
    Ok(EditGate::Send(plan))
}

/// A top-level post edit stays gated. The terminal UI uses this for a root post.
///
/// The reply form is a different page. This path does not open a connection.
pub fn post_blocked(id: &str, inserted_unix: Option<i64>, now_unix: i64, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = clean_id(id)?;
    let hour = hour_line(inserted_unix, now_unix)?;
    let text = format!(
        "dry-run: no connection opened\nedit {id}\nwindow: {hour}\nthe edit form is not built\na top-level post edit is not sent\n/rs/post/POST/edit\n\n{CAPTURE_BLOCK}"
    );
    if dry_run {
        return Ok(text);
    }
    if !confirmed {
        return Ok("nothing sent".into());
    }
    Err(err(format!("{CAPTURE_BLOCK}\n\n{text}")))
}

/// The mounted edit page must be the captured reply form. Anything else is not sent.
pub fn reply_edit_page(html: &str) -> Result<super::write::Probe, WriteError> {
    if html.contains("name=\"form[title]\"") || html.contains("name='form[title]'") {
        return Err(err("a top-level post edit is not sent. Nothing was sent."));
    }
    if !html.contains("js-postForm-form") {
        return Err(err("the edit form was not on the page. Nothing was sent."));
    }
    if !html.contains("data-mode=\"reply\"") && !html.contains("data-mode='reply'") {
        return Err(err("the edit page did not show a reply. Nothing was sent."));
    }
    if !html.contains("method=\"put\"") && !html.contains("method='put'") {
        return Err(err("the edit form is not the captured put form. Nothing was sent."));
    }
    let probe = super::write::probe_html(html);
    if !probe.found {
        return Err(err("the edit form was not on the page. Nothing was sent."));
    }
    if probe.submit.as_deref() != Some("save") || probe.change.as_deref() != Some("validate") {
        return Err(err("the edit form is not the captured save/validate form. Nothing was sent."));
    }
    Ok(probe)
}

/// Join `/rs`, redirect to the edit page, then send validate and save once.
///
/// Under test, a URL on `acp.us` panics. A page that is not a reply form
/// returns before either form event.
pub async fn publish_reply_edit(http: &reqwest::Client, jar: &Jar, plan: &ReplyEdit) -> Result<super::write::Published> {
    let origin = base_url().trim_end_matches('/').to_string();
    let bootstrap = format!("{origin}/rs");
    super::write::check_test_host(&bootstrap);
    let edit_url = format!("{origin}/rs/post/{}/edit", plan.id);
    let resp = http.get(&bootstrap).send().await.context("opening the edit page")?;
    if !resp.status().is_success() {
        bail!("the edit page returned {}. Nothing was retried.", resp.status());
    }
    let html = resp.text().await.context("reading the edit page")?;
    let page = crate::liveview::parse_lv(&bootstrap, &html).context("reading the edit page")?;
    let mut lv = crate::liveview::Lv::connect(&page, jar).await.context("joining the edit page")?;
    let outcome = async {
        lv.live_redirect(&page, &edit_url).await?;
        let mounted = lv.html();
        let probe = reply_edit_page(&mounted).map_err(|e| anyhow!("{e}"))?;
        let mut plan = plan.clone();
        if let Some(cid) = probe.cid {
            plan.cid = cid;
        }
        let payloads = payloads(&plan);
        if payloads.len() != 2 {
            bail!("a reply edit sends validate and save, not {} frames. Nothing was sent.", payloads.len());
        }
        let mut frames = Vec::new();
        let mut sent = 0usize;
        loop {
            match super::write::after_reply(&payloads, sent, &frames) {
                Err(e) => break Err(anyhow!("{e}")),
                Ok(None) => break super::write::interpret(&frames, &origin).map_err(|e| anyhow!("{e}")),
                Ok(Some(payload)) => {
                    let reply = match lv.push("event", payload).await {
                        Ok(reply) => reply,
                        Err(e) => {
                            let msg = crate::logging::redact_secrets(&format!("{e:#}"));
                            break Err(anyhow!("{msg}. Nothing was retried."));
                        }
                    };
                    let errored = reply.get("status").and_then(Value::as_str) == Some("error");
                    frames.push(reply);
                    sent += 1;
                    if sent == payloads.len() && !errored {
                        match lv.collect_for(std::time::Duration::from_secs(3)).await {
                            Ok(extra) => frames.extend(extra),
                            Err(e) => {
                                let msg = crate::logging::redact_secrets(&format!("{e:#}"));
                                break Err(anyhow!(
                                    "the save was sent and the reply could not be read ({msg}). Nothing was retried. Check the site before editing again."
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    .await;
    lv.close().await;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/site/edit.json");

    fn fixture() -> Value {
        serde_json::from_str(FIXTURE).expect("edit fixture")
    }

    #[test]
    fn the_hour_closes_and_a_reply_edit_prints_validate_then_save() {
        let fix = fixture();
        assert_eq!(fix["capture_status"], "reply_body");
        assert_eq!(fix["window_secs"], EDIT_WINDOW_SECS);
        assert_eq!(fix["page"]["method"], "put");
        assert_eq!(fix["page"]["data_mode"], "reply");
        assert_eq!(fix["page"]["phx_target"], 1);
        assert_eq!(fix["page"]["fields"][0], "form[body_input]");
        let now = 10_000;
        assert_eq!(window(Some(now - 60), now), Window::Open);
        assert_eq!(window(Some(now - EDIT_WINDOW_SECS), now), Window::Open);
        assert_eq!(window(Some(now - EDIT_WINDOW_SECS - 1), now), Window::Closed);
        assert_eq!(window(None, now), Window::Unknown);
        assert!(posted_unix("2026-10-19T23:38:10Z").is_some());
        assert!(posted_unix("not a time").is_none());
        let closed = gate("REPLY", "Updated reply.", Some(now - 7_200), now, true, true, true).unwrap_err();
        assert!(closed.message.contains("one hour"), "{}", closed.message);
        assert!(!closed.message.contains("https://acp.us"), "{}", closed.message);
        let empty = gate("REPLY", "  ", Some(now), now, true, true, true).unwrap_err();
        assert!(empty.message.contains("needs a body"), "{}", empty.message);
        let dry = gate("REPLY", "Updated reply.", Some(now - 60), now, false, true, true).unwrap();
        let EditGate::Done(text) = dry else { panic!("dry-run does not send") };
        assert!(text.contains("dry-run: no connection opened"), "{text}");
        assert!(text.contains("within one hour"), "{text}");
        assert!(text.contains("form[body_input]"), "{text}");
        assert!(!text.contains("form[thread_id]"), "{text}");
        assert!(!text.contains("form[title]"), "{text}");
        assert!(text.contains("postRef already on the reply is not copied"), "{text}");
        assert!(text.contains("_track_static is not sent"), "{text}");
        assert!(!text.contains("https://acp.us"), "{text}");
        let plan = prepare("REPLY", "Updated reply.", Some(now - 60), now, true).unwrap();
        assert_eq!(plan.value, fix["validate"]["value"].as_str().unwrap());
        let sent = frames(&plan);
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0][4]["event"], "validate");
        assert_eq!(sent[0][4]["cid"], 1);
        assert_eq!(sent[0][4]["meta"]["_target"], "form[body_input]");
        assert!(sent[0][4].get("uploads").is_some());
        assert_eq!(sent[1][4]["event"], "save");
        assert_eq!(sent[1][4]["meta"], json!({}));
        assert!(sent[1][4].get("uploads").is_none());
        assert_eq!(sent[0][4]["value"], sent[1][4]["value"]);
        assert!(text.contains(&plan.value), "{text}");
        let unknown = gate("REPLY", "Updated reply.", None, now, false, true, true).unwrap();
        let EditGate::Done(unknown) = unknown else { panic!("dry-run") };
        assert!(unknown.contains("not checked"), "{unknown}");
        assert!(matches!(gate("REPLY", "Updated reply.", Some(now), now, false, false, true).unwrap(), EditGate::Done(t) if t == "nothing sent"));
        assert!(matches!(gate("REPLY", "Updated reply.", Some(now), now, true, false, true).unwrap(), EditGate::Send(_)));
        let blocked = post_blocked("POST", Some(now), now, true, false).unwrap_err();
        assert!(blocked.message.contains("are not sent"), "{}", blocked.message);
        assert!(blocked.message.contains("the edit form is not built"), "{}", blocked.message);
        assert!(blocked.message.contains("/rs/post/POST/edit"), "{}", blocked.message);
        assert!(!blocked.message.contains("https://acp.us"), "{}", blocked.message);
        let title = r#"<form id="js-postForm-form" method="put" data-mode="post" phx-submit="save" phx-change="validate" phx-target="1"><input name="form[title]"></form>"#;
        assert!(reply_edit_page(title).unwrap_err().message.contains("top-level post edit"));
        let reply = r#"<div data-mode="reply"><form id="js-postForm-form" method="put" phx-submit="save" phx-change="validate" phx-target="1"><textarea name="form[body_input]"></textarea></form></div>"#;
        let probe = reply_edit_page(reply).unwrap();
        assert_eq!(probe.cid, Some(1));
    }
}
