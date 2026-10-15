//! Scraping: turning site JSON into [`Post`]s and walking a nick's profile
//! (the Posts feed and its "Posts and replies" filter) plus the threads they started.

pub mod feed;
pub mod mentions;
pub mod profile;
pub mod thread;

use crate::prelude::*;

#[derive(Debug, Clone)]
/// One stored post (the `posts` table row), plus the media it references.
pub struct Post {
    /// Site post id (22-character base64url).
    pub post_id: String,
    /// Thread root id for replies; `""` for root posts.
    pub parent_post_id: String,
    /// Nick (username, without `@`).
    pub nick: String,
    /// Title (`""` for replies).
    pub title: String,
    /// Body flattened to plain text.
    pub content: String,
    /// `inserted_at` timestamp (ISO 8601).
    pub posted_at: String,
    /// Media referenced by the post (url, kind).
    pub media: Vec<MediaRef>,
}

/// Flatten a ProseMirror doc (`{type, text?, content:[...]}`) to plain text.
pub fn prosemirror_text(v: &Value, out: &mut String) {
    if let Some(t) = v.get("text").and_then(Value::as_str) {
        out.push_str(t);
    }
    let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
    if ty == "hard_break" || ty == "hardBreak" {
        out.push('\n');
    }
    if let Some(kids) = v.get("content").and_then(Value::as_array) {
        for k in kids {
            prosemirror_text(k, out);
        }
    }
    if matches!(ty, "paragraph" | "heading" | "blockquote" | "list_item" | "listItem" | "code_block" | "codeBlock") {
        out.push('\n');
    }
}

/// String field `k` of a JSON object, or `""`.
pub fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// Convert site post JSON to a [`Post`] with the given parent (`""` for roots), flattening the body to text.
pub fn to_post(v: &Value, parent: &str, fallback_nick: &str) -> Post {
    let mut content = String::new();
    if let Some(b) = v.get("body") {
        let b = match b.as_str() {
            Some(raw) => serde_json::from_str(raw).unwrap_or(Value::Null),
            None => b.clone(),
        };
        prosemirror_text(&b, &mut content);
    }
    let mut content = content.trim().to_string();
    if content.is_empty() {
        content = s(v, "body_text_summary");
    }
    let nick = v.pointer("/author/username").and_then(Value::as_str)
        .map(str::to_string).unwrap_or_else(|| fallback_nick.to_string());
    Post {
        post_id: s(v, "id"),
        parent_post_id: parent.to_string(),
        nick,
        title: s(v, "title"),
        content,
        posted_at: s(v, "inserted_at"),
        media: { let mut m = Vec::new(); collect_media(v, &mut m); m },
    }
}

/// Scrape a nick: their feed (root posts + their replies elsewhere) and all
/// replies in their own threads. Also returns the profile page HTML.
#[derive(Default)]
pub struct ScrapeStats {
    /// Items on the Posts feed plus the "Posts and replies" filter.
    pub feed_items: usize,
    /// Items not stored before (approximate).
    pub new_posts: usize,
    /// Items skipped as already stored.
    pub skipped_known: usize,
    /// Threads fetched for replies.
    pub threads_fetched: usize,
    /// Threads skipped as unchanged.
    pub threads_skipped: usize,
    /// Items only found via the profile's "Posts and replies" filter.
    pub replies_tab_items: usize,
}

/// Scrape a nick: the Posts feed and the "Posts and replies" filter (merged by id), then replies in every thread the nick started that changed since the last run. Returns the posts, the profile HTML and counts.
pub async fn scrape(
    http: &reqwest::Client, jar: &Jar, nick: &str,
    known: &HashSet<String>, reply_counts: &HashMap<String, usize>, full: bool,
) -> Result<(Vec<Post>, String, ScrapeStats)> {
    let nick = nick.trim_start_matches('@');
    let known_opt = if full { None } else { Some(known) };
    // Profile feed is always Latest (/rs/@nick); Trending does not apply to profiles.
    let profile_url = profile_url(nick);
    let mut feed = fetch_feed(http, jar, &profile_url, None, known_opt, true).await?;
    // Also walk the "Posts and replies" filter (?replies=1) (the nick's replies in other threads).
    let mut replies_tab_added = 0;
    match fetch_replies_feed(http, jar, nick, &feed.html, known_opt).await {
        Ok(extra) => {
            replies_tab_added = merge_feed_items(&mut feed.items, extra);
            info!("@{nick}: posts and replies added {replies_tab_added} items");
        }
        Err(e) => err!("warning: posts and replies feed: {e:#}"),
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut roots: Vec<(String, usize)> = Vec::new(); // (id, replies_count from feed)
    let mut stats = ScrapeStats { feed_items: feed.items.len(), replies_tab_items: replies_tab_added, ..Default::default() };
    for it in &feed.items {
        let id = s(it, "id");
        if id.is_empty() || !seen.insert(id.clone()) { continue; }
        let thread = s(it, "thread_id");
        let is_root = thread.is_empty() || thread == id;
        let already = known.contains(&id);
        if already && !full {
            stats.skipped_known += 1;
        } else {
            stats.new_posts += 1;
        }
        if is_root {
            let rc = it.get("replies_count").and_then(Value::as_u64).unwrap_or(0) as usize;
            roots.push((id.clone(), rc));
            if !already || full {
                out.push(to_post(it, "", nick));
            }
        } else if !already || full {
            out.push(to_post(it, &thread, nick));
        }
    }
    for (tid, feed_rc) in roots {
        let db_rc = reply_counts.get(&tid).copied().unwrap_or(0);
        let need = full || !known.contains(&tid) || feed_rc > db_rc;
        if !need {
            stats.threads_skipped += 1;
            continue;
        }
        stats.threads_fetched += 1;
        let (_, replies) = match fetch_thread(http, jar, &tid).await {
            Ok(x) => x,
            Err(e) => { err!("warning: {e:#}"); continue; }
        };
        let mut n = 0;
        for rp in &replies {
            let id = s(rp, "id");
            if id.is_empty() || id == tid || !seen.insert(id.clone()) { continue; }
            if known.contains(&id) && !full {
                stats.skipped_known += 1;
                continue;
            }
            out.push(to_post(rp, &tid, nick));
            stats.new_posts += 1;
            n += 1;
        }
        verbose!("thread {tid}: {n} new replies (feed says {feed_rc}, db had {db_rc})");
    }
    Ok((out, feed.html, stats))
}
