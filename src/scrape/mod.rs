//! Scraping: turning site JSON into [`Post`]s and walking a nick's profile
//! (the Posts feed and its "Posts and replies" filter) plus the threads they started.

pub mod edits;
pub mod feed;
pub mod mentions;
pub mod profile;
pub mod reads;
pub mod refetch;
pub mod sitewide;
pub mod spaces;
pub mod thread;

use crate::prelude::*;

#[derive(Debug, Clone)]
/// One stored post (the `posts` table row), plus the media it references.
pub struct Post {
    /// Site post id (22-character base64url).
    pub post_id: String,
    /// Thread root id for replies; `""` for root posts.
    pub parent_post_id: String,
    /// Immediate parent: the comment this reply answers (the first `postRef`
    /// node in its body), else the thread root; `""` for root posts.
    pub in_reply_to_id: String,
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
    /// The body is the feed's shortened preview (`body_truncated: true`),
    /// not the full text; stored as `body_state = 'truncated'`.
    pub truncated: bool,
    /// Canonical JSON of the ProseMirror `body` object, or `""` when the site
    /// sent plain text. A later save does not blank a JSON body already stored.
    pub body_json: String,
    /// Site score, when the post JSON carried one. A later save does not blank a score already stored.
    pub score: Option<i64>,
}

/// Whether a site post JSON (or its `post`/`reply` wrapper) only carries a
/// shortened body: the feed sets `body_truncated: true` and the full text is
/// on the thread page (`/rs/post/{thread}`), which is what "show more" opens.
pub fn is_truncated(v: &Value) -> bool {
    let t = |x: &Value| x.get("body_truncated").and_then(Value::as_bool).unwrap_or(false);
    t(v) || ["post", "reply"].iter().filter_map(|k| v.get(*k)).any(t)
}

/// Normalise flattened body text: trailing spaces off every line, runs of
/// blank lines collapsed to one blank line (the paragraph break), ends trimmed.
pub fn tidy_text(s: &str) -> String {
    let mut out = String::new();
    let mut blanks = 0;
    for line in s.lines().map(str::trim_end) {
        if line.is_empty() { blanks += 1; continue; }
        if !out.is_empty() { out.push_str(if blanks > 0 { "\n\n" } else { "\n" }); }
        blanks = 0;
        out.push_str(line);
    }
    out
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
    // Block nodes end with a blank line so paragraphs stay separate
    // ([`tidy_text`] collapses the extra ones from nested blocks).
    if matches!(ty, "paragraph" | "heading" | "blockquote" | "list_item" | "listItem" | "code_block" | "codeBlock") {
        out.push_str("\n\n");
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
        if name == "br" {
            out.push('\n');
        } else if matches!(name, "p" | "div" | "li" | "blockquote" | "h1" | "h2" | "h3" | "h4" | "pre") && !out.ends_with("\n\n") {
            out.push_str("\n\n");
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
            let t = tidy_text(&from_body(b));
            if !t.is_empty() { return t; }
        }
    }
    String::new()
}

/// The post a reply answers directly: the first `postRef` node
/// (`attrs.postId`) in its ProseMirror body, wrapped or not, skipping `own`.
/// The site keeps threads flat (`thread_id` only); nesting under a comment
/// is this reference. `None` when the body has no reference.
pub fn reply_ref(v: &Value, own: &str) -> Option<String> {
    fn walk(v: &Value, own: &str) -> Option<String> {
        match v {
            Value::Object(m) => {
                if m.get("type").and_then(Value::as_str) == Some("postRef")
                    && let Some(id) = m.get("attrs").and_then(|a| a.get("postId")).and_then(Value::as_str)
                    && !id.is_empty() && id != own { return Some(id.to_string()); }
                m.get("content").and_then(|c| walk(c, own))
            }
            Value::Array(a) => a.iter().find_map(|x| walk(x, own)),
            Value::String(st) if st.contains("postRef") => serde_json::from_str::<Value>(st.trim()).ok().and_then(|j| walk(&j, own)),
            _ => None,
        }
    }
    let inner = ["post", "reply"].iter().filter_map(|k| v.get(*k));
    std::iter::once(v).chain(inner).find_map(|x| x.get("body").and_then(|b| walk(b, own)))
}

