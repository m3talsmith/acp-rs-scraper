//! Tooltips, searchable Help, and the three-section About modal.

use super::*;
use crate::tui::commands::{command_topic, Action, CommandId, Menu};
use crate::tui::input::{on_key, on_mouse};
use crate::tui::modals::{help_text, AboutModal, HelpModal, Modal, ModalState};
use crate::tui::state::Hit;
use crate::tui::tooltip::tip_for;
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

fn click(a: &mut App, hit: Hit) {
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?} drawn"));
    on_mouse(a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
}

fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }

#[test]
fn every_menu_topic_is_in_help() {
    let menu = Menu::new(&no_args());
    let help = help_text().join("\n");
    for c in &menu.items {
        let topic = command_topic(c.id);
        assert!(help.contains(&format!("[[{topic}]]")), "{topic}");
        for o in &c.opts {
            assert!(!o.help.is_empty(), "{} / {}", c.label, o.label);
            let id = o.help;
            assert!(help.contains(&format!("[[{id}]]")), "{} → {id}", o.label);
        }
    }
}

#[test]
fn star_basket_and_research_do_not_share_a_topic() {
    let a = app();
    let star = tip_for(&a, Hit::Star(0)).unwrap();
    let basket = tip_for(&a, Hit::Basket(0)).unwrap();
    let research = tip_for(&a, Hit::Research(0)).unwrap();
    assert_ne!(star.topic, basket.topic);
    assert_ne!(star.topic, research.topic);
    assert_ne!(basket.topic, research.topic);
    assert!(star.text.contains("(b)"), "{}", star.text);
    assert_eq!(star.topic, "bookmark");
    assert_eq!(basket.topic, "basket");
    assert_eq!(research.topic, "research");
}

#[test]
fn help_search_filters_and_arrows_jump_topics() {
    let mut h = HelpModal::new();
    for c in ['g', 'e', 't', 't', 'i', 'n', 'g'] { h.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)); }
    let view = h.view_lines().join("\n");
    assert!(view.contains("acprs"), "{view}");
    h.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    let lines = h.view_lines();
    assert!(lines[h.scroll].starts_with("[["), "{}", lines[h.scroll]);
    h.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    let lines = h.view_lines();
    assert!(lines[h.scroll].starts_with("[["), "{}", lines[h.scroll]);
}

#[test]
fn about_platform_is_the_vendored_program() {
    let m = AboutModal::section(2);
    assert!(!m.credits);
    assert_eq!(m.section, 2);
    assert_eq!(AboutModal::section(9).section, 2);
    let body = m.body_lines().join("\n");
    assert_eq!(body.matches("https://acp.us/join").count(), 2);
    assert!(body.contains("https://acp.us/program"));
    assert!(body.contains("https://acp.us/docs/2024-program-a51b2b90421960a720072cc1731dfac0.pdf"));
    assert!(body.contains("Cancellation of all Debts"));
    assert!(!body.contains("Neon Lust"));
    let credits = AboutModal::section(0).body_lines().join("\n");
    assert!(credits.contains("The people who built this client."));
    assert!(!credits.contains("Neon Lust"));
}

#[test]
fn hover_tooltip_deep_links_into_help() {
    let mut a = app();
    let s = render(&mut a, 120, 40);
    assert!(s.contains('?'), "header help mark\n{s}");
    assert!(a.hits.iter().any(|(_, h)| *h == Hit::HelpMark));
    a.note_hover(Some(Hit::Star(0)));
    let s = render(&mut a, 120, 40);
    assert!(s.contains("Bookmark (b)"), "{s}");
    assert!(s.contains("? bookmark"), "{s}");
    click(&mut a, Hit::Tip);
    let Some(Modal::Help(h)) = &a.modal else { panic!("help"); };
    assert_eq!(h.lines[h.scroll], "[[bookmark]]");
}

#[test]
fn about_menu_opens_the_chosen_section() {
    let mut a = app();
    let i = a.menu.menu.items.iter().position(|c| c.id == CommandId::About).unwrap();
    a.open_menu(i);
    a.activate_opt(2);
    match &a.modal {
        Some(Modal::About(m)) => {
            assert_eq!(m.section, 2);
            assert!(!m.credits);
        }
        other => panic!("{other:?}"),
    }
    a.act(Action::About);
    match &a.modal {
        Some(Modal::About(m)) => assert_eq!((m.section, m.credits), (0, false)),
        other => panic!("{other:?}"),
    }
    key(&mut a, KeyCode::Char('2'));
    match &a.modal {
        Some(Modal::About(m)) => assert_eq!(m.section, 1),
        other => panic!("{other:?}"),
    }
}
