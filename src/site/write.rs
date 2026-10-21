//! A prepared New post or Reply, and the decision to send it.
//!
//! A root reply matches the approved P0 capture: `postRef` first, then
//! `form[thread_id]` and `form[body_input]`. Validate carries `meta._target`
//! and `uploads: {}`. Save carries an empty `meta` and no `uploads` key.
//! A confirmed text post or reply sends validate, then save, once. A post
//! with attachments is refused. Vote, subscribe, delete, edit, and repost
//! stay gated. `--dry-run` still opens no connection.

use super::attach::{self, PlannedAttachment};
use super::prosemirror::{self, PmError};
use crate::liveview::form_query;
use crate::logging::redact_secrets;
use crate::prelude::*;

/// Seconds between posts and replies.
pub const POST_GAP_SECS: i64 = 30;

/// Why a draft was not turned into a plan, or not sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteError {
    /// What to show the user.
    pub message: String,
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WriteError {}

impl From<PmError> for WriteError {
    fn from(e: PmError) -> Self {
        WriteError { message: e.message }
    }
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

/// What the user is writing. Vote, edit, delete, subscribe, and repost are
/// later minors and are not variants here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `/rs/post/new`.
    Post,
    /// The thread page. The body starts with `postRef`, and the form sends `form[thread_id]`.
    Reply,
}

/// One local nick the compose box can complete. `user_id` is empty until
/// `mention-query` has been captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NickHint {
    /// Nick without `@`.
    pub nick: String,
    /// Site user id, when this install already stored one.
    pub user_id: String,
}

/// A completion row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// What is inserted (`@nick` or `#tag`).
    pub label: String,
    /// Site id. Empty when the query reply has not been captured.
    pub id: String,
}

/// Local completion. The site event `mention-query` / `tag-query` is not sent.
pub fn completions(query: &str, nicks: &[NickHint], tags: &[String]) -> Vec<Completion> {
    let q = query.trim();
    let (sig, rest) = if let Some(r) = q.strip_prefix('@') {
        ('@', r)
    } else if let Some(r) = q.strip_prefix('#') {
        ('#', r)
    } else {
        return Vec::new();
    };
    let rest = rest.to_ascii_lowercase();
    let mut out = Vec::new();
    if sig == '@' {
        for n in nicks {
            if rest.is_empty() || n.nick.to_ascii_lowercase().contains(&rest) {
                out.push(Completion { label: format!("@{}", n.nick), id: n.user_id.clone() });
            }
        }
    } else {
        for tag in tags {
            let low = tag.to_ascii_lowercase();
            if low.starts_with("post-") {
                continue;
            }
            if rest.is_empty() || low.contains(&rest) {
                out.push(Completion { label: format!("#{tag}"), id: String::new() });
            }
        }
    }
    out.truncate(8);
    out
}

/// What the rendered page actually contained. Missing pieces are not filled in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    /// `#js-postForm-form` was in the HTML.
    pub found: bool,
    /// `phx-submit`, when the form has one.
    pub submit: Option<String>,
    /// `phx-change`, when the form has one.
    pub change: Option<String>,
    /// `phx-target`, when it is a number.
    pub cid: Option<u64>,
    /// Hidden inputs other than the fields this client sets itself.
    pub hidden: Vec<(String, String)>,
    /// First `/assets/` script `src`. Phoenix content-hashes that path.
    pub script_src: Option<String>,
}

/// Read the compose form. This does not guess fields that are not in `html`.
pub fn probe_html(html: &str) -> Probe {
    let doc = Html::parse_document(html);
    let Ok(form_sel) = Selector::parse("#js-postForm-form") else { return Probe::default() };
    let Some(form) = doc.select(&form_sel).next() else { return Probe::default() };
    let submit = form.value().attr("phx-submit").map(str::to_string);
    let change = form.value().attr("phx-change").map(str::to_string);
    let cid = form.value().attr("phx-target").and_then(|s| s.trim().parse().ok());
    let mut hidden = Vec::new();
    if let Ok(input_sel) = Selector::parse("input") {
        for input in form.select(&input_sel) {
            let ty = input.value().attr("type").unwrap_or("").to_ascii_lowercase();
            if ty != "hidden" {
                continue;
            }
            let Some(name) = input.value().attr("name") else { continue };
            if matches!(name, "form[title]" | "form[body_input]" | "form[thread_id]") {
                continue;
            }
            hidden.push((name.to_string(), input.value().attr("value").unwrap_or("").to_string()));
        }
    }
    let mut script_src = None;
    if let Ok(script_sel) = Selector::parse("script[src]") {
        for s in doc.select(&script_sel) {
            if let Some(src) = s.value().attr("src")
                && src.contains("/assets/")
            {
                script_src = Some(src.to_string());
                break;
            }
        }
    }
    Probe { found: true, submit, change, cid, hidden, script_src }
}

