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
.body h1,.body h2,.body h3,.body h4,.body h5,.body h6{{margin:.6em 0 .3em;line-height:1.3}}
.body pre{{background:#f4f4f5;padding:.6em .8em;overflow:auto;border-radius:6px}}
.body pre,.body code{{font-family:ui-monospace,SFMono-Regular,monospace}}
.body table{{border-collapse:collapse;margin:.6em 0}} .body th,.body td{{border:1px solid #e4e4e7;padding:.2em .5em}}
.body blockquote{{margin:.4em 0;padding-left:.7em;border-left:3px solid #dc2626;color:#3f3f46}}
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

/// Render Markdown to HTML for a post body or a research note.
///
/// Headings, fenced code, and tables become elements. Raw HTML in the source
/// is escaped, so a post cannot inject a tag.
pub fn paragraphs_html(text: &str) -> String {
    use pulldown_cmark::{Alignment, Event, Options, Parser, Tag, TagEnd};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES | Options::ENABLE_TASKLISTS);
    let mut out = String::new();
    let mut in_head = false;
    let mut in_code = false;
    let mut in_image = false;
    let mut table_align: Vec<Alignment> = Vec::new();
    let mut cell = 0usize;
    let mut tbody = false;
    let align = |a: Alignment| match a {
        Alignment::Left => " style=\"text-align:left\"",
        Alignment::Center => " style=\"text-align:center\"",
        Alignment::Right => " style=\"text-align:right\"",
        Alignment::None => "",
    };
    for ev in Parser::new_ext(text, opts) {
        match ev {
            Event::Start(Tag::Paragraph) => out.push_str("<p>"),
            Event::End(TagEnd::Paragraph) => out.push_str("</p>"),
            Event::Start(Tag::Heading { level, .. }) => out.push_str(&format!("<{level}>")),
            Event::End(TagEnd::Heading(level)) => out.push_str(&format!("</{level}>\n")),
            Event::Start(Tag::BlockQuote(_)) => out.push_str("<blockquote>"),
            Event::End(TagEnd::BlockQuote(_)) => out.push_str("</blockquote>\n"),
            Event::Start(Tag::CodeBlock(_)) => { in_code = true; out.push_str("<pre><code>"); }
            Event::End(TagEnd::CodeBlock) => { in_code = false; out.push_str("</code></pre>\n"); }
            Event::Start(Tag::List(Some(n))) => out.push_str(&format!("<ol start=\"{n}\">\n")),
            Event::Start(Tag::List(None)) => out.push_str("<ul>\n"),
            Event::End(TagEnd::List(ordered)) => out.push_str(if ordered { "</ol>\n" } else { "</ul>\n" }),
            Event::Start(Tag::Item) => out.push_str("<li>"),
            Event::End(TagEnd::Item) => out.push_str("</li>\n"),
            Event::Start(Tag::Emphasis) => out.push_str("<em>"),
            Event::End(TagEnd::Emphasis) => out.push_str("</em>"),
            Event::Start(Tag::Strong) => out.push_str("<strong>"),
            Event::End(TagEnd::Strong) => out.push_str("</strong>"),
            Event::Start(Tag::Strikethrough) => out.push_str("<del>"),
            Event::End(TagEnd::Strikethrough) => out.push_str("</del>"),
            Event::Start(Tag::Link { dest_url, title, .. }) => {
                out.push_str(&format!("<a href=\"{}\"", esc(&dest_url)));
                if !title.is_empty() { out.push_str(&format!(" title=\"{}\"", esc(&title))); }
                out.push('>');
            }
            Event::End(TagEnd::Link) => out.push_str("</a>"),
            Event::Start(Tag::Image { dest_url, .. }) => {
                in_image = true;
                out.push_str(&format!("<img src=\"{}\" alt=\"", esc(&dest_url)));
            }
            Event::End(TagEnd::Image) => { in_image = false; out.push_str("\">"); }
            Event::Start(Tag::Table(a)) => { table_align = a; cell = 0; tbody = false; out.push_str("<table>\n"); }
            Event::End(TagEnd::Table) => {
                if tbody { out.push_str("</tbody>\n"); }
                out.push_str("</table>\n");
            }
            Event::Start(Tag::TableHead) => { in_head = true; out.push_str("<thead>\n"); }
            Event::End(TagEnd::TableHead) => { in_head = false; tbody = true; out.push_str("</thead>\n<tbody>\n"); }
            Event::Start(Tag::TableRow) => { cell = 0; out.push_str("<tr>"); }
            Event::End(TagEnd::TableRow) => out.push_str("</tr>\n"),
            Event::Start(Tag::TableCell) => {
                let tag = if in_head { "th" } else { "td" };
                let style = align(table_align.get(cell).copied().unwrap_or(Alignment::None));
                cell += 1;
                out.push_str(&format!("<{tag}{style}>"));
            }
            Event::End(TagEnd::TableCell) => out.push_str(if in_head { "</th>" } else { "</td>" }),
            Event::Start(Tag::FootnoteDefinition(name)) => {
                out.push_str(&format!("<div class=\"footnote\" id=\"fn-{}\">", esc(&name)));
            }
            Event::End(TagEnd::FootnoteDefinition) => out.push_str("</div>\n"),
            Event::FootnoteReference(name) => {
                let name = esc(&name);
                out.push_str(&format!("<sup id=\"fnref-{name}\"><a href=\"#fn-{name}\">{name}</a></sup>"));
            }
            Event::TaskListMarker(checked) => {
                out.push_str(if checked { "<input type=\"checkbox\" disabled checked> " } else { "<input type=\"checkbox\" disabled> " });
            }
            Event::Text(t) => out.push_str(&esc(&t)),
            Event::Code(t) => out.push_str(&format!("<code>{}</code>", esc(&t))),
            Event::Html(t) | Event::InlineHtml(t) => out.push_str(&esc(&t)),
            Event::InlineMath(t) | Event::DisplayMath(t) => out.push_str(&esc(&t)),
            Event::SoftBreak | Event::HardBreak => {
                if in_code || in_image { out.push('\n'); } else { out.push_str("<br>\n"); }
            }
            Event::Rule => out.push_str("<hr>\n"),
            Event::Start(_) | Event::End(_) => {}
        }
    }
    out
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

    #[test]
    fn markdown_renders_in_the_page_body() {
        let doc = json!({"nick": "@sample", "posts": [{
            "post_id": "M1", "parent_post_id": "", "nick": "sample", "title": "Notes",
            "content": "# Hello\n\n```\ncode\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n<script>alert(1)</script>\n",
            "children": []
        }]});
        let h = build_html(&doc);
        let body = h.split_once("<body>").expect("body").1;
        assert!(body.contains("<h1>Hello</h1>"), "{body}");
        assert!(body.contains("<pre><code>code"), "{body}");
        assert!(body.contains("<table>") && body.contains("<td>1</td>"), "{body}");
        assert!(!body.contains("# Hello") && !body.contains("```") && !body.contains("|---|"), "{body}");
        assert!(body.contains("&lt;script&gt;") && !body.contains("<script>"), "{body}");
    }
}
