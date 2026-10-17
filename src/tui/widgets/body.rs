//! Body: navigation bar (with `<` back from search), the post cards, and the
//! detail stack that slides the list up and nests linked posts.

use crate::config::DETAIL_HEADER_ROWS;
use crate::tui::layout::{CARD_ROWS, card_height, carousel, detail_split, ellipsize, grid_cells, grid_rows, modal_rect, stack_layout, wrap};
use crate::tui::state::{AnimKind, App, Focus, Hit, ViewMode};
use crate::tui::widgets::detail::{badge, detail_layout, header_lines, meta_line, snippet};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};
use crate::prelude::Post;
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
    let (pos, total) = app.stack_pos();
    let mut spans = Vec::new();
    let mut x = nav.x;
    let mut button = |spans: &mut Vec<Span<'static>>, hits: &mut Vec<(Rect, Hit)>, label: &'static str, st: Style, hit: Option<Hit>| {
        let w = unicode_width::UnicodeWidthStr::width(label) as u16;
        if let Some(h) = hit { hits.push((Rect { x, width: w.min((nav.x + nav.width).saturating_sub(x)), ..nav }, h)); }
        spans.push(Span::styled(label, st));
        spans.push(Span::raw(" "));
        x += w + 1;
    };
    if in_search { button(&mut spans, &mut app.hits, " < Back ", t.selected(), Some(Hit::Back)); }
    if total > 0 {
        let on = |b: bool| if b { t.selected() } else { t.muted() };
        button(&mut spans, &mut app.hits, " ◂ prev ", on(pos > 0), Some(Hit::StackPrev));
        button(&mut spans, &mut app.hits, " next ▸ ", on(pos < total), Some(Hit::StackNext));
    }
    let v = app.view();
    if total > 0 { spans.push(Span::styled(format!("{pos}/{total}  "), t.muted())); }
    let title = if v.title.is_empty() { "Select a nick on the left".to_string() } else { v.title.clone() };
    spans.push(Span::styled(title, Style::default().add_modifier(Modifier::BOLD)));
    spans.push(Span::styled(format!("  {} posts", v.posts.len()), t.muted()));
    if !v.stack.is_empty() {
        spans.push(Span::styled(format!("  · detail {}/{}", v.stack.len(), crate::config::MAX_DETAIL_DEPTH), t.muted()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), nav);
    // View switch, right-aligned.
    let vl = format!(" [View: {} ▸] ", app.view_mode.label());
    let vw = (unicode_width::UnicodeWidthStr::width(vl.as_str()) as u16).min(nav.width);
    let vr = Rect { x: nav.x + nav.width - vw, width: vw, ..nav };
    f.render_widget(Paragraph::new(Span::styled(vl, t.accent_bold())), vr);
    app.hits.push((vr, Hit::ViewMode));

    if app.view().posts.is_empty() {
        let msg = if in_search { "No posts match this search. Press < to go back." }
            else if app.nicks.is_empty() { "The database is empty. Open Scrape (or Import) in the menu bar to fill it." }
            else { "Click a nick (Space / Ctrl-click to select several) to read their posts and replies." };
        f.render_widget(Paragraph::new(msg).style(t.muted()), Rect { y: content.y + 1, height: content.height.saturating_sub(1), ..content });
        return;
    }
    let open = !app.view().stack.is_empty();
    match app.view_mode {
        ViewMode::Nested => {
            app.page_rows = (content.height / CARD_ROWS).max(1) as usize;
            if open { draw_with_detail(f, app, content, now); } else { draw_cards(f, app, content); }
        }
        ViewMode::Stack => if open { draw_flat(f, app, content); } else { draw_stack(f, app, content); },
        ViewMode::Pane => {
            let list_h = if open { (content.height * 2 / 5).max(4).min(content.height) } else { content.height };
            draw_stack(f, app, Rect { height: list_h, ..content });
            if open { draw_flat(f, app, Rect { y: content.y + list_h, height: content.height - list_h, ..content }); }
        }
        ViewMode::CarouselInfinite | ViewMode::CarouselFixed => if open { draw_flat(f, app, content); } else { draw_carousel(f, app, content); },
        ViewMode::Grid => {
            draw_grid(f, app, content);
            if open { draw_grid_modal(f, app, r); }
        }
    }
}

fn post_text(p: &Post) -> String { if p.title.trim().is_empty() { p.content.clone() } else { format!("{}\n{}", p.title.trim(), p.content) } }

