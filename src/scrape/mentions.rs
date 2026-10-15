//! Mention matching, profile parsing, and the site-wide mentions crawl.

use crate::prelude::*;

#[allow(clippy::too_many_arguments)]
/// Crawl the site feed (Latest or Trending) up to `limit` posts, confirm candidate mentions against full threads, and return the new posts and mention hits.
pub async fn crawl_mentions(
    http: &reqwest::Client, jar: &Jar, targets: &[MentionTarget],
    limit: usize, seen: &mut HashSet<String>, known: &HashSet<String>, full: bool,
    mode: FeedMode,
) -> Result<(Vec<Post>, Vec<MentionHit>)> {
    let allow_inc = mode.allow_incremental_stop() && !full;
    let known_opt = if allow_inc { Some(known) } else { None };
    let url = mode.site_feed_url();
    info!("mentions feed: {url} (mode={mode})");
    let feed = fetch_feed(http, jar, &url, Some(limit), known_opt, allow_inc).await?;
    let mut posts = Vec::new();
    let mut hits = Vec::new();
    let mut checked = HashSet::new();
    for it in &feed.items {
        let id = s(it, "id");
        let tid = { let t = s(it, "thread_id"); if t.is_empty() { id.clone() } else { t } };
        let mut any = false;
        for t in targets {
            if !match_post(it, t).is_empty() { any = true; break; }
        }
        if !any { continue; }
        if !checked.insert(tid.clone()) { continue; }
        let (th, replies) = match fetch_thread(http, jar, &tid).await {
            Ok(x) => x,
            Err(e) => { err!("warning: mention thread {tid}: {e:#}"); continue; }
        };
        let root = th.get("root").cloned().unwrap_or(th.clone());
        let mut thread_posts = vec![root];
        thread_posts.extend(replies);
        for tp in &thread_posts {
            let pid = s(tp, "id");
            if pid.is_empty() { continue; }
            for t in targets {
                for mt in match_post(tp, t) {
                    hits.push(MentionHit {
                        post_id: pid.clone(), mentioned_nick: t.nick.clone(), match_type: mt,
                    });
                }
            }
            if !seen.insert(pid.clone()) { continue; }
            let parent = {
                let thid = s(tp, "thread_id");
                if thid.is_empty() || thid == pid { String::new() } else { thid }
            };
            let author = tp.pointer("/author/username").and_then(Value::as_str).unwrap_or("");
            posts.push(to_post(tp, &parent, author));
        }
    }
    Ok((posts, hits))
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

/// Walk a ProseMirror doc for `mention` nodes whose attrs.id (user_id) or
/// attrs.label (@username / username) match the target.
pub fn mention_nodes(body: &Value, target: &MentionTarget, out: &mut HashSet<MatchType>) {
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
pub fn flatten_text(v: &Value, out: &mut String) {
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

/// A post's ProseMirror body as JSON (the site sends it as an object or a JSON string).
pub fn body_of(v: &Value) -> Value {
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
            // Only a profile-specific title ("Name (@nick)"); the site-wide
            // og:title (e.g. "Trending") is not a display name.
            if let Some((dn, _)) = t.split_once("(@") {
                p.display_name = Some(dn.trim().to_string());
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
            let u = html_unescape(&t[1]);
            if !is_site_image(&u) { p.avatar_url = Some(u); }
        }
}

/// True for the site's generic share images (e.g. `/banner.jpg`), which
/// `og:image` falls back to on pages without a profile photo.
pub fn is_site_image(u: &str) -> bool {
    let path = u.split(['?', '#']).next().unwrap_or(u);
    let file = path.rsplit('/').next().unwrap_or("");
    let origin_root = path.trim_start_matches("https://").trim_start_matches("http://").matches('/').count() <= 1;
    origin_root && (file.starts_with("banner.") || file.starts_with("og") || file.starts_with("logo") || file.starts_with("icon"))
}

/// Undo the HTML entity escaping used in `data-props` attributes.
pub fn html_unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&#39;", "'")
}

#[cfg(test)]
mod tests;
