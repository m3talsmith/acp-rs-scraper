//! Left nick bar: `@nick` and post count; multi-select.

use crate::tui::state::{App, Focus, Hit};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Draw the nick bar into `r`.
pub fn draw(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    if !app.nicks_open || r.width <= 2 {
        f.render_widget(Paragraph::new(Span::styled("›", t.bar().bold(true))), r);
        app.hits.push((r, Hit::ToggleNicks));
        return;
    }
    let count = if app.nick_filter_active() { format!("{}/{}", app.nicks.len(), app.all_nicks.len()) } else { app.nicks.len().to_string() };
    let window = match (app.nick_order, app.nick_limit) {
        (crate::prelude::ScrapeOrder::Newest, None) => String::new(),
        (order, None) => format!(" · {order}"),
        (order, Some(n)) => format!(" · {order} · {n}"),
    };
    let title = if app.selected.len() > 1 {
        format!(" Nicks ({count}) · {} selected{window} ", app.selected.len())
    } else {
        format!(" Nicks ({count}){window} ")
    };
    let b = crate::tui::theme::frame(&t.pane(app.focus == Focus::Nicks)).title(title);
    let mut inner = b.inner(r);
    f.render_widget(b, r);
    app.hits.push((r, Hit::NickPane));
    // "★ Bookmarks (n)" row on top: opens the bookmarks view.
    if inner.height > 1 {
        let br = Rect { height: 1, ..inner };
        let on = app.search.as_ref().is_some_and(|v| v.bookmarks);
        let st = if on { t.selected() } else { t.accent_bold() };
        f.render_widget(Paragraph::new(Line::from(vec![Span::styled(format!(" ★ Bookmarks ({})", app.marks.len()), st)])), br);
        app.hits.push((br, Hit::Bookmarks));
        inner = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
    }
    // "EB" row under bookmarks: the Executive Board feed.
    if inner.height > 1 {
        let br = Rect { height: 1, ..inner };
        let on = app.search.as_ref().is_some_and(|v| v.site_key.as_deref().is_some_and(|k| k.starts_with("eb")));
        let st = if on { t.selected() } else { t.accent_bold() };
        f.render_widget(Paragraph::new(Line::from(vec![Span::styled(" EB", st)])), br);
        app.hits.push((br, Hit::Eb));
        inner = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
    }
    if inner.height > 1 {
        let br = Rect { height: 1, ..inner };
        let on = app.search.as_ref().is_some_and(|v| v.site_key.as_deref().is_some_and(|k| k.starts_with("explore")));
        let st = if on { t.selected() } else { t.accent_bold() };
        f.render_widget(Paragraph::new(Line::from(vec![Span::styled(" Explore", st)])), br);
        app.hits.push((br, Hit::Explore));
        inner = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
    }
    // Feeds under Explore. The heading opens the editor. Each name opens that feed.
    if inner.height > 1 {
        let br = Rect { height: 1, ..inner };
        let on = app.search.as_ref().is_some_and(|v| v.feed.is_some());
        let st = if on { t.selected() } else { t.accent_bold() };
        f.render_widget(Paragraph::new(Line::from(vec![Span::styled(format!(" Feeds ({})", app.feed_bar.len()), st)])), br);
        app.hits.push((br, Hit::Feeds));
        inner = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
    }
    let shown = app.feed_bar.len().min(6);
    for i in 0..shown {
        if inner.height <= 1 { break; }
        let br = Rect { height: 1, ..inner };
        let row = &app.feed_bar[i];
        let on = app.search.as_ref().and_then(|v| v.feed.as_deref()).is_some_and(|n| n.eq_ignore_ascii_case(&row.name));
        let st = if on { t.selected() } else { t.plain() };
        let label = format!("  {} ({})", row.name, row.unread);
        f.render_widget(Paragraph::new(Span::styled(label, st)), br);
        app.hits.push((br, Hit::Feed(i)));
        inner = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
    }
    // Filter field: "/ text▏  3/390 x".
    if inner.height > 1 && !app.all_nicks.is_empty() {
        let fr = Rect { height: 1, ..inner };
        let active = app.nick_filter_active();
        let tail = if active { format!(" {}/{} x ", app.nicks.len(), app.all_nicks.len()) } else { String::new() };
        let tw = unicode_width::UnicodeWidthStr::width(tail.as_str()) as u16;
        let fw = fr.width.saturating_sub(tw + 3) as usize;
        let mut text = app.nick_filter.display();
        if app.filter_editing { let b = text.char_indices().nth(app.nick_filter.cursor).map(|(b, _)| b).unwrap_or(text.len()); text.insert(b, '▏'); }
        let shown = if text.is_empty() { Span::styled(crate::tui::layout::ellipsize("filter nicks (/)", fw), t.muted()) }
            else { Span::styled(crate::tui::layout::ellipsize(&text, fw), if app.filter_editing { t.selected() } else { t.plain() }) };
        let lead = Span::styled(" / ", if app.filter_editing { t.accent_bold() } else { t.muted() });
        f.render_widget(Paragraph::new(Line::from(vec![lead, shown])), fr);
        app.hits.push((Rect { width: fr.width.saturating_sub(tw), ..fr }, Hit::NickFilter));
        if active {
            let cr = Rect { x: fr.x + fr.width - tw, width: tw, ..fr };
            f.render_widget(Paragraph::new(Span::styled(tail, t.muted())), cr);
            // Only the "x" clears; the count is part of the field.
            app.hits.push((Rect { x: cr.x + tw.saturating_sub(2), width: 2.min(tw), ..cr }, Hit::NickFilterClear));
        }
        inner = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
    }
    if app.nicks.is_empty() {
        let msg = if app.all_nicks.is_empty() { vec![Line::raw("No nicks yet —"), Line::raw("use Scrape or Import.")] }
            else { vec![Line::raw("No nick matches."), Line::raw("Esc or x clears the filter.")] };
        f.render_widget(Paragraph::new(msg).style(t.muted()), inner);
        return;
    }
    let h = inner.height as usize;
    if app.nick_cursor < app.nick_offset { app.nick_offset = app.nick_cursor; }
    if h > 0 && app.nick_cursor >= app.nick_offset + h { app.nick_offset = app.nick_cursor + 1 - h; }
    for (row, i) in (app.nick_offset..app.nicks.len()).take(h).enumerate() {
        let n = &app.nicks[i];
        let sel = app.selected.iter().any(|s| s == &n.nick);
        let cur = i == app.nick_cursor;
        let rr = Rect { x: inner.x, y: inner.y + row as u16, width: inner.width, height: 1 };
        let count = format!("{} ", n.post_count);
        let mark = if sel { "▌" } else { " " };
        let w = (inner.width as usize).saturating_sub(count.len() + 2);
        let name = crate::tui::layout::ellipsize(&format!("@{}", n.nick), w);
        let pad = w.saturating_sub(unicode_width::UnicodeWidthStr::width(name.as_str()));
        let mut st = if sel { t.accent_bold() } else { t.plain() };
        if cur && app.focus == Focus::Nicks { st = t.selected(); } else if cur { st = st.reverse(true); }
        let line = Line::from(vec![Span::styled(mark, t.accent()), Span::styled(format!("{name}{}", " ".repeat(pad)), &st),
            Span::styled(format!(" {count}"), if cur && app.focus == Focus::Nicks { st } else { t.muted() })]);
        f.render_widget(Paragraph::new(line), rr);
        app.hits.push((rr, Hit::Nick(i)));
    }
    if r.width > 2 && r.height > 0 {
        let tab = Rect { x: r.x + r.width - 1, y: r.y, width: 1, height: 1 };
        f.render_widget(Paragraph::new(Span::styled("‹", t.bar())), tab);
        // The ‹ is one cell. A click on the top border beside it, or down the
        // right border, is the same control. Inner rows stay nick hits.
        let top_w = 3.min(r.width);
        app.hits.push((Rect { x: r.x + r.width - top_w, y: r.y, width: top_w, height: 1 }, Hit::ToggleNicks));
        if r.height > 1 {
            app.hits.push((Rect { x: r.x + r.width - 1, y: r.y + 1, width: 1, height: r.height - 1 }, Hit::ToggleNicks));
        }
    }
}
