//! Post menu › Delete asks for yes and does not send.

use super::*;
use crate::tui::input::on_key;
use crate::tui::modals::Modal;
use crate::tui::state::Focus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(a: &mut App, c: KeyCode) {
    on_key(a, KeyEvent::new(c, KeyModifiers::NONE));
}

#[test]
fn delete_requires_yes_and_then_blocks() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('p'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Confirm(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.strict);
    assert_eq!(m.focus, 1, "Cancel is the default");
    key(&mut a, KeyCode::Esc);
    assert!(a.modal.is_none());
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(!status.contains("nothing was published"), "{status}");

    key(&mut a, KeyCode::Char('p'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Enter);
    for c in ['y', 'e', 's'] {
        key(&mut a, KeyCode::Char(c));
    }
    key(&mut a, KeyCode::Enter);
    assert!(a.modal.is_none(), "{:?}", a.modal);
    assert!(a.effects.iter().any(|e| matches!(e, crate::tui::state::Effect::Delete { .. })));
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("deleting"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
}
