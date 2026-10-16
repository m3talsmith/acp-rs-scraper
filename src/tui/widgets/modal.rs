//! Modal rendering: dims the layout, then draws the modal with a small margin.

use crate::tui::layout::{modal_rect, wrap};
use crate::tui::modals::*;
use crate::tui::state::{App, Hit};
use crate::tui::theme::Theme;
use crate::tui::widgets::text::TextField;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

fn dim(f: &mut Frame, area: Rect) {
    let buf = f.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(c) = buf.cell_mut((x, y)) { c.set_style(Style::default().add_modifier(Modifier::DIM)); }
        }
    }
}

fn frame(f: &mut Frame, t: &Theme, r: Rect, title: &str) -> Rect {
    f.render_widget(Clear, r);
    let b = Block::default().borders(Borders::ALL).border_type(BorderType::Double).border_style(t.accent()).title(format!(" {title} "));
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
    Line::from(vec![Span::styled(format!("{label:>18}: "), if focused { t.accent_bold() } else { Style::default() }),
        Span::styled(format!("{v:<30}"), Style::default().add_modifier(Modifier::UNDERLINED))])
}

/// Draw the open modal.
pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    dim(f, area);
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
            let st = if mm.error { t.accent() } else { Style::default() };
            f.render_widget(Paragraph::new(text.into_iter().map(|l| Line::styled(l, st)).collect::<Vec<_>>()),
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
            for (i, label) in SEARCH_FIELDS.iter().enumerate() {
                let rr = Rect { y: inner.y + 1 + i as u16, height: 1, ..inner };
                let line = if i < nf - 1 { field_line(label, &s.fields[i], s.focus == i, &t) } else {
                    let k = match s.kind { crate::tui::data::KindFilter::Any => "Posts and replies", crate::tui::data::KindFilter::Posts => "Posts only", crate::tui::data::KindFilter::Replies => "Replies only" };
                    Line::from(vec![Span::styled(format!("{label:>18}: "), if s.focus == i { t.accent_bold() } else { Style::default() }), Span::raw(format!("‹ {k} ›"))])
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
        Modal::Palette(p) => {
            let r = modal_rect(area, 70, area.height.saturating_sub(4));
            let inner = frame(f, &t, r, "Commands · type to filter · Enter runs");
            f.render_widget(Paragraph::new(field_line("›", &p.query, true, &t)), Rect { height: 1, ..inner });
            let list = Rect { y: inner.y + 2, height: inner.height.saturating_sub(2), ..inner };
            let items = p.filtered();
            let skip = p.cursor.saturating_sub(list.height.saturating_sub(1) as usize);
            for (row, (i, (label, _))) in items.iter().enumerate().skip(skip).take(list.height as usize).enumerate() {
                let rr = Rect { y: list.y + row as u16, height: 1, ..list };
                let st = if i == p.cursor { t.selected() } else { Style::default() };
                f.render_widget(Paragraph::new(Span::styled(format!(" {label}"), st)), rr);
                app.hits.push((rr, Hit::Modal(ModalHit::Item(i))));
            }
        }
    }
    app.modal = Some(m);
}
