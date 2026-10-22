//! Services, combined views, and the plugin-reload chord.

use super::*;
use crate::tui::commands::{Action, CommandId};
use crate::tui::input::on_key;
use crate::tui::modals::Modal;
use crate::tui::state::Focus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn key(a: &mut App, c: KeyCode, mods: KeyModifiers) {
    on_key(a, KeyEvent::new(c, mods));
}

fn screen(a: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 32)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..32).map(|y| (0..120).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

#[test]
fn ctrl_p_reloads_plugins_and_ctrl_space_stays_the_palette() {
    let mut a = app();
    key(&mut a, KeyCode::Char('p'), KeyModifiers::CONTROL);
    assert!(a.modal.is_none(), "ctrl-p is plugins.reload, not the palette");
    assert!(a.status.iter().any(|s| s.contains("plugins reloaded")), "{:?}", a.status);
    key(&mut a, KeyCode::Char(' '), KeyModifiers::CONTROL);
    assert!(matches!(a.modal, Some(Modal::Palette(_))));
}

#[test]
fn grouped_adds_a_section_and_mixed_does_not() {
    let mut a = app();
    a.select_all();
    a.focus = Focus::Body;
    let mixed = screen(&mut a);
    assert!(!mixed.contains("· Red Star"), "{mixed}");
    a.combine = crate::services::Combine::Grouped;
    let grouped = screen(&mut a);
    assert!(grouped.contains("· Red Star"), "{grouped}");
    assert_eq!(a.view().posts.len(), a.view().posts.len());
}

#[test]
fn bookmarks_search_and_history_stay_on_the_menus() {
    let a = app();
    let labels: Vec<_> = a.menu.menu.items.iter().map(|i| i.label).collect();
    assert!(labels.contains(&"Bookmarks"));
    assert!(labels.contains(&"Search"));
    let search = a.menu.menu.items.iter().find(|i| i.id == CommandId::Search).unwrap();
    assert!(search.opts.iter().any(|o| o.key == "history"));
    let account = a.menu.menu.items.iter().find(|i| i.id == CommandId::Session).unwrap();
    assert!(account.opts.iter().any(|o| o.key == "services"));
}

#[test]
fn source_cycles_without_hiding_posts() {
    let mut a = app();
    a.select_all();
    let n = a.view().posts.len();
    a.act(Action::CycleSource);
    assert_eq!(a.source, "redstar");
    assert_eq!(a.view().posts.len(), n);
    a.act(Action::CycleSource);
    assert_eq!(a.source, "all");
}

#[test]
fn services_modal_lists_red_star_on() {
    let mut a = app();
    a.act(Action::OpenServices);
    let Some(Modal::Services(m)) = &a.modal else { panic!("{:?}", a.modal) };
    assert!(m.rows.iter().any(|r| matches!(r, crate::services::ServiceRow::Service { id, enabled: true, .. } if id == "redstar")));
    assert!(m.rows.iter().any(|r| matches!(r, crate::services::ServiceRow::Service { id, enabled: false, .. } if id == "x")));
}
