//! Mention matching and profile parsing.

use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MatchType {
    Mention,
    Nick,
    DisplayName,
}

impl MatchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchType::Mention => "mention",
            MatchType::Nick => "nick",
            MatchType::DisplayName => "display_name",
        }
    }
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
pub struct MentionHit {
    pub post_id: String,
    pub mentioned_nick: String,
    pub match_type: MatchType,
}

/// Who we're looking for. `user_id` makes ProseMirror mention nodes definitive.
#[derive(Debug, Clone)]
pub struct MentionTarget {
    pub nick: String,
    pub user_id: Option<String>,
    pub display_name: Option<String>,
}

/// Walk a ProseMirror doc for `mention` nodes whose attrs.id (user_id) or
/// attrs.label (@username / username) match the target.
fn mention_nodes(body: &Value, target: &MentionTarget, out: &mut HashSet<MatchType>) {
    let ty = body.get("type").and_then(Value::as_str).unwrap_or("");
    if ty == "mention" {
        let id = body.pointer("/attrs/id").and_then(Value::as_str).unwrap_or("");
        let label = body.pointer("/attrs/label").and_then(Value::as_str).unwrap_or("");
        let label = label.trim_start_matches('@');
        let by_id = target.user_id.as_deref().is_some_and(|u| !u.is_empty() && u == id);
        let by_nick = !label.is_empty() && label.eq_ignore_ascii_case(&target.nick);
        if by_id || by_nick {
            out.insert(MatchType::Mention);
        }
    }
    for key in ["content", "marks"] {
        if let Some(kids) = body.get(key).and_then(Value::as_array) {
            for k in kids { mention_nodes(k, target, out); }
        }
    }
}

/// Case-insensitive whole-word match of `needle` in `hay`. Word chars = ASCII
/// alphanumeric + underscore (so "haz" does not match "hazard").
pub fn word_match(hay: &str, needle: &str) -> bool {
    if needle.is_empty() { return false; }
    let h: Vec<char> = hay.chars().collect();
    let n: Vec<char> = needle.chars().collect();
    let nl = n.len();
    if nl > h.len() { return false; }
    let eq = |a: char, b: char| a.eq_ignore_ascii_case(&b);
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    for i in 0..=(h.len() - nl) {
        if !(0..nl).all(|j| eq(h[i + j], n[j])) { continue; }
        let before_ok = i == 0 || !word(h[i - 1]);
        let after_ok = i + nl == h.len() || !word(h[i + nl]);
        if before_ok && after_ok { return true; }
    }
    false
}

/// Flatten ProseMirror text (same idea as main::prosemirror_text) for local matching.
fn flatten_text(v: &Value, out: &mut String) {
    if let Some(t) = v.get("text").and_then(Value::as_str) { out.push_str(t); }
    let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
    if matches!(ty, "hard_break" | "hardBreak") { out.push('\n'); }
    if let Some(kids) = v.get("content").and_then(Value::as_array) {
        for k in kids { flatten_text(k, out); }
    }
    if matches!(ty, "paragraph" | "heading" | "blockquote" | "list_item" | "listItem" | "code_block" | "codeBlock") {
        out.push('\n');
    }
}

fn body_of(v: &Value) -> Value {
    match v.get("body") {
        Some(Value::String(raw)) => serde_json::from_str(raw).unwrap_or(Value::Null),
        Some(b) => b.clone(),
        None => Value::Null,
    }
}

/// All match types that apply to this post JSON for `target`.
pub fn match_post(v: &Value, target: &MentionTarget) -> HashSet<MatchType> {
    let mut out = HashSet::new();
    let body = body_of(v);
    mention_nodes(&body, target, &mut out);

    let mut text = String::new();
    flatten_text(&body, &mut text);
    if text.trim().is_empty() {
        if let Some(s) = v.get("body_text_summary").and_then(Value::as_str) { text = s.to_string(); }
        if let Some(s) = v.get("title").and_then(Value::as_str) {
            text.push(' ');
            text.push_str(s);
        }
        if let Some(s) = v.get("content").and_then(Value::as_str) {
            text.push(' ');
            text.push_str(s);
        }
    } else if let Some(s) = v.get("title").and_then(Value::as_str) {
        text.push(' ');
        text.push_str(s);
    }

    if word_match(&text, &target.nick) {
        out.insert(MatchType::Nick);
    }
    if let Some(dn) = &target.display_name {
        let dn = dn.trim();
        // Avoid matching very short / empty display names (common-name hazard).
        if dn.chars().count() >= 3 && word_match(&text, dn) {
            out.insert(MatchType::DisplayName);
        }
    }
    out
}

