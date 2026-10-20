//! Modal rendering: dims the layout, then draws the modal with a small margin.

use crate::branding::{slogan_style, SLOGAN, SLOGAN_COLS};
use crate::cli::colors_suppressed;
use crate::config::TUI_SUBTITLE;
use crate::tui::layout::{modal_rect, wrap};
use crate::tui::modals::*;
use crate::tui::state::{status_line_is_problem, App, Hit};
use crate::tui::theme::Theme;
use crate::tui::widgets::text::TextField;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use crate::tui::theme::frame as boxed;
use ratatui::widgets::{Clear, Paragraph};

fn dim(f: &mut Frame, t: &Theme, area: Rect) {
    let st = ratatui::style::Style::from(t.dimmed());
    let buf = f.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(c) = buf.cell_mut((x, y)) { c.set_style(st); }
        }
    }
}

fn frame(f: &mut Frame, t: &Theme, r: Rect, title: &str) -> Rect {
    f.render_widget(Clear, r);
    let b = boxed(&t.dialog()).title(format!(" {title} "));
    let inner = b.inner(r);
    f.render_widget(b, r);
    inner
}

fn buttons(f: &mut Frame, app: &mut App, r: Rect, labels: &[String], focus: Option<usize>) {
    let t = app.theme;
    let mut x = r.x;
    for (i, l) in labels.iter().enumerate() {
        let s = format!("[ {l} ]");
        let w = s.chars().count() as u16;
        if x + w > r.x + r.width { break; }
        let st = if focus == Some(i) { t.selected() } else { t.accent_bold() };
        let br = Rect { x, y: r.y, width: w, height: 1 };
        f.render_widget(Paragraph::new(Span::styled(s, st)), br);
        app.hits.push((br, Hit::Modal(ModalHit::Button(i))));
        x += w + 2;
    }
}

fn field_line(label: &str, tf: &TextField, focused: bool, t: &Theme) -> Line<'static> {
    let mut v = tf.display();
    if focused { let b = v.char_indices().nth(tf.cursor).map(|(b, _)| b).unwrap_or(v.len()); v.insert(b, '▏'); }
    Line::from(vec![Span::styled(format!("{label:>18}: "), if focused { t.accent_bold() } else { t.plain() }),
        Span::styled(format!("{v:<30}"), t.field())])
}

fn clip(s: &str, w: usize) -> String {
    let w = w.max(1);
    if s.chars().count() <= w { return s.to_string(); }
    s.chars().take(w.saturating_sub(1)).chain(['…']).collect()
}

