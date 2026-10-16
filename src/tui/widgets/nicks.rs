//! Left nick bar: `@nick` and post count; multi-select.

use crate::tui::state::{App, Focus, Hit};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Draw the nick bar into `r`.
pub fn draw(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    let title = if app.selected.len() > 1 { format!(" Nicks · {} selected ", app.selected.len()) } else { format!(" Nicks ({}) ", app.nicks.len()) };
    let b = Block::default().borders(Borders::ALL).title(title).border_style(t.border(app.focus == Focus::Nicks));
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
    if app.nicks.is_empty() {
        f.render_widget(Paragraph::new(vec![Line::raw("No nicks yet —"), Line::raw("use Scrape or Import.")]).style(t.muted()), inner);
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
        let mut st = if sel { t.accent_bold() } else { Style::default() };
        if cur && app.focus == Focus::Nicks { st = t.selected(); } else if cur { st = st.add_modifier(Modifier::REVERSED); }
        let line = Line::from(vec![Span::styled(mark, t.accent()), Span::styled(format!("{name}{}", " ".repeat(pad)), st),
            Span::styled(format!(" {count}"), if cur && app.focus == Focus::Nicks { st } else { t.muted() })]);
        f.render_widget(Paragraph::new(line), rr);
        app.hits.push((rr, Hit::Nick(i)));
    }
}