/// Profile fields scraped from a profile page.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    pub nick: String,
    pub user_id: Option<String>,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_path: Option<String>,
    pub banner_url: Option<String>,
    pub banner_path: Option<String>,
    pub location: Option<String>,
    pub role: Option<String>,
    pub joined_at: Option<String>,
    pub followers: Option<i64>,
    pub following: Option<i64>,
    pub posts_count: Option<i64>,
    pub raw_json: String,
    pub scraped_at: String,
}

/// Pull whatever profile-like fields we can find in a JSON blob (feed author,
/// profile props, show-info reply, etc.). Prefers the first non-empty value.
pub fn enrich_profile(p: &mut Profile, v: &Value) {
    let g = |keys: &[&str]| -> Option<String> {
        for k in keys {
            if let Some(s) = v.pointer(k).and_then(Value::as_str).filter(|s| !s.is_empty()) {
                return Some(s.to_string());
            }
        }
        None
    };
    let n = |keys: &[&str]| -> Option<i64> {
        for k in keys {
            if let Some(x) = v.pointer(k).and_then(Value::as_i64) { return Some(x); }
            if let Some(x) = v.pointer(k).and_then(Value::as_f64) { return Some(x as i64); }
        }
        None
    };
    if p.user_id.is_none() {
        p.user_id = g(&["/user_id", "/id", "/author/user_id", "/user/id", "/profile/user_id"]);
    }
    if p.display_name.is_none() {
        p.display_name = g(&["/display_name", "/author/display_name", "/user/display_name", "/profile/display_name", "/name"]);
    }
    let un = g(&["/username", "/author/username", "/user/username", "/profile/username", "/nick"]);
    if let Some(u) = un.filter(|u| !u.is_empty()) { p.nick = u.trim_start_matches('@').to_string(); }
    if p.bio.is_none() { p.bio = g(&["/bio", "/about", "/profile/bio", "/description"]); }
    if p.avatar_url.is_none() {
        p.avatar_url = g(&["/profile_photo_url", "/avatar_url", "/avatar", "/author/profile_photo_url", "/user/profile_photo_url", "/image"]);
    }
    if p.banner_url.is_none() {
        p.banner_url = g(&["/banner_url", "/banner", "/cover_url", "/cover"]);
    }
    if p.location.is_none() {
        p.location = g(&["/location", "/author_location", "/chapter", "/author/location"]);
    }
    if p.role.is_none() {
        p.role = g(&["/role", "/author_role", "/author/role"]);
    }
    if p.joined_at.is_none() {
        p.joined_at = g(&["/joined_at", "/inserted_at", "/created_at", "/member_since"]);
    }
    if p.followers.is_none() { p.followers = n(&["/followers_count", "/followers", "/stats/followers"]); }
    if p.following.is_none() { p.following = n(&["/following_count", "/following", "/stats/following"]); }
    if p.posts_count.is_none() { p.posts_count = n(&["/posts_count", "/post_count", "/stats/posts"]); }
}

/// Harvest any author-like objects from a feed items list into `p`.
pub fn enrich_from_feed(p: &mut Profile, items: &[Value], nick: &str) {
    for it in items {
        let author = it.get("author").unwrap_or(it);
        let un = author.get("username").and_then(Value::as_str).unwrap_or("");
        if un.eq_ignore_ascii_case(nick) || un.is_empty() {
            enrich_profile(p, author);
            enrich_profile(p, it);
            if p.user_id.is_some() && p.display_name.is_some() { break; }
        }
    }
}