/// Convert site post JSON to a [`Post`] with the given parent (`""` for roots), flattening the body to text.
pub fn to_post(v: &Value, parent: &str, fallback_nick: &str) -> Post {
    // Some feeds wrap the post (e.g. `{"post": {...}}` / `{"reply": {...}}`).
    let inner = ["post", "reply"].iter().find_map(|k| v.get(*k).filter(|x| x.is_object()));
    let mut content = body_text(v);
    if content.is_empty() && let Some(i) = inner { content = body_text(i); }
    let nick = v.pointer("/author/username").and_then(Value::as_str)
        .map(str::to_string).unwrap_or_else(|| fallback_nick.to_string());
    let post_id = { let i = s(v, "id"); if i.is_empty() { inner.map(|x| s(x, "id")).unwrap_or_default() } else { i } };
    let in_reply_to_id = if parent.is_empty() { String::new() } else {
        reply_ref(v, &post_id).unwrap_or_else(|| parent.to_string())
    };
    Post {
        post_id,
        parent_post_id: parent.to_string(),
        in_reply_to_id,
        nick,
        title: { let t = s(v, "title"); if t.is_empty() { inner.map(|i| s(i, "title")).unwrap_or_default() } else { t } },
        content,
        posted_at: s(v, "inserted_at"),
        media: { let mut m = Vec::new(); collect_media(v, &mut m); m },
        truncated: is_truncated(v),
        body_json: { let j = body_json_of(v); if j.is_empty() { inner.map(body_json_of).unwrap_or_default() } else { j } },
        score: score_of(v).or_else(|| inner.and_then(score_of)),
    }
}

/// `score` from a post JSON object, as an integer when the site sent one.
pub fn score_of(v: &Value) -> Option<i64> {
    let n = v.get("score")?;
    n.as_i64().or_else(|| n.as_u64().and_then(|u| i64::try_from(u).ok())).or_else(|| n.as_str().and_then(|s| s.parse().ok()))
}

/// Compact JSON of a post's ProseMirror `body`, when that body is an object
/// (or a string holding one). Plain text and HTML bodies return `""`.
pub fn body_json_of(v: &Value) -> String {
    fn canon(b: &Value) -> String {
        match b {
            Value::Object(_) => serde_json::to_string(b).unwrap_or_default(),
            Value::String(raw) => {
                let t = raw.trim();
                if t.starts_with('{') && let Ok(j) = serde_json::from_str::<Value>(t) && j.is_object() {
                    return serde_json::to_string(&j).unwrap_or_default();
                }
                String::new()
            }
            _ => String::new(),
        }
    }
    v.get("body").map(canon).filter(|s| !s.is_empty()).unwrap_or_default()
}

/// Tags on a post: TipTap `mention` nodes whose suggestion character or label
/// is `#…`, then `#word` in the plain text. `#post-…` anchors and `##` headings
/// are not tags. Order kept, duplicates (ignoring case) dropped.
pub fn extract_tags(body_json: &str, content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let push = |raw: &str, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        let tag = raw.trim().trim_start_matches('#').trim();
        if !TAG_NAME.is_match(tag) || tag.to_ascii_lowercase().starts_with("post-") { return; }
        if seen.insert(tag.to_ascii_lowercase()) { out.push(tag.to_string()); }
    };
    if let Ok(v) = serde_json::from_str::<Value>(body_json) { walk_tag_mentions(&v, &mut |t| push(t, &mut out, &mut seen)); }
    for c in TAG_WORD.captures_iter(content) {
        if let Some(t) = c.get(1) { push(t.as_str(), &mut out, &mut seen); }
    }
    out
}

