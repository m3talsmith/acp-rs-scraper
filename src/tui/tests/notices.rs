//! Toasts, do-not-disturb, and the notice log.

use super::*;
use crate::tui::commands::Action;
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
fn errors_stack_as_toasts_and_a_click_dismisses_one() {
    let mut a = app();
    a.push_status("error: first fault");
    a.push_status("error: second fault");
    let s = render(&mut a, 120, 30);
    assert!(s.contains("▸ Error"), "{s}");
    assert!(s.contains("first fault"), "{s}");
    assert!(s.contains("second fault"), "{s}");
    click(&mut a, Hit::Toast(0));
    assert_eq!(a.notices.toasts.len(), 1);
    assert_eq!(a.notices.toasts[0].notice.body, "second fault");
    assert_eq!(a.notices.log.len(), 2);
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
    a.act(Action::OpenNotices);
    let s = render(&mut a, 120, 30);
    assert!(s.contains("Do not disturb"), "{s}");
    assert!(s.contains("hidden fault"), "{s}");
    on_key(&mut a, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(!a.notices.settings.dnd);
}
