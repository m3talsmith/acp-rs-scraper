//! The theme is the single source of styling, built on ratatui-lipgloss:
//! its styles must equal the ratatui styles the widgets used before.

use crate::tui::theme::{Theme, frame};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

fn rs(g: crate::tui::theme::Gloss) -> Style { Style::from(&g) }

#[test]
fn text_styles_match_the_ratatui_ones() {
    let t = Theme::default();
    let red = Color::Rgb(0xdc, 0x26, 0x26);
    assert_eq!(rs(t.plain()), Style::default());
    assert_eq!(rs(t.accent()), Style::default().fg(red));
    assert_eq!(rs(t.accent_bold()), Style::default().fg(red).add_modifier(Modifier::BOLD));
    assert_eq!(rs(t.link()), Style::default().fg(red).add_modifier(Modifier::UNDERLINED));
    assert_eq!(rs(t.selected()), Style::default().bg(red).fg(t.on_accent).add_modifier(Modifier::BOLD));
    assert_eq!(rs(t.bar()), Style::default().bg(red).fg(t.on_accent));
    assert_eq!(rs(t.button()), Style::default().bg(red).fg(t.on_accent).add_modifier(Modifier::BOLD));
    assert_eq!(rs(t.muted()), Style::default().fg(t.muted));
    assert_eq!(rs(t.gone()), Style::default().fg(t.muted).add_modifier(Modifier::CROSSED_OUT));
    assert_eq!(rs(t.dimmed()), Style::default().add_modifier(Modifier::DIM));
    assert_eq!(rs(t.label(true)), rs(t.accent_bold()));
    assert_eq!(rs(t.label(false)), Style::default());
}

#[test]
fn prefs_flow_into_every_style() {
    let p = crate::prelude::ThemePrefs { accent: Some("#00ff00".into()), ..Default::default() };
    let t = Theme::from_prefs(Some(&p));
    assert_eq!(rs(t.accent()).fg, Some(Color::Rgb(0, 255, 0)));
    assert_eq!(rs(t.selected()).bg, Some(Color::Rgb(0, 255, 0)));
    assert_eq!(rs(t.reply_tag()).bg, Some(Color::Rgb(0, 255, 0)));
}

fn draw(g: &crate::tui::theme::Gloss, w: u16, h: u16) -> (Buffer, Rect) {
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    let b = frame(g);
    let inner = b.inner(area);
    b.render(area, &mut buf);
    (buf, inner)
}

#[test]
fn boxes_have_the_right_borders_colours_and_padding() {
    let t = Theme::default();
    let (b, inner) = draw(&t.card(true, true), 8, 3);
    assert_eq!(b[(0, 0)].symbol(), "┏");
    assert_eq!(b[(0, 0)].fg, t.accent);
    assert_eq!(inner, Rect::new(2, 1, 4, 1), "one column of padding inside the border");
    let (b, _) = draw(&t.card(true, false), 8, 3);
    assert_eq!((b[(0, 0)].symbol(), b[(0, 0)].fg), ("┏", Color::Reset));
    let (b, _) = draw(&t.card(false, true), 8, 3);
    assert_eq!((b[(0, 0)].symbol(), b[(0, 0)].fg), ("┌", t.border));
    let (b, inner) = draw(&t.pane(true), 6, 3);
    assert_eq!((b[(0, 0)].symbol(), b[(0, 0)].fg), ("┌", t.accent));
    assert_eq!(inner, Rect::new(1, 1, 4, 1));
    let (b, _) = draw(&t.dialog(), 6, 3);
    assert_eq!((b[(5, 2)].symbol(), b[(5, 2)].fg), ("╝", t.accent));
    let (b, _) = draw(&t.detail(false), 6, 3);
    assert_eq!((b[(0, 1)].symbol(), b[(0, 1)].fg), ("║", t.border));
}