/// The draft the compose box or the CLI collected. No network fields.
#[derive(Debug, Clone)]
pub struct Draft {
    /// New post title. Ignored for a reply.
    pub title: String,
    /// Markdown body. Empty when the message is only a quote.
    pub body: String,
    /// Thread id for a reply.
    pub thread: Option<String>,
    /// Comment this reply answers. A root reply leaves this empty.
    pub in_reply_to: Option<String>,
    /// Selected text, inserted as a blockquote.
    pub quote: Option<String>,
    /// Post the quote or the reply points at, when it is not `in_reply_to`.
    pub quote_post: Option<String>,
    /// Local files. Classified in [`prepare`]. Not uploaded.
    pub attachments: Vec<std::path::PathBuf>,
    /// 0-based indexes into [`Self::attachments`] that go to the site.
    /// `None` means the default: every image or video, or the first [`crate::config::SITE_MAX_ATTACHMENTS`] of those when there are more. Other types stay local.
    pub site_attachments: Option<Vec<usize>>,
    /// Rendered page, when a probe already has it. Dry-run usually has none.
    pub html: Option<String>,
    /// `--accept-site-update`.
    pub accept_bundle: bool,
    /// `site_state` bundle last verified. `None` until a capture stores one.
    pub stored_bundle: Option<String>,
    /// Unix seconds. The caller supplies the clock.
    pub now_unix: i64,
    /// Previous confirmed attempt, when this process recorded one.
    pub last_write_unix: Option<i64>,
    /// `ui.writes` / `ACP_WRITES`.
    pub writes_on: bool,
}

/// A New post or Reply reduced to the documented form fields.
#[derive(Debug, Clone, PartialEq)]
pub struct WritePlan {
    /// Post or reply.
    pub kind: Kind,
    /// Page path. Not a fetched URL.
    pub page: String,
    /// Component id: the probe's `phx-target`, else the documented default.
    pub cid: u64,
    /// URL-encoded form fields, in send order.
    pub fields: Vec<(String, String)>,
    /// Documented pieces this build does not invent.
    pub omitted: Vec<String>,
    /// ProseMirror document.
    pub body: Value,
    /// Plain text of that document, for the preview.
    pub preview_text: String,
    /// Title line in the preview.
    pub title: String,
    /// Local files the dry-run lists. Their bytes are not in [`Self::fields`].
    pub attachments: Vec<PlannedAttachment>,
    /// Set when the default first-files policy left some files off the site copy.
    pub site_notice: Option<String>,
}

/// `Some` when another post or reply must wait.
pub fn rate_wait(last: Option<i64>, now: i64) -> Option<u64> {
    let last = last?;
    let wait = POST_GAP_SECS - (now - last);
    (wait > 0).then_some(wait as u64)
}

/// Stored script src against the one just seen.
pub fn bundle_changed(stored: Option<&str>, seen: Option<&str>, accept: bool) -> Option<(String, String)> {
    match (stored, seen) {
        (Some(was), Some(now)) if was != now && !accept => Some((was.to_string(), now.to_string())),
        _ => None,
    }
}

