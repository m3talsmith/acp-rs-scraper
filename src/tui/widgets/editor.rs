//! Drawing the theme editor: the list on the left, the live preview (the real UI
//! drawn off screen over sample posts, then copied in) on the right, the colour
//! picker and questions over it.

use crate::prelude::*;
use crate::themes::{self, ColorSpec, PALETTE_KEYS};
use crate::tui::editor::{Ask, PICKER_TABS, Row, ThemeEditor};
use crate::tui::state::{App, Hit};
use crate::tui::theme::{Gloss, Theme, frame};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use std::time::Instant;

fn swatch(c: Option<Color>) -> Span<'static> {
    match c {
        Some(Color::Reset) => Span::raw("··"),
        Some(c) => Span::styled("██", Gloss::new().foreground(c)),
        None => Span::raw("  "),
    }
}

/// Draw the editor over the whole screen.
pub fn draw(f: &mut Frame, app: &mut App, ed: &mut ThemeEditor, now: Instant) {
    let area = f.area();
    let ui = app.theme;
    let et = ed.theme();
    f.render_widget(Clear, area);
    let dirty = if ed.dirty() { " *" } else { "" };
    let mode = match ed.variant { themes::Variant::Dark => "‹dark› light", themes::Variant::Light => "dark ‹light›" };
    let title = format!(" Theme editor · {}{dirty} · {mode} (Tab) · ^S save ^R revert ^N new ^D duplicate ^X delete · [ ] other theme · Esc close",
        ed.file.name);
    f.render_widget(Paragraph::new(Span::styled(title, ui.bar().bold(true))).style(ui.bar()), Rect { height: 1, ..area });
    let body = Rect { y: area.y + 1, height: area.height.saturating_sub(2), ..area };
    let lw = (body.width / 2).clamp(30, 52).min(body.width);
    let left = Rect { width: lw, ..body };
    let right = Rect { x: body.x + lw, width: body.width.saturating_sub(lw), ..body };

    // The list.
    let lb = frame(&ui.pane(true)).title(format!(" {} ", if ed.file.description.is_empty() { "Theme" } else { &ed.file.description }));
    let inner = lb.inner(left);
    f.render_widget(lb, left);
    let rows = ed.rows();
    let h = inner.height as usize;
    if ed.cursor < ed.scroll { ed.scroll = ed.cursor; }
    if h > 0 && ed.cursor >= ed.scroll + h { ed.scroll = ed.cursor + 1 - h; }
    for (k, (i, r)) in rows.iter().enumerate().skip(ed.scroll).take(h).enumerate() {
        let rr = Rect { y: inner.y + k as u16, height: 1, ..inner };
        let sel = i == ed.cursor;
        let mark = if ed.row_changed(r) { "*" } else { " " };
        let warn = ed.row_warning(r).map(|x| format!(" ⚠{x:.1}:1")).unwrap_or_default();
        let base = if sel { ui.selected() } else { ui.plain() };
        let line = match r {
            Row::Heading(t) => Line::styled(format!(" {t}"), ui.accent_bold()),
            Row::Palette(n) => Line::from(vec![Span::raw(" "), swatch(ed.row_color(r)), Span::styled(format!(" {n:<11} {:<10}{mark}", ed.row_value(r)), &base),
                Span::styled(warn, ui.accent_bold())]),
            Row::Element(e) => {
                let open = if ed.expanded == Some(*e) { "▾" } else { "▸" };
                let sample = if e.is_box() {
                    let st = ratatui::style::Style::from(&et.element(*e));
                    Span::styled("┌┐", Gloss::new().foreground(st.fg.unwrap_or(Color::Reset)).background(et.bg))
                } else { Span::styled("Ab", et.element(*e)) };
                let v = ed.row_value(r);
                let room = (inner.width as usize).saturating_sub(24);
                let v: String = if v.chars().count() > room { v.chars().take(room.saturating_sub(1)).chain(['…']).collect() } else { v };
                Line::from(vec![Span::styled(format!(" {open} "), &base), sample, Span::styled(format!(" {:<15}{mark} ", e.name()), &base),
                    Span::styled(v, if sel { base.clone() } else { ui.muted() }), Span::styled(warn, ui.accent_bold())])
            }
            Row::Field(_, k) => Line::from(vec![Span::raw("     "), swatch(ed.row_color(r)), Span::styled(format!(" {k:<9} {}{mark}", ed.row_value(r)), &base),
                Span::styled(warn, ui.accent_bold())]),
        };
        f.render_widget(Paragraph::new(line), rr);
        app.hits.push((rr, Hit::EditorRow(i)));
    }

    // The preview: the real UI, drawn off screen at a size it supports, then copied in.
    let pb = frame(&ui.pane(false)).title(" Preview (sample posts) ");
    let pin = pb.inner(right);
    f.render_widget(pb, right);
    if pin.width > 0 && pin.height > 0 {
        let (w, hh) = (pin.width.max(TUI_MIN_SIZE.0), pin.height.max(TUI_MIN_SIZE.1));
        if let Ok(mut term) = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, hh))
            && term.draw(|pf| crate::tui::widgets::draw(pf, &mut ed.preview, now)).is_ok() {
            let src = term.backend().buffer().clone();
            let buf = f.buffer_mut();
            for y in 0..pin.height { for x in 0..pin.width {
                if let (Some(c), Some(d)) = (src.cell((x, y)), buf.cell_mut((pin.x + x, pin.y + y))) { *d = c.clone(); }
            } }
        }
    }

    // Footer: the result of the last action, or what the row is.
    let foot = Rect { y: area.y + area.height.saturating_sub(1), height: 1, ..area };
    let (err, text) = ed.message.clone().unwrap_or_else(|| (false, match ed.current() {
        Row::Palette(n) => PALETTE_KEYS.iter().find(|(k, _)| *k == n).map(|(_, a)| a.to_string()).unwrap_or_else(|| format!("@{n}: your own colour for references")) + " · Enter picks · Del resets · a adds a colour",
        Row::Element(e) => format!("{} · Enter shows its keys · Del clears its overrides", e.about()),
        Row::Field(_, k) if ["fg", "bg", "border"].contains(&k) => "Enter picks a colour · Del clears (built-in)".into(),
        Row::Field(..) => "Enter cycles on / off / built-in · Del clears".into(),
        Row::Heading(_) => String::new(),
    }));
    f.render_widget(Paragraph::new(Span::styled(format!(" {text}"), if err { ui.accent_bold() } else { ui.muted() })), foot);

    if ed.picker.is_some() { draw_picker(f, &ui, ed, area); }
    if let Some(p) = &ed.prompt {
        let (q, named) = match &p.ask {
            Ask::UnsavedClose | Ask::UnsavedOpen(_) => (format!("{} has unsaved changes: [S]ave · [D]iscard · [C]ancel", ed.file.name), false),
            Ask::Delete(n) => (format!("Delete {n}.toml? [y]es · [n]o"), false),
            Ask::NewName => ("Name of the new theme:".into(), true),
            Ask::DuplicateName => (format!("Name of the copy of {}:", ed.file.name), true),
            Ask::PaletteName => ("Name of the new palette colour (use it as @name):".into(), true),
        };
        let r = crate::tui::layout::modal_rect(area, 66.min(area.width), 6);
        f.render_widget(Clear, r);
        let b = frame(&ui.dialog()).title(" Theme editor ");
        let i = b.inner(r);
        f.render_widget(b, r);
        let mut lines = vec![Line::raw(q)];
        if named { lines.push(Line::styled(format!("› {}▏", p.field.value), ui.field())); }
        if let Some(e) = &p.error { lines.push(Line::styled(e.clone(), ui.accent_bold())); }
        f.render_widget(Paragraph::new(lines), i);
    }
}