fn walk_tag_mentions(v: &Value, push: &mut dyn FnMut(&str)) {
    match v {
        Value::Object(m) => {
            if m.get("type").and_then(Value::as_str) == Some("mention") {
                let attrs = m.get("attrs");
                let ch = attrs.and_then(|a| a.get("mentionSuggestionChar")).and_then(Value::as_str).unwrap_or("");
                let label = attrs.and_then(|a| a.get("label")).and_then(Value::as_str)
                    .or_else(|| m.get("label").and_then(Value::as_str)).unwrap_or("");
                if ch == "#" || label.starts_with('#') { push(label); }
            }
            if let Some(c) = m.get("content") { walk_tag_mentions(c, push); }
        }
        Value::Array(a) => { for x in a { walk_tag_mentions(x, push); } }
        _ => {}
    }
}

static TAG_NAME: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^[A-Za-z][A-Za-z0-9_-]{0,63}$").expect("tag name")
});
static TAG_WORD: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)(?:^|[^A-Za-z0-9_#])#([A-Za-z][A-Za-z0-9_-]{0,63})").expect("tag word")
});

/// Turn `#tag` outside code into `[#tag](acp-tag:tag)` so a click opens that tag's feed.
/// Fenced and inline code, `##` headings and `#post-…` anchors are left as written.
pub fn link_hashtags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut fenced = false;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let nl = &line[body.len()..];
        let trimmed = body.trim_start();
        let is_fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if fenced || is_fence {
            out.push_str(body);
            out.push_str(nl);
            if is_fence { fenced = !fenced; }
            continue;
        }
        out.push_str(&link_hashtags_line(body));
        out.push_str(nl);
    }
    out
}

fn link_hashtags_line(line: &str) -> String {
    let spans = inline_code_spans(line);
    let mut out = String::new();
    let mut last = 0;
    for c in TAG_WORD.captures_iter(line) {
        let Some(name) = c.get(1) else { continue };
        let hash_at = name.start() - 1;
        if spans.iter().any(|&(a, b)| hash_at >= a && hash_at < b) { continue; }
        if name.as_str().to_ascii_lowercase().starts_with("post-") { continue; }
        out.push_str(&line[last..hash_at]);
        let tag = name.as_str();
        out.push_str("[#");
        out.push_str(tag);
        out.push_str("](acp-tag:");
        out.push_str(tag);
        out.push(')');
        last = name.end();
    }
    out.push_str(&line[last..]);
    out
}

/// Byte spans of inline code on one line (an unclosed run runs to the end).
fn inline_code_spans(line: &str) -> Vec<(usize, usize)> {
    let b = line.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'`' { i += 1; continue; }
        let mut n = 0;
        while i + n < b.len() && b[i + n] == b'`' { n += 1; }
        let mut j = i + n;
        let mut closed = false;
        while j < b.len() {
            if b[j] != b'`' { j += 1; continue; }
            let mut m = 0;
            while j + m < b.len() && b[j + m] == b'`' { m += 1; }
            if m == n { spans.push((i, j + m)); i = j + m; closed = true; break; }
            j += m;
        }
        if !closed { spans.push((i, b.len())); break; }
    }
    spans
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
    /// Feed items that only carried a shortened body (`body_truncated`).
    pub truncated_in_feed: usize,
    /// Of those, how many got their full text from the thread page.
    pub truncated_completed: usize,
}

/// What one nick scrape should keep. The feed window and the thread crawl read these together.
#[derive(Clone, Copy)]
pub struct ScrapeOptions<'a> {
    /// Post ids already stored. Ignored for skipping when `full` is set.
    pub known: &'a HashSet<String>,
    /// Stored reply counts by thread id, used to skip unchanged threads.
    pub reply_counts: &'a HashMap<String, usize>,
    /// Refetch everything instead of stopping at known posts.
    pub full: bool,
    /// Keep only posts this nick wrote, and skip the thread crawl.
    pub said_by: bool,
    /// How many feed items to keep. `None` keeps the feed the crawl reached.
    pub limit: Option<usize>,
    /// Which profile tab to read.
    pub profile_feed: ProfileFeed,
    /// Which end of the feed `limit` keeps.
    pub order: ScrapeOrder,
}