/// Build the plan. This does not look at the clock or the network.
pub fn prepare(d: &Draft) -> Result<WritePlan, WriteError> {
    if !d.writes_on {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let mut attachments = attach::plan_attachments(&d.attachments).map_err(err)?;
    let split = attach::split_for_site(&attachments, d.site_attachments.as_deref()).map_err(err)?;
    for (i, file) in attachments.iter_mut().enumerate() {
        file.on_site = split.site.contains(&i);
    }
    let site_notice = split.notice(&attachments);
    let kind = if d.thread.is_some() { Kind::Reply } else { Kind::Post };
    let thread = d.thread.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if kind == Kind::Reply && thread.is_none() {
        return Err(err("a reply needs a thread id"));
    }
    let title = d.title.trim().to_string();
    if kind == Kind::Post && title.is_empty() {
        return Err(err("a new post needs a title"));
    }
    let quote = d.quote.as_deref().map(str::trim).filter(|s| !s.is_empty());
    // The captured root reply always starts with postRef. postId and threadId
    // are both the thread id. A comment id here is the earlier inference: the
    // capture did not include a reply to a comment.
    let post_ref_ids = match (kind, thread) {
        (Kind::Reply, Some(thread)) => {
            let post = d
                .in_reply_to
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .or_else(|| d.quote_post.as_deref().map(str::trim).filter(|s| !s.is_empty()))
                .unwrap_or(thread);
            Some((post, thread))
        }
        _ => None,
    };
    let mut body = prosemirror::assemble(&d.body, quote, post_ref_ids)?;
    if kind == Kind::Reply {
        // The captured editor stores a blank line as two hardBreak nodes in
        // the same paragraph, not as a second paragraph node.
        collapse_blank_lines(&mut body);
    }
    let probe = d.html.as_deref().map(probe_html).unwrap_or_default();
    let cid = probe.cid.unwrap_or(match kind {
        Kind::Post => 1,
        Kind::Reply => 2,
    });
    let body_json = prosemirror::editor_json(&body);
    let mut fields: Vec<(String, String)> = Vec::new();
    if kind == Kind::Reply {
        if let Some(thread) = thread {
            fields.push(("form[thread_id]".into(), thread.to_string()));
        }
        fields.push(("form[body_input]".into(), body_json));
    } else {
        fields.push(("form[title]".into(), title.clone()));
        fields.push(("form[body_input]".into(), body_json));
        for (k, v) in &probe.hidden {
            fields.push((k.clone(), v.clone()));
        }
    }
    let mut omitted = if kind == Kind::Reply {
        vec![
            "the upload channel (allow_upload / lvu) has not been captured, so media_attachments and file_attachments are not encoded and no file bytes are sent".into(),
            "PDF and audio are not sent. The composer has a separate PDF input and no audio input; neither upload was captured".into(),
            "mention-query and tag-query replies have not been captured; completion is local and mention id is empty".into(),
            "no validate is sent after save. The captured browser sent one, and the server answered unmatched topic".into(),
            "join_ref, ref, and the LiveView topic are assigned on join. The printed frames use placeholders.".into(),
            "a reply to a comment or a quote sets postId to that post. The root-reply capture is the frame. Michael approved sending that shape.".into(),
            "thread:insert and live_redirect in the save reply carry the new post id. They are not sent by this client.".into(),
        ]
    } else {
        vec![
            "_target is not in a captured new-post form, so it is not invented. The sent fields are form[title] and form[body_input].".into(),
            "the upload channel (allow_upload / lvu) has not been captured, so media_attachments and file_attachments are not encoded and no file bytes are sent".into(),
            "mention-query and tag-query replies have not been captured; completion is local and mention id is empty".into(),
            "the new-post save reply was not in the P0 capture. A post id is read from thread:insert or live_redirect, the events the reply capture showed.".into(),
            "the LiveView topic and message ref are assigned on join. The printed frames use placeholders.".into(),
        ]
    };
    if d.html.is_none() {
        omitted.push("hidden fields were not read; no page was opened".into());
    } else if !probe.found {
        omitted.push("#js-postForm-form was not in the page; the documented cid is used".into());
    } else {
        if probe.submit.as_deref() != Some("save") {
            omitted.push(format!(
                "phx-submit is {:?}; the documented event name save is still what the frame uses",
                probe.submit
            ));
        }
        if probe.change.as_deref() != Some("validate") {
            omitted.push(format!(
                "phx-change is {:?}; the documented event name validate is still what the frame uses",
                probe.change
            ));
        }
    }
    let mut preview = String::new();
    crate::scrape::prosemirror_text(&body, &mut preview);
    let page = match thread {
        Some(id) => format!("/rs/post/{id}"),
        None => "/rs/post/new".into(),
    };
    Ok(WritePlan {
        kind,
        page,
        cid,
        fields,
        omitted,
        body,
        preview_text: crate::scrape::tidy_text(&preview),
        title,
        attachments,
        site_notice,
    })
}

/// The captured editor stores a blank line as two `hardBreak` nodes inside one paragraph.
fn collapse_blank_lines(doc: &mut Value) {
    let Some(arr) = doc.get_mut("content").and_then(Value::as_array_mut) else { return };
    let src = std::mem::take(arr);
    let mut out = Vec::new();
    let mut i = 0;
    while i < src.len() {
        if plain_paragraph(&src[i]) {
            let mut inline = paragraph_inlines(&src[i]);
            i += 1;
            while i < src.len() && plain_paragraph(&src[i]) {
                inline.push(json!({"type": "hardBreak"}));
                inline.push(json!({"type": "hardBreak"}));
                inline.extend(paragraph_inlines(&src[i]));
                i += 1;
            }
            if inline.is_empty() {
                out.push(json!({"type": "paragraph"}));
            } else {
                out.push(json!({"type": "paragraph", "content": inline}));
            }
        } else {
            out.push(src[i].clone());
            i += 1;
        }
    }
    *arr = out;
}

fn plain_inline(node: &Value) -> bool {
    matches!(node.get("type").and_then(Value::as_str), Some("text" | "hardBreak" | "mention"))
}

fn plain_paragraph(node: &Value) -> bool {
    if node.get("type").and_then(Value::as_str) != Some("paragraph") {
        return false;
    }
    match node.get("content") {
        None => true,
        Some(Value::Array(items)) => items.iter().all(plain_inline),
        Some(_) => false,
    }
}

fn paragraph_inlines(node: &Value) -> Vec<Value> {
    node.get("content").and_then(Value::as_array).cloned().unwrap_or_default()
}

/// Percent-encoding used by the captured form value: space is `%20`.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `form[thread_id]=…&form[body_input]=…`, with the brackets left literal, as in the capture.
fn captured_form_value(fields: &[(String, String)]) -> String {
    fields.iter().map(|(k, v)| format!("{k}={}", percent_encode(v))).collect::<Vec<_>>().join("&")
}

fn reply_event(ref_id: &str, event: &str, cid: u64, value: &str, validate: bool) -> Value {
    let payload = if validate {
        json!({
            "type": "form",
            "event": event,
            "meta": {"_target": "form[body_input]"},
            "uploads": {},
            "cid": cid,
            "value": value,
        })
    } else {
        json!({
            "type": "form",
            "event": event,
            "meta": {},
            "cid": cid,
            "value": value,
        })
    };
    json!(["1", ref_id, "lv:<id>", "event", payload])
}

/// Two LiveView form events, validate then save. No third frame is built:
/// the captured browser sent a validate after save and got `unmatched topic`.
pub fn liveview_frames(plan: &WritePlan) -> Vec<Value> {
    if plan.kind == Kind::Reply {
        let value = captured_form_value(&plan.fields);
        return vec![reply_event("1", "validate", plan.cid, &value, true), reply_event("2", "save", plan.cid, &value, false)];
    }
    let pairs: Vec<(&str, &str)> = plan.fields.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let value = form_query(&pairs);
    ["validate", "save"]
        .into_iter()
        .enumerate()
        .map(|(i, event)| {
            json!([
                "1",
                (i + 1).to_string(),
                "lv:<id>",
                "event",
                {"type": "form", "event": event, "value": value, "cid": plan.cid}
            ])
        })
        .collect()
}

/// Preview plus redacted frames. No socket is opened by building this string.
pub fn dry_run_text(plan: &WritePlan) -> String {
    let frames = serde_json::to_string_pretty(&liveview_frames(plan)).unwrap_or_default();
    let mut s = String::new();
    s.push_str("dry-run: no connection opened\n");
    s.push_str(match plan.kind {
        Kind::Post => "New post",
        Kind::Reply => "Reply",
    });
    s.push(' ');
    s.push_str(&plan.page);
    if !plan.title.is_empty() {
        s.push_str("\ntitle: ");
        s.push_str(&plan.title);
    }
    s.push_str("\n\n");
    s.push_str(plan.preview_text.trim_end());
    s.push_str("\n\n");
    if plan.attachments.is_empty() {
        s.push_str("attachments: none\n");
    } else if plan.attachments.iter().any(|f| !f.on_site) {
        s.push_str("attachments (local copy keeps every file; the site copy is a subset):\n");
        for file in &plan.attachments {
            let place = if file.on_site { "site" } else { "local-only" };
            s.push_str(&format!("- {place} {} {} ({} bytes) {}\n", file.kind.label(), file.name, file.bytes, file.path.display()));
        }
    } else {
        s.push_str("attachments (local only, not uploaded):\n");
        for file in &plan.attachments {
            s.push_str(&format!("- {} {} ({} bytes) {}\n", file.kind.label(), file.name, file.bytes, file.path.display()));
        }
    }
    if let Some(note) = &plan.site_notice {
        s.push_str(note);
        s.push('\n');
    }
    s.push('\n');
    s.push_str(&redact_secrets(&frames));
    s.push_str("\n\nnot sent:\n");
    for line in &plan.omitted {
        s.push_str("- ");
        s.push_str(line);
        s.push('\n');
    }
    s
}

/// The guarded entry for a write that must not open a socket.
///
/// Text publish does not call this. Vote, subscribe, delete, edit, repost,
/// and `--capture-reply` do. Under test, a URL on `acp.us` panics so a future
/// edit cannot reach the site from the harness.
pub fn refuse_live_write(url: &str) -> Result<()> {
    check_test_host(url);
    bail!("{CAPTURE_BLOCK}");
}

/// Under test, a URL on `acp.us` panics so a future edit cannot reach the site
/// from the harness. Outside tests this is a no-op.
#[cfg(test)]
fn check_test_host(url: &str) {
    if let Ok(parsed) = Url::parse(url) {
        let host = parsed.host_str().unwrap_or("");
        if host == "acp.us" || host.ends_with(".acp.us") {
            panic!("write path contacted {url}");
        }
    }
}

#[cfg(not(test))]
fn check_test_host(_url: &str) {}

/// Shown when a write is still gated.
///
/// Text posts and replies are published. Uploads and the uncaptured
/// interactions are not.
pub const CAPTURE_BLOCK: &str = "text posts and replies are published. Uploads, votes, subscribes, deletes, edits, and reposts are not sent. The upload channel is not confirmed, so a post with attachments is refused. --dry-run prints the frames and does not connect. --capture-reply does not send.";

/// `--capture-reply` stops here. It does not copy `session.json` or open a page.
pub fn capture_stub_message(thread: &str) -> String {
    format!(
        "--capture-reply {thread} did not open a connection and did not copy session.json.\n{CAPTURE_BLOCK}\n\
         The reviewed fixtures are tests/fixtures/site/reply_join.json, reply_validate.json, and reply_save.json. \
         This flag does not send. Text publish uses --new-post and --reply. Still not sent: the upload channel, a validate after save, mention-query and tag-query, and any live read-back."
    )
}

/// What a caller does after the safety checks.
#[derive(Debug, Clone, PartialEq)]
pub enum Gate {
    /// Dry-run text, or a cancel (`nothing sent`). No socket.
    Done(String),
    /// Text-only plan. Send validate, then save, once. Do not retry.
    Send(WritePlan),
}

/// A post id and the URL the server named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    /// Site post id.
    pub id: String,
    /// Absolute URL of the new post.
    pub url: String,
}

