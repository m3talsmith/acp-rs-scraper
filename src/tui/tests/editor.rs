//! The theme editor on a `TestBackend`: the list, live preview, dark / light,
//! colour picking (hex and grid), contrast warnings, save / revert / new /
//! duplicate / delete, and the unsaved-changes question.

use super::*;
use super::theming::{ctrl, draw, key, text, themed_app};
use crate::themes::{self, ColorSpec, Element, Variant};
use crate::tui::editor::{Ask, Row};
use crate::tui::input::on_key;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

fn editor(tag: &str) -> App {
    let mut a = themed_app(tag);
    on_key(&mut a, KeyEvent::new(KeyCode::F(5), KeyModifiers::SHIFT));
    assert!(a.theme_editor.is_some());
    a
}

fn ed(a: &App) -> &crate::tui::editor::ThemeEditor { a.theme_editor.as_ref().unwrap() }

fn type_str(a: &mut App, s: &str) { for c in s.chars() { on_key(a, key(KeyCode::Char(c))); } }

fn go_to(a: &mut App, r: Row) {
    let i = ed(a).rows().iter().position(|x| *x == r).unwrap_or_else(|| panic!("{r:?}"));
    a.theme_editor.as_mut().unwrap().cursor = i;
}

#[test]
fn full_screen_list_and_live_preview_by_the_real_ui() {
    let mut a = editor("screen");
    let b = draw(&mut a, 140, 40);
    let s = text(&b);
    assert!(s.lines().next().unwrap().contains("Theme editor · default") && s.contains("‹dark› light"), "{s}");
    for t in ["Palette", "accent", "#dc2626", "Elements", "link", "card_focused", "Preview (sample posts)"] { assert!(s.contains(t), "no {t}:\n{s}"); }
    // The preview is the real UI (header, menu, nick bar) over the sample posts, not the database.
    for t in ["Red Star Scraper", "Scrape", "@comrade", "May Day"] { assert!(s.contains(t), "preview lacks {t}:\n{s}"); }
    assert!(!s.contains("alice"), "never the real store");
    // A swatch in the accent colour on the accent row.
    let y = s.lines().position(|l| l.contains(" accent ")).unwrap() as u16;
    assert_eq!(b[(2, y)].fg, Color::Rgb(0xdc, 0x26, 0x26));
}

#[test]
fn hex_input_sets_a_colour_marks_it_and_updates_the_preview() {
    let mut a = editor("hex");
    go_to(&mut a, Row::Palette("accent".into()));
    on_key(&mut a, key(KeyCode::Enter));
    assert!(ed(&a).picker.is_some());
    for _ in 0..7 { on_key(&mut a, key(KeyCode::Backspace)); }
    type_str(&mut a, "#00f");
    on_key(&mut a, key(KeyCode::Enter));
    assert!(ed(&a).picker.is_none());
    assert_eq!(ed(&a).file.dark.color("accent"), Color::Rgb(0, 0, 255));
    assert!(ed(&a).dirty() && ed(&a).row_changed(&Row::Palette("accent".into())));
    assert_eq!(ed(&a).preview.theme.accent, Color::Rgb(0, 0, 255), "live preview");
    assert_eq!(a.theme, Theme::acp(), "the app keeps its theme until saved");
    let s = text(&draw(&mut a, 140, 40));
    assert!(s.contains("default *") && s.contains("#0000ff   *"), "{s}");
    // A bad colour is refused in the picker.
    on_key(&mut a, key(KeyCode::Enter));
    for _ in 0..7 { on_key(&mut a, key(KeyCode::Backspace)); }
    type_str(&mut a, "#zz");
    on_key(&mut a, key(KeyCode::Enter));
    assert!(ed(&a).picker.as_ref().unwrap().error.as_deref().unwrap().contains("bad hex"));
    on_key(&mut a, key(KeyCode::Esc));
    assert!(ed(&a).picker.is_none() && a.theme_editor.is_some(), "Esc closes the picker only");
}

