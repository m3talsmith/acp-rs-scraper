//! `--color` and the NO_COLOR hint.

use super::*;
use super::theming::{draw, text, themed_app};
use crate::auth::prefs::save_ui_color_at;
use crate::tui::commands::CommandId;
use crate::tui::input::on_key;
use crate::tui::modals::{HelpModal, Modal};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }

#[test]
fn no_color_hint_shows_on_the_status_line_the_picker_and_the_editor() {
    let mut a = app();
    a.no_color = true;
    a.color_mode = ColorMode::Auto;
    a.note_no_color();
    a.note_no_color();
    assert_eq!(a.status.iter().filter(|s| s.contains("NO_COLOR")).count(), 1, "a repeat is not stacked");
    assert!(a.status.back().unwrap().contains("--color always") && a.status.back().unwrap().contains("Help: color"));
    a.color_mode = ColorMode::Always;
    let before = a.status.len();
    a.note_no_color();
    assert_eq!(a.status.len(), before, "always does not warn");

    let mut a = themed_app("color-hint");
    a.no_color = true;
    a.color_mode = ColorMode::Auto;
    a.open_theme_picker();
    let s = text(&draw(&mut a, 110, 32));
    assert!(s.contains("NO_COLOR is set, so colours are off"), "{s}");
    a.modal = None;
    a.open_theme_editor();
    let s = text(&draw(&mut a, 140, 40));
    assert!(s.contains("use --color always"), "{s}");
    let _ = std::fs::remove_dir_all(a.themes_dir.unwrap());
}

#[test]
fn colour_menu_deep_links_and_applies() {
    let h = HelpModal::topic("color");
    assert!(h.lines[h.scroll].contains("[[color]]"));
    let mut a = app();
    a.no_color = true;
    let view = a.menu.menu.items.iter().position(|i| i.id == CommandId::View).unwrap();
    a.open_menu(view);
    let color = a.menu.menu.items[view].opts.iter().position(|o| o.key == "color").unwrap();
    assert!(a.menu.menu.items[view].opts[color].hint_line().contains("? color"));
    a.menu.opt = color;
    a.activate_opt(color);
    assert_eq!(a.color_mode, ColorMode::Always);
    assert!(a.status.back().unwrap().contains("colour: always"));
    a.close_menu();
    a.open_menu(view);
    a.menu.opt = color;
    key(&mut a, KeyCode::Char('?'));
    let Some(Modal::Help(h)) = &a.modal else { panic!("{:?}", a.modal) };
    assert!(h.lines[h.scroll].contains("[[color]]"));
}

#[test]
fn ui_color_pref_keeps_other_keys() {
    let path = std::env::temp_dir().join(format!("acp-color-{}-{}.toml", std::process::id(), ColorMode::default()));
    std::fs::write(&path, "email = \"me@example.test\"\n").unwrap();
    save_ui_color_at(&path, "always").unwrap();
    let p: Prefs = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(p.email.as_deref(), Some("me@example.test"));
    assert_eq!(p.ui.unwrap().color.as_deref(), Some("always"));
    let _ = std::fs::remove_file(&path);
}
