//! Post detail content, at parity with the HTML export: Post/Reply badge,
//! `@nick · posted_at · post_id`, "Replying to … · thread …", title, body with
//! its paragraphs, attachments (path + open hint), and every cross-link.

use crate::prelude::*;
use crate::tui::layout::{ellipsize, wrap};
use crate::tui::state::{DetailFrame, LinkKind};
use crate::tui::theme::Theme;
use ratatui::text::{Line, Span};

/// `Post` / `Reply` badge.
pub fn badge(p: &Post, t: &Theme) -> Span<'static> {
    if p.parent_post_id.is_empty() {
        Span::styled(" Post ", t.post_tag())
    } else {
        Span::styled(" Reply ", t.reply_tag())
    }
}

/// `[badge] @nick · posted_at · post_id`.
pub fn meta_line(p: &Post, t: &Theme) -> Line<'static> {
    let mut v = vec![badge(p, t), Span::raw(" "),
        Span::styled(format!("@{}", p.nick), t.bold()),
        Span::styled(format!(" · {} · {}", p.posted_at, p.post_id), t.muted())];
    if p.truncated { v.push(Span::styled(" · shortened (run --refetch-truncated)", t.muted())); }
    Line::from(v)
}

/// First non-empty line of the title or body.
pub fn snippet(p: &Post) -> String {
    let s = if p.title.trim().is_empty() { &p.content } else { &p.title };
    s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("(no text)").to_string()
}

/// Two-row header of a collapsed frame.
pub fn header_lines(f: &DetailFrame, width: usize, t: &Theme) -> Vec<Line<'static>> {
    vec![meta_line(&f.post, t), Line::styled(ellipsize(&snippet(&f.post), width), t.muted())]
}

fn link_label(kind: LinkKind) -> &'static str {
    match kind { LinkKind::ReplyTo => "↳ Replying to", LinkKind::Thread => "⌂ Thread", LinkKind::Mentioned => "→ Linked post",
        LinkKind::Reply => "↩ Reply", LinkKind::Attachment => "▣ Open", LinkKind::Tag => "#" }
}

/// Full detail lines for `width` columns, and (line index, link index) for each link row.
pub fn detail_lines(f: &DetailFrame, width: usize, t: &Theme) -> (Vec<Line<'static>>, Vec<(usize, usize)>) {
    let (lines, links, _, _) = detail_layout(f, width, t, 0);
    (lines, links)
}

/// Rows reserved for inline pictures: (first line index, image file).
pub type ImageSlots = Vec<(usize, std::path::PathBuf)>;