/// One post card in `cr` (border with meta line and star, wrapped text, scrolled by `skip` lines).
fn card(f: &mut Frame, app: &mut App, i: usize, cr: Rect, sel: bool) {
    let t = app.theme;
    let Some(p) = app.view().posts.get(i).cloned() else { return };
    let focused = app.focus == Focus::Body;
    let b = Block::default().borders(Borders::ALL).title(meta_line(&p, &t)).padding(Padding::horizontal(1))
        .border_type(if sel { BorderType::Thick } else { BorderType::Plain })
        .border_style(if sel && focused { t.accent() } else if sel { Style::default() } else { t.border(false) });
    let inner = b.inner(cr);
    f.render_widget(b, cr);
    let lines: Vec<Line> = crate::tui::markdown::render(post_text(&p).trim(), inner.width as usize, &t).into_iter()
        .filter(|l| l.width() > 0).take(inner.height as usize).collect();
    f.render_widget(Paragraph::new(lines), inner);
    app.hits.push((cr, Hit::Post(i)));
    if cr.width > 8 {
        let sr = Rect { x: cr.x + cr.width - 5, y: cr.y, width: 3, height: 1 };
        f.render_widget(Paragraph::new(Span::styled(star(app.is_marked(&p.post_id)), t.accent_bold())), sr);
        app.hits.push((sr, Hit::Star(i)));
    }
    if cr.width > 12 {
        let br = Rect { x: cr.x + cr.width - 8, y: cr.y, width: 3, height: 1 };
        f.render_widget(Paragraph::new(Span::styled(basket_icon(app.in_basket(&p.post_id)), t.accent_bold())), br);
        app.hits.push((br, Hit::Basket(i)));
    }
}

/// Stack: one column of full posts (each card as tall as its text, up to the area).
fn draw_stack(f: &mut Frame, app: &mut App, r: Rect) {
    app.hits.push((r, Hit::ListPane));
    let w = r.width;
    let heights: Vec<u16> = app.view().posts.iter().map(|p| card_height(&post_text(p), w.saturating_sub(2), r.height)).collect();
    let v = app.view_mut();
    if v.cursor < v.offset { v.offset = v.cursor; }
    // Keep the cursor's card fully on screen.
    while v.offset < v.cursor && heights[v.offset..=v.cursor].iter().map(|h| *h as u32).sum::<u32>() > r.height as u32 { v.offset += 1; }
    let (off, cur) = (v.offset, v.cursor);
    let mut y = r.y;
    let mut shown = 0;
    for (i, h) in heights.iter().enumerate().skip(off) {
        if y >= r.y + r.height { break; }
        let h = (*h).min(r.y + r.height - y);
        card(f, app, i, Rect { x: r.x, y, width: w, height: h }, i == cur);
        y += h;
        shown += 1;
    }
    app.page_rows = shown.max(1);
}

/// A detail drawn flat (no nesting): the top frame fills `r`.
fn draw_flat(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    let focused = app.focus == Focus::Body;
    let depth = app.view().stack.len();
    let title = if depth > 1 { format!(" {depth} deep · Esc back ") } else { " Esc closes ".into() };
    let b = Block::default().borders(Borders::ALL).border_type(BorderType::Double).title(title)
        .border_style(if focused { t.accent() } else { t.border(false) });
    let inner = b.inner(r);
    f.render_widget(b, r);
    if inner.width < 3 || inner.height == 0 { return; }
    full_frame(f, app, depth - 1, Rect { x: inner.x + 1, width: inner.width - 1, ..inner }, true);
}

