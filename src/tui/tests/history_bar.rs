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

fn labels(a: &App, branch: usize) -> Vec<String> {
    a.branches[branch].steps.iter().map(|s| s.label.clone()).collect()
}

fn search(a: &mut App, keyword: &str) {
    a.run_search(SearchFilter { keyword: keyword.into(), ..Default::default() });
}

#[test]
fn the_action_bar_keeps_every_step_and_scrolls() {
    let mut a = app();
    for i in 0..50 { search(&mut a, &format!("s{i:02}")); }
    assert_eq!(a.action_steps().len(), 50);
    assert_eq!(a.branches.len(), 1);
    a.actions_open = true;
    a.focus = crate::tui::state::Focus::Actions;
    a.action_scroll = 0;
    a.action_cursor = 0;
    let s = render(&mut a, 140, 24);
    assert!(s.contains("/50"), "{s}");
    assert!(s.contains("s00"), "{s}");
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| matches!(h, Hit::Action(_))).unwrap();
    on_mouse(&mut a, MouseEvent { kind: MouseEventKind::ScrollDown, column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    assert!(a.action_scroll >= 3, "{}", a.action_scroll);
    a.focus = crate::tui::state::Focus::Actions;
    on_key(&mut a, KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert!(a.action_scroll > 3, "{}", a.action_scroll);
    a.scroll_actions(1_000);
    let s = render(&mut a, 140, 24);
    assert!(s.contains("s49"), "{s}");
    click(&mut a, Hit::Action(49));
    assert_eq!(a.search.as_ref().unwrap().filter.as_ref().unwrap().keyword, "s49");
    assert_eq!(a.action_steps().len(), 50);
    click(&mut a, Hit::ToggleActions);
    assert!(!a.actions_open);
}

#[test]
fn a_jump_then_a_new_step_forks_the_old_branch() {
    let mut a = app();
    search(&mut a, "strike");
    search(&mut a, "planning");
    a.open_bookmarks();
    assert_eq!(labels(&a, 0), ["Search: \"strike\"", "Search: \"planning\"", "Bookmarks"]);
    a.jump_action(0);
    assert_eq!(a.search.as_ref().unwrap().filter.as_ref().unwrap().keyword, "strike");
    search(&mut a, "docks");
    assert_eq!(a.branches.len(), 2);
    assert_eq!(a.branch_at, 1);
    assert_eq!(labels(&a, 1), ["Search: \"strike\"", "Search: \"docks\""]);
    assert_eq!(labels(&a, 0), ["Search: \"strike\"", "Search: \"planning\"", "Bookmarks"]);
    a.actions_open = true;
    a.focus = crate::tui::state::Focus::Actions;
    on_key(&mut a, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
    match &a.modal {
        Some(crate::tui::modals::Modal::Branches(m)) => assert_eq!(m.cursor, 1),
        other => panic!("{other:?}"),
    }
    on_key(&mut a, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    on_key(&mut a, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(a.branch_at, 0);
    assert!(a.search.as_ref().unwrap().bookmarks);
    assert_eq!(labels(&a, 0), ["Search: \"strike\"", "Search: \"planning\"", "Bookmarks"]);
    assert_eq!(a.action_at, 2);
}

#[test]
fn branches_survive_a_restart_and_clear_history_drops_them() {
    let dir = std::env::temp_dir().join(format!("acp-actions-db-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rs.database");
    crate::storage::db::open_db(&path).unwrap();
    let mut a = App::new(Box::new(crate::tui::data::SqliteStore::new(path.clone())), &no_args(), Theme::acp(), false);
    search(&mut a, "strike");
    a.open_bookmarks();
    a.jump_action(0);
    search(&mut a, "docks");
    assert_eq!(a.branches.len(), 2);
    drop(a);
    let mut b = App::new(Box::new(crate::tui::data::SqliteStore::new(path.clone())), &no_args(), Theme::acp(), false);
    assert_eq!(b.branches.len(), 2);
    assert_eq!(b.branch_at, 1);
    assert_eq!(labels(&b, 1), ["Search: \"strike\"", "Search: \"docks\""]);
    assert_eq!(labels(&b, 0), ["Search: \"strike\"", "Bookmarks"]);
    b.switch_branch(0);
    assert!(b.search.as_ref().unwrap().bookmarks, "the old branch restores its tip");
    assert_eq!(b.action_steps().len(), 2);
    b.act(Action::ClearSearchHistory);
    assert!(b.branches.is_empty());
    assert!(b.store.action_tree_body().unwrap().is_none());
    let c = crate::storage::db::open_db(&path).unwrap();
    assert!(crate::storage::history::list(&c).unwrap().is_empty());
    assert!(crate::storage::history::load_action_body(&c).unwrap().is_none());
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
