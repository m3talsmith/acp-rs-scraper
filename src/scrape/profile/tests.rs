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

#[test]
fn reply_only_nick_photo_found_past_banner_false_lead() {
    let mut p = Profile { nick: "haz".into(), ..Default::default() };
    let items = vec![
        // A reply wrapped in {"reply": ...}; the item root carries the site banner.
        json!({"id": "r1", "image": "https://acp.us/banner.jpg",
               "reply": {"id": "r1", "author": {"username": "Haz", "user_id": "u-haz", "display_name": "Haz",
                   "profile_photo_url": "https://media.acp.us/uploads/profile-u-haz.avif?X-Amz-Signature=x"}}}),
    ];
    enrich_from_feed(&mut p, &items, "haz");
    assert_eq!(p.avatar_url.as_deref(), Some("https://media.acp.us/uploads/profile-u-haz.avif?X-Amz-Signature=x"));
    assert_eq!(p.user_id.as_deref(), Some("u-haz"));
    assert_eq!(p.display_name.as_deref(), Some("Haz"));
}

#[test]
fn banner_is_never_taken_as_avatar_from_json() {
    let mut p = Profile::default();
    enrich_profile(&mut p, &json!({"avatar_url": "https://acp.us/banner.jpg",
        "author_profile_photo_url": "https://media.acp.us/uploads/profile-z.avif"}));
    assert_eq!(p.avatar_url.as_deref(), Some("https://media.acp.us/uploads/profile-z.avif"));
}