fn draw_picker(f: &mut Frame, ui: &Theme, ed: &ThemeEditor, area: Rect) {
    let Some(p) = &ed.picker else { return };
    let grid = ed.grid(p.tab);
    let cols = ThemeEditor::grid_cols(p.tab);
    let grid_rows = grid.len().div_ceil(cols) as u16;
    let r = crate::tui::layout::modal_rect(area, 64.min(area.width), (grid_rows + 9).min(area.height));
    f.render_widget(Clear, r);
    let what = match &p.target { crate::tui::editor::Target::Palette(n) => n.clone(), crate::tui::editor::Target::Field(e, k) => format!("{}.{k}", e.name()) };
    let b = frame(&ui.dialog()).title(format!(" Colour for {what} · Tab: grid · arrows · Enter sets · Esc "));
    let i = b.inner(r);
    f.render_widget(b, r);
    let tabs: Vec<Span> = PICKER_TABS.iter().enumerate().map(|(k, t)| Span::styled(format!(" {t} "), if k == p.tab { ui.selected() } else { ui.plain() })).collect();
    f.render_widget(Paragraph::new(Line::from(tabs)), Rect { height: 1, ..i });
    f.render_widget(Paragraph::new(Line::from(vec![Span::raw("Type: "), Span::styled(format!("{}▏", p.hex.value), ui.field()),
        Span::styled("  (#rrggbb, #rgb, 0-255, an ANSI name, @name, reset)", ui.muted())])), Rect { y: i.y + 1, height: 1, ..i });
    let gy = i.y + 3;
    for (k, (label, c)) in grid.iter().enumerate() {
        let (row, col) = ((k / cols) as u16, (k % cols) as u16);
        let sel = k == p.sel;
        let span = if cols == 1 {
            let v = ed.file.section(ed.variant).resolve(c);
            Line::from(vec![swatch(Some(v)), Span::styled(format!(" {label}"), if sel { ui.selected() } else { ui.plain() })])
        } else {
            let ColorSpec::Value(v) = c else { continue };
            let cell = if sel { "▐▌" } else { "██" };
            let mut st = Gloss::new().foreground(*v);
            if sel { st = st.background(ui.on_accent); }
            Line::from(Span::styled(cell, st))
        };
        let w = if cols == 1 { i.width } else { 3 };
        let rr = Rect { x: i.x + col * w, y: gy + row, width: w, height: 1 };
        if rr.y < i.y + i.height.saturating_sub(2) { f.render_widget(Paragraph::new(span), rr); }
    }
    let chosen = grid.get(p.sel).map(|(l, c)| (l.clone(), c.clone()));
    let mut info = vec![];
    if let Some((l, c)) = chosen {
        info.push(Span::raw(format!("Selected: {l} ")));
        let v = ed.file.section(ed.variant).resolve(&c);
        info.push(swatch(Some(v)));
        if let Some(x) = ed.picker_warning(&c) { info.push(Span::styled(format!("  ⚠ contrast {x:.1}:1 (below {THEME_MIN_CONTRAST}:1)"), ui.accent_bold())); }
    }
    if let Some(e) = &p.error { info = vec![Span::styled(e.clone(), ui.accent_bold())]; }
    f.render_widget(Paragraph::new(Line::from(info)), Rect { y: i.y + i.height.saturating_sub(1), height: 1, ..i });
}
