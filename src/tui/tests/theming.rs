//! Theme files in the TUI: selecting, the header switch, F5 picker with preview,
//! the palette entries and the fallback when a file is missing or broken.

use super::*;
use crate::themes;
use crate::tui::input::{on_key, on_mouse};
use crate::tui::modals::{Modal, PickerChoice};
use crate::tui::state::{Effect, Hit};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::Color;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

/// A temporary themes folder with the shipped default and a green theme.
pub fn themes_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("acp-tui-themes-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("default.toml"), include_str!("../../../themes/default.toml")).unwrap();
    std::fs::write(d.join("green.toml"), "version = 1\ndescription = \"All green\"\n[dark.palette]\naccent = \"#00aa00\"\non_accent = \"#000000\"\n").unwrap();
    d
}

/// The fixture app with theme files on (in a temporary folder), default selected.
pub fn themed_app(tag: &str) -> App {
    let mut a = App::new(Box::new(fixture()), &no_args(), Theme::reset(), false);
    a.themes_dir = Some(themes_dir(tag));
    a.use_theme("default", false);
    a
}

/// A plain key.
pub fn key(c: KeyCode) -> KeyEvent { KeyEvent::new(c, KeyModifiers::NONE) }
/// Ctrl + a letter.
pub fn ctrl(c: char) -> KeyEvent { KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL) }

/// One frame on a `TestBackend`.
pub fn draw(a: &mut App, w: u16, h: u16) -> Buffer {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| crate::tui::widgets::draw(f, a, std::time::Instant::now())).unwrap();
    t.backend().buffer().clone()
}

