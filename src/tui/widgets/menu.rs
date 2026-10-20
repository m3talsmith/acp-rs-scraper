//! Menu bar (categories on the left, Scrape and Start Scanning pinned right) and its
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

fn label_width(label: &str, pad: u16) -> u16 {
    label.chars().count() as u16 + 2 * pad + u16::from(pad == 0)
}

fn cluster_width(n: usize, chars: u16, pad: u16, sep: bool) -> u16 {
    if n == 0 { return 0; }
    let labels = chars + (n as u16) * (2 * pad + u16::from(pad == 0));
    let seps = if sep { (n as u16) - 1 } else { 0 };
    labels + seps
}

/// `(pad, separators, scan label)`. Full scan label when it fits; otherwise the short one.
fn pick_layout(left_n: usize, left_chars: u16, right_n: usize, right_chars: u16, width: u16, full: &'static str, short: &'static str) -> (u16, bool, &'static str) {
    for (pad, sep, scan) in [(1, true, full), (1, false, full), (0, false, full), (1, false, short), (0, false, short)] {
        let total = cluster_width(left_n, left_chars, pad, sep)
            + cluster_width(right_n, right_chars, pad, sep)
            + scan.chars().count() as u16
            + 1;
        if total <= width { return (pad, sep, scan); }
    }
    (0, false, short)
}

fn paint_item(f: &mut Frame, app: &mut App, area: Rect, index: usize, pad: u16) {
    let c = &app.menu.menu.items[index];
    let t = app.theme;
    let active = app.menu.open == Some(index)
        || app.menu.anchor == Some(index)
        || (app.focus == Focus::Menu && app.menu.item == index && app.menu.open.is_none());
    let base = if active { t.selected() } else { t.plain() };
    let hint_at = c.label.chars().position(|ch| ch.to_ascii_lowercase() == c.hint).unwrap_or(0);
    let mut spans = vec![Span::styled(if pad == 1 { " " } else { "" }, &base)];
    for (k, ch) in c.label.chars().enumerate() {
        let st = if k == hint_at { base.clone().underline(true) } else { base.clone() };
        spans.push(Span::styled(ch.to_string(), st));
    }
    spans.push(Span::styled(" ", &base));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
    app.hits.push((area, Hit::MenuItem(index)));
}

struct Cluster<'a> {
    ids: &'a [usize],
    x: u16,
    y: u16,
    limit: u16,
    pad: u16,
    sep: bool,
}

fn paint_cluster(f: &mut Frame, app: &mut App, cluster: Cluster<'_>) {
    let t = app.theme;
    let mut x = cluster.x;
    for (n, &i) in cluster.ids.iter().enumerate() {
        let w = label_width(app.menu.menu.items[i].label, cluster.pad);
        if x + w > cluster.limit { break; }
        paint_item(f, app, Rect { x, y: cluster.y, width: w, height: 1 }, i, cluster.pad);
        x += w;
        if cluster.sep && n + 1 < cluster.ids.len() && x < cluster.limit {
            f.render_widget(Paragraph::new(Span::styled("│", t.muted())), Rect { x, y: cluster.y, width: 1, height: 1 });
            x += 1;
        }
    }
}

/// Draw the menu bar into `r` (one row); the dropdown is drawn by [`draw_dropdown`].
pub fn draw(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    let (left, right, left_chars, right_chars) = {
        let items = &app.menu.menu.items;
        let left: Vec<usize> = items.iter().enumerate().filter(|(_, c)| c.on_bar && !c.pin_right).map(|(i, _)| i).collect();
        let right: Vec<usize> = items.iter().enumerate().filter(|(_, c)| c.on_bar && c.pin_right).map(|(i, _)| i).collect();
        let count = |ids: &[usize]| -> u16 { ids.iter().map(|&i| items[i].label.chars().count() as u16).sum() };
        let (lc, rc) = (count(&left), count(&right));
        (left, right, lc, rc)
    };
    let full = scan_label(app);
    let short = short_scan_label(app);
    let (pad, sep, scan) = pick_layout(left.len(), left_chars, right.len(), right_chars, r.width, full, short);
    let scan_w = (scan.chars().count() as u16).min(r.width);
    let scan_x = r.x + r.width - scan_w;
    let right_w = cluster_width(right.len(), right_chars, pad, sep).min(scan_x.saturating_sub(r.x));
    let right_x = scan_x.saturating_sub(right_w);
    let left_limit = right_x.saturating_sub(1);
    paint_cluster(f, app, Cluster { ids: &left, x: r.x, y: r.y, limit: left_limit, pad, sep });
    paint_cluster(f, app, Cluster { ids: &right, x: right_x, y: r.y, limit: scan_x, pad, sep });
    let sr = Rect { x: scan_x, y: r.y, width: scan_w, height: 1 };
    f.render_widget(Paragraph::new(Span::styled(scan, t.button())), sr);
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
    let placed = app.menu.anchor.unwrap_or(i);
    let x = app.hits.iter().find(|(_, h)| *h == Hit::MenuItem(placed)).map(|(r, _)| r.x).unwrap_or(screen.x);
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
