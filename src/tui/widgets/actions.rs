//! The right-hand action history. Collapsed, it is one column.

use crate::tui::state::{App, Hit};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

/// Draw the action bar into `r`.
pub fn draw(f: &mut Frame, app: &mut App, r: Rect) {
    if r.width == 0 || r.height == 0 { return; }
    let t = app.theme;
    if !app.actions_open || r.width <= 2 {
        f.render_widget(Paragraph::new(Span::styled("‹", t.bar().bold(true))), r);
        app.hits.push((r, Hit::ToggleActions));
        return;
    }
    let b = crate::tui::theme::frame(&t.pane(false)).title(" Actions ");
    let inner = b.inner(r);
    f.render_widget(b, r);
    let tab = Rect { x: r.x, y: r.y, width: 1, height: 1 };
    f.render_widget(Paragraph::new(Span::styled("›", t.bar())), tab);
    app.hits.push((tab, Hit::ToggleActions));
    if app.actions.is_empty() {
        if inner.height > 0 {
            f.render_widget(Paragraph::new(Span::styled(" (none)", t.muted())), Rect { height: 1, ..inner });
        }
        return;
    }
    for (i, snap) in app.actions.iter().enumerate() {
        if i as u16 >= inner.height { break; }
        let rr = Rect { y: inner.y + i as u16, height: 1, ..inner };
        let mark = if i == app.action_at { "● " } else { "  " };
        let st = if i == app.action_at { t.selected() } else { t.plain() };
        let w = inner.width as usize;
        let text = format!("{mark}{}", snap.label);
        let text: String = text.chars().take(w).collect();
        f.render_widget(Paragraph::new(Span::styled(text, st)), rr);
        app.hits.push((rr, Hit::Action(i)));
    }
}
