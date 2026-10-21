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
    let Some(Modal::Confirm(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.title.contains("Vote"), "{}", m.title);
    assert_eq!(m.focus, 1, "Cancel is the default");
    key(&mut a, KeyCode::Esc);
    assert!(a.modal.is_none());
    key(&mut a, KeyCode::Char('+'));
    key(&mut a, KeyCode::Char('y'));
    assert!(a.modal.is_none(), "{:?}", a.modal);
    assert!(a.effects.iter().any(|e| matches!(e, crate::tui::state::Effect::Vote { state, .. } if state == "1")));
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("voting"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
    a.effects.clear();
    key(&mut a, KeyCode::Char('-'));
    let Some(Modal::Confirm(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.focus, 1);
    key(&mut a, KeyCode::Char('y'));
    assert!(a.effects.iter().any(|e| matches!(e, crate::tui::state::Effect::Vote { state, .. } if state == "-1")));
}
