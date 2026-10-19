//! Compose: Cancel sends nothing, Publish is gated, `r` on a comment pre-fills the ref.

use super::*;
use crate::tui::input::on_key;
use crate::tui::modals::{ComposeFocus, ComposeKind, Modal};
use crate::tui::state::Focus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};

fn key(a: &mut App, c: KeyCode) {
    on_key(a, KeyEvent::new(c, KeyModifiers::NONE));
}

fn render(a: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
    t.draw(|f| crate::tui::widgets::draw(f, a, Instant::now() + Duration::from_secs(5))).unwrap();
    let buf = t.backend().buffer().clone();
    (0..30).map(|y| (0..100).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

#[test]
fn new_post_cancel_sends_nothing_and_publish_is_gated() {
    let mut a = app();
    key(&mut a, KeyCode::Char('n'));
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.focus, ComposeFocus::Cancel);
    assert!(matches!(m.kind, ComposeKind::NewPost));
    key(&mut a, KeyCode::Enter);
    assert!(a.modal.is_none());
    assert!(a.status.iter().any(|s| s.contains("nothing sent")));

    key(&mut a, KeyCode::Char('n'));
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::Char('T'));
    key(&mut a, KeyCode::Enter);
    key(&mut a, KeyCode::Char('h'));
    key(&mut a, KeyCode::Char('i'));
    key(&mut a, KeyCode::Tab);
    key(&mut a, KeyCode::Tab);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("still composing: {:?}", a.modal); };
    assert_eq!(m.focus, ComposeFocus::Publish);
    assert_eq!(m.title, "T");
    assert!(m.body.starts_with("hi"), "{}", m.body);
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Message(m)) = &a.modal else { panic!("{:?}", a.modal); };
    let text = m.lines.join("\n");
    assert!(text.contains("nothing was published"), "{text}");
    assert!(a.last_write_unix.is_some());
    let screen = render(&mut a);
    assert!(screen.contains("Write") || screen.contains("nothing was published"), "{screen}");
}

#[test]
fn reply_on_a_comment_prefixes_the_ref_and_quote_uses_the_body() {
    let mut a = app();
    a.select_only(nick_idx(&a, "bob"));
    a.focus = Focus::Body;
    let i = a.view().posts.iter().position(|p| p.post_id == "BOB000002").unwrap();
    a.open_post(i);
    key(&mut a, KeyCode::Char('r'));
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    match &m.kind {
        ComposeKind::Reply { thread, in_reply_to } => {
            assert_eq!(thread, "ALICE0001");
            assert_eq!(in_reply_to.as_deref(), Some("BOB000002"));
        }
        other => panic!("{other:?}"),
    }
    a.modal = None;
    key(&mut a, KeyCode::Char('Q'));
    let plan = {
        let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
        assert_eq!(m.quote.as_deref(), Some("Reply to alice"));
        assert_eq!(m.quote_post.as_deref(), Some("BOB000002"));
        crate::site::prepare(&m.draft(0, None, true)).unwrap()
    };
    assert_eq!(plan.body["content"][0]["type"], "postRef");
    assert_eq!(plan.body["content"][1]["type"], "blockquote");
    key(&mut a, KeyCode::Esc);
    assert!(a.modal.is_none());
}

#[test]
fn mention_completion_inserts_the_nick_and_backspace_tracks_the_body() {
    let mut a = app();
    key(&mut a, KeyCode::Char('n'));
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::Char('@'));
    key(&mut a, KeyCode::Char('a'));
    key(&mut a, KeyCode::Down);
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.body.contains("@alice") || m.body.contains("@bob"), "{}", m.body);
    assert!(m.pending.is_none(), "{:?}", m.pending);
    assert_eq!(m.focus, ComposeFocus::Body);
}

#[test]
fn list_r_is_still_research() {
    let mut a = app();
    a.select_only(nick_idx(&a, "bob"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('r'));
    assert!(matches!(a.modal, Some(Modal::Research(_))), "{:?}", a.modal);
}
