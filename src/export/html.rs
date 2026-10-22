//! Self-contained HTML export page (with the JSON embedded for re-import).

use crate::prelude::*;

/// Escape text for HTML element content and attribute values.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Every `post_id` in an export's post tree.
pub fn export_ids(posts: &Value, out: &mut HashSet<String>) {
    for p in posts.as_array().into_iter().flatten() {
        if let Some(id) = p["post_id"].as_str().filter(|x| !x.is_empty()) { out.insert(id.to_string()); }
        export_ids(&p["children"], out);
    }
}

/// Link target for post `id` (in thread `root`): its `#post-{id}` anchor when
/// it is in this export, else its page on the site (`/rs/post/{root}/{id}`
/// for replies, `/rs/post/{id}` for thread roots, as the site links them).
pub fn post_href(id: &str, root: &str, ids: &HashSet<String>) -> String {
    if ids.contains(id) { format!("#post-{id}") }
    else if root.is_empty() || root == id { format!("{}/rs/post/{id}", base_url()) }
    else { format!("{}/rs/post/{root}/{id}", base_url()) }
}

/// "Replying to" line for a reply: the immediate parent, plus the thread root when that differs.
pub fn reply_links(root: &str, direct: &str, ids: &HashSet<String>) -> String {
    if root.is_empty() { return String::new(); }
    let direct = if direct.is_empty() { root } else { direct };
    let a = |id: &str, r: &str, cls: &str| format!("<a class=\"{cls}\" href=\"{}\">{}</a>", esc(&post_href(id, r, ids)), esc(id));
    let mut s = format!("<div class=\"meta reply-to\">Replying to {}", a(direct, root, "parent"));
    if direct != root { s.push_str(&format!(" · thread {}", a(root, root, "thread"))); }
    s.push_str("</div>");
    s
}

/// (top-level posts, replies) over a whole export post tree.
pub fn count_kinds(posts: &Value) -> (usize, usize) {
    let mut n = (0, 0);
    for p in posts.as_array().into_iter().flatten() {
        if p["parent_post_id"].as_str().unwrap_or_default().is_empty() { n.0 += 1 } else { n.1 += 1 }
        let (a, b) = count_kinds(&p["children"]);
        n.0 += a; n.1 += b;
    }
    n
}

/// Append one post (and, recursively, its replies) as nested HTML.
pub fn html_post(v: &Value, out: &mut String, depth: usize, ids: &HashSet<String>) {
    let g = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    let kind = if g("parent_post_id").is_empty() { "post" } else { "reply" };
    out.push_str(&format!("<article class=\"post kind-{kind} d{}\" id=\"post-{}\">", depth.min(6), esc(&g("post_id"))));
    if !g("title").is_empty() { out.push_str(&format!("<h2>{}</h2>", esc(&g("title")))); }
    out.push_str(&format!("<div class=\"meta\"><span class=\"badge {kind}\">{}</span> <b>@{}</b> · <time>{}</time></div>", if kind == "post" { "Post" } else { "Reply" }, esc(&g("nick")), esc(&g("posted_at"))));
    out.push_str(&reply_links(&g("parent_post_id"), &g("in_reply_to_id"), ids));
    out.push_str(&format!("<div class=\"body\">{}</div>", paragraphs_html(&g("content"))));
    for a in v["attachments"].as_array().into_iter().flatten() {
        let p = esc(a["local_path"].as_str().unwrap_or_default());
        let m = a["mime_type"].as_str().unwrap_or_default();
        if a["kind"] == "thumbnail" { continue; }
        out.push_str("<div class=\"att\">");
        if m.starts_with("image/") { out.push_str(&format!("<img src=\"{p}\" loading=\"lazy\">")); }
        else if m.starts_with("video/") { out.push_str(&format!("<video src=\"{p}\" controls preload=\"metadata\"></video>")); }
        else if m.starts_with("audio/") { out.push_str(&format!("<audio src=\"{p}\" controls></audio>")); }
        else { out.push_str(&format!("<a href=\"{p}\">{p}</a>")); }
        out.push_str("</div>");
    }
    for c in v["children"].as_array().into_iter().flatten() { html_post(c, out, depth + 1, ids); }
    out.push_str("</article>");
}

/// Render an export document as one self-contained HTML page, embedding the JSON for re-import.
pub fn build_html(doc: &Value) -> String {
    build_html_with(doc, &crate::brands::current().title)
}

