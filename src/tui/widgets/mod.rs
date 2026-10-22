//! Drawing: every widget renders from [`App`] and records its clickable areas
//! in `App::hits` (so mouse input hits exactly what was drawn).

pub mod actions;
pub mod body;
pub mod notices;
pub mod detail;
pub mod editor;
pub mod menu;
pub mod modal;
pub mod nicks;
pub mod text;

use crate::branding::slogan_style;
use crate::prelude::*;
use crate::tui::state::{status_line_is_problem, App, Focus, Hit, SessionState};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::time::Instant;

/// The text of one frame. A scenario reads the theme editor's preview from this.
pub fn screen_text(app: &mut App, width: u16, height: u16) -> String {
    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).expect("test backend");
    term.draw(|f| draw(f, app, Instant::now())).expect("draw");
    let buf = term.backend().buffer().clone();
    (0..height)
        .map(|y| (0..width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

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
    if let Some(mut ed) = app.brand_editor.take() {
        ed.draw(f, app);
        app.brand_editor = Some(ed);
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
    if app.notice_page.is_some() {
        notices::draw(f, app, a.body);
    } else {
        body::draw(f, app, a.body, now);
    }
    actions::draw(f, app, a.actions);
    status(f, app, a.status);
    if app.notice_page.is_none() { draw_toasts(f, app, area); }
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
///
/// There is no cap. When the stack would be taller than a quarter of the screen,
/// as many as fit are shown and View all opens the notifications page.
fn draw_toasts(f: &mut Frame, app: &mut App, area: Rect) {
    if app.modal.is_some() || app.notices.toasts.is_empty() || area.width < 8 || area.height < 4 { return; }
    let t = app.theme;
    let n = app.notices.toasts.len();
    let (shown, view_all) = crate::notify::toast_window(n, area.height);
    if shown == 0 && !view_all { return; }
    let h = crate::notify::TOAST_ROWS;
    let w = 42.min(area.width.saturating_sub(2));
    let status_h = if app.show_log { 10 } else { 1 };
    let button_h = if view_all { crate::notify::VIEW_ALL_ROWS } else { 0 };
    let stack_h = shown as u16 * h + button_h;
    let top = area.y + area.height.saturating_sub(status_h + stack_h);
    let x = area.x + area.width.saturating_sub(w);
    let start = n - shown;
    for (row, idx) in (start..n).enumerate() {
        let title = app.notices.toasts[idx].notice.title.clone();
        let body = app.notices.toasts[idx].notice.body.clone();
        let r = Rect { x, y: top + row as u16 * h, width: w, height: h };
        let shown_body: String = body.chars().take(w.saturating_sub(2) as usize).collect();
        f.render_widget(Paragraph::new(vec![
            Line::from(Span::styled(format!("▸ {title}"), t.bar())),
            Line::from(Span::styled(format!("  {shown_body}"), t.plain())),
        ]), r);
        app.hits.push((r, Hit::Toast(idx)));
    }
    if view_all {
        let r = Rect { x, y: top + shown as u16 * h, width: w, height: button_h };
        f.render_widget(Paragraph::new(Span::styled(" View all", t.selected())), r);
        app.hits.push((r, Hit::ViewAllNotices));
    }
}

/// The header: the brand logo, then the title and the session. No word is drawn beside the logo.
pub(crate) fn topbar(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    let bar = t.bar();
    f.render_widget(Paragraph::new("").style(&bar), r);
    // Title and session sit on the middle row of the 4-row header.
    let big_top = r.y + r.height.saturating_sub(3).div_ceil(2);
    let mid = Rect { x: r.x, y: (big_top + 1).min(r.y + r.height.saturating_sub(1)), width: r.width, height: 1.min(r.height) };
    // Logo: square on screen (cells are about twice as tall as wide), the full
    // header height. A theme with `mark` and `mark_end` paints the halberd as a
    // left-to-right gradient; otherwise it uses the bar colours.
    let logo = Rect { x: r.x + 1, y: r.y, width: (r.height * 2).min(r.width.saturating_sub(1)), height: r.height };
    draw_brand_logo(f, app, logo);
    app.hits.push((logo, Hit::Logo));
    let x = logo.x + logo.width + 1;
    let title = app.brand.title.clone();
    let left = if app.node_chip.is_empty() {
        format!("{title}  v{}", env!("CARGO_PKG_VERSION"))
    } else {
        format!("{title}  v{}  {}", env!("CARGO_PKG_VERSION"), app.node_chip)
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
    // A leading space keeps the version from sitting against ‹name› when the row is full.
    let switch = match (app.theme_downloading.is_some(), fault) {
        (true, _) => format!(" ‹{} · downloading…› ▾", app.theme_name),
        (false, Some(f)) => format!(" ‹{}: {f}› ▾", app.theme_name),
        (false, None) => format!(" ‹{}› ▾", app.theme_name),
    };
    let right = format!("  {session} ");
    let rest = Rect { x, width: r.width.saturating_sub(x - r.x), ..mid };
    let left_n = left.chars().count();
    let job_n = job.chars().count();
    let sw_n = switch.chars().count();
    let used = left_n + job_n + sw_n + right.chars().count();
    let pad = (rest.width as usize).saturating_sub(used);
    let job_x = rest.x + (left_n + pad) as u16;
    let sw_x = rest.x + (left_n + pad + job_n) as u16;
    // A mark that is fully on the row stays clickable when the session is clipped.
    let on_row = |at: u16, w: usize| {
        let w = w as u16;
        w > 0 && at >= rest.x && at.saturating_add(w) <= rest.x.saturating_add(rest.width)
    };
    if !job_body.is_empty() && on_row(job_x, job_body.chars().count()) {
        app.hits.push((Rect { x: job_x, y: rest.y, width: job_body.chars().count() as u16, height: 1 }, Hit::Job));
    }
    if on_row(sw_x, sw_n) {
        app.hits.push((Rect { x: sw_x, y: rest.y, width: sw_n as u16, height: 1 }, Hit::ThemeSwitch));
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
            let st = if status_line_is_problem(l) { t.problem() } else { quiet.clone() };
            Line::styled(l.clone(), st)
        }).collect()
    };
    let bottom = (r.height as usize).saturating_sub(1);
    let bottom_cols = lines.get(bottom).map(line_cols).unwrap_or(0);
    let motto = app.brand.motto.clone();
    let motto_cols = crate::branding::motto_cols(&motto);
    let show_slogan = r.height > 0 && crate::branding::motto_in_status(&motto, r.width, bottom_cols);
    f.render_widget(Paragraph::new(lines), r);
    if show_slogan {
        let y = r.y + r.height - 1;
        let x = r.x + r.width - motto_cols;
        f.render_widget(
            Paragraph::new(Span::styled(motto, slogan_style(app.color_mode, app.no_color, true))),
            Rect { x, y, width: motto_cols, height: 1 },
        );
    }
    app.hits.push((r, Hit::Status));
}

fn draw_brand_logo(f: &mut Frame, app: &mut App, logo: Rect) {
    use crate::brands::LogoKind;
    let t = app.theme;
    match app.brand.logo {
        LogoKind::Halberd => {
            let bar = ratatui::style::Style::from(t.bar());
            if let Some((from, to)) = mark_gradient(&t) {
                let bg = bar.bg.unwrap_or(t.bg);
                crate::tui::images::draw_halberd_gradient(f.buffer_mut(), logo, from, to, bg);
            } else {
                crate::tui::images::draw_halberd(f.buffer_mut(), logo, bar);
            }
        }
        LogoKind::Builtin => {
            let drawn = app.images.as_mut().is_some_and(|im| im.draw_logo(f.buffer_mut(), logo));
            if !drawn { crate::tui::images::draw_logo_text(f.buffer_mut(), logo, t.on_accent, t.accent); }
        }
        LogoKind::Text => paint_logo_text(f, logo, &app.brand.logo_text, t.on_accent, t.accent),
        LogoKind::File => {
            let path = app.brand.logo_path.clone();
            let mut drew = false;
            if let Some(path) = path.as_ref() && let Some(im) = app.images.as_mut() {
                match im.draw_file(f.buffer_mut(), logo, path, true) {
                    Ok(()) => drew = true,
                    Err(e) if e.starts_with('…') => drew = true,
                    Err(_) => {}
                }
            }
            if !drew {
                if let Some(path) = path.as_ref() {
                    drew = crate::tui::images::draw_halfblocks_file(f.buffer_mut(), logo, path);
                }
                if !drew { paint_logo_text(f, logo, "★", t.on_accent, t.accent); }
            }
        }
    }
}

fn paint_logo_text(f: &mut Frame, area: Rect, text: &str, fg: ratatui::style::Color, bg: ratatui::style::Color) {
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    let lines: Vec<&str> = if lines.is_empty() { vec!["★"] } else { lines };
    let h = lines.len().min(area.height as usize);
    let y0 = area.y + area.height.saturating_sub(h as u16) / 2;
    let style = ratatui::style::Style::default().fg(fg).bg(bg);
    for (i, line) in lines.iter().take(h).enumerate() {
        let w = unicode_width::UnicodeWidthStr::width(*line).min(area.width as usize) as u16;
        let x = area.x + area.width.saturating_sub(w) / 2;
        let shown: String = line.chars().take(area.width as usize).collect();
        f.render_widget(
            Paragraph::new(Span::styled(shown, style)),
            Rect { x, y: y0 + i as u16, width: w.max(1), height: 1 },
        );
    }
}

/// `mark` and `mark_end` when the theme sets both. Other themes keep the bar colours.
fn mark_gradient(t: &crate::tui::theme::Theme) -> Option<(ratatui::style::Color, ratatui::style::Color)> {
    use ratatui::style::Color;
    (t.mark != Color::Reset && t.mark_end != Color::Reset).then_some((t.mark, t.mark_end))
}

fn line_cols(line: &Line) -> u16 {
    line.width().min(u16::MAX as usize) as u16
}