#[test]
fn grid_tabs_pick_ansi_cube_greys_and_references() {
    let mut a = editor("grid");
    go_to(&mut a, Row::Palette("muted".into()));
    on_key(&mut a, key(KeyCode::Enter));
    let e = a.theme_editor.as_mut().unwrap();
    e.picker.as_mut().unwrap().hex = Default::default();
    assert_eq!(e.grid(0).len(), 16);
    assert_eq!(e.grid(1).len(), 216);
    assert_eq!(e.grid(2).len(), 24);
    assert!(e.grid(3).iter().any(|(l, c)| l == "@accent" && *c == ColorSpec::Ref("accent".into())));
    assert!(!e.grid(3).iter().any(|(l, _)| l == "@muted"), "not itself");
    e.picker.as_mut().unwrap().tab = 1;
    e.picker.as_mut().unwrap().sel = 0;
    on_key(&mut a, key(KeyCode::Right));
    on_key(&mut a, key(KeyCode::Down));
    on_key(&mut a, key(KeyCode::Enter));
    assert_eq!(ed(&a).file.dark.get("muted"), Some(&ColorSpec::Value(Color::Indexed(16 + 1 + 18))));
    // @refs tab, by Tab.
    on_key(&mut a, key(KeyCode::Enter));
    a.theme_editor.as_mut().unwrap().picker.as_mut().unwrap().hex = Default::default();
    on_key(&mut a, key(KeyCode::Tab));
    on_key(&mut a, key(KeyCode::Tab));
    assert_eq!(ed(&a).picker.as_ref().unwrap().tab, 3);
    on_key(&mut a, key(KeyCode::Enter));
    assert_eq!(ed(&a).file.dark.get("muted"), Some(&ColorSpec::Ref("accent".into())));
    assert_eq!(ed(&a).theme().muted, Color::Rgb(0xdc, 0x26, 0x26));
    let s = text(&draw(&mut a, 140, 40));
    assert!(s.contains("@accent"), "{s}");
}

#[test]
fn elements_expand_into_their_keys_and_flags_cycle() {
    let mut a = editor("elements");
    go_to(&mut a, Row::Element(Element::Link));
    on_key(&mut a, key(KeyCode::Enter));
    let rows = ed(&a).rows();
    assert!(rows.contains(&Row::Field(Element::Link, "fg")) && rows.contains(&Row::Field(Element::Link, "dim")));
    assert!(!rows.contains(&Row::Field(Element::Link, "border")), "text has no border");
    go_to(&mut a, Row::Field(Element::Link, "bold"));
    on_key(&mut a, key(KeyCode::Char(' ')));
    assert_eq!(ed(&a).file.dark.style(Element::Link).bold, Some(true));
    on_key(&mut a, key(KeyCode::Char(' ')));
    assert_eq!(ed(&a).file.dark.style(Element::Link).bold, Some(false));
    on_key(&mut a, key(KeyCode::Char(' ')));
    assert_eq!(ed(&a).file.dark.style(Element::Link).bold, None, "back to built-in");
    go_to(&mut a, Row::Field(Element::Link, "fg"));
    on_key(&mut a, key(KeyCode::Enter));
    type_str(&mut a, "@muted");
    on_key(&mut a, key(KeyCode::Enter));
    assert_eq!(ratatui::style::Style::from(&ed(&a).theme().link()).fg, Some(Theme::acp().muted));
    on_key(&mut a, key(KeyCode::Delete));
    assert!(ed(&a).file.dark.style(Element::Link).is_empty());
    on_key(&mut a, key(KeyCode::Left));
    assert_eq!(ed(&a).current(), Row::Element(Element::Link));
    assert!(ed(&a).expanded.is_none());
}

