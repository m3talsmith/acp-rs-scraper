//! Post menu › Repost names the route and does not send.

use super::*;
use crate::tui::input::on_key;
use crate::tui::state::Focus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(a: &mut App, c: KeyCode) {
    on_key(a, KeyEvent::new(c, KeyModifiers::NONE));
}

#[test]
fn repost_from_the_post_menu_is_blocked() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('p'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Enter);
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("nothing was published"), "{status}");
    assert!(status.contains("the repost form is not built"), "{status}");
    assert!(status.contains("/rs/post/new?repost="), "{status}");
    assert!(status.contains("capture still needed"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
}
