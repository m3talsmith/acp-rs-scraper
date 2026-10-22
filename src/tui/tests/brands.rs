//! Brand picker, editor, and header. The Red Star title is Red Star.

use super::*;
use crate::brands::{self, LogoKind};
use crate::tui::input::on_key;
use crate::tui::commands::CommandId;
use crate::tui::modals::{help_for, help_text, AboutModal, BrandChoice, Modal, ModalResult};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::time::Instant;

fn render(a: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn key(a: &mut App, c: KeyCode, mods: KeyModifiers) {
    on_key(a, KeyEvent::new(c, mods));
}

fn home(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("acp-brand-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(p.join(".acp/brands")).unwrap();
    p
}

#[test]
fn the_default_header_title_is_red_star() {
    let mut a = app();
    let s = render(&mut a, 140, 40);
    assert!(s.contains("Red Star"), "{s}");
    assert!(!s.contains("ACP Red Star"), "{s}");
    assert!(s.contains(crate::branding::SLOGAN), "{s}");
    assert!(s.contains("‹default› ▾"), "{s}");
    assert!(!s.contains("«default» ▾"), "{s}");
}

#[test]
fn a_plain_brand_drops_acp_and_the_motto_and_keeps_the_theme() {
    let mut a = app();
    a.theme_name = "red-star".into();
    a.brand = brands::builtin_plain();
    a.brand_name = "plain".into();
    let s = render(&mut a, 140, 40);
    assert!(s.contains("Red Star"), "{s}");
    assert!(s.contains('★'), "{s}");
    assert!(s.contains("‹red-star› ▾"), "{s}");
    assert!(!s.contains('«'), "{s}");
    assert!(!s.contains("ACP"), "{s}");
    assert!(!s.contains(crate::branding::SLOGAN), "{s}");
}

#[test]
fn the_picker_previews_branding_and_esc_restores_the_theme() {
    let dir = home("picker");
    let themes = dir.join(".acp/themes");
    std::fs::create_dir_all(&themes).unwrap();
    let mut a = app();
    a.brands_dir = Some(dir.join(".acp/brands"));
    a.themes_dir = Some(themes);
    a.theme_name = "soviet".into();
    a.open_theme_picker();
    let Some(Modal::ThemePicker(p)) = &a.modal else { panic!("picker") };
    assert!(p.entries.iter().any(|e| e.name == "plain" && e.title == "Red Star"));
    assert!(p.entries.iter().any(|e| e.name == "red-star" && e.title == "Red Star"));
    key(&mut a, KeyCode::Char('j'), KeyModifiers::NONE);
    key(&mut a, KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(a.brand.title, "Red Star");
    assert_eq!(a.theme_name, "soviet", "preview does not select");
    key(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    assert!(a.modal.is_none());
    assert_eq!(a.brand.title, "Red Star");
    assert_eq!(a.theme_name, "soviet");
    assert!(a.effects.is_empty(), "Esc does not save");
}

#[test]
fn choosing_a_theme_applies_its_branding() {
    let dir = home("choose");
    let themes = dir.join(".acp/themes");
    std::fs::create_dir_all(&themes).unwrap();
    let mut a = app();
    a.brands_dir = Some(dir.join(".acp/brands"));
    a.themes_dir = Some(themes);
    a.theme_name = "cuba".into();
    a.open_theme_picker();
    let Some(Modal::ThemePicker(p)) = &mut a.modal else { panic!() };
    p.cursor = p.entries.iter().position(|e| e.name == "plain").unwrap();
    a.close_modal(ModalResult::ClosedSubmitted);
    assert_eq!(a.theme_name, "plain");
    assert_eq!(a.brand.logo, LogoKind::Text);
    assert!(a.brand.motto.is_empty());
    assert_eq!(a.brand.title, "Red Star");
    assert!(a.effects.iter().any(|e| matches!(e, crate::tui::state::Effect::SaveUiTheme(n) if n == "plain")));
    assert!(!a.effects.iter().any(|e| matches!(e, crate::tui::state::Effect::SaveUiBrand(_))));
}

#[test]
fn the_theme_editor_saves_branding_into_the_theme() {
    let dir = home("edit");
    let themes = dir.join(".acp/themes");
    std::fs::create_dir_all(&themes).unwrap();
    std::fs::write(themes.join("default.toml"), "version = 1\nname = \"default\"\ndescription = \"d\"\n[dark.palette]\naccent = \"#112233\"\n").unwrap();
    let mut a = app();
    a.brands_dir = Some(dir.join(".acp/brands"));
    a.themes_dir = Some(themes.clone());
    a.use_theme("default", false);
    a.open_theme_editor();
    key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    key(&mut a, KeyCode::Char('u'), KeyModifiers::CONTROL);
    for c in "North Star".chars() { key(&mut a, KeyCode::Char(c), KeyModifiers::NONE); }
    assert_eq!(a.theme_editor.as_ref().unwrap().preview.brand.title, "North Star");
    key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    key(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    let text = std::fs::read_to_string(themes.join("default.toml")).unwrap();
    assert!(text.contains("[brand]"), "{text}");
    assert!(text.contains("North Star"), "{text}");
    let _ = BrandChoice::None;
}

#[test]
fn a_neutral_brand_hides_the_party_wording() {
    let plain = brands::builtin_plain();
    let help = help_for(&plain).join("\n");
    assert!(help.contains("Red Star is a local client"), "{help}");
    assert!(!help.contains("ACP Red Star"), "{help}");
    assert!(!help.contains("ACP platform"), "{help}");
    assert!(!help.contains("Sync with acp.us"), "{help}");
    assert!(!help.contains(crate::branding::SLOGAN), "{help}");
    let acp = help_text().join("\n");
    assert!(acp.contains("ACP Red Star is a local client"), "{acp}");
    assert!(acp.contains("ACP platform"), "{acp}");
    assert!(acp.contains("Sync with acp.us"), "{acp}");
    assert!(acp.contains("RED STAR") && acp.contains("halberd"), "{acp}");
    assert!(!help.contains("RED STAR"), "{help}");

    let mut a = app();
    a.menu.menu.apply_brand(&plain);
    assert!(a.menu.menu.get(CommandId::Bookmarks).opts.iter().all(|o| o.label != "Sync with acp.us"));
    assert!(a.menu.menu.get(CommandId::About).opts.iter().all(|o| !o.label.contains("ACP")));
    a.menu.menu.apply_brand(&brands::builtin_default());
    assert!(a.menu.menu.get(CommandId::Bookmarks).opts.iter().any(|o| o.label == "Sync with acp.us"));
    assert!(a.menu.menu.get(CommandId::About).opts.iter().any(|o| o.label == "ACP platform"));

    brands::hold_installed(&plain, || {
        let body = AboutModal::section(0).body_lines().join("\n");
        assert!(body.contains("Red Star"), "{body}");
        assert!(!body.contains("ACP platform"), "{body}");
        assert!(!body.contains("ACP Red Star"), "{body}");
    });
    brands::hold_installed(&brands::builtin_default(), || {
        let body = AboutModal::section(2).body_lines().join("\n");
        assert!(body.contains("3 ACP platform"), "{body}");
        assert!(body.contains("https://acp.us/program"), "{body}");
    });
}

#[test]
fn the_picker_row_uses_that_themes_title() {
    let dir = home("rows");
    let themes = dir.join(".acp/themes");
    std::fs::create_dir_all(&themes).unwrap();
    let mut a = app();
    a.brands_dir = Some(dir.join(".acp/brands"));
    a.themes_dir = Some(themes);
    a.open_theme_picker();
    let s = render(&mut a, 100, 30);
    assert!(s.contains("red-star") && s.contains("Red Star"), "{s}");
    assert!(s.contains("plain") && s.contains("Red Star"), "{s}");
    let Some(Modal::ThemePicker(p)) = &a.modal else { panic!("picker") };
    let plain = p.entries.iter().find(|e| e.name == "plain").unwrap();
    assert_eq!(plain.title, "Red Star");
    assert!(!plain.title.contains("ACP"));
}

#[test]
fn a_logo_path_that_escapes_is_refused() {
    let dir = home("escape");
    let brands = dir.join(".acp/brands");
    std::fs::write(brands.join("bad.toml"), "version = 1\ntitle = \"X\"\nlogo = \"file\"\nlogo_file = \"../x.png\"\n").unwrap();
    let mut a = app();
    a.brands_dir = Some(brands);
    assert!(!a.use_brand("bad", false));
    assert_eq!(a.brand.title, "Red Star");
    assert_eq!(a.brand_fault, Some("broken"));
    let kept = std::fs::read_to_string(a.brands_dir.unwrap().join("bad.toml")).unwrap();
    assert!(kept.contains("../x.png"), "the broken file is not overwritten");
}
