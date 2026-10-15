//! Unit tests for the "Posts and replies" filter URLs and reply parenting.

use super::*;

#[test]
fn profile_filter_urls() {
    assert_eq!(profile_replies_url("alice"), format!("{}/rs/@alice?replies=1", base_url()));
    assert_eq!(profile_top_url("alice"), format!("{}/rs/@alice/top?t=all", base_url()));
}

#[test]
fn reply_from_replies_feed_gets_thread_parent() {
    let p = to_post(&json!({"id": "r1", "thread_id": "t9", "inserted_at": "2026-10-15T00:00:00Z",
        "author": {"username": "alice"}, "body": {}}), "t9", "alice");
    assert_eq!((p.post_id.as_str(), p.parent_post_id.as_str()), ("r1", "t9"));
}

#[test]
fn site_share_images_are_not_avatars() {
    assert!(is_site_image("https://acp.us/banner.jpg"));
    assert!(is_site_image("https://acp.us/banner.jpg?v=2"));
    assert!(!is_site_image("https://media.acp.us/uploads/avatar-abc.avif"));
    assert!(!is_site_image("https://acp.us/media/avatar-dave.png"));
}
