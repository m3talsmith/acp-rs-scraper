//! Quote repost. The captured page is `/rs/post/new?repost=ID`.
//!
//! The save sends `form[quoted_post_id]`, `form[title]`, and `form[body_input]`
//! (a short comment, not a `postRef`). A plain repost with an empty comment
//! was not saved in the capture, so an empty body is refused before a connection.

use super::write::{captured_form_value, collapse_blank_lines, form_event, WriteError};
use crate::prelude::*;

/// A confirmed quote repost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepostPlan {
    /// The post being reposted.
    pub id: String,
    /// `phx-target`. The capture used 1.
    pub cid: u64,
    /// `form[quoted_post_id]=…&form[title]=…&form[body_input]=…`.
    pub value: String,
    /// Plain text of the comment, for the preview.
    pub preview: String,
    /// Title sent with the comment.
    pub title: String,
}

/// Dry-run text, a cancel, or a plan to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepostGate {
    /// Printed text. No socket.
    Done(String),
    /// Confirmed. Open the repost page, then validate, then save, once.
    Send(RepostPlan),
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

/// The route the click opens.
pub fn route(id: &str) -> String {
    format!("/rs/post/new?repost={id}")
}

/// A plain repost (empty comment) was not in the capture.
pub const PLAIN_REPOST_MISSING: &str = "A plain repost was not captured. The missing frame is a save on /rs/post/new?repost=ID with an empty form[body_input]. Nothing was sent.";

