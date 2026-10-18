//! Edit history (`/rs/post/{thread}/{id}/edits`).
//!
//! The page's `forum/Edits` props (`edits`, `post`) are stored in `post_versions`
//! with `source = site`. The TUI shows each version as a line diff against the
//! previous one. Local drafts stay out of this (P6); the table already allows
//! `peer`, `local` and `draft`.

use crate::prelude::*;

/// `THREAD/POST` from the command line, or a bare post id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditsSpec {
    /// Both ids were in the argument.
    Pair {
        /// Thread root.
        thread: String,
        /// The post.
        post: String,
    },
    /// A post id. The thread is looked up (`parent_post_id`, or the id itself for a root).
    Id(String),
}

/// Parse `--edits`. `THREAD/POST`, or a post id with no slash.
pub fn parse_edits_spec(spec: &str) -> Result<EditsSpec> {
    let spec = spec.trim();
    if spec.is_empty() { bail!("edits need a post id (THREAD/POST, or a post id stored locally)"); }
    if let Some((thread, post)) = spec.split_once('/') {
        let (thread, post) = (thread.trim(), post.trim());
        if thread.is_empty() || post.is_empty() || post.contains('/') {
            bail!("edits take THREAD/POST or a post id, not {spec:?}");
        }
        return Ok(EditsSpec::Pair { thread: thread.into(), post: post.into() });
    }
    if spec.contains(char::is_whitespace) { bail!("edits take THREAD/POST or a post id, not {spec:?}"); }
    Ok(EditsSpec::Id(spec.into()))
}

/// Resolve `--edits` to `(thread, post)`. A bare id is read from the database.
pub fn resolve_edits_spec(spec: &str) -> Result<(String, String)> {
    match parse_edits_spec(spec)? {
        EditsSpec::Pair { thread, post } => Ok((thread, post)),
        EditsSpec::Id(id) => {
            let path = home()?.join(DB_REL_PATH);
            if !path.exists() { bail!("post {id} is not in the database; pass THREAD/POST"); }
            let c = open_db(&path)?;
            match thread_of_post(&c, &id)? {
                Some(thread) => Ok((thread, id)),
                None => bail!("post {id} is not in the database; pass THREAD/POST"),
            }
        }
    }
}

/// `/rs/post/{thread}/{id}/edits`.
pub fn edits_url(thread: &str, post: &str) -> String {
    format!("{}/rs/post/{}/{}/edits", base_url(), thread.trim(), post.trim())
}

/// Database / pending-job key (`edits:{thread}/{post}`).
pub fn edits_key(thread: &str, post: &str) -> String {
    format!("edits:{}/{}", thread.trim(), post.trim())
}

/// Split [`edits_key`] back into `(thread, post)`.
pub fn parse_edits_key(key: &str) -> Option<(&str, &str)> {
    let rest = key.strip_prefix("edits:")?;
    let (thread, post) = rest.split_once('/')?;
    if thread.is_empty() || post.is_empty() || post.contains('/') { return None; }
    Some((thread, post))
}

/// FNV-1a 64 of `body_json` and `edited_at`, as 16 hex chars.
/// A `0xff` byte sits between the two so `("ab","c")` and `("a","bc")` differ.
pub fn version_hash(body_json: &str, edited_at: &str) -> String {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut h = OFFSET;
    for b in body_json.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(PRIME); }
    h ^= 0xff;
    h = h.wrapping_mul(PRIME);
    for b in edited_at.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(PRIME); }
    format!("{h:016x}")
}

/// Plain text of a stored version body (a ProseMirror document, or text).
pub fn version_plain(body_json: &str) -> String {
    let t = body_json.trim();
    let raw = if (t.starts_with('{') || t.starts_with('[')) && let Ok(v) = serde_json::from_str::<Value>(t) {
        let text = body_text(&json!({"body": v}));
        if text.is_empty() { t.to_string() } else { text }
    } else {
        t.to_string()
    };
    tidy_text(&raw)
}

/// Line diff of `before` → `after`. Context lines start with two spaces,
/// removed lines with `- `, added lines with `+ `. A trailing newline on the
/// inputs does not add an empty line.
pub fn line_diff(before: &str, after: &str) -> String {
    let a = diff_lines(before);
    let b = diff_lines(after);
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] { dp[i + 1][j + 1] + 1 } else { dp[i + 1][j].max(dp[i][j + 1]) };
        }
    }
    let mut out = String::new();
    let (mut i, mut j) = (0, 0);
    let push = |out: &mut String, mark: &str, line: &str| { out.push_str(mark); out.push_str(line); out.push('\n'); };
    while i < n && j < m {
        if a[i] == b[j] { push(&mut out, "  ", a[i]); i += 1; j += 1; }
        else if dp[i + 1][j] >= dp[i][j + 1] { push(&mut out, "- ", a[i]); i += 1; }
        else { push(&mut out, "+ ", b[j]); j += 1; }
    }
    while i < n { push(&mut out, "- ", a[i]); i += 1; }
    while j < m { push(&mut out, "+ ", b[j]); j += 1; }
    out
}