fn draw_job(f: &mut Frame, app: &mut App, area: Rect, m: &JobModal, t: &Theme) {
    let many = m.rows.len() > 1;
    let list_rows = if many { (m.rows.len() as u16).min(6) } else { 0 };
    let r = modal_rect(area, 88, (list_rows + 16).max(12));
    let title = if many { "Jobs · ↑↓ select · j/k log · Esc closes" } else { "Job · j/k PgUp PgDn scroll · Esc closes" };
    let inner = frame(f, t, r, title);
    if inner.height < 2 || inner.width < 4 { return; }
    let row = |y: u16| Rect { y, height: 1, ..inner };
    let mut y = inner.y;
    if many {
        let skip = m.cursor.saturating_sub(list_rows.saturating_sub(1) as usize);
        for (k, (i, job)) in m.rows.iter().enumerate().skip(skip).take(list_rows as usize).enumerate() {
            let rr = row(y + k as u16);
            let mark = if i == m.cursor { "▸" } else { " " };
            let text = format!(" {mark} {} · {} · {} · pid {}", job.name, job.source, job.state, job.pid);
            let st = if i == m.cursor { t.selected() } else { t.plain() };
            f.render_widget(Paragraph::new(Span::styled(clip(&text, inner.width as usize), st)), rr);
            app.hits.push((rr, Hit::Modal(ModalHit::Item(i))));
        }
        y += list_rows;
    }
    let Some(job) = m.rows.get(m.cursor) else { return };
    let w = inner.width as usize;
    let line = |text: String, st: ratatui::style::Style| Line::styled(clip(&text, w), st);
    let stage = if job.running { format!("{} {}", job.spinner, job.stage) } else { job.stage.clone() };
    let mut lines = vec![
        line(format!("Job: {}", job.name), t.plain().into()),
        line(format!("Started by: {}", job.source), t.plain().into()),
        line(format!("State: {}", job.state), if job.running { t.accent().into() } else { t.plain().into() }),
        line(format!("Started: {}", job.started_at), t.plain().into()),
        line(format!("Elapsed: {}", job.elapsed), t.plain().into()),
        line(format!("Stage: {stage}"), t.plain().into()),
        line(format!("Pid: {}", job.pid), t.muted().into()),
    ];
    if !job.running { lines.push(line(format!("Exit: {}", job.exit), t.plain().into())); }
    if !job.error.is_empty() { lines.push(line(format!("Error: {}", job.error), t.accent().into())); }
    if !job.cmd.is_empty() { lines.push(line(format!("Command: {}", job.cmd), t.muted().into())); }
    lines.push(line("Log:".into(), t.bold().into()));
    let used = lines.len() as u16;
    let btn_y = inner.y + inner.height.saturating_sub(1);
    let body_h = btn_y.saturating_sub(y);
    let head_h = used.min(body_h);
    f.render_widget(Paragraph::new(lines), Rect { y, height: head_h, ..inner });
    let log_top = y + head_h;
    let log_h = btn_y.saturating_sub(log_top) as usize;
    let max_scroll = job.tail.len().saturating_sub(log_h);
    let scroll = m.scroll.min(max_scroll);
    for (k, line) in job.tail.iter().skip(scroll).take(log_h).enumerate() {
        let rr = row(log_top + k as u16);
        let st = if status_line_is_problem(line) { t.accent() } else { t.muted() };
        f.render_widget(Paragraph::new(Span::styled(clip(line, inner.width as usize), st)), rr);
    }
    let labels: Vec<String> = if job.running { vec!["Cancel".into(), "Close".into()] } else { vec!["Close".into()] };
    buttons(f, app, Rect { y: btn_y, height: 1, ..inner }, &labels, None);
}

