//! Toasts, the notifications page, and the saved list.

use super::*;
use crate::tui::commands::Action;
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

fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }

fn steps(a: &App) -> usize { a.action_steps().len() }

#[test]
fn errors_stack_as_toasts_and_a_click_dismisses_one() {
    let mut a = app();
    a.push_status("error: first fault");
    a.push_status("error: second fault");
    let s = render(&mut a, 120, 30);
    assert!(s.contains("▸ Error"), "{s}");
    assert!(s.contains("first fault"), "{s}");
    assert!(s.contains("second fault"), "{s}");
    assert!(!s.contains("View all"), "{s}");
    click(&mut a, Hit::Toast(0));
    assert_eq!(a.notices.toasts.len(), 1);
    assert_eq!(a.notices.toasts[0].notice.body, "second fault");
    assert_eq!(a.notices.log.len(), 2);
}

#[test]
fn notices_are_not_capped() {
    let mut a = app();
    for i in 0..24 {
        a.push_status(format!("error: fault-{i}"));
    }
    assert_eq!(a.notices.toasts.len(), 24);
    assert_eq!(a.notices.log.len(), 24);
    assert_eq!(a.store.notices().unwrap().len(), 24);
}

#[test]
fn a_tall_stack_shows_view_all_inside_a_quarter() {
    let mut a = app();
    let before = steps(&a);
    for body in ["zz-old-one", "zz-old-two", "zz-new-three", "zz-new-four"] {
        a.push_status(format!("error: {body}"));
    }
    let s = render(&mut a, 80, 20);
    assert!(s.contains("View all"), "{s}");
    assert!(s.contains("zz-new-four"), "{s}");
    assert!(s.contains("zz-new-three"), "{s}");
    assert!(!s.contains("zz-old-one"), "{s}");
    assert_eq!(a.notices.log.len(), 4);
    click(&mut a, Hit::ViewAllNotices);
    assert!(a.notice_page.is_some());
    assert_eq!(steps(&a), before);
    let s = render(&mut a, 80, 30);
    assert!(s.contains("Notifications"), "{s}");
    assert!(s.contains("zz-old-one"), "{s}");
    assert!(s.contains("zz-new-four"), "{s}");
}

#[test]
fn do_not_disturb_keeps_the_log_and_skips_the_toast() {
    let mut a = app();
    a.notices.settings.dnd = true;
    a.push_status("error: hidden fault");
    assert!(a.notices.toasts.is_empty());
    assert_eq!(a.notices.log[0].body, "hidden fault");
    let s = render(&mut a, 120, 30);
    assert!(!s.contains("▸ Error"), "{s}");
    a.act(Action::OpenNotifications);
    let s = render(&mut a, 120, 30);
    assert!(s.contains("Do not disturb"), "{s}");
    assert!(s.contains("hidden fault"), "{s}");
    key(&mut a, KeyCode::Char(' '));
    assert!(!a.notices.settings.dnd);
}

#[test]
fn order_search_delete_and_clear_all_leave_the_action_bar_alone() {
    let mut a = app();
    a.run_search(SearchFilter { keyword: "strike".into(), ..Default::default() });
    let kept = steps(&a);
    let branches = a.branches.len();
    a.push_status("error: alpha-fault");
    a.push_status("warning: beta-fault");
    a.act(Action::OpenNotifications);
    assert_eq!(a.visible_notices()[0].body, "beta-fault");
    let s = render(&mut a, 120, 36);
    let beta = s.find("beta-fault").unwrap();
    let alpha = s.find("alpha-fault").unwrap();
    assert!(beta < alpha, "{s}");
    key(&mut a, KeyCode::Char('o'));
    assert_eq!(a.visible_notices()[0].body, "alpha-fault");
    let s = render(&mut a, 120, 36);
    assert!(s.find("alpha-fault").unwrap() < s.find("beta-fault").unwrap(), "{s}");
    key(&mut a, KeyCode::Char('/'));
    for c in ['b', 'e', 't', 'a'] { key(&mut a, KeyCode::Char(c)); }
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.visible_notices().len(), 1);
    assert_eq!(a.visible_notices()[0].body, "beta-fault");
    let s = render(&mut a, 120, 36);
    assert!(s.contains("beta-fault"), "{s}");
    assert!(!s.contains("alpha-fault"), "{s}");
    click(&mut a, Hit::NoticeClear(0));
    assert_eq!(a.notices.log.len(), 1);
    assert!(a.notices.log.iter().all(|n| n.body != "beta-fault"));
    assert!(a.store.notices().unwrap().iter().all(|n| n.body != "beta-fault"));
    key(&mut a, KeyCode::Char('X'));
    assert!(a.notices.log.is_empty());
    assert!(a.store.notices().unwrap().is_empty());
    assert_eq!(steps(&a), kept);
    assert_eq!(a.branches.len(), branches);
}

#[test]
fn kind_filter_keeps_one_kind() {
    let mut a = app();
    a.push_status("error: kind-error");
    a.push_status("warning: kind-warning");
    a.act(Action::OpenNotifications);
    key(&mut a, KeyCode::Char('f'));
    assert_eq!(a.visible_notices().len(), 1);
    assert_eq!(a.visible_notices()[0].body, "kind-error");
    let s = render(&mut a, 120, 36);
    assert!(s.contains("Kind: Error"), "{s}");
    assert!(s.contains("Error: kind-error"), "{s}");
    assert!(!s.contains("Warning: kind-warning"), "{s}");
}

#[test]
fn notifications_survive_a_restart() {
    let dir = std::env::temp_dir().join(format!("acp-notices-db-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rs.database");
    crate::storage::db::open_db(&path).unwrap();
    let mut a = App::new(Box::new(crate::tui::data::SqliteStore::new(path.clone())), &no_args(), Theme::acp(), false);
    a.push_status("error: kept-fault");
    a.push_status("warning: later-fault");
    assert_eq!(a.notices.log.len(), 2);
    drop(a);
    let b = App::new(Box::new(crate::tui::data::SqliteStore::new(path.clone())), &no_args(), Theme::acp(), false);
    assert!(b.notices.toasts.is_empty(), "a restart does not raise the old toasts again");
    assert_eq!(b.notices.log.len(), 2);
    assert_eq!(b.notices.log[0].body, "later-fault");
    assert_eq!(b.notices.log[1].body, "kept-fault");
    let rows = b.store.notices().unwrap();
    assert_eq!(rows[0].body, "later-fault");
    assert_eq!(rows[1].kind, "error");
}