/// What [`detail_layout`] returns: the lines, the attachment link rows, the image slots and
/// the body's Markdown links (line indexes into the lines).
pub type DetailLayout = (Vec<Line<'static>>, Vec<(usize, usize)>, ImageSlots, Vec<crate::tui::markdown::MdLink>);

/// [`detail_lines`] plus `img_rows` blank rows under the text for each stored
/// picture (0 = no pictures), and where those rows start.
/// Also returns the body's Markdown links with their line indices in the result (for OSC 8).
pub fn detail_layout(f: &DetailFrame, width: usize, t: &Theme, img_rows: u16)
    -> DetailLayout {
    let mut slots = Vec::new();
    let mut lines = vec![meta_line(&f.post, t)];
    let mut links = Vec::new();
    let link_line = |i: usize, lines: &mut Vec<Line<'static>>, links: &mut Vec<(usize, usize)>| {
        let l = &f.links[i];
        let sel = i == f.link_cursor;
        if l.kind == LinkKind::Attachment {
            let a = f.attachments.iter().find(|a| a.local_path.display().to_string() == l.id);
            let mime = a.map(|a| a.mime_type.as_str()).unwrap_or("file");
            let exists = std::path::Path::new(&l.id).exists();
            links.push((lines.len(), i));
            let name = std::path::Path::new(&l.id).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| l.id.clone());
            lines.push(Line::from(vec![
                Span::styled(format!("{} {mime}", link_label(l.kind)), if sel { t.selected() } else { t.accent() }),
                Span::raw("  "),
                Span::styled(name, if exists { t.accent().underline(true) } else { t.muted().strikethrough(true) }),
                Span::styled(if exists { "" } else { "  (file missing)" }, t.muted()),
            ]));
            return;
        }
        if l.kind == LinkKind::Tag {
            let st = if sel { t.selected() } else { t.accent() };
            links.push((lines.len(), i));
            lines.push(Line::from(vec![
                Span::styled(format!("#{}", l.id), st.underline(true)),
                Span::styled("  tag feed", t.muted()),
            ]));
            return;
        }
        let who = match &l.target {
            Some(p) => format!("@{} · {} · {}", p.nick, p.posted_at, ellipsize(&snippet(p), width.saturating_sub(40).max(10))),
            None => "not in database".into(),
        };
        let st = if sel { t.selected() } else { t.accent() };
        links.push((lines.len(), i));
        lines.push(Line::from(vec![
            Span::styled(format!("{} {}", link_label(l.kind), l.id), st.underline(true)),
            Span::styled(format!("  {who}"), if l.target.is_some() { t.muted() } else { t.muted().italic(true) }),
        ]));
    };
    // "Replying to … · thread …" right under the meta line, like the HTML export.
    for i in 0..f.links.len() {
        if matches!(f.links[i].kind, LinkKind::ReplyTo | LinkKind::Thread) { link_line(i, &mut lines, &mut links); }
    }
    if let Some(n) = &f.notice { lines.push(Line::styled(format!("⚠ {n}"), t.accent_bold())); }
    lines.push(Line::raw(""));
    if !f.post.title.trim().is_empty() {
        for l in wrap(f.post.title.trim(), width) { lines.push(Line::styled(l, t.bold())); }
        lines.push(Line::raw(""));
    }
    let md = crate::tui::markdown::render_doc(&link_hashtags(f.post.content.trim_end()), width, t);
    let at = lines.len();
    let md_links: Vec<_> = md.links.into_iter().map(|mut l| { l.line += at; l }).collect();
    lines.extend(md.lines);
    if img_rows > 0 {
        for a in &f.attachments {
            if !crate::tui::images::is_image(&a.mime_type, &a.local_path) || !a.local_path.exists() { continue; }
            lines.push(Line::raw(""));
            let name = a.local_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            lines.push(Line::styled(format!("🖼 {name}"), t.muted()));
            slots.push((lines.len(), a.local_path.clone()));
            for _ in 0..img_rows { lines.push(Line::raw("")); }
        }
    }
    let atts: Vec<usize> = (0..f.links.len()).filter(|i| f.links[*i].kind == LinkKind::Attachment).collect();
    if !atts.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(format!("Attachments ({}) · Enter / click opens", atts.len()), t.bold()));
        for i in atts { link_line(i, &mut lines, &mut links); }
    }
    let others: Vec<usize> = (0..f.links.len()).filter(|i| matches!(f.links[*i].kind, LinkKind::Mentioned)).collect();
    if !others.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("Linked posts", t.bold()));
        for i in others { link_line(i, &mut lines, &mut links); }
    }
    let tags: Vec<usize> = (0..f.links.len()).filter(|i| f.links[*i].kind == LinkKind::Tag).collect();
    if !tags.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(format!("Tags ({})", tags.len()), t.bold()));
        for i in tags { link_line(i, &mut lines, &mut links); }
    }
    let replies: Vec<usize> = (0..f.links.len()).filter(|i| matches!(f.links[*i].kind, LinkKind::Reply)).collect();
    if !replies.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(format!("Replies ({})", replies.len()), t.bold()));
        for i in replies { link_line(i, &mut lines, &mut links); }
    }
    if !f.links.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("←/→ choose a link · Enter opens it · Esc closes", t.muted()));
    }
    (lines, links, slots, md_links)
}