#[test]
fn tab_flips_dark_and_light() {
    let mut a = editor("variant");
    on_key(&mut a, key(KeyCode::Tab));
    assert_eq!(ed(&a).variant, Variant::Light);
    assert!(text(&draw(&mut a, 140, 40)).contains("dark ‹light›"));
    go_to(&mut a, Row::Palette("accent".into()));
    on_key(&mut a, key(KeyCode::Enter));
    for _ in 0..7 { on_key(&mut a, key(KeyCode::Backspace)); }
    type_str(&mut a, "#111111");
    on_key(&mut a, key(KeyCode::Enter));
    assert_eq!(ed(&a).file.section(Variant::Light).color("accent"), Color::Rgb(17, 17, 17));
    assert_eq!(ed(&a).file.dark.color("accent"), Color::Rgb(0xdc, 0x26, 0x26), "dark untouched");
    on_key(&mut a, key(KeyCode::Tab));
    assert_eq!(ed(&a).preview.theme.accent, Color::Rgb(0xdc, 0x26, 0x26));
}

#[test]
fn low_contrast_is_flagged() {
    let mut a = editor("contrast");
    go_to(&mut a, Row::Palette("on_accent".into()));
    on_key(&mut a, key(KeyCode::Enter));
    for _ in 0..7 { on_key(&mut a, key(KeyCode::Backspace)); }
    type_str(&mut a, "#e04040");
    let e = ed(&a);
    let w = e.picker_warning(&themes::parse_color_str("#e04040").unwrap()).expect("warned while picking");
    assert!(w < 3.0);
    on_key(&mut a, key(KeyCode::Enter));
    assert!(ed(&a).row_warning(&Row::Palette("on_accent".into())).is_some());
    assert!(ed(&a).row_warning(&Row::Element(Element::Selected)).is_some(), "selected rows use on_accent on accent");
    assert!(ed(&a).row_warning(&Row::Palette("muted".into())).is_none());
    let s = text(&draw(&mut a, 140, 40));
    assert!(s.contains("⚠1."), "{s}");
}

#[test]
fn save_writes_the_file_and_applies_the_theme_in_use() {
    let mut a = editor("save");
    let dir = a.themes_dir.clone().unwrap();
    go_to(&mut a, Row::Palette("accent".into()));
    on_key(&mut a, key(KeyCode::Enter));
    for _ in 0..7 { on_key(&mut a, key(KeyCode::Backspace)); }
    type_str(&mut a, "#00aa00");
    on_key(&mut a, key(KeyCode::Enter));
    on_key(&mut a, ctrl('s'));
    assert!(!ed(&a).dirty());
    let l = themes::load(&dir, "default").unwrap();
    assert_eq!(l.file.dark.color("accent"), Color::Rgb(0, 0xaa, 0));
    assert_eq!(a.theme.accent, Color::Rgb(0, 0xaa, 0), "the theme in use changes at once");
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(dir.join("default.toml")).unwrap().permissions().mode() & 0o777, 0o600); }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn revert_puts_the_saved_file_back() {
    let mut a = editor("revert");
    go_to(&mut a, Row::Palette("muted".into()));
    on_key(&mut a, key(KeyCode::Delete));
    assert!(ed(&a).dirty());
    on_key(&mut a, ctrl('r'));
    assert!(!ed(&a).dirty());
}