/// Walk an HTML document looking for Svelte `data-props` JSON that looks like a profile.
pub fn enrich_from_html(p: &mut Profile, html: &str) {
    // Any data-props blob that mentions username / display_name.
    for cap in regex::Regex::new(r#"data-props="([^"]+)""#).unwrap().captures_iter(html) {
        let raw = html_unescape(&cap[1]);
        if let Ok(v) = serde_json::from_str::<Value>(&raw)
            && (v.get("username").is_some() || v.get("display_name").is_some()
                || v.pointer("/user/username").is_some() || v.pointer("/profile/username").is_some())
            {
                enrich_profile(p, &v);
            }
    }
    // og: tags as a last resort.
    if p.display_name.is_none()
        && let Some(t) = regex::Regex::new(r#"property="og:title"\s+content="([^"]+)""#)
            .unwrap().captures(html).or_else(|| regex::Regex::new(r#"content="([^"]+)"\s+property="og:title""#).unwrap().captures(html))
        {
            let t = html_unescape(&t[1]);
            // "Display Name (@nick)" or similar.
            if let Some((dn, _)) = t.split_once("(@") {
                p.display_name = Some(dn.trim().to_string());
            } else if !t.is_empty() {
                p.display_name = Some(t);
            }
        }
    if p.bio.is_none()
        && let Some(t) = regex::Regex::new(r#"property="og:description"\s+content="([^"]+)""#)
            .unwrap().captures(html).or_else(|| regex::Regex::new(r#"content="([^"]+)"\s+property="og:description""#).unwrap().captures(html))
        {
            p.bio = Some(html_unescape(&t[1]));
        }
    if p.avatar_url.is_none()
        && let Some(t) = regex::Regex::new(r#"property="og:image"\s+content="([^"]+)""#)
            .unwrap().captures(html).or_else(|| regex::Regex::new(r#"content="([^"]+)"\s+property="og:image""#).unwrap().captures(html))
        {
            p.avatar_url = Some(html_unescape(&t[1]));
        }
}

fn html_unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn word_boundaries() {
        assert!(word_match("hello haz world", "haz"));
        assert!(word_match("Haz,", "haz"));
        assert!(word_match("@haz said", "haz")); // 'h' after @ — @ is not a word char
        assert!(!word_match("hazard", "haz"));
        assert!(!word_match("ahaz", "haz"));
        assert!(word_match("Jane Doe wrote", "Jane Doe"));
        assert!(!word_match("Jane Does", "Jane Doe"));
        assert!(!word_match("x", ""));
    }

    #[test]
    fn mention_node_by_id_and_label() {
        let body = json!({"type":"doc","content":[
            {"type":"paragraph","content":[
                {"type":"mention","attrs":{"id":"u1","label":"@haz"}},
                {"type":"text","text":" hello"}
            ]}
        ]});
        let post = json!({"id":"p1","body": body});
        let t = MentionTarget { nick: "haz".into(), user_id: Some("u1".into()), display_name: Some("Hazel".into()) };
        let m = match_post(&post, &t);
        assert!(m.contains(&MatchType::Mention));
        // mention node has no text child; plain-nick match needs body text
        assert!(!m.contains(&MatchType::Nick));
        // display name not in text
        assert!(!m.contains(&MatchType::DisplayName));
    }

    #[test]
    fn plain_nick_and_display() {
        let post = json!({"id":"p1","body":{"type":"doc","content":[
            {"type":"paragraph","content":[{"type":"text","text":"Thanks haz and Hazel Smith for this"}]}
        ]},"title":""});
        let t = MentionTarget { nick: "haz".into(), user_id: None, display_name: Some("Hazel Smith".into()) };
        let m = match_post(&post, &t);
        assert!(!m.contains(&MatchType::Mention));
        assert!(m.contains(&MatchType::Nick));
        assert!(m.contains(&MatchType::DisplayName));
    }

    #[test]
    fn short_display_name_skipped() {
        let post = json!({"id":"p1","content":"Jo is here", "body": Value::Null, "title":""});
        let t = MentionTarget { nick: "someone".into(), user_id: None, display_name: Some("Jo".into()) };
        let m = match_post(&post, &t);
        assert!(!m.contains(&MatchType::DisplayName));
    }
}
