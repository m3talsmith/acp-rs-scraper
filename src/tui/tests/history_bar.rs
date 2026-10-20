//! Search history rows, and the action bar that jumps back to a search.

use super::*;
use crate::tui::commands::{Action, CommandId};
use crate::tui::data::SearchFilter;
use crate::tui::input::{on_key, on_mouse};
use crate::tui::state::Hit;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use std::time::Instant;

fn render(a: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| crate::tui::widgets::draw(f, a, Instant::now())).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn click(a: &mut App, hit: Hit) {
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?}"));
    on_mouse(a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
}

#[test]
fn history_keeps_the_query_and_clear_leaves_posts() {
    let mut a = app();
    a.run_search(SearchFilter { keyword: "strike".into(), ..Default::default() });
    a.run_search(SearchFilter { keyword: "planning".into(), limit: Some(1), ..Default::default() });
    let rows = a.store.search_history().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].keyword, "planning");
    assert_eq!(rows[0].limit, Some(1));
    assert!(!rows[0].site);
    a.act(Action::ClearSearchHistory);
    assert!(a.store.search_history().unwrap().is_empty());
    assert!(a.store.nicks().unwrap().iter().any(|n| n.nick == "alice"));
}

#[test]
fn a_cap_drops_older_queries() {
    let mut a = app();
    a.menu.menu.set_text(CommandId::Search, "keep", "1");
    a.run_search(SearchFilter { keyword: "strike".into(), ..Default::default() });
    a.run_search(SearchFilter { keyword: "planning".into(), ..Default::default() });
    let rows = a.store.search_history().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].keyword, "planning");
}

#[test]
fn picking_history_reruns_the_query() {
    let mut a = app();
    a.run_search(SearchFilter { keyword: "strike".into(), ..Default::default() });
    a.clear_search();
    a.act(Action::SearchHistory);
    on_key(&mut a, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let view = a.search.as_ref().unwrap();
    assert_eq!(view.filter.as_ref().unwrap().keyword, "strike");
}

#[test]
fn the_action_bar_jumps_back_to_a_search() {
    let mut a = app();
    a.run_search(SearchFilter { keyword: "strike".into(), ..Default::default() });
    a.open_bookmarks();
    assert!(a.search.as_ref().unwrap().bookmarks);
    a.actions_open = true;
    let s = render(&mut a, 140, 36);
    assert!(s.contains("Search:"), "{s}");
    click(&mut a, Hit::Action(0));
    let view = a.search.as_ref().unwrap();
    assert_eq!(view.filter.as_ref().unwrap().keyword, "strike");
    assert!(!view.bookmarks);
}

#[test]
fn the_nick_bar_collapses() {
    let mut a = app();
    let s = render(&mut a, 120, 30);
    assert!(s.contains("@alice"), "{s}");
    click(&mut a, Hit::ToggleNicks);
    assert!(!a.nicks_open);
    let s = render(&mut a, 120, 30);
    assert!(s.contains('›'), "{s}");
    click(&mut a, Hit::ToggleActions);
    assert!(a.actions_open);
}

#[test]
fn a_search_export_argv_carries_the_query_json() {
    let mut m = crate::tui::commands::Menu::new(&no_args());
    m.set_text(crate::tui::commands::CommandId::Export, "path", "/tmp/out.json");
    m.set_choice(crate::tui::commands::CommandId::Export, "scope", "Current view");
    let f = SearchFilter { keyword: "strike".into(), from: "2026-10-01".into(), limit: Some(2), ..Default::default() };
    let argv = m.export_argv(&[], Some(&f)).unwrap();
    let args = parse_with_mode(&argv).unwrap().0;
    let v: serde_json::Value = serde_json::from_str(args.search_json.as_deref().unwrap()).unwrap();
    assert_eq!(v["keyword"], "strike");
    assert_eq!(v["from"], "2026-10-01");
    assert_eq!(v["limit"], 2);
    assert!(args.post_ids.is_none());
}
