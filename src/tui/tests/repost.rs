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
    let Some(crate::tui::modals::Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(matches!(m.kind, crate::tui::modals::ComposeKind::Repost { .. }));
    assert_eq!(m.focus, crate::tui::modals::ComposeFocus::Cancel);
    key(&mut a, KeyCode::Enter);
    assert!(a.modal.is_none(), "{:?}", a.modal);
    assert!(a.effects.is_empty());
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("nothing sent"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
}

#[test]
fn a_repost_without_a_quote_stays_on_the_compose_box() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('p'));
    for _ in 0..5 {
        key(&mut a, KeyCode::Char('j'));
    }
    key(&mut a, KeyCode::Enter);
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::BackTab);
    for c in ['N', 'o', 't', 'e', 'd'] {
        key(&mut a, KeyCode::Char(c));
    }
    for _ in 0..4 {
        key(&mut a, KeyCode::Tab);
    }
    key(&mut a, KeyCode::Enter);
    let Some(crate::tui::modals::Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    let err = m.error.clone().unwrap_or_default();
    assert!(err.contains("does not allow a plain repost"), "{err}");
    assert!(a.effects.is_empty());
}