/// Draw the open modal.
fn draw_compose(f: &mut Frame, app: &mut App, area: Rect, m: &ComposeModal, t: &Theme) {
    let pick_n = m.site_pick.as_ref().map(|p| p.len()).unwrap_or(0) as u16;
    let extra = if pick_n > 0 { pick_n.saturating_sub(2) + 2 } else { 0 };
    let r = modal_rect(area, 72, 16 + extra);
    let inner = frame(f, t, r, &m.title());
    if inner.height < 4 || inner.width < 8 { return; }
    let row = |y: u16| Rect { y, height: 1, ..inner };
    let mut y = inner.y;
    let btn_y = inner.y + inner.height.saturating_sub(1);
    if matches!(m.kind, ComposeKind::NewPost) {
        let st = if m.focus == ComposeFocus::Title { t.accent_bold() } else { t.plain() };
        f.render_widget(Paragraph::new(Span::styled(clip(&format!(" Title: {}", m.title), inner.width as usize), st)), row(y));
        app.hits.push((row(y), Hit::Modal(ModalHit::Field(0))));
        y += 1;
    }
    let st = if m.focus == ComposeFocus::Body { t.accent_bold() } else { t.plain() };
    let body = m.body.replace('\n', "↵");
    f.render_widget(Paragraph::new(Span::styled(clip(&format!(" Body: {body}"), inner.width as usize), st)), row(y));
    app.hits.push((row(y), Hit::Modal(ModalHit::Field(1))));
    y += 1;
    let st = if m.focus == ComposeFocus::Attach { t.accent_bold() } else { t.plain() };
    let shown = if m.attach_path.is_empty() { "path · Tab completes · Enter adds".into() } else { m.attach_path.clone() };
    f.render_widget(Paragraph::new(Span::styled(clip(&format!(" Attach: {shown}"), inner.width as usize), st)), row(y));
    app.hits.push((row(y), Hit::Modal(ModalHit::Field(2))));
    y += 1;
    if let Some(pick) = &m.site_pick {
        let max = crate::config::SITE_MAX_ATTACHMENTS;
        let n = pick.iter().filter(|on| **on).count();
        for (i, on) in pick.iter().enumerate() {
            if y >= btn_y { break; }
            let path = m.attachments.get(i);
            let name = path.and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let eligible = path.is_some_and(|p| crate::config::site_accepts_path(p));
            let mark = if *on { "[x]" } else { "[ ]" };
            let line = if eligible {
                format!(" {mark} {} {name}", i + 1)
            } else {
                format!(" {mark} {} {name} — {}", i + 1, crate::config::site_ineligible_reason())
            };
            let focused = m.focus == ComposeFocus::SitePick && i == m.site_at;
            let st = if !eligible { t.muted() } else if focused { t.selected() } else { t.plain() };
            f.render_widget(Paragraph::new(Span::styled(clip(&line, inner.width as usize), st)), row(y));
            app.hits.push((row(y), Hit::Modal(ModalHit::Item(i))));
            y += 1;
        }
        if y < btn_y {
            f.render_widget(Paragraph::new(Span::styled(clip(&format!(" {n}/{max} selected for Red Star"), inner.width as usize), t.accent())), row(y));
            y += 1;
        }
        if y < btn_y {
            let tip = format!("At most {max} images or videos. Others stay local.  ? site-attachments");
            f.render_widget(Paragraph::new(Span::styled(clip(&tip, inner.width as usize), t.muted())), row(y));
            y += 1;
        }
    } else {
        for path in m.attachments.iter().take(2) {
            if y >= btn_y { break; }
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            f.render_widget(Paragraph::new(Span::styled(clip(&format!("  {name}"), inner.width as usize), t.muted())), row(y));
            y += 1;
        }
    }
    if let Some(q) = &m.quote {
        f.render_widget(Paragraph::new(Span::styled(clip(&format!(" Quote: {q}"), inner.width as usize), t.muted())), row(y));
        y += 1;
    }
    if m.focus == ComposeFocus::Attach && !m.path_matches.is_empty() {
        let start = m.path_at.saturating_sub(3);
        for (i, text) in m.path_matches.iter().enumerate().skip(start).take(4) {
            let st = if i == m.path_at { t.selected() } else { t.plain() };
            f.render_widget(Paragraph::new(Span::styled(clip(&format!("  {text}"), inner.width as usize), st)), row(y));
            app.hits.push((row(y), Hit::Modal(ModalHit::Item(i))));
            y += 1;
        }
    } else if m.site_pick.is_none() {
        let matches = m.pending.as_ref().map(|q| crate::site::completions(q, &m.nicks, &m.tags)).unwrap_or_default();
        for (i, row_c) in matches.iter().take(4).enumerate() {
            let st = if m.focus == ComposeFocus::Complete && i == m.complete_at { t.selected() } else { t.plain() };
            f.render_widget(Paragraph::new(Span::styled(clip(&format!("  {}", row_c.label), inner.width as usize), st)), row(y));
            app.hits.push((row(y), Hit::Modal(ModalHit::Item(i))));
            y += 1;
        }
    }
    let note = m.error.clone().unwrap_or_else(|| "Attach lists local files. Publish does not upload them.".into());
    if y < btn_y {
        f.render_widget(Paragraph::new(Span::styled(clip(&note, inner.width as usize), if m.error.is_some() { t.accent() } else { t.muted() })), row(y));
    }
    let focus = match m.focus {
        ComposeFocus::Cancel => Some(0),
        ComposeFocus::Publish => Some(1),
        _ => None,
    };
    buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &["Cancel".into(), "Publish".into()], focus);
}

