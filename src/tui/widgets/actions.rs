//! The right-hand action history. Collapsed, it is one column.
//!
//! The list is not capped. A branch line shows which path is on screen.

use crate::tui::state::{App, Focus, Hit};
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
    let steps_n = app.action_steps().len();
    let b = crate::tui::theme::frame(&t.pane(app.focus == Focus::Actions)).title(" Actions ");
    let inner = b.inner(r);
    f.render_widget(b, r);
    let tab = Rect { x: r.x, y: r.y, width: 1, height: 1 };
    f.render_widget(Paragraph::new(Span::styled("›", t.bar())), tab);
    app.hits.push((tab, Hit::ToggleActions));
    if inner.height == 0 || inner.width == 0 { return; }
    let list_h = inner.height.saturating_sub(1);
    app.action_page = (list_h as usize).max(1);
    let max_scroll = steps_n.saturating_sub(app.action_page.min(list_h as usize).max(1));
    if app.action_scroll > max_scroll { app.action_scroll = max_scroll; }
    let head = Rect { height: 1, ..inner };
    let here = app.branch_at + 1;
    let many = app.branches.len().max(1);
    let page = list_h as usize;
    let line = if steps_n > page && page > 0 {
        let start = app.action_scroll + 1;
        let end = (app.action_scroll + page).min(steps_n);
        format!(" {here}/{many}  {start}-{end}/{steps_n}")
    } else {
        format!(" {here}/{many}")
    };
    let w = inner.width as usize;
    let line: String = line.chars().take(w).collect();
    f.render_widget(Paragraph::new(Span::styled(line, t.accent_bold())), head);
    app.hits.push((head, Hit::ActionBranches));
    if steps_n == 0 {
        if list_h > 0 {
            let rr = Rect { y: inner.y + 1, height: 1, ..inner };
            f.render_widget(Paragraph::new(Span::styled(" (none)", t.muted())), rr);
            app.hits.push((rr, Hit::ActionPane));
        }
        return;
    }
    let show_bar = steps_n > page && page > 0 && inner.width > 2;
    let text_w = if show_bar { inner.width - 1 } else { inner.width };
    let steps: Vec<_> = app.action_steps().iter().map(|s| s.label.clone()).collect();
    for row in 0..list_h {
        let i = app.action_scroll + row as usize;
        let rr = Rect { y: inner.y + 1 + row, height: 1, width: text_w, ..inner };
        if i >= steps.len() {
            app.hits.push((Rect { y: rr.y, height: list_h - row, ..inner }, Hit::ActionPane));
            break;
        }
        let current = i == app.action_at;
        let curs = app.focus == Focus::Actions && i == app.action_cursor;
        let mark = if current { "● " } else { "  " };
        let st = if curs || current { t.selected() } else { t.plain() };
        let text = format!("{mark}{}", steps[i]);
        let text: String = text.chars().take(text_w as usize).collect();
        f.render_widget(Paragraph::new(Span::styled(text, st)), rr);
        app.hits.push((rr, Hit::Action(i)));
        if show_bar {
            let bar = Rect { x: inner.x + text_w, y: rr.y, width: 1, height: 1 };
            let ch = if thumb(row as usize, page, steps_n, app.action_scroll) { "█" } else { "│" };
            f.render_widget(Paragraph::new(Span::styled(ch, t.muted())), bar);
            app.hits.push((bar, Hit::ActionPane));
        }
    }
}

fn thumb(row: usize, track: usize, n: usize, scroll: usize) -> bool {
    if track == 0 || n <= track { return false; }
    let thumb_h = (track * track / n).max(1).min(track);
    let max_scroll = n - track;
    let top = scroll * (track - thumb_h) / max_scroll;
    row >= top && row < top + thumb_h
}
