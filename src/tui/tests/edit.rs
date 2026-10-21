//! Post menu › Edit keeps the one-hour window and does not send.

use super::*;
use crate::tui::input::on_key;
use crate::tui::state::Focus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(a: &mut App, c: KeyCode) {
    on_key(a, KeyEvent::new(c, KeyModifiers::NONE));
}

#[test]
fn an_old_post_is_outside_the_edit_window() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('p'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Enter);
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("one hour"), "{status}");
    assert!(status.contains("Nothing was sent"), "{status}");
    assert!(!status.contains("nothing was published"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
}