/// Apply the safety checks. A confirmed text post or reply is [`Gate::Send`].
/// A post with any attachment is refused, files included, so they are not dropped.
pub fn decide(d: &Draft, confirmed: bool, dry_run: bool) -> Result<Gate, WriteError> {
    let plan = prepare(d)?;
    if let Some(src) = d.html.as_deref().and_then(|h| probe_html(h).script_src) {
        if let Some((was, now)) = bundle_changed(d.stored_bundle.as_deref(), Some(&src), d.accept_bundle) {
            return Err(err(format!(
                "the site bundle changed ({was} → {now}). Writes stay off until --accept-site-update. Nothing was sent."
            )));
        }
    }
    if dry_run {
        return Ok(Gate::Done(dry_run_text(&plan)));
    }
    if !confirmed {
        return Ok(Gate::Done("nothing sent".into()));
    }
    if !plan.attachments.is_empty() {
        let names = plan.attachments.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");
        return Err(err(format!(
            "this post has attachments ({names}). Uploads are not confirmed, so the files were kept and nothing was sent. Remove them, then publish the text.\n\n{CAPTURE_BLOCK}\n\n{}",
            dry_run_text(&plan)
        )));
    }
    if let Some(wait) = rate_wait(d.last_write_unix, d.now_unix) {
        return Err(err(format!("rate limit: wait {wait}s before another post or reply. Nothing was sent.")));
    }
    Ok(Gate::Send(plan))
}