/// A buffer's text.
pub fn text(b: &Buffer) -> String {
    let a = b.area;
    (0..a.height).map(|y| (0..a.width).map(|x| b[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

#[test]
fn the_default_file_gives_the_old_look() {
    let mut a = themed_app("look");
    assert_eq!(a.theme, Theme::acp());
    assert!(a.theme_error.is_none() && a.effects.is_empty());
    let b = draw(&mut a, 110, 30);
    assert_eq!(b[(40, 0)].bg, Color::Rgb(0xdc, 0x26, 0x26), "the red bar");
}

#[test]
fn a_missing_or_broken_theme_falls_back_with_a_notice() {
    let mut a = themed_app("fallback");
    let dir = a.themes_dir.clone().unwrap();
    assert!(!a.use_theme("nope", false));
    assert_eq!(a.theme, Theme::reset());
    let Some(Modal::Message(m)) = &a.modal else { panic!("a notice") };
    assert!(m.lines.join(" ").contains("nope.toml is missing"));
    a.modal = None;
    std::fs::write(dir.join("bad.toml"), "version = 9\n").unwrap();
    assert!(!a.use_theme("bad", false));
    let err = a.theme_error.clone().unwrap();
    assert!(err.contains("bad.toml") && err.contains("newer"), "{err}");
    assert_eq!(std::fs::read_to_string(dir.join("bad.toml")).unwrap(), "version = 9\n", "never overwritten");
    a.modal = None;
    let s = text(&draw(&mut a, 110, 30));
    assert!(s.contains("‹bad ⚠› ▾") && s.contains("Scrape"), "the layout still works:\n{s}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn f5_opens_the_picker_and_it_previews_then_uses_or_restores() {
    let mut a = themed_app("picker");
    on_key(&mut a, key(KeyCode::F(5)));
    let Some(Modal::ThemePicker(p)) = &a.modal else { panic!("picker") };
    assert_eq!(p.entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["default", "green"]);
    assert_eq!(p.rows(), 3, "the themes, then Edit themes…");
    let s = text(&draw(&mut a, 110, 30));
    assert!(s.contains("All green") && s.contains("Edit themes…"), "{s}");
    on_key(&mut a, key(KeyCode::Down));
    assert_eq!(a.theme.accent, Color::Rgb(0, 0xaa, 0), "previewed");
    on_key(&mut a, key(KeyCode::Esc));
    assert_eq!(a.theme, Theme::acp(), "Esc restores");
    assert!(a.effects.is_empty());
    on_key(&mut a, key(KeyCode::F(5)));
    on_key(&mut a, key(KeyCode::Down));
    on_key(&mut a, key(KeyCode::Enter));
    assert_eq!((a.theme_name.as_str(), a.theme.accent), ("green", Color::Rgb(0, 0xaa, 0)));
    assert_eq!(a.effects, vec![Effect::SaveUiTheme("green".into())], "the choice is saved");
    on_key(&mut a, key(KeyCode::F(5)));
    on_key(&mut a, key(KeyCode::End));
    let Some(Modal::ThemePicker(p)) = &a.modal else { panic!() };
    assert_eq!(p.chosen(), PickerChoice::Edit);
    on_key(&mut a, key(KeyCode::Enter));
    assert!(a.theme_editor.is_some() && a.modal.is_none(), "Edit themes… opens the editor");
}

#[test]
fn the_header_switch_shows_the_name_and_opens_the_picker() {
    let mut a = themed_app("switch");
    let s = text(&draw(&mut a, 110, 30));
    assert!(s.lines().nth(2).unwrap().contains("‹default› ▾"), "{s}");
    let (r, _) = *a.hits.iter().find(|(_, h)| *h == Hit::ThemeSwitch).expect("clickable");
    on_mouse(&mut a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x + 1, row: r.y, modifiers: KeyModifiers::NONE });
    assert!(matches!(a.modal, Some(Modal::ThemePicker(_))));
}

#[test]
fn hotkeys_f6_scans_and_shift_f5_edits() {
    let mut a = themed_app("keys");
    on_key(&mut a, KeyEvent::new(KeyCode::F(5), KeyModifiers::SHIFT));
    assert!(a.theme_editor.is_some());
    on_key(&mut a, key(KeyCode::Esc));
    assert!(a.theme_editor.is_none(), "no changes: Esc just closes");
    on_key(&mut a, key(KeyCode::F(17)));
    assert!(a.theme_editor.is_some(), "F17 is Shift-F5 on some terminals");
    a.theme_editor = None;
    on_key(&mut a, key(KeyCode::F(6)));
    assert!(a.modal.is_some() || !a.effects.is_empty(), "F6 starts scanning (or says why not)");
}

#[test]
fn palette_has_the_theme_commands() {
    let mut a = themed_app("palette");
    let labels: Vec<String> = a.palette_entries().into_iter().map(|e| e.0).collect();
    for l in ["Theme: use… (F5)", "Theme: list", "Theme: edit… (Shift-F5)", "Theme: reload", "Theme: export", "Start Scanning (F6)"] {
        assert!(labels.iter().any(|x| x == l), "{l}");
    }
    a.act(crate::tui::commands::Action::ThemeList);
    let Some(Modal::Message(m)) = &a.modal else { panic!() };
    let t = m.lines.join("\n");
    assert!(t.contains("● default") && t.contains("green  All green"), "{t}");
    a.modal = None;
    a.act(crate::tui::commands::Action::ThemeExport);
    assert!(matches!(&a.effects[..], [Effect::ExportTheme(f)] if f.name == "default"));
    let dir = a.themes_dir.clone().unwrap();
    std::fs::write(dir.join("default.toml"), include_str!("../../../themes/default.toml").replace("#dc2626", "#123456")).unwrap();
    a.act(crate::tui::commands::Action::ThemeReload);
    assert_eq!(a.theme.accent, Color::Rgb(0x12, 0x34, 0x56));
}

#[test]
fn light_mode_uses_the_light_variant() {
    let mut a = themed_app("light");
    std::fs::write(a.themes_dir.as_ref().unwrap().join("two.toml"), "version = 1\n[dark.palette]\naccent = \"#111111\"\n[light.palette]\naccent = \"#eeeeee\"\n").unwrap();
    a.theme_mode = themes::Mode::System;
    a.dark_bg = Some(false);
    a.use_theme("two", false);
    assert_eq!(a.theme.accent, Color::Rgb(0xee, 0xee, 0xee));
    a.dark_bg = Some(true);
    a.refresh_theme();
    assert_eq!(a.theme.accent, Color::Rgb(0x11, 0x11, 0x11));
}

/// An app on a first run: an empty themes folder, `default` selected but missing.
fn first_run_app(tag: &str) -> (App, PathBuf) {
    let d = std::env::temp_dir().join(format!("acp-tui-dl-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let mut a = App::new(Box::new(fixture()), &no_args(), Theme::reset(), false);
    a.themes_dir = Some(d.clone());
    assert!(!a.use_theme("default", false));
    a.modal = None;
    (a, d)
}

#[test]
fn theme_init_starts_a_background_download_of_the_default() {
    let (mut a, d) = first_run_app("init");
    a.act(crate::tui::commands::Action::ThemeInit);
    assert!(matches!(&a.effects[..], [Effect::DownloadTheme(n, true)] if n == "default"));
    assert_eq!(a.theme_downloading.as_deref(), Some("default"));
    a.effects.clear();
    a.act(crate::tui::commands::Action::ThemeInit);
    assert!(a.effects.is_empty(), "one download at a time");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_downloaded_default_is_applied_live_when_it_is_still_the_one_wanted() {
    let (mut a, d) = first_run_app("apply");
    a.start_theme_download("default", false);
    assert_eq!(a.theme.accent, Color::Reset, "terminal colours meanwhile");
    std::fs::write(d.join("default.toml"), include_str!("../../../themes/default.toml")).unwrap();
    a.theme_downloaded("default", false, Ok(themes::download::Fetched::Written(d.join("default.toml"))));
    assert_eq!(a.theme.accent, Theme::acp().accent);
    assert!(a.theme_error.is_none() && a.theme_downloading.is_none());
    assert!(a.modal.is_none(), "an automatic download only notes it in the status line");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_download_does_not_override_a_theme_the_user_picked_meanwhile() {
    let (mut a, d) = first_run_app("picked");
    a.start_theme_download("default", false);
    std::fs::write(d.join("green.toml"), "version = 1\n[dark.palette]\naccent = \"#00aa00\"\n").unwrap();
    assert!(a.use_theme("green", true));
    std::fs::write(d.join("default.toml"), include_str!("../../../themes/default.toml")).unwrap();
    a.theme_downloaded("default", false, Ok(themes::download::Fetched::Written(d.join("default.toml"))));
    assert_eq!((a.theme_name.as_str(), a.theme.accent), ("green", Color::Rgb(0, 0xaa, 0)));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_failed_download_keeps_the_terminal_colours_and_explains_the_fix() {
    let (mut a, d) = first_run_app("fail");
    a.start_theme_download("default", false);
    a.theme_downloaded("default", false, Err("no answer within 3 s (timed out)".into()));
    assert_eq!(a.theme.accent, Color::Reset);
    let n = a.theme_error.clone().unwrap();
    assert!(n.contains("timed out") && n.contains("--theme-init"), "{n}");
    assert!(matches!(&a.modal, Some(Modal::Message(_))));
    assert!(!d.join("default.toml").exists());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn with_no_themes_the_picker_offers_download_and_starter() {
    let (mut a, d) = first_run_app("pick-empty");
    a.open_theme_picker();
    let s = text(&draw(&mut a, 120, 30));
    for t in ["Download the default theme", "Create starter theme", "Edit themes…"] { assert!(s.contains(t), "no {t}:\n{s}"); }
    let Some(Modal::ThemePicker(p)) = &a.modal else { panic!() };
    assert_eq!((p.rows(), p.chosen()), (3, PickerChoice::Download));
    on_key(&mut a, key(KeyCode::Enter));
    assert!(matches!(&a.effects[..], [Effect::DownloadTheme(n, true)] if n == "default"));
    // A themed folder doesn't offer them.
    let mut b = themed_app("pick-full");
    b.open_theme_picker();
    let s = text(&draw(&mut b, 120, 30));
    assert!(!s.contains("Create starter theme") && !s.contains("Download the default theme"), "{s}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn create_starter_theme_from_the_picker_writes_it_and_uses_it() {
    let (mut a, d) = first_run_app("pick-starter");
    a.open_theme_picker();
    on_key(&mut a, key(KeyCode::Down));
    on_key(&mut a, key(KeyCode::Enter));
    let l = themes::load(&d, "starter").unwrap();
    assert!(l.warnings.is_empty());
    assert_eq!((a.theme_name.as_str(), a.theme.accent), ("starter", Color::Red));
    assert!(a.theme_error.is_none());
    assert!(a.effects.iter().any(|e| matches!(e, Effect::SaveUiTheme(n) if n == "starter")), "chosen in the picker: saved");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn theme_starter_in_the_palette_takes_a_name_and_never_overwrites() {
    use crate::tui::modals::{ModalState, PaletteModal};
    let mut a = themed_app("pal-starter");
    let d = a.themes_dir.clone().unwrap();
    let run = |a: &mut App, q: &str| {
        let mut p = PaletteModal::new(a.palette_entries());
        for c in q.chars() { p.on_key(key(KeyCode::Char(c))); }
        assert!(p.chosen().is_some(), "{q} finds an entry");
        a.modal = Some(Modal::Palette(p));
        on_key(a, key(KeyCode::Enter));
    };
    run(&mut a, "theme starter mine");
    assert!(themes::load(&d, "mine").unwrap().file.description.starts_with("Starter theme"));
    assert_eq!(a.theme_name, "default", "the palette doesn't switch a working theme");
    std::fs::write(d.join("mine.toml"), "version = 1\ndescription = 'Mine'\n").unwrap();
    a.modal = None;
    run(&mut a, "theme starter mine");
    assert_eq!(themes::load(&d, "mine").unwrap().file.description, "Mine", "kept");
    a.modal = None;
    run(&mut a, "theme starter");
    assert!(d.join("starter.toml").exists(), "no name: starter");
    a.modal = None;
    run(&mut a, "theme starter");
    assert!(d.join("starter-2.toml").exists(), "then starter-2");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_header_says_downloading_while_the_first_run_download_runs() {
    let (mut a, d) = first_run_app("hdr");
    a.start_theme_download("default", false);
    let s = text(&draw(&mut a, 140, 30));
    assert!(s.contains("‹default · downloading…› ▾"), "{s}");
    a.theme_downloaded("default", false, Err("offline".into()));
    let s = text(&draw(&mut a, 140, 30));
    assert!(!s.contains("downloading…") && s.contains("‹default ⚠› ▾"), "{s}");
    let n = a.theme_error.clone().unwrap();
    assert!(n.contains("--theme-starter"), "the failure suggests the starter: {n}");
    let _ = std::fs::remove_dir_all(d);
}