#[test]
fn esc_with_changes_asks_save_discard_or_cancel() {
    let mut a = editor("unsaved");
    let dir = a.themes_dir.clone().unwrap();
    go_to(&mut a, Row::Palette("muted".into()));
    on_key(&mut a, key(KeyCode::Delete));
    on_key(&mut a, key(KeyCode::Esc));
    assert_eq!(ed(&a).prompt.as_ref().unwrap().ask, Ask::UnsavedClose);
    assert!(text(&draw(&mut a, 140, 40)).contains("[S]ave · [D]iscard · [C]ancel"));
    on_key(&mut a, key(KeyCode::Char('c')));
    assert!(a.theme_editor.is_some() && ed(&a).prompt.is_none() && ed(&a).dirty(), "cancel keeps editing");
    on_key(&mut a, key(KeyCode::Esc));
    on_key(&mut a, key(KeyCode::Char('d')));
    assert!(a.theme_editor.is_none(), "discard closes");
    assert_eq!(themes::load(&dir, "default").unwrap().file.dark.color("muted"), Color::Rgb(0x9c, 0xa3, 0xaf), "nothing written");
    let mut a = editor("unsaved2");
    go_to(&mut a, Row::Palette("muted".into()));
    on_key(&mut a, key(KeyCode::Delete));
    on_key(&mut a, key(KeyCode::Esc));
    on_key(&mut a, key(KeyCode::Char('s')));
    assert!(a.theme_editor.is_none());
    assert_eq!(themes::load(a.themes_dir.as_ref().unwrap(), "default").unwrap().file.dark.color("muted"), Color::Reset, "saved");
}