/// [`build_html`] with an explicit product title in the document header.
pub fn build_html_with(doc: &Value, product: &str) -> String {
    let product = esc(product);
    let title = esc(doc["title"].as_str().or(doc["nick"].as_str()).unwrap_or_default());
    let nick = title.clone();
    let mut body = String::new();
    for pr in doc["profiles"].as_array().into_iter().flatten() {
        let dn = pr["display_name"].as_str().unwrap_or("");
        let un = pr["nick"].as_str().unwrap_or("");
        let bio = pr["bio"].as_str().unwrap_or("");
        let av = pr["avatar_path"].as_str().or(pr["avatar_url"].as_str()).unwrap_or("");
        body.push_str("<header class=\"profile\">");
        if !av.is_empty() { body.push_str(&format!("<img class=\"avatar\" src=\"{}\" alt=\"\">", esc(av))); }
        body.push_str(&format!("<div><h2>{} <span class=\"meta\">@{}</span></h2>", esc(dn), esc(un)));
        if !bio.is_empty() { body.push_str(&format!("<p>{}</p>", esc(bio))); }
        let bits: Vec<String> = ["location","role","joined_at"].iter().filter_map(|k| pr[*k].as_str().filter(|s| !s.is_empty()).map(|s| format!("{k}: {}", esc(s)))).collect();
        if !bits.is_empty() { body.push_str(&format!("<p class=\"meta\">{}</p>", bits.join(" · "))); }
        body.push_str("</div></header>");
    }
    let mut ids = HashSet::new();
    export_ids(&doc["posts"], &mut ids);
    // Thread roots (top-level posts) and replies in separate sections, so a
    // nick's own posts are never lost among their replies.
    let top: Vec<&Value> = doc["posts"].as_array().into_iter().flatten().collect();
    let (roots, orphans): (Vec<&Value>, Vec<&Value>) = top.into_iter()
        .partition(|p| p["parent_post_id"].as_str().unwrap_or_default().is_empty());
    if doc["research"].is_object() { body.push_str(&research_html(&doc["research"], &ids)); }
    let (np, nr) = count_kinds(&doc["posts"]);
    body.push_str(&format!("<p class=\"meta counts\">{np} posts · {nr} replies</p>"));
    body.push_str(&format!("<h2 id=\"posts\">Posts ({})</h2>", roots.len()));
    if roots.is_empty() { body.push_str("<p class=\"meta\">No top-level posts in this export.</p>"); }
    for p in roots { html_post(p, &mut body, 0, &ids); }
    if !orphans.is_empty() {
        body.push_str(&format!("<h2 id=\"replies\">Replies in other threads ({})</h2>", orphans.len()));
        for p in orphans { html_post(p, &mut body, 0, &ids); }
    }
    let mentions = doc["mentions"].as_array().cloned().unwrap_or_default();
    if !mentions.is_empty() {
        body.push_str("<h2>Mentions</h2><ul class=\"mentions\">");
        for h in &mentions {
            body.push_str(&format!(
                "<li><a href=\"#post-{}\">{}</a> → @{} <span class=\"meta\">({})</span></li>",
                esc(h["post_id"].as_str().unwrap_or("")),
                esc(h["post_id"].as_str().unwrap_or("")),
                esc(h["mentioned_nick"].as_str().unwrap_or("")),
                esc(h["match_type"].as_str().unwrap_or("")),
            ));
        }
        body.push_str("</ul>");
    }
    format!(r#"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>{product} — {nick}</title>
<style>
body{{font:16px/1.5 system-ui,sans-serif;max-width:800px;margin:2em auto;padding:0 1em;color:#18181b;background:#fafafa}}
.post{{background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:.8em 1em;margin:.8em 0}}
.post .post{{margin-left:1.2em;border-left:3px solid #dc2626;border-radius:0 8px 8px 0}}
h2{{margin:.2em 0;font-size:1.25em}} .meta{{color:#71717a;font-size:.85em}} .body{{margin:.5em 0;white-space:normal}} .body p{{margin:0 0 .7em}}
.att img,.att video{{max-width:100%;border-radius:6px}} .att{{margin:.4em 0}}
.profile{{display:flex;gap:1em;align-items:flex-start;background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:1em;margin:1em 0}}
.profile .avatar{{width:72px;height:72px;border-radius:50%;object-fit:cover}}
.badge{{display:inline-block;font-size:.75em;font-weight:600;padding:0 .45em;border-radius:4px;color:#fff;background:#71717a}} .badge.post{{background:#dc2626}}
.post.kind-post.d0{{border-left:4px solid #dc2626}}
.research{{background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:.8em 1em;margin:1em 0}} .research-items li{{margin:.5em 0}}
.note{{margin:.3em 0 .3em .5em;padding-left:.7em;border-left:3px solid #dc2626;color:#3f3f46}}
.mentions{{padding-left:1.2em}} .reply-to a{{color:#dc2626}}
</style><script type="application/json" id="{data_id}">{data}</script></head><body><p class="meta">{product}</p><h1>{nick}</h1><p class="meta">Exported {}</p>{body}</body></html>"#,
        esc(doc["exported_at"].as_str().unwrap_or_default()),
        data_id = HTML_DATA_ID,
        data = doc.to_string().replace('<', "\\u003c"))
}

/// The research section: description, then every item with its kind, title, note and source.
pub fn research_html(r: &Value, ids: &HashSet<String>) -> String {
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or_default().to_string();
    let items = r["items"].as_array().cloned().unwrap_or_default();
    let mut h = format!("<section class=\"research\"><h2 id=\"research\">Research: {} <span class=\"meta\">({} items)</span></h2>",
        esc(&s(r, "name")), items.len());
    if !s(r, "description").is_empty() { h.push_str(&paragraphs_html(&s(r, "description"))); }
    h.push_str("<ol class=\"research-items\">");
    for i in &items {
        let kind = s(i, "kind");
        let (title, rf) = (s(i, "title"), s(i, "ref"));
        h.push_str(&format!("<li class=\"item kind-{}\"><span class=\"badge\">{}</span> ", esc(&kind), esc(&kind)));
        match kind.as_str() {
            "post" | "reply" | "search-result" => {
                let href = if ids.contains(&rf) { format!("#post-{}", esc(&rf)) } else { format!("{}/rs/post/{}", esc(base_url()), esc(&rf)) };
                h.push_str(&format!("<a href=\"{href}\">{}</a> <span class=\"meta\">{}</span>", esc(&title), esc(&rf)));
                if let Some(q) = i["meta"]["query"].as_str() { h.push_str(&format!(" <span class=\"meta\">search: {}</span>", esc(q))); }
            }
            "web-url" => {
                h.push_str(&format!("<a href=\"{}\">{}</a> <span class=\"meta\">{}</span>", esc(&rf), esc(&title), esc(&rf)));
                if let Some(f) = i["file"].as_str() { h.push_str(&format!(" · <a href=\"{}\">snapshot</a>", esc(f))); }
            }
            _ => {
                let link = i["file"].as_str().map(str::to_string).unwrap_or_else(|| format!("file://{rf}"));
                h.push_str(&format!("<a href=\"{}\">{}</a> <span class=\"meta\">{}</span>", esc(&link), esc(&title), esc(&rf)));
            }
        }
        h.push_str(&format!(" <span class=\"meta\">added {}</span>", esc(&s(i, "added_at"))));
        if !s(i, "note").is_empty() { h.push_str(&format!("<blockquote class=\"note\">{}</blockquote>", paragraphs_html(&s(i, "note")))); }
        let content = s(i, "content");
        if !content.trim().is_empty() {
            h.push_str(&format!("<details><summary>Text ({} characters)</summary><div class=\"body\">{}</div></details>",
                content.chars().count(), paragraphs_html(&content)));
        }
        h.push_str("</li>");
    }
    h.push_str("</ol></section>");
    h
}

/// Body text as HTML paragraphs: blank lines split `<p>`s, single newlines
/// inside a paragraph become `<br>`.
pub fn paragraphs_html(text: &str) -> String {
    text.split("\n\n").map(str::trim).filter(|p| !p.is_empty())
        .map(|p| format!("<p>{}</p>", esc(p).replace('\n', "<br>")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posts_and_replies_get_separate_sections_and_badges() {
        let doc = json!({"nick": "@haz", "posts": [
            {"post_id": "R1", "parent_post_id": "", "nick": "haz", "content": "my post", "children": [
                {"post_id": "R1a", "parent_post_id": "R1", "nick": "haz", "content": "self reply", "children": []}]},
            {"post_id": "C1", "parent_post_id": "T9", "nick": "haz", "content": "comment", "children": []},
        ]});
        assert_eq!(count_kinds(&doc["posts"]), (1, 2));
        let h = build_html(&doc);
        let posts = h.find("<h2 id=\"posts\">Posts (1)</h2>").expect("posts section");
        let replies = h.find("<h2 id=\"replies\">Replies in other threads (1)</h2>").expect("replies section");
        let r1 = h.find("id=\"post-R1\"").unwrap();
        let c1 = h.find("id=\"post-C1\"").unwrap();
        assert!(posts < r1 && r1 < replies && replies < c1, "posts before replies");
        assert!(h.contains("class=\"post kind-post d0\" id=\"post-R1\""));
        assert!(h.contains("class=\"post kind-reply d0\" id=\"post-C1\""));
        assert!(h.contains("1 posts · 2 replies"));
    }

    #[test]
    fn the_header_uses_the_product_title_and_the_nick_stays_the_heading() {
        let doc = json!({"nick": "@reader", "posts": []});
        let h = build_html_with(&doc, "Red Star");
        assert!(h.contains("<title>Red Star — @reader</title>"), "{h}");
        assert!(h.contains("<p class=\"meta\">Red Star</p><h1>@reader</h1>"), "{h}");
        assert!(!h.contains("ACP Red Star"), "{h}");
    }

    #[test]
    fn reply_links_prefer_in_export_anchors() {
        let ids: HashSet<String> = ["c1".to_string()].into();
        let h = reply_links("t1", "c1", &ids);
        assert!(h.contains("class=\"parent\" href=\"#post-c1\">c1</a>"), "{h}");
        assert!(h.contains(&format!("class=\"thread\" href=\"{}/rs/post/t1\">t1</a>", base_url())), "{h}");
        // Parent not exported: link to the reply on the site; root-only reply has no thread link.
        let h = reply_links("t1", "c2", &HashSet::new());
        assert!(h.contains(&format!("href=\"{}/rs/post/t1/c2\"", base_url())), "{h}");
        assert!(!reply_links("t1", "t1", &ids).contains("thread"));
        assert_eq!(reply_links("", "", &ids), "");
    }
}
