//! Menu bar (commands from the CLI flags, Start Scanning pinned right) and its
//! dropdown of options with checkbox / dropdown / text field / button inputs.

use crate::tui::commands::Widget;
use crate::tui::state::{App, Focus, Hit};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use crate::tui::theme::frame as boxed;
use ratatui::widgets::{Clear, Paragraph};

/// Label of the pinned scan button.
pub fn scan_label(app: &App) -> &'static str { if app.job.is_some() { " ■ Cancel " } else { " ▶ Start Scanning " } }

/// The scan button on a narrow menu bar.
pub fn short_scan_label(app: &App) -> &'static str { if app.job.is_some() { " ■ Cancel " } else { " ▶ Scan " } }

/// Draw the menu bar into `r` (one row); the dropdown is drawn by [`draw_dropdown`].
pub fn draw(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    let mut x = r.x;
    // Wide: " Label " │ " Label "; narrow: drop the separators, then the padding,
    // then shorten the scan button so every command (Quit included) still shows.
    let labels: u16 = app.menu.menu.items.iter().map(|c| c.label.chars().count() as u16).sum();
    let n = app.menu.menu.items.len() as u16;
    let full = scan_label(app);
    let scan = if labels + 2 * n + full.chars().count() as u16 + 1 > r.width { short_scan_label(app) } else { full };
    let scan_w = scan.chars().count() as u16;
    let limit = r.x + r.width.saturating_sub(scan_w + 1);
    let room = limit.saturating_sub(r.x);
    let (pad, sep) = if labels + 3 * n <= room { (1, true) } else if labels + 2 * n <= room { (1, false) } else { (0, false) };
    for (i, c) in app.menu.menu.items.iter().enumerate() {
        let label = c.label;
        let w = label.chars().count() as u16 + 2 * pad + u16::from(pad == 0);
        if x + w > limit { break; }
        let active = app.menu.open == Some(i) || (app.focus == Focus::Menu && app.menu.item == i);
        let base = if active { t.selected() } else { t.plain() };
        let hint = label.chars().position(|ch| ch.to_ascii_lowercase() == c.hint).unwrap_or(0);
        let mut spans = vec![Span::styled(if pad == 1 { " " } else { "" }, &base)];
        for (k, ch) in label.chars().enumerate() {
            let st = if k == hint { base.clone().underline(true) } else { base.clone() };
            spans.push(Span::styled(ch.to_string(), st));
        }
        spans.push(Span::styled(" ", &base));
        let rr = Rect { x, y: r.y, width: w, height: 1 };
        f.render_widget(Paragraph::new(Line::from(spans)), rr);
        app.hits.push((rr, Hit::MenuItem(i)));
        x += w;
        if sep && x < limit { f.render_widget(Paragraph::new(Span::styled("│", t.muted())), Rect { x, y: r.y, width: 1, height: 1 }); x += 1; }
    }
    let sr = Rect { x: r.x + r.width.saturating_sub(scan_w), y: r.y, width: scan_w.min(r.width), height: 1 };
    let st = t.button();
    f.render_widget(Paragraph::new(Span::styled(scan, st)), sr);
    app.hits.push((sr, Hit::StartScan));
}

/// Widget text right of an option label.
pub fn widget_text(w: &Widget, editing: bool) -> String {
    match w {
        Widget::Check(_) => String::new(),
        Widget::Choice { choices, idx } => format!("‹ {} ›", choices[*idx]),
        Widget::Text(tf) => {
            let mut s = tf.display();
            if editing { let b = s.char_indices().nth(tf.cursor).map(|(b, _)| b).unwrap_or(s.len()); s.insert(b, '▏'); }
            format!("[{s:<12}]")
        }
        Widget::Button(_) => String::new(),
    }
}

/// Draw the open dropdown under its item (on top of the layout).
pub fn draw_dropdown(f: &mut Frame, app: &mut App, screen: Rect) {
    let Some(i) = app.menu.open else { return };
    let t = app.theme;
    let x = app.hits.iter().find(|(_, h)| *h == Hit::MenuItem(i)).map(|(r, _)| r.x).unwrap_or(screen.x);
    let c = &app.menu.menu.items[i];
    let rows: Vec<(Line, bool)> = c.opts.iter().enumerate().map(|(k, o)| {
        let cur = k == app.menu.opt;
        let editing = cur && app.menu.editing;
        let req = if o.required { "*" } else { " " };
        let base = if cur { t.selected() } else { t.plain() };
        let line = match &o.widget {
            Widget::Check(v) => Line::from(vec![Span::styled(req, t.accent()), Span::styled(format!(" [{}] {}", if *v { "x" } else { " " }, o.label), &base)]),
            Widget::Button(_) => Line::from(vec![Span::styled(req, t.accent()), Span::styled(format!("  < {} >", o.label), if cur { base.clone() } else { t.accent_bold() })]),
            w => Line::from(vec![Span::styled(req, t.accent()), Span::styled(format!("     {} ", o.label), &base),
                Span::styled(widget_text(w, editing), if editing { t.accent_bold() } else { base.clone() })]),
        };
        (line, cur)
    }).collect();
    let hint = c.opts.get(app.menu.opt).map(crate::tui::commands::Opt::hint_line).unwrap_or_default();
    let w = rows.iter().map(|(l, _)| l.width()).max().unwrap_or(10).max(hint.chars().count() + 2).max(c.label.len() + 4) as u16 + 4;
    let h = rows.len() as u16 + 3;
    let w = w.min(screen.width);
    let x = x.min(screen.x + screen.width - w);
    let r = Rect { x, y: screen.y + 2, width: w, height: h.min(screen.height.saturating_sub(2)) };
    f.render_widget(Clear, r);
    let b = boxed(&t.dialog()).title(format!(" {} ", c.label));
    let inner = b.inner(r);
    f.render_widget(b, r);
    for (k, (line, _)) in rows.into_iter().enumerate() {
        if k as u16 >= inner.height { break; }
        let rr = Rect { x: inner.x, y: inner.y + k as u16, width: inner.width, height: 1 };
        f.render_widget(Paragraph::new(line), rr);
        app.hits.push((rr, Hit::MenuOpt(k)));
    }
    if inner.height as usize > c.opts.len() {
        f.render_widget(Paragraph::new(Span::styled(format!(" {hint}"), t.muted())),
            Rect { x: inner.x, y: inner.y + c.opts.len() as u16, width: inner.width, height: 1 });
    }
}
