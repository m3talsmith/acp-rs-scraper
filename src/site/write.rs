//! A prepared New post or Reply, and the decision to send it.
//!
//! The form fields and the LiveView `type: form` encoding are the ones this
//! client already uses for reads. `_target`, the upload channel, the
//! mention-query reply, and the server's save reply are not invented: they
//! stay on the omitted list until a guided capture confirms them. Confirming
//! a publish does not open a connection.

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
    /// The thread page, with `form[thread_id]`.
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
    /// `None` means the default: every file, or the first [`crate::config::SITE_MAX_ATTACHMENTS`] when there are more.
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
    let split = attach::split_for_site(attachments.len(), d.site_attachments.as_deref()).map_err(err)?;
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
    // A reply to the thread root uses form[thread_id] and no postRef.
    // A reply to a comment, or a quote of a post, puts postRef first.
    let post_ref_ids = match (kind, d.in_reply_to.as_deref(), d.quote_post.as_deref(), thread) {
        (Kind::Reply, Some(post), _, Some(thread)) if !post.trim().is_empty() && post.trim() != thread => {
            Some((post.trim(), thread))
        }
        (Kind::Reply, _, Some(post), Some(thread)) if !post.trim().is_empty() => Some((post.trim(), thread)),
        _ => None,
    };
    let body = prosemirror::assemble(&d.body, quote, post_ref_ids)?;
    let probe = d.html.as_deref().map(probe_html).unwrap_or_default();
    let cid = probe.cid.unwrap_or(match kind {
        Kind::Post => 1,
        Kind::Reply => 2,
    });
    let body_json = serde_json::to_string(&body).map_err(|e| err(e.to_string()))?;
    let mut fields: Vec<(String, String)> = Vec::new();
    if kind == Kind::Post {
        fields.push(("form[title]".into(), title.clone()));
    }
    fields.push(("form[body_input]".into(), body_json));
    if let Some(thread) = thread {
        fields.push(("form[thread_id]".into(), thread.to_string()));
    }
    for (k, v) in &probe.hidden {
        fields.push((k.clone(), v.clone()));
    }
    let mut omitted = vec![
        "_target is not in a captured form, so it is not invented".into(),
        "the upload channel (allow_upload / lvu) has not been captured, so media_attachments and file_attachments are not encoded and no file bytes are sent".into(),
        "mention-query and tag-query replies have not been captured; completion is local and mention id is empty".into(),
        "the save reply and the read-back of the new post have not been captured".into(),
        "the LiveView topic and message ref are assigned on join; these frames use placeholders and no join is opened".into(),
    ];
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

/// Two LiveView form events, validate then save, in the encoding [`crate::liveview::Lv::form_fields`] already sends.
pub fn liveview_frames(plan: &WritePlan) -> Vec<Value> {
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
    s.push_str("\n\nomitted until a guided capture confirms them:\n");
    for line in &plan.omitted {
        s.push_str("- ");
        s.push_str(line);
        s.push('\n');
    }
    s
}

/// The only write entry that would open a socket. It never does.
///
/// Under test, a URL on `acp.us` panics so a future edit cannot reach the site
/// from the harness. Outside tests it returns the capture error.
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

/// Shown when a confirmed publish is refused.
pub const CAPTURE_BLOCK: &str = "nothing was published. The P0 guided capture has not confirmed the submit payload or the upload channel, so validate, save, and file bytes are not sent. Run with --dry-run to print the documented frames and the local attachment list. --capture-reply does not send either.";

/// `--capture-reply` stops here. It does not copy `session.json` or open a page.
pub fn capture_stub_message(thread: &str) -> String {
    format!(
        "--capture-reply {thread} did not open a connection and did not copy session.json.\n{CAPTURE_BLOCK}\n\
         Still stubbed: the read-only warm-up (join, form cids, /rs/notifications), _target, the changeset error shape, the upload channel, mention-query and tag-query replies, and read-back of the saved post."
    )
}

/// Apply the safety checks. `Ok` is a dry-run print or a cancel. A confirmed
/// publish is an error, because the send is stubbed, and the message includes
/// the frames that were not sent.
pub fn decide(d: &Draft, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    let plan = prepare(d)?;
    if let Some(src) = d.html.as_deref().and_then(|h| probe_html(h).script_src) {
        if let Some((was, now)) = bundle_changed(d.stored_bundle.as_deref(), Some(&src), d.accept_bundle) {
            return Err(err(format!(
                "the site bundle changed ({was} → {now}). Writes stay off until --accept-site-update. Nothing was sent."
            )));
        }
    }
    if dry_run {
        return Ok(dry_run_text(&plan));
    }
    if !confirmed {
        return Ok("nothing sent".into());
    }
    if let Some(wait) = rate_wait(d.last_write_unix, d.now_unix) {
        return Err(err(format!("rate limit: wait {wait}s before another post or reply. Nothing was sent.")));
    }
    Err(err(format!("{CAPTURE_BLOCK}\n\n{}", dry_run_text(&plan))))
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