/// The event payload of each printed frame (index 4). Two payloads: validate, then save.
pub fn event_payloads(plan: &WritePlan) -> Vec<Value> {
    liveview_frames(plan).into_iter().filter_map(|frame| frame.get(4).cloned()).collect()
}

/// After `sent` payloads and the frames received so far, the next payload to push.
///
/// `Ok(None)` means both events were accepted and nothing else may be sent.
/// An error reply stops the exchange. A later call does not push again.
pub fn after_reply(payloads: &[Value], sent: usize, frames: &[Value]) -> Result<Option<Value>, WriteError> {
    if let Some(reason) = reply_error(frames) {
        return Err(err(format!("the site rejected the post: {reason}. Nothing was retried.")));
    }
    if sent < payloads.len() {
        return Ok(Some(payloads[sent].clone()));
    }
    Ok(None)
}

/// Read a post id and URL out of save replies, `thread:insert`, and `live_redirect`.
pub fn interpret(frames: &[Value], origin: &str) -> Result<Published, WriteError> {
    let mut ids = Vec::new();
    let mut urls = Vec::new();
    for frame in frames {
        note_frame(frame, &mut ids, &mut urls);
    }
    if ids.is_empty() && urls.is_empty() {
        if let Some(reason) = reply_error(frames) {
            return Err(err(format!("the site rejected the post: {reason}. Nothing was retried.")));
        }
        return Err(err("the server did not return a post id. Nothing was retried. Check the site before posting again."));
    }
    let url = match urls.first() {
        Some(to) if to.starts_with("http://") || to.starts_with("https://") => to.clone(),
        Some(to) => format!("{}{}", origin.trim_end_matches('/'), if to.starts_with('/') { to.clone() } else { format!("/{to}") }),
        None => format!("{}/rs/post/{}", origin.trim_end_matches('/'), ids.first().map(String::as_str).unwrap_or("")),
    };
    let id = ids.into_iter().next().or_else(|| url_tail(&url)).unwrap_or_default();
    if id.is_empty() {
        return Err(err(format!("the server returned {url} without a post id. Nothing was retried.")));
    }
    Ok(Published { id, url })
}

