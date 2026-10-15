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
    assert_eq!(body_text(&json!({"body": "<p>a &amp; <b>b</b></p><p>c</p>"})), "a & b\nc");
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
