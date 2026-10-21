//! Post menu › Subscribe prepares the hook and does not send it.

use super::*;
use crate::tui::input::on_key;
use crate::tui::modals::Modal;
use crate::tui::state::Focus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(a: &mut App, c: KeyCode) {
    on_key(a, KeyEvent::new(c, KeyModifiers::NONE));
}

#[test]
fn subscribe_from_the_post_menu_is_blocked() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('p'));
    let Some(Modal::Post(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.items.iter().any(|i| i.label() == "Subscribe"));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Enter);
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("are not sent"), "{status}");
    assert!(status.contains("\"event\": \"subscribe\""), "{status}");
    assert!(status.contains("state 1"), "{status}");
    assert!(status.contains("capture still needed"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
}
