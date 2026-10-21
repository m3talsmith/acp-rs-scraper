//! Drawing: every widget renders from [`App`] and records its clickable areas
//! in `App::hits` (so mouse input hits exactly what was drawn).

pub mod actions;
pub mod body;
pub mod detail;
pub mod editor;
pub mod menu;
pub mod modal;
pub mod nicks;
pub mod text;

use crate::branding::{slogan_in_status, slogan_style, SLOGAN, SLOGAN_COLS};
use crate::prelude::*;
use crate::tui::state::{status_line_is_problem, App, Focus, Hit, SessionState};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::time::Instant;

/// Draw the whole screen.
pub fn draw(f: &mut Frame, app: &mut App, now: Instant) {
    app.refresh_job_modal_at(now);
    app.hits.clear();
    app.osc_links.clear();
    if let Some(im) = app.images.as_mut() { im.frame.clear(); }
    let area = f.area();
    if area.width < TUI_MIN_SIZE.0 || area.height < TUI_MIN_SIZE.1 {
        f.render_widget(Paragraph::new(format!("Terminal too small: {}×{} (need {}×{}). q quits.",
            area.width, area.height, TUI_MIN_SIZE.0, TUI_MIN_SIZE.1)), area);
        if app.egg.is_some() { draw_egg(f, app, area, now); }
        return;
    }
    // The whole screen in the theme's base colours (the terminal's when unset).
    f.buffer_mut().set_style(area, ratatui::style::Style::from(&app.theme.base()));
    if let Some(mut ed) = app.theme_editor.take() {
        editor::draw(f, app, &mut ed, now);
        app.theme_editor = Some(ed);
        if app.modal.is_some() { modal::draw(f, app, area); }
        crate::tui::tooltip::paint(f, app, area);
        if app.egg.is_some() { draw_egg(f, app, area, now); }
        return;
    }
    let log_rows = if app.show_log { 10 } else { 1 };
    let a = crate::tui::layout::areas_open(area, log_rows, app.nicks_open, app.actions_open);
    topbar(f, app, a.top);
    menu::draw(f, app, a.menu);
    nicks::draw(f, app, a.nicks);
    body::draw(f, app, a.body, now);
    actions::draw(f, app, a.actions);
    status(f, app, a.status);
    draw_toasts(f, app, area);
    menu::draw_dropdown(f, app, area);
    if app.modal.is_some() { modal::draw(f, app, area); }
    crate::tui::tooltip::paint(f, app, area);
    if app.egg.is_some() { draw_egg(f, app, area, now); }
}

/// A short full-screen clip. A small area is one asterisk (and the burst's line).
fn draw_egg(f: &mut Frame, app: &App, area: Rect, now: Instant) {
    let Some(play) = app.egg else { return };
    let lines = crate::eggs::frame_lines(play.kind, now.saturating_duration_since(play.start), area.width, area.height);
    let st = if play.plain { ratatui::style::Style::default() } else { ratatui::style::Style::from(&app.theme.accent()) };
    f.render_widget(ratatui::widgets::Clear, area);
    let block_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    let x0 = area.x + area.width.saturating_sub(block_w.min(area.width)) / 2;
    let y0 = area.y + area.height.saturating_sub(lines.len() as u16) / 2;
    let buf = f.buffer_mut();
    for (i, line) in lines.iter().enumerate() {
        let y = y0 + i as u16;
        if y >= area.y + area.height { break; }
        let mut x = x0;
        for ch in line.chars() {
            if x >= area.x + area.width { break; }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(&ch.to_string());
                cell.set_style(st);
            }
            x += 1;
        }
    }
}

/// Stacked toasts in the lower right, just above the status line. A click dismisses one.
fn draw_toasts(f: &mut Frame, app: &mut App, area: Rect) {
    if app.modal.is_some() || app.notices.toasts.is_empty() || area.width < 8 || area.height < 4 { return; }
    let t = app.theme;
    let rows: Vec<(String, String)> = app.notices.toasts.iter()
        .map(|toast| (toast.notice.title.clone(), toast.notice.body.clone())).collect();
    let n = rows.len();
    let h = 2u16;
    let w = 42.min(area.width.saturating_sub(2));
    let status_h = if app.show_log { 10 } else { 1 };
    let top = area.y + area.height.saturating_sub(status_h + n as u16 * h);
    let x = area.x + area.width.saturating_sub(w);
    for (i, (title, body)) in rows.iter().enumerate() {
        let r = Rect { x, y: top + i as u16 * h, width: w, height: h };
        let shown: String = body.chars().take(w.saturating_sub(2) as usize).collect();
        f.render_widget(Paragraph::new(vec![
            Line::from(Span::styled(format!("▸ {title}"), t.bar())),
            Line::from(Span::styled(format!("  {shown}"), t.plain())),
        ]), r);
        app.hits.push((r, Hit::Toast(i)));
    }
}

