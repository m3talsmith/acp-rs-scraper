//! Modal rendering: dims the layout, then draws the modal with a small margin.

use crate::tui::layout::{modal_rect, wrap};
use crate::tui::modals::*;
use crate::tui::state::{App, Hit};
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

/// Draw the open modal.
pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = app.theme;
    dim(f, &theme, area);
    app.hits.push((area, Hit::Backdrop));
    let t = app.theme;
    let Some(m) = app.modal.take() else { return };
    match &m {
        Modal::Help(h) => {
            let r = modal_rect(area, area.width, area.height);
            let inner = frame(f, &t, r, "Help · ↑↓ PgUp PgDn scroll · Esc closes");
            let body = Rect { height: inner.height.saturating_sub(1), ..inner };
            let lines: Vec<Line> = h.lines.iter().skip(h.scroll).take(body.height as usize).map(|l| Line::raw(l.clone())).collect();
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
            let mut lines = vec![Line::raw("")];
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
                    lines.push(Line::styled("Paste the code (or the whole login link) from the email; Enter verifies.", t.muted()));
                }
                LoginStep::Busy(s) => lines.push(Line::styled(format!("⟳ {s}"), t.accent())),
            }
            if let Some(e) = &l.error { lines.push(Line::raw("")); lines.push(Line::styled(format!("✗ {e}"), t.accent_bold())); }
            lines.push(Line::raw(""));
            lines.push(Line::styled("The session is saved in ~/.acp/session.json (mode 0600); browsing works offline without it.", t.muted()));
            f.render_widget(Paragraph::new(lines), Rect { height: inner.height.saturating_sub(2), ..inner });
            let submit = if matches!(l.step, LoginStep::Code) { "Verify" } else { "Send code" };
            buttons(f, app, Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner }, &[submit.into(), "Cancel".into()], None);
        }
        Modal::Search(s) => {
            let r = modal_rect(area, 72, SEARCH_FIELDS.len() as u16 + 8);
            let inner = frame(f, &t, r, "Search · Tab moves · Enter searches");
            let nf = SEARCH_FIELDS.len();
            let kind_i = nf - 2;
            for (i, label) in SEARCH_FIELDS.iter().enumerate() {
                let rr = Rect { y: inner.y + 1 + i as u16, height: 1, ..inner };
                let line = if i < kind_i { field_line(label, &s.fields[i], s.focus == i, &t) } else if i == kind_i {
                    let k = match s.kind { crate::tui::data::KindFilter::Any => "Posts and replies", crate::tui::data::KindFilter::Posts => "Posts only", crate::tui::data::KindFilter::Replies => "Replies only" };
                    Line::from(vec![Span::styled(format!("{label:>18}: "), if s.focus == i { t.accent_bold() } else { t.plain() }), Span::raw(format!("‹ {k} ›"))])
                } else {
                    let w = if s.site { "Site" } else { "Local" };
                    Line::from(vec![Span::styled(format!("{label:>18}: "), if s.focus == i { t.accent_bold() } else { t.plain() }), Span::raw(format!("‹ {w} ›"))])
                };
                f.render_widget(Paragraph::new(line), rr);
                app.hits.push((rr, Hit::Modal(ModalHit::Field(i))));
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
            let rows = p.rows() as u16;
            let r = modal_rect(area, 76, (rows + 4).min(area.height.saturating_sub(2)));
            let inner = frame(f, &t, r, &p.title());
            let list = Rect { y: inner.y + 1, height: inner.height.saturating_sub(1), ..inner };
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