fn url_tail(url: &str) -> Option<String> {
    url.split('?').next().unwrap_or(url).trim_end_matches('/').rsplit('/').next().filter(|s| !s.is_empty()).map(str::to_string)
}

fn note_frame(frame: &Value, ids: &mut Vec<String>, urls: &mut Vec<String>) {
    if let Some(arr) = frame.as_array()
        && arr.len() >= 5
    {
        let event = arr[3].as_str().unwrap_or("");
        let payload = &arr[4];
        match event {
            "live_redirect" => {
                if let Some(to) = payload.get("to").and_then(Value::as_str)
                    && !to.is_empty()
                {
                    urls.push(to.to_string());
                }
            }
            "diff" => note_diff(payload, ids),
            _ => {}
        }
        return;
    }
    if frame.get("response").and_then(|r| r.get("diff")).is_some() {
        note_diff(&frame["response"]["diff"], ids);
    }
}

fn note_diff(payload: &Value, ids: &mut Vec<String>) {
    let Some(events) = payload.get("e").and_then(Value::as_array) else { return };
    for ev in events {
        let Some(pair) = ev.as_array() else { continue };
        if pair.first().and_then(Value::as_str) != Some("thread:insert") {
            continue;
        }
        if let Some(id) = pair.get(1).and_then(|v| v.get("id")).and_then(Value::as_str)
            && !id.is_empty()
        {
            ids.push(id.to_string());
        }
    }
}

fn reply_error(frames: &[Value]) -> Option<String> {
    let mut reason = None;
    for frame in frames {
        let payload = if let Some(arr) = frame.as_array()
            && arr.len() >= 5
            && arr[3].as_str() == Some("phx_reply")
        {
            &arr[4]
        } else if frame.get("status").is_some() {
            frame
        } else {
            continue;
        };
        if payload.get("status").and_then(Value::as_str) == Some("error") {
            let raw = payload.pointer("/response/reason").and_then(Value::as_str).unwrap_or("error");
            reason = Some(crate::logging::redact_secrets(raw));
        }
    }
    reason
}

