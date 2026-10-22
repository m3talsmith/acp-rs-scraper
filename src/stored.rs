//! Stored rows. The site client that used to fill these is not in this binary.

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::Value;

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

#[derive(Debug, Clone)]
/// Media URL referenced by a post, before download.
pub struct MediaRef {
    /// Page / media URL.
    pub url: String,
    /// Media kind (`image`, `video`, `thumbnail`, ...).
    pub kind: String,
}

#[derive(Debug, Clone)]
/// A downloaded attachment of a post.
pub struct Attachment {
    /// Site post id (22-character base64url).
    pub post_id: String,
    /// Page / media URL.
    pub url: String,
    /// Media kind (`image`, `video`, `thumbnail`, ...).
    pub kind: String,
    /// Absolute path under ~/.acp/ai/data/attachments.
    pub local_path: PathBuf,
    /// MIME type of the file.
    pub mime_type: String,
    /// This file is on the Red Star copy of the post. The local copy keeps it either way.
    pub on_site: bool,
}

/// File extension for a known media MIME type.
pub fn ext_from_mime(m: &str) -> Option<&'static str> {
    Some(match m.split(';').next()?.trim() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/svg+xml" => "svg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mp4" => "m4a",
        "application/pdf" => "pdf",
        _ => return None,
    })
}

/// MIME type for a file extension (fallback `application/octet-stream`).
pub fn mime_from_ext(e: &str) -> &'static str {
    match e {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// How a post mentions a nick.
pub enum MatchType {
    /// An @mention node pointing at the nick's user id or label.
    Mention,
    /// The bare nick as a whole word.
    Nick,
    /// The profile display name as a whole word (3+ characters).
    DisplayName,
}

impl MatchType {
    /// Database / export spelling of the match type.
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchType::Mention => "mention",
            MatchType::Nick => "nick",
            MatchType::DisplayName => "display_name",
        }
    }
    /// Parse the database / export spelling of a match type.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mention" => Some(Self::Mention),
            "nick" => Some(Self::Nick),
            "display_name" => Some(Self::DisplayName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
/// One confirmed mention: post, mentioned nick, and how.
pub struct MentionHit {
    /// Site post id (22-character base64url).
    pub post_id: String,
    /// Nick that was mentioned.
    pub mentioned_nick: String,
    /// How it was mentioned.
    pub match_type: MatchType,
}

/// Who we're looking for. `user_id` makes ProseMirror mention nodes definitive.
#[derive(Debug, Clone)]
pub struct MentionTarget {
    /// Nick (username, without `@`).
    pub nick: String,
    /// Site user id.
    pub user_id: Option<String>,
    /// Display name.
    pub display_name: Option<String>,
}

/// Profile fields scraped from a profile page.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    /// Nick (username, without `@`).
    pub nick: String,
    /// Site user id.
    pub user_id: Option<String>,
    /// Display name.
    pub display_name: Option<String>,
    /// Profile bio text.
    pub bio: Option<String>,
    /// Avatar image URL.
    pub avatar_url: Option<String>,
    /// Downloaded avatar path.
    pub avatar_path: Option<String>,
    /// Banner image URL.
    pub banner_url: Option<String>,
    /// Downloaded banner path.
    pub banner_path: Option<String>,
    /// Profile location.
    pub location: Option<String>,
    /// Member role.
    pub role: Option<String>,
    /// Join date.
    pub joined_at: Option<String>,
    /// Follower count.
    pub followers: Option<i64>,
    /// Following count.
    pub following: Option<i64>,
    /// Post count shown on the profile.
    pub posts_count: Option<i64>,
    /// The whole profile as JSON.
    pub raw_json: String,
    /// When it was scraped (RFC 3339).
    pub scraped_at: String,
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

/// A media URL without its query string or fragment.
pub fn canonical_url(u: &str) -> &str {
    u.split(['?', '#']).next().unwrap_or(u)
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
