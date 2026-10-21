//! Reply edit and top-level post edit. The captured page is a redirect join
//! to `/rs/post/{id}/edit`.
//!
//! A confirmed edit sends validate, then save, once. A reply's only field is
//! `form[body_input]`, with a leading `postRef` copied from the mounted editor.
//! A top-level post sends `form[title]` and `form[body_input]`. The site does
//! not support changing attachments on an edit. The one-hour window is a product
//! constraint.

use super::write::{captured_form_value, collapse_blank_lines, reply_event, WriteError};
use crate::prelude::*;

/// An edit stays open for one hour after the post.
pub const EDIT_WINDOW_SECS: i64 = 3600;

/// `--attachment` with `--edit`. The site has no control for this, so nothing is sent.
pub const ATTACHMENT_EDIT_REFUSAL: &str = "The site does not support changing attachments on an edit. Nothing was sent.";

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
    /// Markdown the user typed. A leading quote is prepended after the editor opens.
    pub source: String,
    /// `form[body_input]=…`, percent-encoded the way the capture was.
    /// A leading `postRef` is not in this string until the editor has been read.
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
        source: body.to_string(),
        value,
        preview: crate::scrape::tidy_text(&preview),
        omitted: vec![
            "An existing quote on this reply is kept. It is not shown here, because this preview does not open the editor.".into(),
            "_track_static is not sent on the redirect join.".into(),
            "keystroke validates are not sent. One validate, then one save.".into(),
            "no validate is sent after save. The captured browser sent one, and the server answered unmatched topic.".into(),
            "mention-query, view-post, refresh-posts, and lv:clear-flash are not sent.".into(),
            "join_ref, ref, and the LiveView topic are assigned on join. The printed frames use placeholders.".into(),
            "The site does not support changing attachments on an edit.".into(),
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
    s.push_str("These frames are sent when that page is a reply (data-mode reply, method put, body field only).\n");
    s.push_str("If the editor is a top-level post, its current title is sent with this body and is not shown here.\n\n");
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

/// A top-level post edit. A blank title or body is copied from the editor after confirm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostEdit {
    /// Post id. The edit URL is `/rs/post/{id}/edit`.
    pub id: String,
    /// `phx-target`. The capture used 1.
    pub cid: u64,
    /// New title. `None` means the mounted title is kept.
    pub title: Option<String>,
    /// New markdown body. `None` means the mounted document is kept.
    pub body: Option<String>,
    /// What the preview can show before the editor opens.
    pub preview: String,
    /// How the hour was described.
    pub window: String,
}

/// Dry-run text, a cancel, or a post edit to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostEditGate {
    /// Printed text. No socket.
    Done(String),
    /// Confirmed. Open the editor, then validate, then save, once.
    Send(PostEdit),
}