/// Join the compose page and send validate, then save, once.
///
/// Under test, a URL on `acp.us` panics. Callers in tests use [`after_reply`]
/// and [`interpret`] with fixtures instead of this function.
pub async fn publish_text(http: &reqwest::Client, jar: &Jar, plan: &WritePlan, accept_bundle: bool) -> Result<Published> {
    if !plan.attachments.is_empty() {
        bail!("this post has attachments. Uploads are not confirmed, so nothing was sent.");
    }
    let origin = base_url().trim_end_matches('/').to_string();
    let url = format!("{origin}{}", plan.page);
    check_test_host(&url);
    let resp = http.get(&url).send().await.context("opening the compose page")?;
    if !resp.status().is_success() {
        bail!("the compose page returned {}. Nothing was retried.", resp.status());
    }
    let html = resp.text().await.context("reading the compose page")?;
    if !html.contains("js-postForm-form") {
        bail!("the compose form was not on the page. Log in, then try once. Nothing was retried.");
    }
    let probe = probe_html(&html);
    if let Some(src) = probe.script_src.as_deref() {
        let stored = stored_bundle();
        if let Some((was, now)) = bundle_changed(stored.as_deref(), Some(src), accept_bundle) {
            bail!("the site bundle changed ({was} → {now}). Writes stay off until --accept-site-update. Nothing was sent.");
        }
    }
    let mut plan = plan.clone();
    if let Some(cid) = probe.cid {
        plan.cid = cid;
    }
    let payloads = event_payloads(&plan);
    if payloads.len() != 2 {
        bail!("a text publish sends validate and save, not {} frames. Nothing was sent.", payloads.len());
    }
    let page = crate::liveview::parse_lv(&url, &html)?;
    let mut lv = crate::liveview::Lv::connect(&page, jar).await.context("joining the compose page")?;
    let mut frames = Vec::new();
    let mut sent = 0usize;
    let outcome = loop {
        match after_reply(&payloads, sent, &frames) {
            Err(e) => break Err(anyhow!("{e}")),
            Ok(None) => break interpret(&frames, &origin).map_err(|e| anyhow!("{e}")),
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
                            break Err(anyhow!("the save was sent and the reply could not be read ({msg}). Nothing was retried. Check the site before posting again."));
                        }
                    }
                }
            }
        }
    };
    lv.close().await;
    outcome
}

fn stored_bundle() -> Option<String> {
    let home = crate::config::home().ok()?;
    let db = home.join(crate::config::DB_REL_PATH);
    let c = crate::storage::db::open_db(&db).ok()?;
    crate::storage::db::site_state_get(&c, "bundle").ok().flatten()
}

/// Remember a confirmed publish so the next attempt inside 30 seconds waits.
pub fn store_last_write(now: i64) {
    let Ok(home) = crate::config::home() else { return };
    let Ok(c) = crate::storage::db::open_db(&home.join(crate::config::DB_REL_PATH)) else { return };
    let _ = crate::storage::db::site_state_set(&c, "last_write_unix", &now.to_string());
}

/// The last confirmed publish, when the encrypted database can be opened.
pub fn load_last_write() -> Option<i64> {
    let home = crate::config::home().ok()?;
    let c = crate::storage::db::open_db(&home.join(crate::config::DB_REL_PATH)).ok()?;
    crate::storage::db::site_state_get(&c, "last_write_unix").ok().flatten()?.parse().ok()
}

/// `ui.writes` defaults to on. `ACP_WRITES=off` (or `0`, `false`, `no`) wins.
/// `ACP_WRITES=on` (or `1`, `true`, `yes`) turns writes on even when the file says off.
pub fn writes_enabled_from(pref: Option<bool>, env: Option<&str>) -> bool {
    if let Some(v) = env {
        let v = v.trim().to_ascii_lowercase();
        if matches!(v.as_str(), "0" | "off" | "false" | "no") {
            return false;
        }
        if matches!(v.as_str(), "1" | "on" | "true" | "yes") {
            return true;
        }
    }
    pref.unwrap_or(true)
}

/// The switch for this process: the environment, then `~/.acp/preferences.toml`.
pub fn writes_enabled() -> bool {
    let env = std::env::var("ACP_WRITES").ok();
    let pref = crate::auth::prefs::load_prefs().ui.and_then(|u| u.writes);
    writes_enabled_from(pref, env.as_deref())
}
