//! Headless renders with ratatui's TestBackend, and mouse input through the recorded hit map.

use super::*;
use crate::tui::input::{hit_at, on_key, on_mouse};
use crate::tui::modals::Modal;
use crate::tui::state::{Focus, Hit};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use std::time::Instant;

fn render(a: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn click(a: &mut App, hit: Hit, mods: KeyModifiers) {
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?} drawn"));
    on_mouse(a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: mods });
}

fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }

#[test]
fn red_star_theme_paints_the_welcome_screen() {
    let (f, w) = crate::themes::parse(include_str!("../../../themes/red-star.toml"), "red-star").unwrap();
    assert!(w.is_empty(), "{w:?}");
    let theme = Theme::from_file(&f, crate::themes::Variant::Dark);
    let mut a = App::new(Box::new(crate::tui::data::MemStore::default()), &no_args(), theme, false);
    a.theme_name = "red-star".into();
    let (w, h) = (160u16, 45u16);
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|frame| crate::tui::widgets::draw(frame, &mut a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    let s = (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n");
    assert!(s.contains(crate::config::TUI_SUBTITLE), "{s}");
    assert!(s.contains(crate::branding::SLOGAN), "{s}");
    let body = &buf[(20, 16)];
    assert_eq!(body.style().bg, Some(ratatui::style::Color::Rgb(0x0e, 0x04, 0x04)), "ink background");
}

#[test]
fn rs_theme_paints_the_welcome_screen() {
    let (f, w) = crate::themes::parse(include_str!("../../../themes/rs.toml"), "rs").unwrap();
    assert!(w.is_empty(), "{w:?}");
    let theme = Theme::from_file(&f, crate::themes::Variant::Dark);
    let mut a = app();
    a.theme = theme;
    a.theme_name = "rs".into();
    let (w, h) = (160u16, 45u16);
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|frame| crate::tui::widgets::draw(frame, &mut a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    let s = (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n");
    assert!(s.contains("‹rs› ▾"), "{s}");
    assert!(s.contains("@alice"), "{s}");
    assert_eq!(buf[(40, 0)].bg, ratatui::style::Color::Rgb(0x0b, 0x0b, 0x0b), "header is the card black, not the logo red");
    assert_eq!(buf[(2, 1)].bg, ratatui::style::Color::Rgb(0xc0, 0x1d, 0x1c), "the logo stays ACP red");
    assert_eq!(buf[(80, 20)].bg, ratatui::style::Color::Rgb(0x11, 0x11, 0x13), "page background");
}

#[test]
fn acp_theme_paints_light_and_dark() {
    let (f, w) = crate::themes::parse(include_str!("../../../themes/acp.toml"), "acp").unwrap();
    assert!(w.is_empty(), "{w:?}");
    for (variant, page, logo) in [
        (crate::themes::Variant::Light, ratatui::style::Color::Rgb(0xff, 0xf1, 0xf1), ratatui::style::Color::Rgb(0xc0, 0x1c, 0x1c)),
        (crate::themes::Variant::Dark, ratatui::style::Color::Rgb(0x00, 0x00, 0x00), ratatui::style::Color::Rgb(0xc0, 0x1c, 0x1c)),
    ] {
        let theme = Theme::from_file(&f, variant);
        let mut a = app();
        a.theme = theme;
        a.theme_name = "acp".into();
        let (w, h) = (160u16, 45u16);
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        let now = Instant::now() + std::time::Duration::from_secs(5);
        t.draw(|frame| crate::tui::widgets::draw(frame, &mut a, now)).unwrap();
        let buf = t.backend().buffer().clone();
        let s = (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n");
        assert!(s.contains("‹acp› ▾"), "{variant:?}\n{s}");
        assert_eq!(buf[(40, 0)].bg, page, "{variant:?} header matches the page, not a red bar");
        assert_eq!(buf[(2, 1)].bg, logo, "{variant:?} logo");
        assert_eq!(buf[(80, 20)].bg, page, "{variant:?} page");
    }
}

#[test]
fn base_layout_renders_title_menu_nicks_and_empty_state() {
    let mut a = app();
    let s = render(&mut a, 110, 30);
    assert!(s.contains(TUI_SUBTITLE));
    assert!(s.contains(env!("CARGO_PKG_VERSION")));
    let menu = s.lines().find(|l| l.contains("▶ Scan")).unwrap_or("");
    for item in ["View", "Search", "Compose", "Library", "Bookmarks", "Research", "Sync", "Discover", "Account", "Debug", "Help", "About", "Quit", "Scrape", "▶ Scan"] {
        assert!(menu.contains(item), "{item}\n{menu}");
    }
    let help = menu.find("Help").unwrap();
    let about = menu.find("About").unwrap();
    let quit = menu.find("Quit").unwrap();
    let scrape = menu.find("Scrape").unwrap();
    assert!(help < about && about < quit && quit < scrape, "{menu}");
    for tucked in ["Export", "Import", "Maintain", "Session"] {
        assert!(!menu.contains(tucked), "{tucked} stays off the bar\n{menu}");
    }
    assert!(render(&mut a, 140, 30).contains("▶ Start Scanning"), "the full label when there is room");
    assert!(s.contains("@alice") && s.contains("@bob"));
    assert!(s.contains("Click a nick"));
    assert!(s.contains("logged out"));
}

#[test]
fn header_shows_the_node_name_when_one_is_configured() {
    let mut a = app();
    a.node_chip = "◉ laptop".to_string();
    let s = render(&mut a, 140, 30);
    assert!(s.contains("◉ laptop"), "{s}");
    assert!(s.contains(env!("CARGO_PKG_VERSION")), "{s}");
}

#[test]
fn empty_database_has_friendly_state() {
    let mut a = App::new(Box::new(crate::tui::data::MemStore::default()), &no_args(), crate::tui::theme::Theme::acp(), false);
    let s = render(&mut a, 100, 24);
    assert!(s.contains("No nicks yet"));
}

#[test]
fn click_nick_then_post_then_link_nests() {
    let mut a = app();
    render(&mut a, 110, 34);
    let b = nick_idx(&a, "bob");
    click(&mut a, Hit::Nick(b), KeyModifiers::NONE);
    let s = render(&mut a, 110, 34);
    assert!(s.contains("See https://acp.us/rs/post/ALICE0001"));
    let i = a.view().posts.iter().position(|p| p.post_id == "BOB000001").unwrap();
    click(&mut a, Hit::Post(i), KeyModifiers::NONE);
    let s = render(&mut a, 110, 34);
    assert!(s.contains("Linked post ALICE0001") && s.contains("not in database"), "{s}");
    assert!(s.contains("detail 1/8"));
    click(&mut a, Hit::Link(0), KeyModifiers::NONE);
    let s = render(&mut a, 110, 34);
    assert_eq!(a.view().stack.len(), 2);
    assert!(s.contains("Second paragraph."), "linked post body shown: {s}");
    assert!(s.contains("Attachments (1)") && s.contains("image/jpeg"));
    assert!(s.contains("detail 2/8"));
    // The previous detail is now a header strip; clicking it pops back.
    click(&mut a, Hit::Frame(0), KeyModifiers::NONE);
    assert_eq!(a.view().stack.len(), 1);
    key(&mut a, KeyCode::Esc);
    assert!(a.view().stack.is_empty());
}

#[test]
fn ctrl_click_multi_selects() {
    let mut a = app();
    render(&mut a, 100, 30);
    click(&mut a, Hit::Nick(0), KeyModifiers::NONE);
    render(&mut a, 100, 30);
    click(&mut a, Hit::Nick(1), KeyModifiers::CONTROL);
    assert_eq!(a.selected.len(), 2);
    let s = render(&mut a, 100, 30);
    assert!(s.contains("2 selected"));
}

#[test]
fn menu_dropdown_shows_typed_options_and_required_marker() {
    let mut a = app();
    render(&mut a, 120, 34);
    let scrape = a.menu.menu.items.iter().position(|c| c.id == crate::tui::commands::CommandId::Scrape).unwrap();
    click(&mut a, Hit::MenuItem(scrape), KeyModifiers::NONE);
    let s = render(&mut a, 120, 34);
    assert!(s.contains("[x] Mentions"), "checkbox");
    assert!(s.contains("‹ Latest ›"), "dropdown");
    assert!(s.contains("*     Nicks"), "required text field");
    assert!(s.contains("< Start Scanning >"), "button");
    // Click the Mode dropdown option: cycles.
    click(&mut a, Hit::MenuOpt(0), KeyModifiers::NONE);
    assert_eq!(a.menu.menu.choice(crate::tui::commands::CommandId::Scrape, "mode"), "Trending");
}

#[test]
fn search_view_has_back_button() {
    let mut a = app();
    a.run_search(crate::tui::data::SearchFilter { keyword: "strike".into(), ..Default::default() });
    let s = render(&mut a, 100, 30);
    assert!(s.contains("< Back") && s.contains("Search: \"strike\""));
    click(&mut a, Hit::Back, KeyModifiers::NONE);
    assert!(a.search.is_none());
}

#[test]
fn modals_render_over_dimmed_layout() {
    let mut a = app();
    key(&mut a, KeyCode::Char('?'));
    let s = render(&mut a, 100, 30);
    assert!(s.contains("Help") && s.contains("Terminal UI keys"), "{s}");
    assert!(!s.contains("not the master key"), "{s}");
    let Some(Modal::Help(help)) = &a.modal else { panic!("help modal"); };
    assert_eq!(help.scroll, 0);
    assert!(help.lines.iter().any(|l| l == "[[recovery]]"));
    key(&mut a, KeyCode::Esc);
    assert!(a.modal.is_none());
    on_key(&mut a, KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT));
    assert!(matches!(a.modal, Some(Modal::Search(_))));
    let s = render(&mut a, 100, 30);
    assert!(s.contains("Keyword") && s.contains("Posts and replies"));
    key(&mut a, KeyCode::Esc);
    on_key(&mut a, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL));
    assert!(matches!(a.modal, Some(Modal::Palette(_))));
    let s = render(&mut a, 100, 30);
    assert!(s.contains("Start Scanning (F6)"));
    key(&mut a, KeyCode::Esc);
    a.open_login(Some((crate::tui::jobs::JobKind::Scan, vec!["x".into()])));
    let s = render(&mut a, 100, 30);
    assert!(s.contains("Login") && s.contains("not in this binary"), "{s}");
    assert!(!s.contains("Send code"), "{s}");
    click(&mut a, Hit::Modal(crate::tui::modals::ModalHit::Button(0)), KeyModifiers::NONE);
    assert!(a.modal.is_none());
}

#[test]
fn keyboard_only_flow() {
    let mut a = app();
    assert_eq!(a.focus, Focus::Nicks);
    key(&mut a, KeyCode::Down);
    key(&mut a, KeyCode::Char(' '));
    key(&mut a, KeyCode::Up);
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.selected, ["alice"]);
    assert_eq!(a.focus, Focus::Body);
    render(&mut a, 100, 30);
    key(&mut a, KeyCode::Down);
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.view().stack.len(), 1);
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.view().stack.len(), 2, "Enter opens the selected link (reply-to)");
    key(&mut a, KeyCode::Esc);
    key(&mut a, KeyCode::Esc);
    key(&mut a, KeyCode::Tab);
    assert_eq!(a.focus, Focus::Menu);
    key(&mut a, KeyCode::Char('x'));
    assert!(a.menu.open.is_none(), "Export is under Library, not a bar letter");
    key(&mut a, KeyCode::Char('v'));
    assert_eq!(a.menu.open, Some(0), "letter hint opens View");
    key(&mut a, KeyCode::Esc);
    key(&mut a, KeyCode::Esc);
    assert_ne!(a.focus, Focus::Menu);
}

