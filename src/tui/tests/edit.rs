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

#[test]
fn a_recent_post_asks_for_yes_and_then_blocks() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    let recent = (chrono::Utc::now() - chrono::Duration::seconds(30)).to_rfc3339();
    let id = a.base.posts[a.base.cursor].post_id.clone();
    for post in a.base.posts.iter_mut().chain(a.base.source.iter_mut()) {
        if post.post_id == id {
            post.posted_at = recent.clone();
        }
    }
    key(&mut a, KeyCode::Char('p'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Char('j'));
    key(&mut a, KeyCode::Enter);
    let Some(crate::tui::modals::Modal::Confirm(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.strict);
    assert_eq!(m.focus, 1, "Cancel is the default");
    for c in ['y', 'e', 's'] {
        key(&mut a, KeyCode::Char(c));
    }
    key(&mut a, KeyCode::Enter);
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("the edit form is not built"), "{status}");
    assert!(status.contains("are not sent"), "{status}");
    assert!(status.contains("/rs/post/POST/edit"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
}