fn draw_about(f: &mut Frame, app: &mut App, area: Rect, m: &AboutModal, t: &Theme) {
    let lines = m.body_lines();
    let h = area.height.saturating_sub(2).max(8);
    let w = (area.width.saturating_sub(4)).max(76).max(SLOGAN_COLS.saturating_add(8));
    let r = modal_rect(area, w, h);
    f.render_widget(Clear, r);
    let title = if m.credits {
        Line::from(format!(" {} ", m.title()))
    } else {
        Line::from(Span::styled(format!(" {} ", m.title()), slogan_style(app.color_mode, app.no_color, false)))
    };
    let b = boxed(&t.dialog()).title(title);
    let inner = b.inner(r);
    f.render_widget(b, r);
    let body_h = inner.height.saturating_sub(1);
    let start = m.scroll.min(lines.len().saturating_sub(body_h as usize));
    for (i, line) in lines.iter().skip(start).take(body_h as usize).enumerate() {
        let rr = Rect { y: inner.y + i as u16, height: 1, ..inner };
        let star = !m.credits && line.contains('★');
        let st = if star && !colors_suppressed(app.color_mode, app.no_color) { t.accent_bold() } else { t.plain() };
        f.render_widget(Paragraph::new(Span::styled(line.clone(), st)), rr);
        if star { app.hits.push((rr, Hit::Modal(ModalHit::Item(0)))); }
    }
    buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &["Close".into()], Some(0));
}