/// Scrape a nick: the Posts feed and the "Posts and replies" filter (merged by id), then replies in every thread the nick started that changed since the last run. Returns the posts, the profile HTML and counts.
pub async fn scrape(
    http: &reqwest::Client,
    jar: &Jar,
    nick: &str,
    opts: &ScrapeOptions<'_>,
) -> Result<(Vec<Post>, String, ScrapeStats)> {
    let &ScrapeOptions { known, reply_counts, full, said_by, limit, profile_feed, order } = opts;
    let nick = nick.trim_start_matches('@');
    let known_opt = if full { None } else { Some(known) };
    // `--order oldest` with `--limit` has to reach the end of a newest-first list.
    let tail = order == ScrapeOrder::Oldest && limit.is_some();
    let page_limit = if tail { None } else { limit };
    let allow_incremental = profile_feed == ProfileFeed::Newest && !tail;
    let profile_url = match profile_feed {
        ProfileFeed::Newest => profile_url(nick),
        ProfileFeed::Top => profile_top_url(nick),
    };
    let mut feed = fetch_feed(http, jar, &profile_url, page_limit, known_opt, allow_incremental).await?;
    // Posts and replies is the Latest profile's filter. The Top feed has no captured replies URL.
    let posts_items = std::mem::take(&mut feed.items);
    let replies = if profile_feed == ProfileFeed::Newest {
        match fetch_replies_feed(http, jar, nick, &feed.html, known_opt, page_limit, allow_incremental).await {
            Ok(extra) => Some(extra),
            Err(e) => { err!("warning: posts and replies feed: {e:#}"); None }
        }
    } else {
        None
    };
    let (items, replies_tab_added) = window_feed_items(posts_items, replies, limit, order);
    feed.items = items;
    if profile_feed != ProfileFeed::Newest || order != ScrapeOrder::Newest {
        info!("@{nick}: --profile-feed {profile_feed} --order {order}");
    }
    if let Some(n) = limit {
        let end = match order { ScrapeOrder::Newest => "start", ScrapeOrder::Oldest => "end" };
        info!("@{nick}: --limit {n}: keeping the {end} of the feed ({len} items)", len = feed.items.len());
    }
    info!("@{nick}: posts and replies added {replies_tab_added} items");
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
    // Fill posts whose feed item had no readable body, or only a shortened
    // one (`body_truncated`), from their thread page, where the root and every
    // reply are rendered in full (one fetch per thread, shared by its posts).
    let mut thread_cache: HashMap<String, Vec<Value>> = HashMap::new();
    stats.truncated_in_feed = out.iter().filter(|p| p.truncated).count();
    for p in out.iter_mut().filter(|p| p.truncated || (p.content.is_empty() && p.media.is_empty())) {
        let tid = if p.parent_post_id.is_empty() { p.post_id.clone() } else { p.parent_post_id.clone() };
        if !thread_cache.contains_key(&tid) {
            let got = match fetch_thread(http, jar, &tid).await {
                Ok((th, mut r)) => { if let Some(root) = th.get("root") { r.push(root.clone()); } r }
                Err(e) => { verbose!("thread {tid}: {e:#}"); Vec::new() }
            };
            thread_cache.insert(tid.clone(), got);
            polite_pause().await;
        }
        if let Some(full) = thread_cache[&tid].iter().find(|x| s(x, "id") == p.post_id) {
            let fp = to_post(full, &p.parent_post_id, nick);
            if fp.content.is_empty() && p.truncated { continue; }
            if p.truncated && !fp.truncated { stats.truncated_completed += 1; }
            p.truncated = fp.truncated;
            p.content = fp.content;
            if p.in_reply_to_id == p.parent_post_id { p.in_reply_to_id = fp.in_reply_to_id; }
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
        let cached = thread_cache.remove(&tid);
        let fetched = match cached { Some(r) => Ok((Value::Null, r)), None => fetch_thread(http, jar, &tid).await };
        let (_, replies) = match fetched {
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


/// Pause between thread-page fetches made only to complete bodies
/// ([`THREAD_FETCH_PAUSE_MS`]), so a large backfill stays gentle on the site.
pub async fn polite_pause() {
    tokio::time::sleep(std::time::Duration::from_millis(THREAD_FETCH_PAUSE_MS)).await;
}

#[cfg(test)]
mod tests;