#[test]
fn too_small_terminal() {
    let mut a = app();
    let s = render(&mut a, 40, 10);
    assert!(s.contains("Terminal too small"));
    assert!(hit_at(&a, 1, 1).is_none());
}

/// Read-only smoke render of your real database: `cargo test smoke_real_db -- --ignored --nocapture`.
#[test]
#[ignore]
fn smoke_real_db() {
    let db = home().unwrap().join(DB_REL_PATH);
    let mut a = App::new(Box::new(crate::tui::data::SqliteStore::new(db)), &no_args(), crate::tui::theme::Theme::acp(), false);
    let hz = a.nicks.iter().position(|n| n.nick.eq_ignore_ascii_case("hazaldin")).unwrap_or(0);
    a.select_only(hz);
    a.open_post(0);
    if !a.top().map(|f| f.links.is_empty()).unwrap_or(true) { a.open_link(0); }
    println!("{}", render(&mut a, 140, 45));
    // Every view mode, list and detail (ACP_SMOKE_ALL=1 prints them all).
    for m in crate::tui::state::ViewMode::ALL {
        a.set_view_mode(m);
        a.close_details();
        let list = render(&mut a, 140, 45);
        a.open_post(0);
        let detail = render(&mut a, 140, 45);
        if std::env::var("ACP_SMOKE_ALL").is_ok() { println!("== {m:?}\n{list}\n{detail}"); }
    }
    a.open_bookmarks();
    println!("{}", render(&mut a, 140, 45));
}