/// Build a top-level edit. At least one of `title` and `body` is required.
pub fn prepare_post(id: &str, title: Option<&str>, body: Option<&str>, inserted_unix: Option<i64>, now_unix: i64, writes_on: bool) -> Result<PostEdit, WriteError> {
    if !writes_on {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = clean_id(id)?.to_string();
    let window = hour_line(inserted_unix, now_unix)?;
    let title = title.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let body = body.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if title.is_none() && body.is_none() {
        return Err(err("a top-level edit needs a title or a body"));
    }
    let preview = match (&title, &body) {
        (Some(title), Some(body)) => format!("title: {title}\n{body}"),
        (Some(title), None) => format!("title: {title}\nThe body is the mounted document. It is kept and is not shown here, because this preview does not open the editor."),
        (None, Some(body)) => body.clone(),
        (None, None) => return Err(err("a top-level edit needs a title or a body")),
    };
    Ok(PostEdit { id, cid: 1, title, body, preview, window })
}

/// Preview for a top-level edit. When both fields are known, the frames are exact.
pub fn post_dry_run(plan: &PostEdit) -> String {
    let mut s = String::new();
    s.push_str("dry-run: no connection opened\nedit ");
    s.push_str(&plan.id);
    s.push_str("\nwindow: ");
    s.push_str(&plan.window);
    s.push_str("\n\n");
    s.push_str(plan.preview.trim_end());
    s.push('\n');
    s.push_str("\nopen (sent after confirm): phx_leave, then phx_join redirect /rs/post/");
    s.push_str(&plan.id);
    s.push_str("/edit\nsticky false, _mounts 0. _track_static is not sent.\n");
    s.push_str("The page is data-mode thread, method put, with form[title] and form[body_input].\n");
    if let (Some(title), Some(body)) = (&plan.title, &plan.body) {
        if let Ok(doc) = doc_from_markdown(body) {
            let value = post_value(title, &doc);
            let frames = vec![
                super::write::form_event("1", "validate", plan.cid, &value, "form[body_input]", true),
                super::write::form_event("2", "save", plan.cid, &value, "form[body_input]", false),
            ];
            s.push('\n');
            s.push_str(&crate::logging::redact_secrets(&serde_json::to_string_pretty(&frames).unwrap_or_default()));
            s.push('\n');
        }
    } else if plan.title.is_some() {
        s.push_str("One validate targets form[title], then one save. The body bytes are read from the editor and are not in this print.\n");
    } else {
        s.push_str("One validate targets form[body_input], then one save. The title is read from the editor and is not in this print.\n");
    }
    s.push_str("\nnot sent:\n- keystroke validates are not sent. One validate, then one save.\n- The site does not support changing attachments on an edit.\n");
    s
}

/// Apply the safety checks for a top-level edit.
#[allow(clippy::too_many_arguments)]
pub fn gate_post(id: &str, title: Option<&str>, body: Option<&str>, inserted_unix: Option<i64>, now_unix: i64, confirmed: bool, dry_run: bool, writes_on: bool) -> Result<PostEditGate, WriteError> {
    let plan = prepare_post(id, title, body, inserted_unix, now_unix, writes_on)?;
    if dry_run {
        return Ok(PostEditGate::Done(post_dry_run(&plan)));
    }
    if !confirmed {
        return Ok(PostEditGate::Done("nothing sent".into()));
    }
    Ok(PostEditGate::Send(plan))
}

fn doc_from_markdown(markdown: &str) -> Result<Value, WriteError> {
    let mut doc = super::assemble(markdown, None, None)?;
    collapse_blank_lines(&mut doc);
    Ok(doc)
}

fn post_value(title: &str, doc: &Value) -> String {
    let body_json = super::prosemirror::editor_json(doc);
    captured_form_value(&[("form[title]".into(), title.to_string()), ("form[body_input]".into(), body_json)])
}

fn value_with_quote(markdown: &str, node: &Value) -> Result<String, WriteError> {
    let mut doc = doc_from_markdown(markdown)?;
    if let Some(content) = doc.get_mut("content").and_then(|c| c.as_array_mut()) {
        content.insert(0, node.clone());
    }
    let body_json = super::prosemirror::editor_json(&doc);
    Ok(captured_form_value(&[("form[body_input]".into(), body_json)]))
}

/// The editor document mounted in `data-props` next to `form[body_input]`.
pub fn mounted_doc(html: &str) -> Option<Value> {
    let flat = html.replace("&quot;", "\"").replace("&#34;", "\"");
    let marker = "\"name\":\"form[body_input]\"";
    let idx = flat.find(marker)?;
    let start = idx.saturating_sub(20_000);
    let window = &flat[start..idx];
    let body_at = window.rfind("\"body\":")?;
    let rel = window[body_at..].find('{')?;
    let slice = &window[body_at + rel..];
    let mut de = serde_json::Deserializer::from_str(slice);
    serde::Deserialize::deserialize(&mut de).ok()
}

/// A leading `postRef` on the mounted reply, when the editor document has one.
pub fn leading_post_ref(html: &str) -> Option<Value> {
    let doc = mounted_doc(html)?;
    let node = doc.get("content")?.as_array()?.first()?.clone();
    if node.get("type").and_then(Value::as_str) != Some("postRef") {
        return None;
    }
    let post_id = node.pointer("/attrs/postId").and_then(Value::as_str)?;
    let thread_id = node.pointer("/attrs/threadId").and_then(Value::as_str)?;
    if clean_id(post_id).is_err() || clean_id(thread_id).is_err() {
        return None;
    }
    Some(node)
}

fn mounted_title(html: &str) -> Option<String> {
    let doc = scraper::Html::parse_document(html);
    let Ok(sel) = scraper::Selector::parse("input[name='form[title]']") else { return None };
    doc.select(&sel).next().and_then(|el| el.value().attr("value")).map(str::to_string).filter(|s| !s.is_empty())
}

fn page_is_post(html: &str) -> bool {
    let mode = html.contains("data-mode=\"thread\"") || html.contains("data-mode='thread'");
    let title = html.contains("name=\"form[title]\"") || html.contains("name='form[title]'");
    let method = html.contains("method=\"put\"") || html.contains("method='put'");
    mode && title && method
}

/// The mounted edit page must be the captured reply form. Anything else is not sent.
pub fn reply_edit_page(html: &str) -> Result<super::write::Probe, WriteError> {
    if html.contains("name=\"form[title]\"") || html.contains("name='form[title]'") {
        return Err(err("the edit page was not a reply or a top-level post. Nothing was sent."));
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
        let payloads = if page_is_post(&mounted) {
            let title = mounted_title(&mounted).ok_or_else(|| anyhow!("the post title was not on the page. Nothing was sent."))?;
            let doc = doc_from_markdown(&plan.source).map_err(|e| anyhow!("{e}"))?;
            let value = post_value(&title, &doc);
            let cid = super::write::probe_html(&mounted).cid.unwrap_or(plan.cid);
            vec![
                super::write::form_event("1", "validate", cid, &value, "form[body_input]", true).get(4).cloned().unwrap_or(Value::Null),
                super::write::form_event("2", "save", cid, &value, "form[body_input]", false).get(4).cloned().unwrap_or(Value::Null),
            ]
        } else {
            let probe = reply_edit_page(&mounted).map_err(|e| anyhow!("{e}"))?;
            let mut plan = plan.clone();
            if let Some(cid) = probe.cid {
                plan.cid = cid;
            }
            if let Some(node) = leading_post_ref(&mounted) {
                plan.value = value_with_quote(&plan.source, &node).map_err(|e| anyhow!("{e}"))?;
            }
            payloads(&plan)
        };
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

/// Join the edit page and send one validate and one save for a top-level post.
///
/// A blank title or body is the mounted one. `_target` is `form[title]` when
/// only the title changed, and `form[body_input]` when the body is part of the edit.
pub async fn publish_post(http: &reqwest::Client, jar: &Jar, plan: &PostEdit) -> Result<super::write::Published> {
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
        if !page_is_post(&mounted) {
            bail!("the edit page was not a top-level post. Nothing was sent.");
        }
        let title = match &plan.title {
            Some(title) => title.clone(),
            None => mounted_title(&mounted).ok_or_else(|| anyhow!("the post title was not on the page. Nothing was sent."))?,
        };
        let doc = match &plan.body {
            Some(body) => doc_from_markdown(body).map_err(|e| anyhow!("{e}"))?,
            None => mounted_doc(&mounted).ok_or_else(|| anyhow!("the post body was not on the page. Nothing was sent."))?,
        };
        let value = post_value(&title, &doc);
        let target = if plan.body.is_some() { "form[body_input]" } else { "form[title]" };
        let cid = super::write::probe_html(&mounted).cid.unwrap_or(plan.cid);
        let payloads = vec![
            super::write::form_event("1", "validate", cid, &value, target, true).get(4).cloned().unwrap_or(Value::Null),
            super::write::form_event("2", "save", cid, &value, target, false).get(4).cloned().unwrap_or(Value::Null),
        ];
        let mut frames = Vec::new();
        let mut sent = 0usize;
        loop {
            match super::write::after_reply(&payloads, sent, &frames) {
                Err(e) => break Err(anyhow!("{e}")),
                Ok(None) => break super::write::interpret(&frames, &origin).map_err(|e| anyhow!("{e}")),
                Ok(Some(payload)) => {
                    let reply = match lv.push("event", payload).await {
                        Ok(reply) => reply,
                        Err(e) => break Err(anyhow!("{}. Nothing was retried.", crate::logging::redact_secrets(&format!("{e:#}")))),
                    };
                    let errored = reply.get("status").and_then(Value::as_str) == Some("error");
                    frames.push(reply);
                    sent += 1;
                    if sent == payloads.len() && !errored {
                        match lv.collect_for(std::time::Duration::from_secs(3)).await {
                            Ok(extra) => frames.extend(extra),
                            Err(e) => {
                                break Err(anyhow!(
                                    "the save was sent and the reply could not be read ({}). Nothing was retried. Check the site before editing again.",
                                    crate::logging::redact_secrets(&format!("{e:#}"))
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
        assert!(text.contains("An existing quote on this reply is kept"), "{text}");
        assert!(text.contains("is not shown here"), "{text}");
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
        let post = prepare_post("POST", Some("Renamed"), Some("updated"), Some(now), now, true).unwrap();
        assert_eq!(post.title.as_deref(), Some("Renamed"));
        let title_only = gate_post("POST", Some("Renamed"), None, Some(now), now, false, true, true).unwrap();
        let PostEditGate::Done(title_only) = title_only else { panic!("dry-run") };
        assert!(title_only.contains("form[title]"), "{title_only}");
        assert!(title_only.contains("are not in this print"), "{title_only}");
        let body_only = gate_post("POST", None, Some("updated"), Some(now), now, false, true, true).unwrap();
        let PostEditGate::Done(body_only) = body_only else { panic!("dry-run") };
        assert!(body_only.contains("is not in this print"), "{body_only}");
        assert!(matches!(gate_post("POST", Some("Renamed"), None, Some(now), now, true, false, true).unwrap(), PostEditGate::Send(_)));
        let quoted = r#"<div data-props="{&quot;body&quot;:{&quot;type&quot;:&quot;doc&quot;,&quot;content&quot;:[{&quot;type&quot;:&quot;postRef&quot;,&quot;attrs&quot;:{&quot;postId&quot;:&quot;QPOST&quot;,&quot;threadId&quot;:&quot;QTHREAD&quot;}}]},&quot;name&quot;:&quot;form[body_input]&quot;}"></div>"#;
        let node = leading_post_ref(quoted).expect("leading postRef");
        let with_quote = value_with_quote("updated", &node).unwrap();
        assert!(with_quote.contains("postRef"), "{with_quote}");
        assert!(with_quote.contains("QPOST"), "{with_quote}");
        let title = r#"<form id="js-postForm-form" method="put" data-mode="post" phx-submit="save" phx-change="validate" phx-target="1"><input name="form[title]"></form>"#;
        assert!(reply_edit_page(title).unwrap_err().message.contains("not a reply or a top-level post"));
        let reply = r#"<div data-mode="reply"><form id="js-postForm-form" method="put" phx-submit="save" phx-change="validate" phx-target="1"><textarea name="form[body_input]"></textarea></form></div>"#;
        let probe = reply_edit_page(reply).unwrap();
        assert_eq!(probe.cid, Some(1));
    }
}