/// Draw the open modal over `area`, or nothing when none is open.
pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = app.theme;
    dim(f, &theme, area);
    app.hits.push((area, Hit::Backdrop));
    let t = app.theme;
    let Some(m) = app.modal.take() else { return };
    match &m {
        Modal::Help(h) => {
            let r = modal_rect(area, area.width, area.height);
            let inner = frame(f, &t, r, "Help · type to filter · ← → topics · Esc closes");
            let body = Rect { height: inner.height.saturating_sub(1), ..inner };
            let shown = h.view_lines();
            let lines: Vec<Line> = shown.iter().skip(h.scroll).take(body.height as usize).map(|l| Line::raw(l.clone())).collect();
            f.render_widget(Paragraph::new(lines), body);
            buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &["Close".into()], Some(0));
        }
        Modal::Message(mm) => {
            let w = 76;
            let text: Vec<String> = mm.lines.iter().flat_map(|l| wrap(l, w as usize - 4)).collect();
            let r = modal_rect(area, w, text.len() as u16 + 5);
            let inner = frame(f, &t, r, &mm.title);
            let st = if mm.error { t.accent() } else { t.plain() };
            f.render_widget(Paragraph::new(text.into_iter().map(|l| Line::styled(l, &st)).collect::<Vec<_>>()),
                Rect { height: inner.height.saturating_sub(2), ..inner });
            buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &["OK".into()], Some(0));
        }
        Modal::Confirm(c) => {
            let w = 76;
            let text: Vec<String> = c.lines.iter().flat_map(|l| wrap(l, w as usize - 4)).collect();
            let extra = if c.strict { 2 } else { 0 };
            let r = modal_rect(area, w, text.len() as u16 + 5 + extra);
            let inner = frame(f, &t, r, &c.title);
            let mut lines: Vec<Line> = text.into_iter().map(Line::raw).collect();
            if c.strict { lines.push(Line::raw("")); lines.push(field_line("Type yes", &c.typed, true, &t)); }
            if let Some(e) = &c.error { lines.push(Line::styled(e.clone(), t.accent())); }
            f.render_widget(Paragraph::new(lines), Rect { height: inner.height.saturating_sub(2), ..inner });
            buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &c.buttons, Some(c.focus));
        }
        Modal::Login(l) => {
            let r = modal_rect(area, area.width, area.height);
            let inner = frame(f, &t, r, &l.title());
            let mut lines = vec![
                Line::styled(TUI_SUBTITLE, t.bold()),
                Line::styled(SLOGAN, slogan_style(app.color_mode, app.no_color, false)),
                Line::raw(""),
            ];
            if let Some(why) = &l.reason { lines.push(Line::styled(why.clone(), t.accent_bold())); lines.push(Line::raw("")); }
            lines.push(Line::raw("acp.us logs you in with a one-time code sent by email."));
            lines.push(Line::raw(""));
            match &l.step {
                LoginStep::Email => {
                    lines.push(field_line("Email", &l.email, true, &t));
                    lines.push(Line::raw(""));
                    lines.push(Line::styled("Enter sends the code.", t.muted()));
                }
                LoginStep::Code => {
                    lines.push(Line::styled(format!("{:>18}: {}", "Email", l.email.value), t.muted()));
                    lines.push(field_line("Code or login link", &l.code, true, &t));
                    lines.push(Line::raw(""));
                    lines.push(Line::styled("Enter verifies. F2 or Resend code sends another code on this login.  ? login", t.muted()));
                }
                LoginStep::Busy(s) => lines.push(Line::styled(format!("⟳ {s}"), t.accent())),
            }
            if let Some(e) = &l.error { lines.push(Line::raw("")); lines.push(Line::styled(format!("✗ {e}"), t.accent_bold())); }
            lines.push(Line::raw(""));
            lines.push(Line::styled("The session is saved in ~/.acp/session.json (mode 0600); browsing works offline without it.", t.muted()));
            f.render_widget(Paragraph::new(lines), Rect { height: inner.height.saturating_sub(2), ..inner });
            let labels: Vec<String> = if l.on_code_step() {
                vec!["Verify".into(), "Resend code".into(), "Cancel".into()]
            } else {
                vec!["Send code".into(), "Cancel".into()]
            };
            buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &labels, None);
        }
        Modal::Search(s) => {
            let r = modal_rect(area, 76, SEARCH_FIELDS.len() as u16 + 9);
            let inner = frame(f, &t, r, "Search · Tab moves · Enter searches");
            let nf = SEARCH_FIELDS.len();
            for (i, label) in SEARCH_FIELDS.iter().enumerate() {
                let rr = Rect { y: inner.y + 1 + i as u16, height: 1, ..inner };
                let line = if let Some(fi) = SearchModal::text_index(i) {
                    field_line(label, &s.fields[fi], s.focus == i, &t)
                } else if i == SEARCH_KIND {
                    let k = match s.kind { crate::tui::data::KindFilter::Any => "Posts and replies", crate::tui::data::KindFilter::Posts => "Posts only", crate::tui::data::KindFilter::Replies => "Replies only" };
                    Line::from(vec![Span::styled(format!("{label:>18}: "), if s.focus == i { t.accent_bold() } else { t.plain() }), Span::raw(format!("‹ {k} ›"))])
                } else if i == SEARCH_ORDER {
                    Line::from(vec![Span::styled(format!("{label:>18}: "), if s.focus == i { t.accent_bold() } else { t.plain() }), Span::raw(format!("‹ {} ›", s.order))])
                } else {
                    let w = if s.site { "Site" } else { "Local" };
                    Line::from(vec![Span::styled(format!("{label:>18}: "), if s.focus == i { t.accent_bold() } else { t.plain() }), Span::raw(format!("‹ {w} ›"))])
                };
                f.render_widget(Paragraph::new(line), rr);
                app.hits.push((rr, Hit::Modal(ModalHit::Field(i))));
            }
            let tip = match s.focus { SEARCH_ORDER => Some(SEARCH_ORDER_TIP), SEARCH_LIMIT => Some(SEARCH_LIMIT_TIP), _ => None };
            if let Some(tip) = tip {
                f.render_widget(Paragraph::new(Span::styled(tip, t.muted())), Rect { y: inner.y + nf as u16 + 1, height: 1, ..inner });
            }
            if let Some(e) = &s.error {
                f.render_widget(Paragraph::new(Span::styled(e.clone(), t.accent())), Rect { y: inner.y + nf as u16 + 2, height: 1, ..inner });
            }
            let labels: Vec<String> = SEARCH_BUTTONS.iter().map(|b| b.to_string()).collect();
            let focus = s.focus.checked_sub(nf);
            buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &labels, focus);
        }
        Modal::Research(m) => {
            let list_rows = (m.projects.len() as u16).clamp(1, 10);
            let add = m.mode == ResearchMode::Add;
            let h = list_rows + if add { 13 } else { 10 };
            let r = modal_rect(area, 84, h.min(area.height));
            let inner = frame(f, &t, r, &format!("{} · Tab moves · Enter {}", m.title(), if add { "adds" } else { "opens" }));
            let mut y = inner.y + 1;
            let row = |y: u16| Rect { y, height: 1, ..inner };
            let lab = |s: &str, on: bool| Span::styled(format!("{s:>18}: "), if on { t.accent_bold() } else { t.plain() });
            if add {
                let what = m.what.get(m.what_idx).map(|w| w.0.clone()).unwrap_or_else(|| "nothing here".into());
                let n = if m.what.len() > 1 { format!("  ({}/{} · ←/→)", m.what_idx + 1, m.what.len()) } else { String::new() };
                f.render_widget(Paragraph::new(Line::from(vec![lab("Add", m.focus == RField::What), Span::raw(format!("‹ {what} ›")),
                    Span::styled(n, t.muted())])), row(y));
                app.hits.push((row(y), Hit::Modal(ModalHit::Field(0))));
                y += 1;
                f.render_widget(Paragraph::new(field_line("Note", &m.note, m.focus == RField::Note, &t)), row(y));
                app.hits.push((row(y), Hit::Modal(ModalHit::Field(1))));
                y += 2;
            }
            f.render_widget(Paragraph::new(Span::styled(format!("Projects ({})", m.projects.len()),
                if m.focus == RField::List { t.accent_bold() } else { t.bold() })), row(y));
            y += 1;
            if m.projects.is_empty() {
                f.render_widget(Paragraph::new(Span::styled("  none yet — type a name below to create one", t.muted())), row(y));
            }
            let skip = m.sel.saturating_sub(list_rows as usize - 1);
            for (k, (i, (name, n, desc))) in m.projects.iter().enumerate().skip(skip).take(list_rows as usize).enumerate() {
                let rr = row(y + k as u16);
                let sel = i == m.sel;
                let st = if sel && m.focus == RField::List { t.selected() } else if sel { t.accent_bold() } else { t.plain() };
                let text = format!(" {} {name}  ({n} items){}", if sel { "▸" } else { " " }, if desc.is_empty() { String::new() } else { format!(" — {desc}") });
                f.render_widget(Paragraph::new(Span::styled(text, st)), rr);
                app.hits.push((rr, Hit::Modal(ModalHit::Item(i))));
            }
            y += list_rows + 1;
            f.render_widget(Paragraph::new(Span::styled("…or a new project", t.bold())), row(y));
            y += 1;
            f.render_widget(Paragraph::new(field_line("Name", &m.name, m.focus == RField::Name, &t)), row(y));
            app.hits.push((row(y), Hit::Modal(ModalHit::Field(2))));
            y += 1;
            f.render_widget(Paragraph::new(field_line("Description", &m.desc, m.focus == RField::Desc, &t)), row(y));
            app.hits.push((row(y), Hit::Modal(ModalHit::Field(3))));
            y += 1;
            if let Some(e) = &m.error { f.render_widget(Paragraph::new(Span::styled(e.clone(), t.accent())), row(y)); }
            let focus = match m.focus { RField::Button(b) => Some(b), _ => None };
            buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &m.buttons(), focus);
        }
        Modal::ThemePicker(p) => {
            let hint = app.color_hint();
            let extra = u16::from(hint.is_some());
            let rows = p.rows() as u16;
            let r = modal_rect(area, 76, (rows + 4 + extra).min(area.height.saturating_sub(2)));
            let inner = frame(f, &t, r, &p.title());
            let list = Rect { y: inner.y + 1, height: inner.height.saturating_sub(1 + extra), ..inner };
            let skip = p.cursor.saturating_sub(list.height.saturating_sub(1) as usize);
            for (row, i) in (0..p.rows()).skip(skip).take(list.height as usize).enumerate() {
                let rr = Rect { y: list.y + row as u16, height: 1, ..list };
                let sel = i == p.cursor;
                let (text, bad) = match p.entries.get(i) {
                    Some(e) => {
                        let cur = if e.name == p.current { "●" } else { " " };
                        match &e.error {
                            Some(err) => (format!(" {cur} {:<16} ⚠ {err}", e.name), true),
                            None => (format!(" {cur} {:<16} {}", e.name, e.description), false),
                        }
                    }
                    None => (match p.action_row(i) {
                        Some(crate::tui::modals::PickerChoice::Download) => "   Download the default theme".to_string(),
                        Some(crate::tui::modals::PickerChoice::Starter) => "   Create starter theme".to_string(),
                        _ => "   Edit themes…  (Shift-F5)".to_string(),
                    }, false),
                };
                let st = if sel { t.selected() } else if bad { t.muted() } else { t.plain() };
                let w = rr.width as usize;
                let text: String = if text.chars().count() > w { text.chars().take(w.saturating_sub(1)).chain(['…']).collect() } else { text };
                f.render_widget(Paragraph::new(Span::styled(text, st)), rr);
                app.hits.push((rr, Hit::Modal(ModalHit::Item(i))));
            }
            if let Some(h) = hint {
                let rr = Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner };
                f.render_widget(Paragraph::new(Span::styled(format!(" {h}"), t.muted())), rr);
            }
        }
        Modal::Post(p) => {
            let r = modal_rect(area, 42, p.items.len() as u16 + 4);
            let inner = frame(f, &t, r, &format!("Post {} · Enter opens", p.post_id));
            for (i, action) in p.items.iter().enumerate() {
                let rr = Rect { y: inner.y + i as u16, height: 1, ..inner };
                let st = if i == p.cursor { t.selected() } else { t.plain() };
                f.render_widget(Paragraph::new(Span::styled(format!(" {}", action.label()), st)), rr);
                app.hits.push((rr, Hit::Modal(ModalHit::Item(i))));
            }
        }
        Modal::Job(m) => draw_job(f, app, area, m, &t),
        Modal::Compose(m) => draw_compose(f, app, area, m, &t),
        Modal::About(m) => draw_about(f, app, area, m, &t),
        Modal::History(h) => {
            let rows = h.rows.len().max(1) as u16;
            let r = modal_rect(area, 72, rows + 4);
            let inner = frame(f, &t, r, "Search history · Enter re-runs · Esc closes");
            if h.rows.is_empty() {
                f.render_widget(Paragraph::new(Span::styled(" No saved searches.", t.muted())), Rect { height: 1, ..inner });
            }
            for (i, q) in h.rows.iter().enumerate() {
                if i as u16 >= inner.height.saturating_sub(1) { break; }
                let rr = Rect { y: inner.y + i as u16, height: 1, ..inner };
                let st = if i == h.cursor { t.selected() } else { t.plain() };
                f.render_widget(Paragraph::new(Span::styled(format!(" {}", q.label()), st)), rr);
                app.hits.push((rr, Hit::Modal(ModalHit::Item(i))));
            }
            buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &["Close".into()], None);
        }
        Modal::Palette(p) => {
            let r = modal_rect(area, 70, area.height.saturating_sub(4));
            let inner = frame(f, &t, r, "Commands · type to filter · Enter runs");
            f.render_widget(Paragraph::new(field_line("›", &p.query, true, &t)), Rect { height: 1, ..inner });
            let list = Rect { y: inner.y + 2, height: inner.height.saturating_sub(2), ..inner };
            let items = p.filtered();
            let skip = p.cursor.saturating_sub(list.height.saturating_sub(1) as usize);
            for (row, (i, (label, _))) in items.iter().enumerate().skip(skip).take(list.height as usize).enumerate() {
                let rr = Rect { y: list.y + row as u16, height: 1, ..list };
                let st = if i == p.cursor { t.selected() } else { t.plain() };
                f.render_widget(Paragraph::new(Span::styled(format!(" {label}"), st)), rr);
                app.hits.push((rr, Hit::Modal(ModalHit::Item(i))));
            }
        }
    }
    app.modal = Some(m);
}