fn diff_lines(s: &str) -> Vec<&str> {
    if s.is_empty() { return Vec::new(); }
    let s = s.strip_suffix('\n').unwrap_or(s);
    s.split('\n').collect()
}

/// Versions from a `forum/Edits` `data-props` object, in page order.
///
/// Accepts an `edits` / `versions` / `history` array (or the same under `post`)
/// of strings, ProseMirror docs, or objects with `body` / `body_json` / `content`.
/// Timestamps are `edited_at`, then `inserted_at`, `updated_at`, `created_at`.
/// A missing timestamp becomes `~NNNN` so the page order survives a reload.
/// The current `post` is included when its id matches `post_id` (or is absent)
/// and its body is not already in the list.
pub fn parse_edits(props: &Value, post_id: &str, observed_at: &str) -> Vec<PostVersion> {
    let mut items: Vec<Value> = Vec::new();
    for key in ["edits", "versions", "history", "edit_history"] {
        if let Some(a) = props.get(key).and_then(Value::as_array) {
            items.extend(a.iter().cloned());
            break;
        }
        if let Some(a) = props.pointer(&format!("/post/{key}")).and_then(Value::as_array) {
            items.extend(a.iter().cloned());
            break;
        }
    }
    if let Some(post) = props.get("post").filter(|p| p.is_object()) {
        let id = s(post, "id");
        if id.is_empty() || id == post_id { items.push(post.clone()); }
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (i, item) in items.iter().enumerate() {
        let Some(body) = stored_body(item) else { continue };
        let edited_at = if item.is_object() { stamp(item, i) } else { format!("~{i:04}") };
        let hash = version_hash(&body, &edited_at);
        if !seen.insert(hash.clone()) { continue; }
        out.push(PostVersion {
            post_id: post_id.into(), version_hash: hash, body_json: body, edited_at,
            observed_at: observed_at.into(), source: "site".into(),
        });
    }
    out
}

fn stamp(obj: &Value, index: usize) -> String {
    for k in ["edited_at", "inserted_at", "updated_at", "created_at"] {
        if let Some(s) = obj.get(k).and_then(Value::as_str) {
            let s = s.trim();
            if !s.is_empty() { return s.to_string(); }
        }
    }
    format!("~{index:04}")
}

fn stored_body(item: &Value) -> Option<String> {
    if item.is_string() { return canon_body(item); }
    let obj = item.as_object()?;
    for k in ["body", "body_json", "content", "text", "markdown"] {
        if let Some(v) = obj.get(k) && let Some(s) = canon_body(v) { return Some(s); }
    }
    if obj.get("type").and_then(Value::as_str) == Some("doc") { return serde_json::to_string(item).ok(); }
    None
}

fn canon_body(v: &Value) -> Option<String> {
    match v {
        Value::Object(_) | Value::Array(_) => serde_json::to_string(v).ok().filter(|s| !s.is_empty()),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() { return None; }
            if (t.starts_with('{') || t.starts_with('['))
                && let Ok(j) = serde_json::from_str::<Value>(t)
                && (j.is_object() || j.is_array())
            {
                return serde_json::to_string(&j).ok();
            }
            Some(s.clone())
        }
        _ => None,
    }
}

/// One card per version, oldest first. `content` is a fenced diff against the
/// previous version (the first version is all additions). `post_id` stays the
/// real post so the post menu still refers to it. `thread_id` is the parent
/// when this post is a reply.
pub fn edits_as_posts(versions: &[PostVersion], thread_id: &str) -> Vec<Post> {
    let mut vs = versions.to_vec();
    vs.sort_by(|a, b| a.edited_at.cmp(&b.edited_at).then(a.version_hash.cmp(&b.version_hash)));
    let n = vs.len();
    let mut prev = String::new();
    let mut out = Vec::new();
    for (i, v) in vs.iter().enumerate() {
        let text = version_plain(&v.body_json);
        let diff = line_diff(&prev, &text);
        prev = text;
        let parent = if thread_id.is_empty() || thread_id == v.post_id { String::new() } else { thread_id.to_string() };
        let when = if v.edited_at.starts_with('~') { "undated".to_string() } else { v.edited_at.clone() };
        let body = if diff.trim().is_empty() { "(no text)\n".into() } else { format!("```diff\n{diff}```\n") };
        out.push(Post {
            post_id: v.post_id.clone(), parent_post_id: parent, in_reply_to_id: String::new(),
            nick: v.source.clone(), title: format!("version {} of {n} · {when}", i + 1),
            content: body, posted_at: if v.edited_at.starts_with('~') { String::new() } else { v.edited_at.clone() },
            media: Vec::new(), truncated: false, body_json: v.body_json.clone(), score: None,
        });
    }
    out
}

