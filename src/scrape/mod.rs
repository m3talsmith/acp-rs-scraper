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

/// Strip HTML tags (block tags become newlines) and decode common entities.
pub fn html_to_text(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('>') else { out.push_str(&rest[i..]); rest = ""; break };
        let tag = rest[i + 1..i + j].trim_start_matches('/').to_ascii_lowercase();
        let name = tag.split(|c: char| !c.is_ascii_alphanumeric()).next().unwrap_or("");
        if matches!(name, "br" | "p" | "div" | "li" | "blockquote" | "h1" | "h2" | "h3" | "h4" | "pre") && !out.ends_with('\n') {
            out.push('\n');
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    out.replace("&nbsp;", " ").replace("&quot;", "\"").replace("&#39;", "'")
        .replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// Plain text of a post body in any of the shapes the site sends: a
/// ProseMirror doc (object or JSON string), a plain-text / Markdown / HTML
/// string, or an object with `html` / `text` / `markdown`. Falls back to the
/// `body_text_summary`, `body_text`, `body_html`, `text` and string `content` fields.
pub fn body_text(v: &Value) -> String {
    fn from_body(b: &Value) -> String {
        match b {
            Value::String(raw) => {
                let raw_t = raw.trim();
                if raw_t.starts_with('{') && let Ok(j) = serde_json::from_str::<Value>(raw_t) {
                    return from_body(&j);
                }
                if raw_t.contains('<') && raw_t.contains('>') { html_to_text(raw_t) } else { raw_t.to_string() }
            }
            Value::Object(m) => {
                let mut out = String::new();
                prosemirror_text(b, &mut out);
                if out.trim().is_empty() {
                    for k in ["text", "markdown", "plain", "html", "doc", "content"] {
                        if let Some(x) = m.get(k) {
                            let t = if x.is_array() { let mut o = String::new(); prosemirror_text(b, &mut o); o } else { from_body(x) };
                            if !t.trim().is_empty() { return t; }
                        }
                    }
                }
                out
            }
            Value::Array(a) => { let mut out = String::new(); for x in a { prosemirror_text(x, &mut out); } out }
            _ => String::new(),
        }
    }
    for k in ["body", "body_html", "body_text", "body_text_summary", "text", "content", "html"] {
        if let Some(b) = v.get(k) {
            let t = from_body(b).trim().to_string();
            if !t.is_empty() { return t; }
        }
    }
    String::new()
}

/// Convert site post JSON to a [`Post`] with the given parent (`""` for roots), flattening the body to text.
pub fn to_post(v: &Value, parent: &str, fallback_nick: &str) -> Post {
    // Some feeds wrap the post (e.g. `{"post": {...}}` / `{"reply": {...}}`).
    let inner = ["post", "reply"].iter().find_map(|k| v.get(*k).filter(|x| x.is_object()));
    let mut content = body_text(v);
    if content.is_empty() && let Some(i) = inner { content = body_text(i); }
    let nick = v.pointer("/author/username").and_then(Value::as_str)
        .map(str::to_string).unwrap_or_else(|| fallback_nick.to_string());
    Post {
        post_id: s(v, "id"),
        parent_post_id: parent.to_string(),
        nick,
        title: { let t = s(v, "title"); if t.is_empty() { inner.map(|i| s(i, "title")).unwrap_or_default() } else { t } },
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
    /// Feed items dropped by `--said-by` because another member wrote them.
    pub skipped_other: usize,
    /// Posts whose body had to be read from their thread page.
    pub bodies_from_thread: usize,
}

/// Scrape a nick: the Posts feed and the "Posts and replies" filter (merged by id), then replies in every thread the nick started that changed since the last run. Returns the posts, the profile HTML and counts.
pub async fn scrape(
    http: &reqwest::Client, jar: &Jar, nick: &str,
    known: &HashSet<String>, reply_counts: &HashMap<String, usize>, full: bool, said_by: bool,
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
        if said_by && !said_by_nick(it, nick) { stats.skipped_other += 1; continue; }
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
    // Fill posts whose feed item had no readable body from their thread page,
    // where the full reply is always rendered (one fetch per thread).
    let mut thread_cache: HashMap<String, Vec<Value>> = HashMap::new();
    for p in out.iter_mut().filter(|p| p.content.is_empty() && p.media.is_empty()) {
        let tid = if p.parent_post_id.is_empty() { p.post_id.clone() } else { p.parent_post_id.clone() };
        if !thread_cache.contains_key(&tid) {
            let got = match fetch_thread(http, jar, &tid).await {
                Ok((th, mut r)) => { if let Some(root) = th.get("root") { r.push(root.clone()); } r }
                Err(e) => { verbose!("thread {tid}: {e:#}"); Vec::new() }
            };
            thread_cache.insert(tid.clone(), got);
        }
        if let Some(full) = thread_cache[&tid].iter().find(|x| s(x, "id") == p.post_id) {
            let fp = to_post(full, &p.parent_post_id, nick);
            p.content = fp.content;
            if p.title.is_empty() { p.title = fp.title; }
            if p.media.is_empty() { p.media = fp.media; }
        }
        stats.bodies_from_thread += 1;
    }
    if said_by {
        stats.threads_skipped += roots.len();
        roots.clear();
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

/// Whether a feed item was written by `nick` (`--said-by`). Items with no
/// author data are kept: profile feeds only list the nick's own items.
pub fn said_by_nick(v: &Value, nick: &str) -> bool {
    let m = |u: &str| u.trim_start_matches('@').eq_ignore_ascii_case(nick);
    let inner = ["reply", "post"].iter().find_map(|k| v.get(*k).filter(|x| x.is_object())).unwrap_or(v);
    for item in [inner, v] {
        if let Some(u) = item.pointer("/author/username").and_then(Value::as_str) { return m(u); }
        for k in ["author_username", "username", "nick"] {
            if let Some(u) = item.get(k).and_then(Value::as_str) && !u.is_empty() { return m(u); }
        }
    }
    true
}

#[cfg(test)]
mod tests;
