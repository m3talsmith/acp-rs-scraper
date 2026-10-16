//! View modes: switching, per-mode navigation, the grid modal, and headless renders of each.

use super::*;
use crate::tui::commands::Action;
use crate::tui::input::{on_key, on_mouse};
use crate::tui::layout::{carousel, card_height, grid_cells, GRID_CELL_H};
use crate::tui::state::{Hit, ViewMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::{Terminal, backend::TestBackend};

fn render(a: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}
fn click(a: &mut App, hit: Hit) {
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?} drawn"));
    on_mouse(a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
}
fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }
fn both(a: &mut App) { a.select_all(); a.focus = crate::tui::state::Focus::Body; }

#[test]
fn v_and_the_nav_bar_cycle_every_mode() {
    let mut a = app();
    both(&mut a);
    for m in ViewMode::ALL.iter().skip(1) { key(&mut a, KeyCode::Char('v')); assert_eq!(a.view_mode, *m); }
    key(&mut a, KeyCode::Char('v'));
    assert_eq!(a.view_mode, ViewMode::Nested);
    let s = render(&mut a, 120, 32);
    assert!(s.contains("[View: Nested ▸]"));
    click(&mut a, Hit::ViewMode);
    assert_eq!(a.view_mode, ViewMode::Stack);
    a.act(Action::SetView(ViewMode::Grid));
    assert_eq!(a.view_mode, ViewMode::Grid);
}

#[test]
fn every_mode_renders_list_and_detail() {
    for m in ViewMode::ALL {
        let mut a = app();
        both(&mut a);
        a.set_view_mode(m);
        let s = render(&mut a, 120, 32);
        assert!(s.contains("@alice"), "{m:?} list:\n{s}");
        assert!(a.hits.iter().any(|(_, h)| matches!(h, Hit::Post(_))), "{m:?} has clickable posts");
        a.open_post(0);
        let s = render(&mut a, 120, 32);
        assert!(a.hits.iter().any(|(_, h)| *h == Hit::DetailPane), "{m:?} draws the detail:\n{s}");
        assert!(a.hits.iter().any(|(_, h)| *h == Hit::StarTop), "{m:?} detail star");
    }
}

#[test]
fn carousel_infinite_wraps_fixed_stops() {
    let mut a = app();
    both(&mut a);
    let n = a.view().posts.len();
    a.set_view_mode(ViewMode::CarouselInfinite);
    key(&mut a, KeyCode::Left);
    assert_eq!(a.view().cursor, n - 1, "wraps to the last post");
    key(&mut a, KeyCode::Right);
    assert_eq!(a.view().cursor, 0);
    a.set_view_mode(ViewMode::CarouselFixed);
    key(&mut a, KeyCode::Left);
    assert_eq!(a.view().cursor, 0, "stops at the first post");
    let s = render(&mut a, 120, 32);
    assert!(s.contains(&format!("1/{n}")));
}

#[test]
fn grid_moves_by_rows_and_opens_a_closable_modal() {
    let mut a = app();
    both(&mut a);
    a.set_view_mode(ViewMode::Grid);
    render(&mut a, 120, 40);
    assert!(a.grid_cols >= 2);
    key(&mut a, KeyCode::Down);
    assert_eq!(a.view().cursor, a.grid_cols);
    key(&mut a, KeyCode::Left);
    assert_eq!(a.view().cursor, a.grid_cols - 1);
    key(&mut a, KeyCode::Enter);
    let s = render(&mut a, 120, 40);
    assert!(s.contains("[ Close ]"));
    click(&mut a, Hit::CloseDetail);
    assert!(a.view().stack.is_empty());
}

#[test]
fn pane_follows_the_list() {
    let mut a = app();
    both(&mut a);
    a.set_view_mode(ViewMode::Pane);
    a.open_post(0);
    key(&mut a, KeyCode::Down);
    assert_eq!(a.view().cursor, 1);
    assert_eq!(a.top().unwrap().post.post_id, a.view().posts[1].post_id, "the pane shows the highlighted post");
}

#[test]
fn layout_helpers() {
    let r = Rect { x: 0, y: 0, width: 100, height: 20 };
    let (cols, cells) = grid_cells(r, 10, 0);
    assert_eq!(cols, 2);
    assert_eq!(cells.len(), 2 * (20 / GRID_CELL_H) as usize);
    assert_eq!(cells[1].1.x, 50);
    let c = carousel(r);
    assert_eq!(c.prev.width + c.center.width + c.next.width, 100);
    assert_eq!(card_height("one two three", 20, 10), 3);
    assert_eq!(card_height(&"word ".repeat(200), 20, 10), 10);
}
