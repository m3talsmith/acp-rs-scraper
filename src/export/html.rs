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

/// Append one post (and, recursively, its replies) as nested HTML.
pub fn html_post(v: &Value, out: &mut String, depth: usize, ids: &HashSet<String>) {
    let g = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    out.push_str(&format!("<article class=\"post d{}\" id=\"post-{}\">", depth.min(6), esc(&g("post_id"))));
    if !g("title").is_empty() { out.push_str(&format!("<h2>{}</h2>", esc(&g("title")))); }
    out.push_str(&format!("<div class=\"meta\"><b>@{}</b> · <time>{}</time></div>", esc(&g("nick")), esc(&g("posted_at"))));
    out.push_str(&reply_links(&g("parent_post_id"), &g("in_reply_to_id"), ids));
    out.push_str(&format!("<div class=\"body\">{}</div>", esc(&g("content")).replace('\n', "<br>")));
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
    body.push_str("<h2>Posts</h2>");
    let mut ids = HashSet::new();
    export_ids(&doc["posts"], &mut ids);
    for p in doc["posts"].as_array().into_iter().flatten() { html_post(p, &mut body, 0, &ids); }
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
<meta name="viewport" content="width=device-width,initial-scale=1"><title>{nick}</title>
<style>
body{{font:16px/1.5 system-ui,sans-serif;max-width:800px;margin:2em auto;padding:0 1em;color:#18181b;background:#fafafa}}
.post{{background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:.8em 1em;margin:.8em 0}}
.post .post{{margin-left:1.2em;border-left:3px solid #dc2626;border-radius:0 8px 8px 0}}
h2{{margin:.2em 0;font-size:1.25em}} .meta{{color:#71717a;font-size:.85em}} .body{{margin:.5em 0;white-space:normal}}
.att img,.att video{{max-width:100%;border-radius:6px}} .att{{margin:.4em 0}}
.profile{{display:flex;gap:1em;align-items:flex-start;background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:1em;margin:1em 0}}
.profile .avatar{{width:72px;height:72px;border-radius:50%;object-fit:cover}}
.mentions{{padding-left:1.2em}} .reply-to a{{color:#dc2626}}
</style><script type="application/json" id="{data_id}">{data}</script></head><body><h1>{nick}</h1><p class="meta">Exported {}</p>{body}</body></html>"#,
        esc(doc["exported_at"].as_str().unwrap_or_default()),
        data_id = HTML_DATA_ID,
        data = doc.to_string().replace('<', "\\u003c"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
