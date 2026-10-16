//! Body: navigation bar (with `<` back from search), the post cards, and the
//! detail stack that slides the list up and nests linked posts.

use crate::config::DETAIL_HEADER_ROWS;
use crate::tui::layout::{CARD_ROWS, detail_split, ellipsize, stack_layout, wrap};
use crate::tui::state::{AnimKind, App, Focus, Hit};
use crate::tui::widgets::detail::{badge, detail_lines, header_lines, meta_line, snippet};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use std::time::Instant;

/// Draw the Body into `r`.
pub fn draw(f: &mut Frame, app: &mut App, r: Rect, now: Instant) {
    let t = app.theme;
    let focused = app.focus == Focus::Body;
    let b = Block::default().borders(Borders::ALL).border_style(t.border(focused));
    let inner = b.inner(r);
    f.render_widget(b, r);
    if inner.height < 2 { return; }
    // Navigation bar.
    let nav = Rect { height: 1, ..inner };
    let content = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
    let in_search = app.search.is_some();
    if in_search { app.hits.push((Rect { width: 8.min(nav.width), ..nav }, Hit::Back)); }
    let v = app.view();
    let mut spans = Vec::new();
    if in_search {
        spans.push(Span::styled(" < Back ", t.selected()));
        spans.push(Span::raw(" "));
    }
    let title = if v.title.is_empty() { "Select a nick on the left".to_string() } else { v.title.clone() };
    spans.push(Span::styled(title, Style::default().add_modifier(Modifier::BOLD)));
    spans.push(Span::styled(format!("  {} posts", v.posts.len()), t.muted()));
    if !v.stack.is_empty() {
        spans.push(Span::styled(format!("  · detail {}/{}", v.stack.len(), crate::config::MAX_DETAIL_DEPTH), t.muted()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), nav);

    if v.posts.is_empty() {
        let msg = if in_search { "No posts match this search. Press < to go back." }
            else if app.nicks.is_empty() { "The database is empty. Open Scrape (or Import) in the menu bar to fill it." }
            else { "Click a nick (Space / Ctrl-click to select several) to read their posts and replies." };
        f.render_widget(Paragraph::new(msg).style(t.muted()), Rect { y: content.y + 1, height: content.height.saturating_sub(1), ..content });
        return;
    }
    app.page_rows = (content.height / CARD_ROWS).max(1) as usize;
    if app.view().stack.is_empty() { draw_cards(f, app, content); } else { draw_with_detail(f, app, content, now); }
}

fn draw_cards(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    app.hits.push((r, Hit::ListPane));
    let rows = app.page_rows;
    let focused = app.focus == Focus::Body;
    let v = app.view_mut();
    if v.cursor < v.offset { v.offset = v.cursor; }
    if v.cursor >= v.offset + rows { v.offset = v.cursor + 1 - rows; }
    let (off, cur) = (v.offset, v.cursor);
    let posts: Vec<_> = v.posts.iter().enumerate().skip(off).take(rows).map(|(i, p)| (i, p.clone())).collect();
    for (k, (i, p)) in posts.into_iter().enumerate() {
        let cr = Rect { x: r.x, y: r.y + k as u16 * CARD_ROWS, width: r.width, height: CARD_ROWS.min(r.height.saturating_sub(k as u16 * CARD_ROWS)) };
        let sel = i == cur;
        let b = Block::default().borders(Borders::ALL).title(meta_line(&p, &t))
            .border_type(if sel { BorderType::Thick } else { BorderType::Plain })
            .border_style(if sel && focused { t.accent() } else if sel { Style::default() } else { t.border(false) });
        let inner = b.inner(cr);
        f.render_widget(b, cr);
        let marked = app.is_marked(&p.post_id);
        if cr.width > 8 {
            let sr = Rect { x: cr.x + cr.width - 5, y: cr.y, width: 3, height: 1 };
            f.render_widget(Paragraph::new(Span::styled(star(marked), t.accent_bold())), sr);
            app.hits.push((sr, Hit::Star(i)));
        }
        let text = if p.title.trim().is_empty() { p.content.clone() } else { format!("{}\n{}", p.title.trim(), p.content) };
        let lines: Vec<Line> = wrap(text.trim(), inner.width as usize).into_iter().filter(|l| !l.trim().is_empty())
            .take(inner.height as usize).map(Line::raw).collect();
        f.render_widget(Paragraph::new(lines), inner);
        app.hits.insert(app.hits.len() - usize::from(cr.width > 8), (cr, Hit::Post(i)));
    }
}

/// Bookmark star: `★` bookmarked, `☆` not (click toggles).
pub fn star(on: bool) -> &'static str { if on { " ★ " } else { " ☆ " } }

fn compact(f: &mut Frame, app: &mut App, i: usize, r: Rect) {
    let t = app.theme;
    let Some(p) = app.view().posts.get(i).cloned() else { return };
    let w = r.width as usize;
    let head = format!(" @{} · {} ", p.nick, p.posted_at.get(..16).unwrap_or(&p.posted_at));
    let rest = ellipsize(&snippet(&p), w.saturating_sub(head.len() + 8));
    let mark = if app.is_marked(&p.post_id) { "★" } else { " " };
    f.render_widget(Paragraph::new(Line::from(vec![Span::styled(mark, t.accent_bold()), badge(&p, &t), Span::styled(head, t.muted()), Span::raw(rest)])), r);
    app.hits.push((r, Hit::Post(i)));
}

fn draw_with_detail(f: &mut Frame, app: &mut App, r: Rect, now: Instant) {
    let t = app.theme;
    let (kind, p) = app.anim_progress(now);
    let open_p = if kind == Some(AnimKind::Open) { p } else { 1.0 };
    let push_p = if kind == Some(AnimKind::Push) { p } else { 1.0 };
    let (n, cur) = (app.view().posts.len(), app.view().cursor);
    let split = detail_split(r, n, cur, open_p);
    for (i, rr) in split.above.clone() { compact(f, app, i, rr); }
    for (i, rr) in split.below.clone() { compact(f, app, i, rr); }
    let focused = app.focus == Focus::Body;
    let b = Block::default().borders(Borders::ALL).border_type(BorderType::Double).border_style(if focused { t.accent() } else { t.border(false) });
    let inner = b.inner(split.detail);
    f.render_widget(b, split.detail);
    let depth = app.view().stack.len();
    let sl = stack_layout(inner, depth, push_p);
    if let Some(nb) = sl.nested {
        f.render_widget(Block::default().borders(Borders::ALL).border_type(BorderType::Double).border_style(t.accent()), nb);
    }
    for (d, fr, collapsed) in sl.frames {
        if fr.height == 0 || fr.width < 3 { continue; }
        let fr = Rect { x: fr.x + 1, width: fr.width - 1, ..fr };
        if collapsed && fr.height <= DETAIL_HEADER_ROWS + 1 {
            let lines = header_lines(&app.view().stack[d], fr.width as usize, &t);
            f.render_widget(Paragraph::new(lines), fr);
            app.hits.push((fr, Hit::Frame(d)));
            continue;
        }
        // Full content (the top frame, or the previous one while it is being pushed up).
        let w = fr.width.saturating_sub(1) as usize;
        let (lines, links) = detail_lines(&app.view().stack[d], w, &t);
        let h = fr.height as usize;
        let top = d + 1 == depth;
        let fm = &mut app.view_mut().stack[d];
        if top && fm.follow_link
            && let Some((li, _)) = links.iter().find(|(_, k)| *k == fm.link_cursor) {
                let li = *li as u16;
                if li < fm.scroll { fm.scroll = li; }
                if li as usize >= fm.scroll as usize + h { fm.scroll = (li as usize + 1 - h) as u16; }
                fm.follow_link = false;
            }
        let max = lines.len().saturating_sub(h) as u16;
        if fm.scroll > max { fm.scroll = max; }
        let scroll = fm.scroll;
        f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), fr);
        if top {
            app.hits.push((fr, Hit::DetailPane));
            if fr.width > 6 {
                let marked = app.is_marked(&app.view().stack[d].post.post_id);
                let sr = Rect { x: fr.x + fr.width - 4, y: fr.y, width: 3, height: 1 };
                f.render_widget(Paragraph::new(Span::styled(star(marked), t.accent_bold())), sr);
                app.hits.push((sr, Hit::StarTop));
            }
            for (li, k) in links {
                let y = li as i64 - scroll as i64;
                if y >= 0 && (y as usize) < h {
                    app.hits.push((Rect { x: fr.x, y: fr.y + y as u16, width: fr.width, height: 1 }, Hit::Link(k)));
                }
            }
        } else {
            app.hits.push((fr, Hit::Frame(d)));
        }
    }
}