#[test]
fn new_duplicate_and_delete() {
    let mut a = editor("files");
    let dir = a.themes_dir.clone().unwrap();
    on_key(&mut a, ctrl('d'));
    type_str(&mut a, "../evil");
    on_key(&mut a, key(KeyCode::Enter));
    assert!(ed(&a).prompt.as_ref().unwrap().error.as_deref().unwrap().contains("a-z"), "path tricks refused");
    for _ in 0..7 { on_key(&mut a, key(KeyCode::Backspace)); }
    type_str(&mut a, "green");
    on_key(&mut a, key(KeyCode::Enter));
    assert!(ed(&a).prompt.as_ref().unwrap().error.as_deref().unwrap().contains("already exists"));
    for _ in 0..5 { on_key(&mut a, key(KeyCode::Backspace)); }
    type_str(&mut a, "copy");
    on_key(&mut a, key(KeyCode::Enter));
    assert_eq!(ed(&a).file.name, "copy");
    assert!(ed(&a).dirty(), "not saved yet");
    on_key(&mut a, ctrl('s'));
    assert_eq!(themes::load(&dir, "copy").unwrap().file.dark.color("accent"), Color::Rgb(0xdc, 0x26, 0x26));
    on_key(&mut a, ctrl('n'));
    type_str(&mut a, "fresh");
    on_key(&mut a, key(KeyCode::Enter));
    assert_eq!((ed(&a).file.name.as_str(), ed(&a).file.description.as_str()), ("fresh", "A new theme"));
    on_key(&mut a, ctrl('s'));
    assert!(dir.join("fresh.toml").exists());
    // Delete asks first; the theme in use can't be deleted.
    on_key(&mut a, ctrl('x'));
    assert!(text(&draw(&mut a, 140, 40)).contains("Delete fresh.toml? [y]es · [n]o"));
    on_key(&mut a, key(KeyCode::Char('n')));
    assert!(dir.join("fresh.toml").exists());
    on_key(&mut a, ctrl('x'));
    on_key(&mut a, key(KeyCode::Char('y')));
    assert!(!dir.join("fresh.toml").exists());
    assert_eq!(ed(&a).file.name, "default", "back to the theme in use");
    on_key(&mut a, ctrl('x'));
    on_key(&mut a, key(KeyCode::Char('y')));
    assert!(dir.join("default.toml").exists() && ed(&a).message.as_ref().unwrap().1.contains("in use"));
    // [ and ] step through the theme files.
    on_key(&mut a, key(KeyCode::Char(']')));
    assert_eq!(ed(&a).file.name, "green");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_broken_file_opens_as_new_and_is_only_replaced_on_save() {
    let mut a = themed_app("broken-ed");
    let dir = a.themes_dir.clone().unwrap();
    std::fs::write(dir.join("bad.toml"), "x = [").unwrap();
    a.use_theme("bad", false);
    a.modal = None;
    a.open_theme_editor();
    assert!(ed(&a).message.as_ref().unwrap().1.contains("bad.toml"));
    assert!(ed(&a).saved.is_none());
    on_key(&mut a, key(KeyCode::Esc));
    on_key(&mut a, key(KeyCode::Char('d')));
    assert_eq!(std::fs::read_to_string(dir.join("bad.toml")).unwrap(), "x = [", "untouched");
    let _ = std::fs::remove_dir_all(dir);
}

/// Cells of the preview (right of `x0`) drawn in reverse video.
fn reversed(b: &ratatui::buffer::Buffer, x0: u16) -> usize {
    let mut n = 0;
    for y in 2..b.area.height.saturating_sub(2) { for x in x0..b.area.width.saturating_sub(1) {
        if b[(x, y)].modifier.contains(ratatui::style::Modifier::REVERSED) { n += 1; }
    } }
    n
}

#[test]
fn the_selected_colour_is_highlighted_in_the_preview_and_h_toggles_it() {
    let mut a = editor("hl");
    assert!(ed(&a).highlight, "on by default");
    go_to(&mut a, Row::Palette("accent".into()));
    let on = draw(&mut a, 140, 40);
    let s = text(&on);
    assert!(s.contains("‹@accent› highlighted (h)"), "{s}");
    let x0 = 53; // the list is 52 cells wide at 140 columns; the preview is right of it
    let lit = reversed(&on, x0);
    on_key(&mut a, key(KeyCode::Char('h')));
    assert!(!ed(&a).highlight);
    let off = draw(&mut a, 140, 40);
    assert!(text(&off).contains("· h: highlight"));
    assert!(lit > reversed(&off, x0), "highlight marks cells: {lit} vs {}", reversed(&off, x0));
    on_key(&mut a, key(KeyCode::Char('h')));
    assert!(ed(&a).highlight);
    // The highlight never changes the theme being edited.
    assert!(!ed(&a).dirty());
}

#[test]
fn the_preview_title_says_when_the_selected_element_is_not_on_screen() {
    let mut a = editor("hl-off");
    go_to(&mut a, Row::Element(Element::Dialog));
    let s = text(&draw(&mut a, 140, 40));
    assert!(s.contains("‹dialog› isn't on screen"), "{s}");
    go_to(&mut a, Row::Element(Element::Bar));
    let s = text(&draw(&mut a, 140, 40));
    assert!(s.contains("‹bar› highlighted"), "{s}");
}

#[test]
fn the_marked_theme_replaces_only_the_selected_row_with_unused_colours() {
    let mut a = editor("marked");
    a.theme_editor.as_mut().unwrap().cursor = 0;
    assert!(ed(&a).marked_theme().is_none(), "headings mark nothing");
    go_to(&mut a, Row::Palette("muted".into()));
    let (t, m) = (ed(&a).theme(), ed(&a).marked_theme().unwrap());
    assert_ne!(m.muted, t.muted);
    assert_eq!((m.accent, m.border, m.fg), (t.accent, t.border, t.fg));
    assert!(![t.accent, t.muted, t.border, t.on_accent].contains(&m.muted), "a colour the theme doesn't use");
    go_to(&mut a, Row::Element(Element::Link));
    let m = ed(&a).marked_theme().unwrap();
    assert_ne!(ratatui::style::Style::from(&m.link()).fg, ratatui::style::Style::from(&ed(&a).theme().link()).fg);
    assert_eq!(m.accent, ed(&a).theme().accent);
}

#[test]
fn left_still_collapses_an_element_now_that_h_highlights() {
    let mut a = editor("collapse");
    go_to(&mut a, Row::Element(Element::Link));
    on_key(&mut a, key(KeyCode::Right));
    assert_eq!(ed(&a).expanded, Some(Element::Link));
    on_key(&mut a, key(KeyCode::Char('h')));
    assert_eq!(ed(&a).expanded, Some(Element::Link), "h toggles the highlight instead");
    on_key(&mut a, key(KeyCode::Left));
    assert_eq!(ed(&a).expanded, None);
}
