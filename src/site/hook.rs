//! One LiveView hook on the post page (`/rs/post/ID`).
//!
//! Vote, subscribe, bookmark, and delete all join that page and push `event`
//! once. The printed frame is the payload that is pushed.

use super::write::{WriteError, check_test_host};
use crate::prelude::*;

/// A confirmed hook that still has to be sent.
#[derive(Debug, Clone, PartialEq)]
pub struct HookCall {
    /// Post id. The page is `/rs/post/{id}`.
    pub id: String,
    /// The `event` payload.
    pub payload: Value,
}

/// Dry-run text, a cancel, or a hook to send.
#[derive(Debug, Clone, PartialEq)]
pub enum HookGate {
    /// Printed text. No socket.
    Done(String),
    /// Confirmed. Join the post page and push [`HookCall::payload`] once.
    Send(HookCall),
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

/// A post id the site accepts in a hook: no spaces, not empty.
pub fn clean_id(id: &str, what: &str) -> Result<String, WriteError> {
    let id = id.trim();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
        return Err(err(format!("a {what} needs a post id")));
    }
    Ok(id.to_string())
}

/// Phoenix event frame. `join_ref`, `ref`, and the topic are placeholders.
pub fn frame(payload: &Value) -> Value {
    json!(["1", "1", "lv:<id>", "event", payload])
}

/// Dry-run text. `note` is what the reply is expected to show.
pub fn dry_run(intro: &str, payload: &Value, note: &str) -> String {
    let frame = serde_json::to_string_pretty(&frame(payload)).unwrap_or_default();
    format!("dry-run: no connection opened\n{intro}\n\n{frame}\n\n{note}\n")
}

/// Join `/rs/post/{id}` and push the hook once.
///
/// Under test, a URL on `acp.us` panics. The returned value is the `phx_reply`.
pub async fn publish_hook(http: &reqwest::Client, jar: &Jar, call: &HookCall) -> Result<Value> {
    let origin = base_url().trim_end_matches('/').to_string();
    let url = format!("{origin}/rs/post/{}", call.id);
    check_test_host(&url);
    let resp = http.get(&url).send().await.context("opening the post")?;
    if !resp.status().is_success() {
        bail!("the post page returned {}. Nothing was retried.", resp.status());
    }
    let final_url = resp.url().to_string();
    let html = resp.text().await.context("reading the post")?;
    let page = crate::liveview::parse_lv(&final_url, &html).context("reading the post")?;
    let mut lv = crate::liveview::Lv::connect(&page, jar).await.context("joining the post")?;
    let outcome = lv.push("event", call.payload.clone()).await;
    lv.close().await;
    let reply = outcome.map_err(|e| anyhow!("{}. Nothing was retried.", crate::logging::redact_secrets(&format!("{e:#}"))))?;
    if reply.get("status").and_then(Value::as_str) == Some("error") {
        bail!("the site rejected it. Nothing was retried.");
    }
    Ok(reply)
}

/// `score`, `vote`, and `upvote_percentage` from a vote reply, when present.
pub fn vote_summary(id: &str, state: &str, reply: &Value) -> String {
    let post = reply.pointer("/response/diff/r/post");
    let score = post.and_then(|p| p.get("score")).map(|v| v.to_string()).unwrap_or_else(|| "absent".into());
    let vote = match post.and_then(|p| p.get("vote")) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => "none".into(),
        Some(other) => other.to_string(),
    };
    let pct = post.and_then(|p| p.get("upvote_percentage")).map(|v| v.to_string()).unwrap_or_else(|| "absent".into());
    format!("voted {id} state {state}\nscore {score}\nvote {vote}\nupvote_percentage {pct}")
}

/// `subscribed` from a subscribe reply.
pub fn subscribe_summary(id: &str, reply: &Value) -> String {
    let subscribed = reply.pointer("/response/diff/r/post/subscribed").and_then(Value::as_bool);
    match subscribed {
        Some(true) => format!("subscribed {id}\nsubscribed true"),
        Some(false) => format!("subscribed {id}\nsubscribed false"),
        None => format!("subscribed {id}"),
    }
}

/// `bookmarked` and `bookmarks_count` from a bookmark reply.
pub fn bookmark_summary(id: &str, reply: &Value) -> String {
    let post = reply.pointer("/response/diff/r/post");
    let on = post.and_then(|p| p.get("bookmarked")).and_then(Value::as_bool);
    let count = post.and_then(|p| p.get("bookmarks_count")).map(|v| v.to_string()).unwrap_or_else(|| "absent".into());
    match on {
        Some(true) => format!("bookmarked {id}\nbookmarked true\nbookmarks_count {count}"),
        Some(false) => format!("bookmarked {id}\nbookmarked false\nbookmarks_count {count}"),
        None => format!("bookmarked {id}\nbookmarks_count {count}"),
    }
}

/// Delete succeeds when the reply carries `live_redirect`. The captured target is `/rs`.
pub fn delete_summary(id: &str, reply: &Value) -> Result<String, WriteError> {
    let to = reply.pointer("/response/live_redirect/to").and_then(Value::as_str).or_else(|| {
        reply.pointer("/live_redirect/to").and_then(Value::as_str)
    });
    let Some(to) = to else {
        return Err(err("the delete was sent and the reply had no live_redirect. Nothing was retried. Check the site before deleting again."));
    };
    let _ = crate::storage::db::note_tombstone(id);
    Ok(format!("deleted {id}\nthe site opened {to}\nthe local copy stays, with a tombstone"))
}
