//! Drawing: every widget renders from [`App`] and records its clickable areas
//! in `App::hits` (so mouse input hits exactly what was drawn).

pub mod body;
pub mod detail;
pub mod menu;
pub mod modal;
pub mod nicks;
pub mod text;

use crate::prelude::*;
use crate::tui::layout::areas;
use crate::tui::state::{App, SessionState};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::time::Instant;

/// Draw the whole screen.
pub fn draw(f: &mut Frame, app: &mut App, now: Instant) {
    app.hits.clear();
    app.osc_links.clear();
    let area = f.area();
    if area.width < TUI_MIN_SIZE.0 || area.height < TUI_MIN_SIZE.1 {
        f.render_widget(Paragraph::new(format!("Terminal too small: {}×{} (need {}×{}). q quits.",
            area.width, area.height, TUI_MIN_SIZE.0, TUI_MIN_SIZE.1)), area);
        return;
    }
    let log_rows = if app.show_log { 10 } else { 1 };
    let a = areas(area, log_rows);
    topbar(f, app, a.top);
    menu::draw(f, app, a.menu);
    nicks::draw(f, app, a.nicks);
    body::draw(f, app, a.body, now);
    status(f, app, a.status);
    menu::draw_dropdown(f, app, area);
    if app.modal.is_some() { modal::draw(f, app, area); }
}

fn topbar(f: &mut Frame, app: &App, r: Rect) {
    let t = app.theme;
    let bar = Style::default().bg(t.accent).fg(t.on_accent);
    let left = format!(" ★ {TUI_TITLE}  v{} ", env!("CARGO_PKG_VERSION"));
    let session = match app.session {
        SessionState::LoggedOut => "○ logged out",
        SessionState::Saved => "◐ session saved",
        SessionState::Valid => "● logged in",
        SessionState::Checking => "… checking session",
    };
    let job = app.job.as_ref().map(|j| format!("⟳ {} running (pid {})  ", j.kind.name(), j.pid)).unwrap_or_default();
    let right = format!("{job}{session} ");
    let pad = (r.width as usize).saturating_sub(left.chars().count() + right.chars().count());
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(left, bar.add_modifier(Modifier::BOLD)), Span::styled(" ".repeat(pad), bar), Span::styled(right, bar),
    ])), r);
}

fn status(f: &mut Frame, app: &App, r: Rect) {
    let t = app.theme;
    let n = r.height as usize;
    let lines: Vec<Line> = if app.status.is_empty() {
        vec![Line::styled("? help · Tab focus · Space multi-select · Shift-S search · Ctrl-Space commands · F5 scan · q quit", t.muted())]
    } else {
        app.status.iter().rev().take(n).rev().map(|l| {
            let st = if l.starts_with("error") || l.starts_with("warning") { t.accent() } else { t.muted() };
            Line::styled(l.clone(), st)
        }).collect()
    };
    f.render_widget(Paragraph::new(lines), r);
}
