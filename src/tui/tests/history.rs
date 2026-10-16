//! Detail stack previous / next: the nav bar buttons, keys, and what clears the history.

use super::*;
use crate::tui::input::{on_key, on_mouse};
use crate::tui::state::{Hit, ViewMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};

fn render(a: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 32)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..32).map(|y| (0..120).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}
fn click(a: &mut App, hit: Hit) {
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?} drawn"));
    on_mouse(a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
}
fn ids(a: &App) -> Vec<String> { a.view().stack.iter().map(|f| f.post.post_id.clone()).collect() }

/// bob's BOB000001 links ALICE0001; open it, then the link.
fn two_deep() -> App {
    let mut a = app();
    a.select_only(nick_idx(&a, "bob"));
    let i = a.view().posts.iter().position(|p| p.post_id == "BOB000001").unwrap();
    a.open_post(i);
    let l = a.top().unwrap().links.iter().position(|l| l.id == "ALICE0001").unwrap();
    a.open_link(l);
    a
}

#[test]
fn prev_and_next_walk_the_stack() {
    let mut a = two_deep();
    assert_eq!(a.stack_pos(), (2, 2));
    let s = render(&mut a);
    assert!(s.contains("◂ prev") && s.contains("next ▸") && s.contains("2/2"));
    click(&mut a, Hit::StackPrev);
    assert_eq!(ids(&a), ["BOB000001"]);
    assert_eq!(a.stack_pos(), (1, 2));
    on_key(&mut a, KeyEvent::new(KeyCode::Char(','), KeyModifiers::NONE));
    assert!(a.view().stack.is_empty(), "prev from the first post goes back to the list");
    assert_eq!(a.stack_pos(), (0, 2));
    on_key(&mut a, KeyEvent::new(KeyCode::Right, KeyModifiers::ALT));
    assert_eq!(ids(&a), ["BOB000001"]);
    render(&mut a);
    click(&mut a, Hit::StackNext);
    assert_eq!(ids(&a), ["BOB000001", "ALICE0001"]);
    assert!(!a.stack_next(), "nothing further");
}

#[test]
fn opening_something_new_clears_next() {
    let mut a = two_deep();
    a.stack_prev();
    a.open_link(0);
    assert_eq!(a.stack_pos().0, a.stack_pos().1, "a new link drops the forward history");
    a.stack_prev();
    a.open_post(0);
    assert_eq!(a.stack_pos(), (1, 1));
}

#[test]
fn esc_and_header_clicks_are_reopenable_too() {
    let mut a = two_deep();
    on_key(&mut a, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(a.stack_pos(), (1, 2));
    a.stack_next();
    a.pop_to(0);
    assert_eq!(a.stack_pos(), (1, 2));
    a.stack_next();
    assert_eq!(ids(&a).len(), 2);
}

#[test]
fn flat_modes_allow_deeper_stacks_and_grid_close_keeps_history() {
    let mut a = two_deep();
    a.set_view_mode(ViewMode::Grid);
    a.close_details();
    assert_eq!(a.stack_pos(), (0, 2));
    a.stack_next();
    a.stack_next();
    assert_eq!(ids(&a), ["BOB000001", "ALICE0001"]);
}
