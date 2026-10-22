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

/// Foreground colours of the non-space cells in the header logo slot.
fn header_ink(buf: &ratatui::buffer::Buffer, x0: u16, x1: u16) -> Vec<ratatui::style::Color> {
    let mut out = Vec::new();
    for y in 0..4 {
        for x in x0..x1 {
            let cell = &buf[(x, y)];
            if cell.symbol() != " " { out.push(cell.fg); }
        }
    }
    out
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
    assert_eq!(body.style().bg, Some(ratatui::style::Color::Rgb(0x04, 0x12, 0x1a)), "night background");
    let bar = &buf[(40, 2)];
    assert_eq!(bar.bg, ratatui::style::Color::Rgb(0x03, 0x09, 0x0f), "header is the deep ground");
    let mark = header_ink(&buf, 0, 12);
    assert!(mark.len() >= 2, "halberd ink {mark:?}");
    assert!(mark.iter().any(|c| *c != ratatui::style::Color::Rgb(0xe6, 0xfb, 0xf7)), "the blade is not the bar text");
    assert_ne!(mark[0], mark[mark.len() - 1], "the blade runs teal to blue: {mark:?}");
}

#[test]
fn red_star_light_paints_paper_and_the_same_mark() {
    let (f, w) = crate::themes::parse(include_str!("../../../themes/red-star.toml"), "red-star").unwrap();
    assert!(w.is_empty(), "{w:?}");
    let theme = Theme::from_file(&f, crate::themes::Variant::Light);
    let mut a = App::new(Box::new(crate::tui::data::MemStore::default()), &no_args(), theme, false);
    a.theme_name = "red-star".into();
    let (w, h) = (160u16, 45u16);
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|frame| crate::tui::widgets::draw(frame, &mut a, Instant::now())).unwrap();
    let buf = t.backend().buffer().clone();
    assert_eq!(buf[(80, 20)].bg, ratatui::style::Color::Rgb(0xf7, 0xf5, 0xef), "paper");
    assert_eq!(buf[(40, 2)].bg, ratatui::style::Color::Rgb(0xe7, 0xf4, 0xf1), "pale header wash");
    let mark = header_ink(&buf, 1, 9);
    assert!(mark.len() >= 2 && mark[0] != mark[mark.len() - 1], "light blade runs teal to blue: {mark:?}");
    let s = (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n");
    assert!(s.contains(crate::branding::SLOGAN) && s.contains("Red Star"), "{s}");
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
    assert_eq!(buf[(40, 0)].bg, ratatui::style::Color::Rgb(0x0b, 0x0b, 0x0b), "header is the card black");
    assert_eq!(buf[(2, 1)].bg, ratatui::style::Color::Rgb(0x0b, 0x0b, 0x0b), "the halberd uses the bar colour");
    assert_eq!(buf[(80, 20)].bg, ratatui::style::Color::Rgb(0x11, 0x11, 0x13), "page background");
}

#[test]
fn acp_theme_paints_light_and_dark() {
    let (f, w) = crate::themes::parse(include_str!("../../../themes/acp.toml"), "acp").unwrap();
    assert!(w.is_empty(), "{w:?}");
    for (variant, page) in [
        (crate::themes::Variant::Light, ratatui::style::Color::Rgb(0xff, 0xf1, 0xf1)),
        (crate::themes::Variant::Dark, ratatui::style::Color::Rgb(0x00, 0x00, 0x00)),
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
        assert_eq!(buf[(2, 1)].bg, page, "{variant:?} halberd uses the bar");
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
fn menu_dropdown_opens_under_the_bar() {
    let mut a = app();
    let view = a.menu.menu.items.iter().position(|c| c.id == crate::tui::commands::CommandId::View).unwrap();
    a.focus = Focus::Menu;
    a.menu.item = view;
    key(&mut a, KeyCode::Enter);
    let (text, buf) = frame(&mut a, 100, 30);
    let bar = rect_of(&a, Hit::MenuItem(view));
    let opt = rect_of(&a, Hit::MenuOpt(0));
    assert!(opt.y > bar.y, "first option {opt:?} is not below the bar {bar:?}");
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[bar.y as usize].contains("View"), "bar row stays visible: {}", lines[bar.y as usize]);
    assert!(!lines[bar.y as usize].contains('═'), "popup covered the bar: {}", lines[bar.y as usize]);
    assert!(lines[(bar.y + 1) as usize].contains('═'), "popup top edge: {}", lines[(bar.y + 1) as usize]);
    let open_bg = buf[(bar.x.saturating_add(1), bar.y)].bg;
    let other = a.hits.iter().find_map(|(r, h)| match h {
        Hit::MenuItem(i) if *i != view => Some(*r),
        _ => None,
    }).unwrap();
    assert_ne!(open_bg, buf[(other.x.saturating_add(1), other.y)].bg, "open category stays highlighted");

    let mut a = app();
    render(&mut a, 90, 24);
    let bars: Vec<usize> = a.hits.iter().filter_map(|(_, h)| match h { Hit::MenuItem(i) => Some(*i), _ => None }).collect();
    let last = *bars.iter().max_by_key(|i| rect_of(&a, Hit::MenuItem(**i)).x).unwrap();
    for i in bars {
        click(&mut a, Hit::MenuItem(i), KeyModifiers::NONE);
        if a.menu.open != Some(i) { continue; }
        frame(&mut a, 90, 24);
        let bar = rect_of(&a, Hit::MenuItem(i));
        let top = rect_of(&a, Hit::MenuOpt(0));
        assert_eq!(top.y, bar.y + 2, "option under the border, category {i}");
        assert!(top.y > bar.y);
        for (r, h) in &a.hits {
            if let Hit::MenuOpt(_) = h {
                assert!(r.y > bar.y, "{h:?} overlaps the bar");
                assert!(r.x + r.width <= 90, "{h:?} runs past the screen {r:?}");
            }
        }
    }
    assert_eq!(a.menu.open, Some(last), "last category opened");

    let mut a = app();
    a.open_menu(view);
    let n = a.menu.menu.items[view].opts.len();
    a.menu.opt = n - 1;
    frame(&mut a, 60, 19);
    let bar = rect_of(&a, Hit::MenuItem(view));
    let shown = rect_of(&a, Hit::MenuOpt(n - 1));
    assert!(shown.y > bar.y && shown.y < 19, "scrolled option {shown:?} bar {bar:?}");
    assert!(a.hits.iter().all(|(_, h)| *h != Hit::MenuOpt(0)), "the first option scrolled off");
    click(&mut a, Hit::MenuOpt(n - 1), KeyModifiers::NONE);
}

fn frame(a: &mut App, w: u16, h: u16) -> (String, ratatui::buffer::Buffer) {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    let text = (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n");
    (text, buf)
}

fn rect_of(a: &App, hit: Hit) -> ratatui::layout::Rect {
    a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?}")).0
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