/// The header: the red star logo, a big "ACP", the title and the session, all
/// vertically centred in [`TUI_HEADER_ROWS`] rows.
fn topbar(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    let bar = t.bar();
    f.render_widget(Paragraph::new("").style(&bar), r);
    // The big "ACP" is 3 rows: in the 4-row header it gets the spare row above it,
    // and the title / session line sits on its middle row.
    let big_top = r.y + r.height.saturating_sub(3).div_ceil(2);
    let mid = Rect { x: r.x, y: (big_top + 1).min(r.y + r.height.saturating_sub(1)), width: r.width, height: 1.min(r.height) };
    // Logo: square on screen (cells are about twice as tall as wide), the full
    // header height, in on-accent over the red bar. Without terminal graphics it
    // is a sextant silhouette of the same PNG.
    let logo = Rect { x: r.x + 1, y: r.y, width: (r.height * 2).min(r.width.saturating_sub(1)), height: r.height };
    let drawn = app.images.as_mut().is_some_and(|im| im.draw_logo(f.buffer_mut(), logo));
    if !drawn { crate::tui::images::draw_logo_text(f.buffer_mut(), logo, t.on_accent, t.accent); }
    app.hits.push((logo, Hit::Logo));
    let mut x = logo.x + logo.width + 1;
    let big_w = 3 * 8;
    if r.height >= 3 && x + big_w + 2 < r.width {
        let big = tui_big_text::BigText::builder()
            .pixel_size(tui_big_text::PixelSize::ThirdHeight)
            .style(t.bar().bold(true))
            .lines(vec![Line::from("ACP")])
            .build();
        f.render_widget(big, Rect { x, y: big_top, width: big_w, height: 3 });
        x += big_w + 2;
    } else {
        f.render_widget(Paragraph::new(Span::styled("ACP", t.bar().bold(true))), Rect { x, ..mid });
        x += 5;
    }
    let left = if app.node_chip.is_empty() {
        format!("{}  v{}", TUI_SUBTITLE, env!("CARGO_PKG_VERSION"))
    } else {
        format!("{}  v{}  {}", TUI_SUBTITLE, env!("CARGO_PKG_VERSION"), app.node_chip)
    };
    let session = match app.session {
        SessionState::LoggedOut => "○ logged out",
        SessionState::Saved => "◐ session saved",
        SessionState::Valid => "● logged in",
        SessionState::Checking => "… checking session",
    };
    let job_body = app.header_running().unwrap_or_default();
    let job = if job_body.is_empty() { String::new() } else { format!("{job_body}  ") };
    // The theme switch: ‹name› ▾; a click opens the picker.
    // `‹default: missing› ▾` / `‹mine: broken› ▾` when the file can't be used.
    let fault = app.theme_fault.or(app.theme_error.is_some().then_some("broken"));
    let switch = match (app.theme_downloading.is_some(), fault) {
        (true, _) => format!("‹{} · downloading…› ▾", app.theme_name),
        (false, Some(f)) => format!("‹{}: {f}› ▾", app.theme_name),
        (false, None) => format!("‹{}› ▾", app.theme_name),
    };
    let right = format!("  {session} ");
    let rest = Rect { x, width: r.width.saturating_sub(x - r.x), ..mid };
    let used = left.chars().count() + job.chars().count() + switch.chars().count() + right.chars().count();
    let pad = (rest.width as usize).saturating_sub(used);
    let sw_x = rest.x + (left.chars().count() + pad + job.chars().count()) as u16;
    if (used as u16) < rest.width {
        if !job_body.is_empty() {
            let job_x = rest.x + (left.chars().count() + pad) as u16;
            let w = job_body.chars().count() as u16;
            if w > 0 { app.hits.push((Rect { x: job_x, y: rest.y, width: w, height: 1 }, Hit::Job)); }
        }
        app.hits.push((Rect { x: sw_x, y: rest.y, width: switch.chars().count() as u16, height: 1 }, Hit::ThemeSwitch));
    }
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(left, t.bar().bold(true)), Span::styled(" ".repeat(pad), &bar), Span::styled(job, &bar),
        Span::styled(switch, t.bar().underline(true)), Span::styled(right, &bar),
    ])), rest);
    if r.width > 2 && r.height > 0 {
        let mark = Rect { x: r.x + r.width - 2, y: r.y, width: 1, height: 1 };
        f.render_widget(Paragraph::new(Span::styled("?", t.bar().bold(true))), mark);
        app.hits.push((mark, Hit::HelpMark));
    }
}

fn status(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    let focused = app.focus == Focus::Status;
    let n = r.height as usize;
    let quiet = if focused { t.muted().underline(true) } else { t.muted() };
    let lines: Vec<Line> = if app.status.is_empty() {
        vec![Line::styled("? help · Tab focus · Space multi-select · Shift-S search · Ctrl-Space commands · F6 scan · F5 theme · q quit", quiet)]
    } else {
        app.status.iter().rev().take(n).rev().map(|l| {
            let st = if status_line_is_problem(l) { t.accent() } else { quiet.clone() };
            Line::styled(l.clone(), st)
        }).collect()
    };
    let bottom = (r.height as usize).saturating_sub(1);
    let bottom_cols = lines.get(bottom).map(line_cols).unwrap_or(0);
    let show_slogan = r.height > 0 && slogan_in_status(r.width, bottom_cols);
    f.render_widget(Paragraph::new(lines), r);
    if show_slogan {
        let y = r.y + r.height - 1;
        let x = r.x + r.width - SLOGAN_COLS;
        f.render_widget(
            Paragraph::new(Span::styled(SLOGAN, slogan_style(app.color_mode, app.no_color, true))),
            Rect { x, y, width: SLOGAN_COLS, height: 1 },
        );
    }
    app.hits.push((r, Hit::Status));
}

fn line_cols(line: &Line) -> u16 {
    line.width().min(u16::MAX as usize) as u16
}