/// `--edits`: read `/rs/post/{thread}/{post}/edits` and store the versions.
pub async fn read_edits(http: &reqwest::Client, jar: &Jar, thread: &str, post: &str) -> Result<usize> {
    let url = edits_url(thread, post);
    let resp = http.get(&url).send().await.with_context(|| format!("reading edits of {post}"))?;
    let final_url = resp.url().to_string();
    let html = resp.text().await?;
    let mut found = props(&html, EDITS_PROPS_SEL);
    if found.is_none() && let Ok(page) = parse_lv(&final_url, &html) && let Ok(lv) = Lv::connect(&page, jar).await {
        found = props(&lv.html(), EDITS_PROPS_SEL);
    }
    let props = found.with_context(|| format!("no forum/Edits on {url}"))?;
    let observed = chrono::Local::now().to_rfc3339();
    let rows = parse_edits(&props, post.trim(), &observed);
    if rows.is_empty() { bail!("forum/Edits on {url} had no versions"); }
    let mut c = open_db(&home()?.join(DB_REL_PATH))?;
    save_post_versions(&mut c, &rows)?;
    Ok(rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_separates_the_two_halves_and_stays_put() {
        assert_eq!(version_hash("ab", "c").len(), 16);
        assert_eq!(version_hash("body", "t"), version_hash("body", "t"));
        assert_ne!(version_hash("ab", "c"), version_hash("a", "bc"));
        assert_ne!(version_hash("body", "t"), version_hash("body", "u"));
    }

    #[test]
    fn line_diff_is_context_removed_and_added() {
        assert_eq!(line_diff("a\nb\nc", "a\nx\nc"), "  a\n- b\n+ x\n  c\n");
        assert_eq!(line_diff("", "a\nb"), "+ a\n+ b\n");
        assert_eq!(line_diff("a\n", ""), "- a\n");
        assert_eq!(line_diff("a\n", "a"), "  a\n");
    }

    #[test]
    fn spec_is_a_pair_or_a_bare_id() {
        assert_eq!(parse_edits_spec(" T1/P1 ").unwrap(), EditsSpec::Pair { thread: "T1".into(), post: "P1".into() });
        assert_eq!(parse_edits_spec("P1").unwrap(), EditsSpec::Id("P1".into()));
        assert!(parse_edits_spec("").is_err());
        assert!(parse_edits_spec("T1/").is_err());
        assert!(parse_edits_spec("T1/P1/extra").is_err());
    }

    #[test]
    fn key_round_trips() {
        assert_eq!(edits_url("T", "P"), format!("{}/rs/post/T/P/edits", base_url()));
        assert_eq!(parse_edits_key(&edits_key("T", "P")), Some(("T", "P")));
        assert_eq!(parse_edits_key("top:week"), None);
    }

    #[test]
    fn props_keep_page_order_and_drop_the_duplicate_current_post() {
        let props = json!({
            "edits": [
                {"body": "alpha\nbeta", "edited_at": "2026-01-01T00:00:00Z"},
                {"body": "alpha\ngamma", "inserted_at": "2026-02-01T00:00:00Z"}
            ],
            "post": {"id": "P1", "body": "alpha\ngamma", "inserted_at": "2026-02-01T00:00:00Z"}
        });
        let rows = parse_edits(&props, "P1", "obs");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.source == "site" && r.post_id == "P1" && r.observed_at == "obs"));
        let cards = edits_as_posts(&rows, "");
        assert_eq!(cards[0].title, "version 1 of 2 · 2026-01-01T00:00:00Z");
        assert!(cards[0].content.contains("+ alpha\n+ beta\n"), "{}", cards[0].content);
        assert!(cards[1].content.contains("  alpha\n- beta\n+ gamma\n"), "{}", cards[1].content);
        assert_eq!(cards[1].post_id, "P1");
        assert!(cards[1].parent_post_id.is_empty());
        let reply = edits_as_posts(&rows, "T1");
        assert_eq!(reply[0].parent_post_id, "T1");
    }

    #[test]
    fn a_string_list_without_dates_keeps_its_order() {
        let rows = parse_edits(&json!({"edits": ["first", "second"]}), "P", "o");
        assert_eq!(rows.iter().map(|r| r.edited_at.as_str()).collect::<Vec<_>>(), ["~0000", "~0001"]);
        let cards = edits_as_posts(&rows, "");
        assert!(cards[0].title.contains("undated"));
        assert!(cards[1].content.contains("- first\n+ second\n"), "{}", cards[1].content);
    }

    #[test]
    fn html_props_round_trip_through_the_selector() {
        let html = r#"<div id="forum/Edits" data-props='{"edits":[{"body":"one","edited_at":"2026-03-01T00:00:00Z"}],"post":{"id":"OTHER","body":"nope"}}'></div>"#;
        let props = props(html, EDITS_PROPS_SEL).expect("props");
        let rows = parse_edits(&props, "P9", "o");
        assert_eq!(rows.len(), 1, "a post with a different id is not this post's version");
        assert_eq!(version_plain(&rows[0].body_json), "one");
    }

    #[test]
    fn a_prosemirror_body_flattens_for_the_diff() {
        let doc = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hello"}]}]});
        let rows = parse_edits(&json!({"edits": [{"body": doc, "updated_at": "2026-04-01T00:00:00Z"}]}), "P", "o");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].body_json.starts_with('{'));
        assert_eq!(version_plain(&rows[0].body_json), "hello");
    }
}
