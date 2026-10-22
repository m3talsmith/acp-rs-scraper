//! Brand picker, editor, and header. The default brand stays ACP Red Star.

use super::*;
use crate::brands::{self, LogoKind};
use crate::tui::input::on_key;
use crate::tui::modals::{BrandChoice, Modal, ModalResult};
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
fn the_default_header_is_still_acp_red_star() {
    let mut a = app();
    let s = render(&mut a, 140, 40);
    assert!(s.contains("ACP Red Star"), "{s}");
    assert!(s.contains(crate::branding::SLOGAN), "{s}");
    assert!(s.contains("«default» ▾"), "{s}");
    assert!(s.contains("‹default› ▾"), "{s}");
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
    assert!(s.contains("«plain» ▾"), "{s}");
    assert!(s.contains("‹red-star› ▾"), "{s}");
    assert!(!s.contains("ACP"), "{s}");
    assert!(!s.contains(crate::branding::SLOGAN), "{s}");
}

#[test]
fn the_picker_previews_and_esc_restores_without_touching_the_theme() {
    let dir = home("picker");
    let mut a = app();
    a.brands_dir = Some(dir.join(".acp/brands"));
    a.theme_name = "soviet".into();
    a.open_brand_picker();
    let Some(Modal::BrandPicker(p)) = &a.modal else { panic!("picker") };
    assert!(p.entries.iter().any(|e| e.name == "plain"));
    key(&mut a, KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(a.brand.title, "Red Star");
    assert_eq!(a.theme_name, "soviet");
    key(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    assert!(a.modal.is_none());
    assert_eq!(a.brand.title, "ACP Red Star");
    assert_eq!(a.theme_name, "soviet");
    assert!(a.effects.is_empty(), "Esc does not save");
}

#[test]
fn choosing_a_brand_persists_it_and_leaves_the_theme() {
    let dir = home("choose");
    let mut a = app();
    a.brands_dir = Some(dir.join(".acp/brands"));
    a.theme_name = "cuba".into();
    a.open_brand_picker();
    let Some(Modal::BrandPicker(p)) = &mut a.modal else { panic!() };
    p.cursor = p.entries.iter().position(|e| e.name == "plain").unwrap();
    a.close_modal(ModalResult::ClosedSubmitted);
    assert_eq!(a.brand_name, "plain");
    assert_eq!(a.brand.logo, LogoKind::Text);
    assert!(a.brand.motto.is_empty());
    assert_eq!(a.theme_name, "cuba");
    assert!(a.effects.iter().any(|e| matches!(e, crate::tui::state::Effect::SaveUiBrand(n) if n == "plain")));
}

#[test]
fn the_editor_saves_a_new_brand_and_reverts() {
    let dir = home("edit");
    let brands = dir.join(".acp/brands");
    let mut a = app();
    a.brands_dir = Some(brands.clone());
    a.open_brand_editor();
    key(&mut a, KeyCode::Char('n'), KeyModifiers::CONTROL);
    for c in "demo".chars() { key(&mut a, KeyCode::Char(c), KeyModifiers::NONE); }
    key(&mut a, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(a.brand_editor.as_ref().unwrap().draft.name, "demo");
    key(&mut a, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(brands.join("demo.toml").is_file());
    let text = std::fs::read_to_string(brands.join("demo.toml")).unwrap();
    assert!(text.contains("Red Star") || text.contains("demo"), "{text}");
    key(&mut a, KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert_eq!(a.brand.title, a.brand_editor.as_ref().unwrap().saved.title);
    let _ = BrandChoice::None;
}

#[test]
fn a_logo_path_that_escapes_is_refused() {
    let dir = home("escape");
    let brands = dir.join(".acp/brands");
    std::fs::write(brands.join("bad.toml"), "version = 1\ntitle = \"X\"\nlogo = \"file\"\nlogo_file = \"../x.png\"\n").unwrap();
    let mut a = app();
    a.brands_dir = Some(brands);
    assert!(!a.use_brand("bad", false));
    assert_eq!(a.brand.title, "ACP Red Star");
    assert_eq!(a.brand_fault, Some("broken"));
    let kept = std::fs::read_to_string(a.brands_dir.unwrap().join("bad.toml")).unwrap();
    assert!(kept.contains("../x.png"), "the broken file is not overwritten");
}
