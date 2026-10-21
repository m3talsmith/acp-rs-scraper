//! The notifications page, drawn in the body. It is not an action-history step.

use crate::notify::NoticeKind;
use crate::tui::state::{App, Focus, Hit};
use crate::tui::theme::frame as boxed;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

/// Draw the notifications page into the body.
pub fn draw(f: &mut Frame, app: &mut App, r: Rect) {
    if r.width == 0 || r.height == 0 { return; }
    let t = app.theme;
    let b = boxed(&t.pane(app.focus == Focus::Body)).title(" Notifications ");
    let inner = b.inner(r);
    f.render_widget(b, r);
    if inner.width == 0 || inner.height < 4 { return; }
    let rows = app.visible_notices();
    let Some(mut page) = app.notice_page.clone() else { return; };
    let list_h = (inner.height as usize).saturating_sub(4);
    if rows.is_empty() { page.cursor = 0; page.scroll = 0; }
    else if page.cursor >= rows.len() { page.cursor = rows.len() - 1; }
    if list_h == 0 || page.cursor < page.scroll {
        page.scroll = page.cursor;
    } else if page.cursor >= page.scroll + list_h {
        page.scroll = page.cursor + 1 - list_h;
    }
    if let Some(p) = &mut app.notice_page { *p = page.clone(); }

    let w = inner.width as usize;
    let clear_all = "Clear all";
    let clear_w = clear_all.chars().count() as u16;
    let head = Rect { height: 1, ..inner };
    f.render_widget(Paragraph::new(Span::styled(" saved notices", t.muted())), head);
    if inner.width > clear_w {
        let rr = Rect { x: inner.x + inner.width - clear_w, width: clear_w, ..head };
        f.render_widget(Paragraph::new(Span::styled(clear_all, t.selected())), rr);
        app.hits.push((rr, Hit::NoticeClearAll));
    }

    let search_y = Rect { y: inner.y + 1, height: 1, ..inner };
    let q = if page.editing { format!("{}▌", page.query) } else { page.query.clone() };
    let search = format!("Search: {q}");
    f.render_widget(Paragraph::new(Span::styled(clip(&search, w), if page.editing { t.selected() } else { t.plain() })), search_y);
    app.hits.push((search_y, Hit::NoticeSearch));

    let controls = Rect { y: inner.y + 2, height: 1, ..inner };
    let order = format!("Order: {}", if page.newest_first { "newest" } else { "oldest" });
    let kind = format!("Kind: {}", page.kind_label());
    let dnd_mark = if app.notices.settings.dnd { "x" } else { " " };
    let dnd = format!("[{dnd_mark}] Do not disturb");
    let mut x = controls.x;
    for (text, hit) in [(order, Hit::NoticeOrder), (kind, Hit::NoticeKind), (dnd, Hit::NoticeSwitch(0))] {
        let width = text.chars().count() as u16;
        if x >= controls.x + controls.width { break; }
        let ww = width.min(controls.x + controls.width - x);
        let rr = Rect { x, width: ww, ..controls };
        f.render_widget(Paragraph::new(Span::styled(text, t.plain())), rr);
        app.hits.push((rr, hit));
        x = x.saturating_add(width).saturating_add(2);
    }

    let switches = Rect { y: inner.y + 3, height: 1, ..inner };
    x = switches.x;
    let label = "Toast ";
    f.render_widget(Paragraph::new(Span::styled(label, t.muted())), Rect { width: label.len() as u16, ..switches });
    x += label.len() as u16;
    for (i, kind) in NoticeKind::ALL.into_iter().enumerate() {
        let mark = if app.notices.settings.on[kind.index()] { "x" } else { " " };
        let text = format!("[{mark}] {} ", kind.label());
        let width = text.chars().count() as u16;
        if x >= switches.x + switches.width { break; }
        let ww = width.min(switches.x + switches.width - x);
        let rr = Rect { x, width: ww, ..switches };
        f.render_widget(Paragraph::new(Span::styled(text, t.plain())), rr);
        app.hits.push((rr, Hit::NoticeSwitch(i + 1)));
        x += width;
    }

    let list_top = inner.y + 4;
    if rows.is_empty() {
        let rr = Rect { y: list_top, height: 1, ..inner };
        f.render_widget(Paragraph::new(Span::styled(" (none)", t.muted())), rr);
        return;
    }
    for (row, n) in rows.iter().enumerate().skip(page.scroll).take(list_h) {
        let y = list_top + (row - page.scroll) as u16;
        let rr = Rect { y, height: 1, ..inner };
        let clear = " clear";
        let clear_n = clear.chars().count();
        let text_w = w.saturating_sub(clear_n);
        let line = format!("{}: {}", n.kind.label(), n.body);
        let st = if row == page.cursor { t.selected() } else { t.plain() };
        f.render_widget(Paragraph::new(Span::styled(clip(&line, text_w), st)), Rect { width: text_w as u16, ..rr });
        app.hits.push((Rect { width: text_w as u16, ..rr }, Hit::NoticeRow(row)));
        if text_w < w {
            let cr = Rect { x: rr.x + text_w as u16, width: clear_n as u16, ..rr };
            f.render_widget(Paragraph::new(Span::styled(clear, t.accent())), cr);
            app.hits.push((cr, Hit::NoticeClear(row)));
        }
    }
}

fn clip(s: &str, width: usize) -> String {
    s.chars().take(width).collect()
}