/// Carousel: the current post in the middle, its neighbours peeking at the sides.
fn draw_carousel(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    app.hits.push((r, Hit::ListPane));
    app.page_rows = 1;
    let n = app.view().posts.len();
    let cur = app.view().cursor.min(n - 1);
    let c = carousel(r);
    let wrapv = app.view_mode == ViewMode::CarouselInfinite;
    let prev = if cur > 0 { Some(cur - 1) } else if wrapv && n > 1 { Some(n - 1) } else { None };
    let next = if cur + 1 < n { Some(cur + 1) } else if wrapv && n > 1 { Some(0) } else { None };
    for (side, idx, arrow) in [(c.prev, prev, "◂"), (c.next, next, "▸")] {
        let b = Block::default().borders(Borders::ALL).border_style(t.border(false));
        let inner = b.inner(side);
        f.render_widget(b, side);
        if let Some(i) = idx {
            let p = &app.view().posts[i];
            let mut lines = vec![Line::styled(arrow, t.accent_bold()), Line::styled(ellipsize(&format!("@{}", p.nick), inner.width as usize), t.muted())];
            lines.extend(wrap(&snippet(p), inner.width as usize).into_iter().take(inner.height.saturating_sub(2) as usize).map(|l| Line::styled(l, t.muted())));
            f.render_widget(Paragraph::new(lines), inner);
            app.hits.push((side, Hit::Post(i)));
        }
    }
    card(f, app, cur, c.center, true);
    let pos = format!(" {}/{n}{} ", cur + 1, if wrapv { " ∞" } else { "" });
    let pr = Rect { x: c.center.x + 2, y: c.center.y + c.center.height.saturating_sub(1), width: (pos.chars().count() as u16).min(c.center.width.saturating_sub(4)), height: 1 };
    f.render_widget(Paragraph::new(Span::styled(pos, t.muted())), pr);
}

/// Grid of cards; scrolls by rows to keep the cursor visible.
fn draw_grid(f: &mut Frame, app: &mut App, r: Rect) {
    app.hits.push((r, Hit::ListPane));
    let n = app.view().posts.len();
    let (cols, _) = grid_cells(r, n, 0);
    let rows = grid_rows(r);
    app.grid_cols = cols;
    app.page_rows = rows * cols;
    let v = app.view_mut();
    let crow = v.cursor / cols;
    let mut orow = v.offset / cols;
    if crow < orow { orow = crow; }
    if crow >= orow + rows { orow = crow + 1 - rows; }
    v.offset = orow * cols;
    let (first, cur) = (v.offset, v.cursor);
    let (_, cells) = grid_cells(r, n, first);
    for (i, cr) in cells { card(f, app, i, cr, i == cur); }
}

/// Grid: the open post as a modal over the Body, with a Close button.
fn draw_grid_modal(f: &mut Frame, app: &mut App, body: Rect) {
    let t = app.theme;
    let mr = modal_rect(body, body.width.saturating_sub(6).max(40), body.height.saturating_sub(2));
    f.render_widget(Clear, mr);
    app.hits.push((mr, Hit::DetailPane));
    let depth = app.view().stack.len();
    let b = Block::default().borders(Borders::ALL).border_type(BorderType::Double).border_style(t.accent())
        .title(if depth > 1 { format!(" Post · {depth} deep · Esc back ") } else { " Post ".into() });
    let inner = b.inner(mr);
    f.render_widget(b, mr);
    if inner.height < 3 || inner.width < 12 { return; }
    let close = " [ Close ] ";
    let cw = close.chars().count() as u16;
    let cr = Rect { x: inner.x + inner.width.saturating_sub(cw), y: inner.y + inner.height - 1, width: cw.min(inner.width), height: 1 };
    full_frame(f, app, depth - 1, Rect { x: inner.x + 1, width: inner.width - 1, height: inner.height - 1, ..inner }, true);
    f.render_widget(Paragraph::new(Span::styled(close, t.selected())), cr);
    app.hits.push((cr, Hit::CloseDetail));
}

fn draw_cards(f: &mut Frame, app: &mut App, r: Rect) {
    app.hits.push((r, Hit::ListPane));
    let rows = app.page_rows;
    let v = app.view_mut();
    if v.cursor < v.offset { v.offset = v.cursor; }
    if v.cursor >= v.offset + rows { v.offset = v.cursor + 1 - rows; }
    let (off, cur, n) = (v.offset, v.cursor, v.posts.len());
    for (k, i) in (off..n).take(rows).enumerate() {
        let cr = Rect { x: r.x, y: r.y + k as u16 * CARD_ROWS, width: r.width, height: CARD_ROWS.min(r.height.saturating_sub(k as u16 * CARD_ROWS)) };
        card(f, app, i, cr, i == cur);
    }
}

/// Columns before the file name on an attachment row ("▣ Open mime  ").
fn lines_label_width(atts: &[crate::prelude::Attachment], id: &str) -> usize {
    let mime = atts.iter().find(|a| a.local_path.display().to_string() == id).map(|a| a.mime_type.as_str()).unwrap_or("file");
    unicode_width::UnicodeWidthStr::width(format!("▣ Open {mime}  ").as_str())
}

