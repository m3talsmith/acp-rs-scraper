//! Unit tests for post body extraction across the shapes the site sends.

use super::*;

#[test]
fn prosemirror_object_and_string_bodies() {
    let doc = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hi"}]}]});
    assert_eq!(body_text(&json!({"body": doc})), "hi");
    assert_eq!(body_text(&json!({"body": doc.to_string()})), "hi");
}

#[test]
fn html_and_plain_string_bodies() {
    assert_eq!(body_text(&json!({"body": "<p>a &amp; <b>b</b></p><p>c</p>"})), "a & b\n\nc");
    assert_eq!(body_text(&json!({"body": "just text"})), "just text");
}

#[test]
fn fallback_fields_and_wrapped_posts() {
    assert_eq!(body_text(&json!({"body": null, "body_html": "<p>x</p>"})), "x");
    assert_eq!(body_text(&json!({"body": {}, "body_text_summary": "sum"})), "sum");
    assert_eq!(body_text(&json!({"body": {"html": "<i>y</i>"}})), "y");
    let p = to_post(&json!({"id": "r1", "post": {"title": "T", "body": "z"}}), "t", "n");
    assert_eq!((p.title.as_str(), p.content.as_str()), ("T", "z"));
}

#[test]
fn empty_body_stays_empty() {
    assert_eq!(body_text(&json!({"id": "r1", "body": null})), "");
}

#[test]
fn said_by_matches_own_items_only() {
    use serde_json::json;
    assert!(said_by_nick(&json!({"author": {"username": "@Haz"}}), "haz"));
    assert!(!said_by_nick(&json!({"author": {"username": "bob"}}), "haz"));
    assert!(said_by_nick(&json!({"reply": {"author_username": "haz"}}), "haz"));
    assert!(!said_by_nick(&json!({"post": {"author": {"username": "bob"}}}), "haz"));
    assert!(said_by_nick(&json!({"id": "1"}), "haz"));
}

#[test]
fn reply_to_comment_uses_post_ref_as_immediate_parent() {
    let body = json!({"type": "doc", "content": [
        {"type": "postRef", "attrs": {"postId": "c1", "threadId": "t1"}},
        {"type": "paragraph", "content": [{"type": "text", "text": "agreed"}]}]});
    let p = to_post(&json!({"id": "r2", "thread_id": "t1", "body": body}), "t1", "n");
    assert_eq!((p.parent_post_id.as_str(), p.in_reply_to_id.as_str()), ("t1", "c1"));
    // Wrapped and string-encoded bodies too.
    let w = to_post(&json!({"id": "r3", "reply": {"id": "r3", "body": body.to_string()}}), "t1", "n");
    assert_eq!(w.in_reply_to_id, "c1");
    // A reply straight to the thread root, and a root post.
    assert_eq!(to_post(&json!({"id": "r4", "body": {}}), "t1", "n").in_reply_to_id, "t1");
    assert_eq!(to_post(&json!({"id": "t1", "body": body}), "", "n").in_reply_to_id, "");
}

#[test]
fn profile_landing_redirect_to_trending_fails() {
    let e = check_profile_landing("https://acp.us/rs/@haz", "https://acp.us/rs", "").unwrap_err();
    assert!(e.to_string().contains("no profile @haz"));
}

#[test]
fn profile_landing_ok_and_flash() {
    assert!(check_profile_landing("https://acp.us/rs/@hazaldin?replies=1", "https://acp.us/rs/@hazaldin?replies=1", "<html>").is_ok());
    assert!(check_profile_landing("https://acp.us/rs", "https://acp.us/rs", "").is_ok());
    assert!(check_profile_landing("https://acp.us/rs/@x", "https://acp.us/rs/@x",
        "No profile exists with the username @x").is_err());
    assert_eq!(profile_nick_of("https://acp.us/rs/@Haz/posts"), Some("Haz".into()));
}

#[test]
fn paragraphs_are_separated_by_blank_lines() {
    let p = |t: &str| json!({"type": "paragraph", "content": [{"type": "text", "text": t}]});
    let doc = json!({"type": "doc", "content": [p("one. "), p("two"), {"type": "bulletList", "content": [
        {"type": "listItem", "content": [p("item")]}]}]});
    assert_eq!(body_text(&json!({"body": doc})), "one.\n\ntwo\n\nitem");
    let br = json!({"type": "doc", "content": [{"type": "paragraph", "content": [
        {"type": "text", "text": "a"}, {"type": "hard_break"}, {"type": "text", "text": "b"}]}]});
    assert_eq!(body_text(&json!({"body": br})), "a\nb");
    assert_eq!(body_text(&json!({"body": "<p>x<br>y</p><p>z</p>"})), "x\ny\n\nz");
}

#[test]
fn tidy_text_collapses_blank_runs() {
    assert_eq!(tidy_text("  \na  \n\n\n\nb\n\n"), "a\n\nb");
}

#[test]
fn truncated_feed_items_are_flagged() {
    assert!(is_truncated(&json!({"body_truncated": true})));
    assert!(is_truncated(&json!({"reply": {"body_truncated": true}})));
    assert!(!is_truncated(&json!({"body_truncated": false, "truncate": true})));
    let p = to_post(&json!({"id": "x", "body_truncated": true, "body": "short"}), "", "n");
    assert!(p.truncated);
    assert!(!to_post(&json!({"id": "x", "body": "full"}), "", "n").truncated);
}
