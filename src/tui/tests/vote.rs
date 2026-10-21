//! `+` and `-` prepare a vote and do not send it.

use super::*;
use crate::tui::input::on_key;
use crate::tui::modals::Modal;
use crate::tui::state::Focus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(a: &mut App, c: KeyCode) {
    on_key(a, KeyEvent::new(c, KeyModifiers::NONE));
}

#[test]
fn plus_prepares_an_upvote_and_does_not_send_it() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('+'));
    let Some(Modal::Message(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.title.contains("Vote"), "{}", m.title);
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("are not sent"), "{status}");
    assert!(status.contains("\"event\": \"vote\""), "{status}");
    assert!(status.contains("state 1"), "{status}");
    assert!(status.contains("capture still needed"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
    a.modal = None;
    key(&mut a, KeyCode::Char('-'));
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("state -1"), "{status}");
    assert!(status.contains("are not sent"), "{status}");
}