/// Bookmark star: `★` bookmarked, `☆` not (click toggles).
pub fn star(on: bool) -> &'static str { if on { " ★ " } else { " ☆ " } }
/// The export-basket button: `+` adds the post, `✓` means it's in (click again to take it out).
pub fn basket_icon(on: bool) -> &'static str { if on { " ✓ " } else { " + " } }

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
        full_frame(f, app, d, fr, d + 1 == depth);
    }
}

/// Draw the pictures whose reserved rows are fully on screen; a picture cut by
/// the frame edge shows a one-line hint instead (graphics can't be clipped).
fn draw_images(f: &mut Frame, app: &mut App, slots: &[(usize, std::path::PathBuf)], rows: u16, scroll: u16, fr: Rect) {
    let t = app.theme;
    // Graphics go on top of everything, so under a modal or an open menu draw half blocks.
    let graphics = app.modal.is_none() && app.menu.open.is_none();
    let Some(im) = app.images.as_mut() else { return };
    for (li, path) in slots {
        let y = *li as i64 - scroll as i64;
        let end = y + rows as i64;
        if end <= 0 || y >= fr.height as i64 { continue; }
        let full = y >= 0 && end <= fr.height as i64;
        let ry = fr.y + y.max(0) as u16;
        let hint = |f: &mut Frame, text: String| {
            f.render_widget(Paragraph::new(Span::styled(format!("  {text}"), t.muted())), Rect { x: fr.x, y: ry, width: fr.width, height: 1 });
        };
        if !full { hint(f, "▣ picture — scroll to show it".into()); continue; }
        let area = Rect { x: fr.x, y: ry, width: fr.width.saturating_sub(1), height: rows };
        if let Err(e) = im.draw_file(f.buffer_mut(), area, path, graphics) { hint(f, e); }
    }
}

/// Draw frame `d` of the detail stack in full into `fr`; the top one gets its link, star and wheel hits.
fn full_frame(f: &mut Frame, app: &mut App, d: usize, fr: Rect, top: bool) {
    let t = app.theme;
    // One more column of padding on each side of the text.
    let fr = if fr.width > 6 { Rect { x: fr.x + 1, width: fr.width - 2, ..fr } } else { fr };
    let w = fr.width.saturating_sub(1) as usize;
    let h = fr.height as usize;
    let img_rows = if app.images.is_some() { crate::tui::images::IMAGE_ROWS.min(fr.height.saturating_sub(2)) } else { 0 };
    let (lines, links, slots) = detail_layout(&app.view().stack[d], w, &t, img_rows);
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
    draw_images(f, app, &slots, img_rows, scroll, fr);
    if !top { app.hits.push((fr, Hit::Frame(d))); return; }
    app.hits.push((fr, Hit::DetailPane));
    for (li, k) in links {
        let y = li as i64 - scroll as i64;
        if y >= 0 && (y as usize) < h {
            app.hits.push((Rect { x: fr.x, y: fr.y + y as u16, width: fr.width, height: 1 }, Hit::Link(k)));
            // Attachment rows also become OSC 8 hyperlinks (file://) where the terminal supports them.
            let l = &app.view().stack[d].links[k];
            if l.kind == crate::tui::state::LinkKind::Attachment && std::path::Path::new(&l.id).exists() {
                let label_w = lines_label_width(&app.view().stack[d].attachments, &l.id);
                let name = std::path::Path::new(&l.id).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let x = fr.x + label_w as u16;
                if x < fr.x + fr.width {
                    let shown = ellipsize(&name, (fr.x + fr.width - x) as usize);
                    app.osc_links.push((x, fr.y + y as u16, shown, format!("file://{}", l.id)));
                }
            }
        }
    }
    if fr.width > 6 {
        let marked = app.is_marked(&app.view().stack[d].post.post_id);
        let sr = Rect { x: fr.x + fr.width - 4, y: fr.y, width: 3, height: 1 };
        f.render_widget(Paragraph::new(Span::styled(star(marked), t.accent_bold())), sr);
        app.hits.push((sr, Hit::StarTop));
    }
    if fr.width > 10 {
        let inb = app.in_basket(&app.view().stack[d].post.post_id);
        let br = Rect { x: fr.x + fr.width - 7, y: fr.y, width: 3, height: 1 };
        f.render_widget(Paragraph::new(Span::styled(basket_icon(inb), t.accent_bold())), br);
        app.hits.push((br, Hit::BasketTop));
    }
}