/// Build the quote-repost plan. This does not open a socket.
pub fn prepare(id: &str, title: &str, body: &str, writes_on: bool) -> Result<RepostPlan, WriteError> {
    if !writes_on {
        return Err(err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent."));
    }
    let id = super::hook::clean_id(id, "repost")?;
    if body.trim().is_empty() {
        return Err(err(PLAIN_REPOST_MISSING));
    }
    if title.trim().is_empty() {
        return Err(err("the captured repost save includes a title. Pass --repost-title. Nothing was sent."));
    }
    let mut doc = super::assemble(body, None, None)?;
    collapse_blank_lines(&mut doc);
    let body_json = super::prosemirror::editor_json(&doc);
    let value = captured_form_value(&[
        ("form[quoted_post_id]".into(), id.clone()),
        ("form[title]".into(), title.trim().to_string()),
        ("form[body_input]".into(), body_json),
    ]);
    let mut preview = String::new();
    crate::scrape::prosemirror_text(&doc, &mut preview);
    Ok(RepostPlan {
        id,
        cid: 1,
        value,
        preview: crate::scrape::tidy_text(&preview),
        title: title.trim().to_string(),
    })
}

/// Validate, then save. The printed payloads are the ones a confirmed repost sends.
pub fn frames(plan: &RepostPlan) -> Vec<Value> {
    vec![form_event("1", "validate", plan.cid, &plan.value, "form[body_input]", true), form_event("2", "save", plan.cid, &plan.value, "form[body_input]", false)]
}

/// Preview plus the two frames.
pub fn dry_run_text(plan: &RepostPlan) -> String {
    let frames = serde_json::to_string_pretty(&frames(plan)).unwrap_or_default();
    format!(
        "dry-run: no connection opened\nrepost {}\nroute: {}\ntitle: {}\n\n{}\n\nopen (sent after confirm): phx_leave, then phx_join redirect {}\nsticky false, _mounts 0. _track_static is not sent.\n\n{}\n\nnot sent:\n- keystroke validates are not sent. One validate, then one save.\n- no validate is sent after save.\n- A plain repost with an empty comment is not sent.\n",
        plan.id,
        route(&plan.id),
        plan.title,
        plan.preview.trim_end(),
        route(&plan.id),
        crate::logging::redact_secrets(&frames)
    )
}

/// Apply the safety checks.
pub fn present(id: &str, title: &str, body: &str, confirmed: bool, dry_run: bool) -> Result<RepostGate, WriteError> {
    let plan = prepare(id, title, body, super::write::writes_enabled())?;
    if dry_run {
        return Ok(RepostGate::Done(dry_run_text(&plan)));
    }
    if !confirmed {
        return Ok(RepostGate::Done("nothing sent".into()));
    }
    Ok(RepostGate::Send(plan))
}

/// The mounted repost page must be the captured form.
pub fn repost_page(html: &str) -> Result<super::write::Probe, WriteError> {
    if !html.contains("form[quoted_post_id]") {
        return Err(err("the repost form was not on the page. Nothing was sent."));
    }
    if !html.contains("name=\"form[title]\"") && !html.contains("name='form[title]'") {
        return Err(err("the repost form has no title. Nothing was sent."));
    }
    let probe = super::write::probe_html(html);
    if !probe.found || probe.submit.as_deref() != Some("save") || probe.change.as_deref() != Some("validate") {
        return Err(err("the repost form is not the captured save/validate form. Nothing was sent."));
    }
    Ok(probe)
}

/// Join `/rs`, redirect to the repost page, then send validate and save once.
pub async fn publish_repost(http: &reqwest::Client, jar: &Jar, plan: &RepostPlan) -> Result<super::write::Published> {
    let origin = base_url().trim_end_matches('/').to_string();
    let bootstrap = format!("{origin}/rs");
    super::write::check_test_host(&bootstrap);
    let url = format!("{origin}{}", route(&plan.id));
    let resp = http.get(&bootstrap).send().await.context("opening the repost page")?;
    if !resp.status().is_success() {
        bail!("the repost page returned {}. Nothing was retried.", resp.status());
    }
    let html = resp.text().await.context("reading the repost page")?;
    let page = crate::liveview::parse_lv(&bootstrap, &html).context("reading the repost page")?;
    let mut lv = crate::liveview::Lv::connect(&page, jar).await.context("joining the repost page")?;
    let outcome = async {
        lv.live_redirect(&page, &url).await?;
        let mounted = lv.html();
        let probe = repost_page(&mounted).map_err(|e| anyhow!("{e}"))?;
        let mut plan = plan.clone();
        if let Some(cid) = probe.cid {
            plan.cid = cid;
        }
        let payloads: Vec<Value> = frames(&plan).into_iter().filter_map(|frame| frame.get(4).cloned()).collect();
        let mut frames_in = Vec::new();
        let mut sent = 0usize;
        loop {
            match super::write::after_reply(&payloads, sent, &frames_in) {
                Err(e) => break Err(anyhow!("{e}")),
                Ok(None) => break super::write::interpret(&frames_in, &origin).map_err(|e| anyhow!("{e}")),
                Ok(Some(payload)) => {
                    let reply = match lv.push("event", payload).await {
                        Ok(reply) => reply,
                        Err(e) => break Err(anyhow!("{}. Nothing was retried.", crate::logging::redact_secrets(&format!("{e:#}")))),
                    };
                    let errored = reply.get("status").and_then(Value::as_str) == Some("error");
                    frames_in.push(reply);
                    sent += 1;
                    if sent == payloads.len() && !errored {
                        match lv.collect_for(std::time::Duration::from_secs(3)).await {
                            Ok(extra) => frames_in.extend(extra),
                            Err(e) => {
                                break Err(anyhow!(
                                    "the save was sent and the reply could not be read ({}). Nothing was retried. Check the site before reposting again.",
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

    #[test]
    fn a_quote_repost_prints_the_captured_fields_and_an_empty_comment_is_refused() {
        let empty = prepare("POST", "Title", "  ", true).unwrap_err();
        assert!(empty.message.contains("empty form[body_input]"), "{}", empty.message);
        let no_title = prepare("POST", " ", "a short quote", true).unwrap_err();
        assert!(no_title.message.contains("--repost-title"), "{}", no_title.message);
        let plan = prepare("POST", "Noted", "a short quote", true).unwrap();
        assert!(plan.value.contains("form[quoted_post_id]=POST"), "{}", plan.value);
        assert!(plan.value.contains("form[title]="), "{}", plan.value);
        assert!(plan.value.contains("form[body_input]="), "{}", plan.value);
        assert!(!plan.value.contains("postRef"), "{}", plan.value);
        let sent = frames(&plan);
        assert_eq!(sent[0][4]["event"], "validate");
        assert_eq!(sent[0][4]["meta"]["_target"], "form[body_input]");
        assert!(sent[0][4].get("uploads").is_some());
        assert_eq!(sent[1][4]["event"], "save");
        assert!(sent[1][4].get("uploads").is_none());
        assert_eq!(sent[0][4]["value"], sent[1][4]["value"]);
        let RepostGate::Done(dry) = present("POST", "Noted", "a short quote", false, true).unwrap() else { panic!("dry-run") };
        assert!(dry.contains("dry-run: no connection opened"), "{dry}");
        assert!(dry.contains("/rs/post/new?repost=POST"), "{dry}");
        assert!(dry.contains(&plan.value), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        let RepostGate::Done(cancel) = present("POST", "Noted", "a short quote", false, false).unwrap() else { panic!("cancel") };
        assert_eq!(cancel, "nothing sent");
        let html = r#"<form id="js-postForm-form" method="post" phx-submit="save" phx-change="validate" phx-target="1"><input type="hidden" name="form[quoted_post_id]" value="POST"><input name="form[title]"><textarea name="form[body_input]"></textarea></form>"#;
        assert_eq!(repost_page(html).unwrap().cid, Some(1));
    }
}
