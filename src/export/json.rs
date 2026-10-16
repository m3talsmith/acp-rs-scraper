//! Export document (format version 2) as JSON.

use crate::prelude::*;

/// `rel`: map attachment url -> path written into the export.
pub fn build_json(nick: &str, posts: &[Post], atts: &[Attachment], profiles: &[Profile], hits: &[MentionHit], rel: &dyn Fn(&Attachment) -> String) -> Value {
    let mut by_post: HashMap<&str, Vec<Value>> = HashMap::new();
    for a in atts {
        by_post.entry(a.post_id.as_str()).or_default().push(json!({
            "url": a.url, "kind": a.kind, "mime_type": a.mime_type, "local_path": rel(a),
        }));
    }
    let ids: HashSet<&str> = posts.iter().map(|p| p.post_id.as_str()).collect();
    let mut kids: HashMap<&str, Vec<&Post>> = HashMap::new();
    let mut roots = Vec::new();
    for p in posts {
        if p.parent_post_id.is_empty() || !ids.contains(p.parent_post_id.as_str()) {
            roots.push(p);
        } else {
            kids.entry(p.parent_post_id.as_str()).or_default().push(p);
        }
    }
    fn node(p: &Post, kids: &HashMap<&str, Vec<&Post>>, by: &HashMap<&str, Vec<Value>>) -> Value {
        let mut ch: Vec<&Post> = kids.get(p.post_id.as_str()).cloned().unwrap_or_default();
        ch.sort_by(|a, b| a.posted_at.cmp(&b.posted_at));
        json!({
            "post_id": p.post_id, "kind": post_kind(p), "parent_post_id": p.parent_post_id, "in_reply_to_id": p.in_reply_to_id, "nick": p.nick,
            "title": p.title, "content": p.content, "posted_at": p.posted_at, "truncated": p.truncated,
            "attachments": by.get(p.post_id.as_str()).cloned().unwrap_or_default(),
            "children": ch.iter().map(|c| node(c, kids, by)).collect::<Vec<_>>(),
        })
    }
    // Top level: the nick's own thread posts first (newest first), then
    // replies whose parent is not in this export (newest first), so posts
    // are never buried among replies.
    roots.sort_by(|a, b| (!a.parent_post_id.is_empty()).cmp(&!b.parent_post_id.is_empty()).then(b.posted_at.cmp(&a.posted_at)));
    let n_posts = posts.iter().filter(|p| p.parent_post_id.is_empty()).count();
    let profiles_v: Vec<Value> = profiles.iter().map(|p| json!({
        "nick": p.nick, "user_id": p.user_id, "display_name": p.display_name, "bio": p.bio,
        "avatar_url": p.avatar_url, "avatar_path": p.avatar_path,
        "banner_url": p.banner_url, "banner_path": p.banner_path,
        "location": p.location, "role": p.role, "joined_at": p.joined_at,
        "followers": p.followers, "following": p.following, "posts_count": p.posts_count,
        "scraped_at": p.scraped_at,
    })).collect();
    let mentions_v: Vec<Value> = hits.iter().map(|h| json!({
        "post_id": h.post_id, "mentioned_nick": h.mentioned_nick, "match_type": h.match_type.as_str(),
    })).collect();
    json!({
        "nick": nick,
        "format": EXPORT_FORMAT,
        "format_version": EXPORT_FORMAT_VERSION,
        "exported_at": chrono::Local::now().to_rfc3339(),
        "counts": { "posts": n_posts, "replies": posts.len() - n_posts },
        "profiles": profiles_v,
        "mentions": mentions_v,
        "posts": roots.iter().map(|p| node(p, &kids, &by_post)).collect::<Vec<_>>(),
    })
}

/// `"post"` for a thread root (top-level post), `"reply"` for a reply/comment.
pub fn post_kind(p: &Post) -> &'static str {
    if p.parent_post_id.is_empty() { "post" } else { "reply" }
}
