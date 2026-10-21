//! Compose: Cancel sends nothing, a text Publish is queued, a file is refused.

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
    // Cancel, Attach, Body, Title.
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::Char('T'));
    key(&mut a, KeyCode::Enter);
    key(&mut a, KeyCode::Char('h'));
    key(&mut a, KeyCode::Char('i'));
    // Body, Attach, Cancel, Publish.
    key(&mut a, KeyCode::Tab);
    key(&mut a, KeyCode::Tab);
    key(&mut a, KeyCode::Tab);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("still composing: {:?}", a.modal); };
    assert_eq!(m.focus, ComposeFocus::Publish);
    assert_eq!(m.title, "T");
    assert!(m.body.starts_with("hi"), "{}", m.body);
    key(&mut a, KeyCode::Enter);
    assert!(a.modal.is_none(), "the compose box closes while the send runs: {:?}", a.modal);
    assert!(a.status.iter().any(|s| s.contains("publishing")), "{:?}", a.status);
    assert!(a.effects.iter().any(|e| matches!(e, crate::tui::state::Effect::Publish(_))));
    assert!(a.last_write_unix.is_some());
    let screen = render(&mut a);
    assert!(!screen.contains("nothing was published"), "{screen}");
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
fn reply_on_a_root_post_uses_the_thread_as_post_ref() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    let i = a.view().posts.iter().position(|p| p.post_id == "ALICE0001").unwrap();
    a.open_post(i);
    key(&mut a, KeyCode::Char('r'));
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    match &m.kind {
        ComposeKind::Reply { thread, in_reply_to } => {
            assert_eq!(thread, "ALICE0001");
            assert!(in_reply_to.is_none());
        }
        other => panic!("{other:?}"),
    }
    let plan = {
        let Some(Modal::Compose(m)) = &mut a.modal else { panic!("{:?}", a.modal); };
        m.body = "hello".into();
        crate::site::prepare(&m.draft(0, None, true)).unwrap()
    };
    assert_eq!(plan.body["content"][0]["attrs"]["postId"], "ALICE0001");
    assert_eq!(plan.body["content"][0]["attrs"]["threadId"], "ALICE0001");
    let draft = {
        let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
        m.draft(0, None, true)
    };
    let crate::site::Gate::Send(plan) = crate::site::decide(&draft, true, false).unwrap() else {
        panic!("a text reply is ready to send");
    };
    let payloads = crate::site::event_payloads(&plan);
    assert_eq!(payloads.len(), 2);
    assert_eq!(payloads[0]["event"], "validate");
    assert_eq!(payloads[1]["event"], "save");
    assert!(plan.attachments.is_empty());
}

#[test]
fn mention_completion_inserts_the_nick_and_backspace_tracks_the_body() {
    let mut a = app();
    key(&mut a, KeyCode::Char('n'));
    // Cancel, Attach, Body.
    key(&mut a, KeyCode::BackTab);
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
fn attach_line_lists_a_local_file_and_publish_stays_gated() {
    let dir = std::env::temp_dir().join(format!("acp-compose-attach-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let jpg = dir.join("photo.jpg");
    std::fs::write(&jpg, b"jpeg").unwrap();
    let mut a = app();
    key(&mut a, KeyCode::Char('n'));
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::Char('T'));
    key(&mut a, KeyCode::Enter);
    key(&mut a, KeyCode::Char('h'));
    key(&mut a, KeyCode::Char('i'));
    key(&mut a, KeyCode::Tab);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.focus, ComposeFocus::Attach);
    for c in jpg.display().to_string().chars() {
        key(&mut a, KeyCode::Char(c));
    }
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.attach_path.is_empty(), "{}", m.attach_path);
    assert_eq!(m.attachments.len(), 1);
    assert!(m.error.is_none(), "{:?}", m.error);
    let plan = crate::site::prepare(&m.draft(0, None, true)).unwrap();
    assert_eq!(plan.attachments[0].kind, crate::site::AttachmentKind::Media);
    let listed = crate::site::dry_run_text(&plan);
    assert!(listed.contains("media photo.jpg"), "{listed}");
    assert!(listed.contains("allow_upload"), "{listed}");
    assert!(!listed.contains("nothing was published"), "{listed}");
    let frames = serde_json::to_string(&crate::site::liveview_frames(&plan)).unwrap();
    assert!(!frames.contains("photo.jpg"), "{frames}");
    assert!(!frames.contains("allow_upload"), "{frames}");
    assert!(!frames.contains("lvu"), "{frames}");
    let screen = render(&mut a);
    assert!(screen.contains("photo.jpg"), "{screen}");
    assert!(screen.contains("Attach"), "{screen}");
    key(&mut a, KeyCode::Tab);
    key(&mut a, KeyCode::Tab);
    key(&mut a, KeyCode::Enter);
    assert!(a.modal.is_none(), "publish closes the compose box: {:?}", a.modal);
    assert!(a.status.iter().any(|s| s.contains("publishing")), "{:?}", a.status);
    assert!(a.effects.iter().any(|e| matches!(e, crate::tui::state::Effect::Publish(_))));
    assert!(a.last_write_unix.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reply_attach_line_accepts_a_pdf_and_drops_it_on_backspace() {
    let dir = std::env::temp_dir().join(format!("acp-compose-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pdf = dir.join("notes.pdf");
    let bad = dir.join("nope.txt");
    std::fs::write(&pdf, b"pdf").unwrap();
    std::fs::write(&bad, b"nope").unwrap();
    let mut a = app();
    a.select_only(nick_idx(&a, "bob"));
    a.focus = Focus::Body;
    let i = a.view().posts.iter().position(|p| p.post_id == "BOB000002").unwrap();
    a.open_post(i);
    key(&mut a, KeyCode::Char('r'));
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::Char('o'));
    key(&mut a, KeyCode::Char('k'));
    key(&mut a, KeyCode::Tab);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(matches!(m.kind, ComposeKind::Reply { .. }));
    assert_eq!(m.focus, ComposeFocus::Attach);
    for c in pdf.display().to_string().chars() {
        key(&mut a, KeyCode::Char(c));
    }
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    let plan = crate::site::prepare(&m.draft(0, None, true)).unwrap();
    assert_eq!(plan.attachments[0].kind, crate::site::AttachmentKind::File);
    assert_eq!(plan.attachments[0].kind.input_name(), "file_attachments");
    let frames = serde_json::to_string(&crate::site::liveview_frames(&plan)).unwrap();
    assert!(!frames.contains("notes.pdf"), "{frames}");
    assert!(!frames.contains("file_attachments"), "{frames}");
    for c in bad.display().to_string().chars() {
        key(&mut a, KeyCode::Char(c));
    }
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.error.is_none(), "{:?}", m.error);
    assert_eq!(m.attachments.len(), 2, "a text file stays attached locally");
    let plan = crate::site::prepare(&m.draft(0, None, true)).unwrap();
    assert!(plan.attachments[0].on_site, "a pdf is uploaded");
    assert!(!plan.attachments[1].on_site, "a text file stays off the site");
    assert_eq!(plan.attachments[0].kind.input_name(), "file_attachments");
    assert!(m.attach_path.is_empty());
    key(&mut a, KeyCode::Backspace);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.attachments.len(), 1, "backspace drops the text file");
    key(&mut a, KeyCode::Backspace);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.attachments.is_empty(), "{:?}", m.attachments);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn attach_tab_completes_a_real_segment_and_an_empty_line_still_moves() {
    let dir = std::env::temp_dir().join(format!(
        "acp-compose-tab-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let photo = dir.join("photo.jpg");
    std::fs::write(&photo, b"j").unwrap();
    std::fs::write(dir.join("other.jpg"), b"o").unwrap();
    let mut a = app();
    key(&mut a, KeyCode::Char('n'));
    key(&mut a, KeyCode::BackTab);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.focus, ComposeFocus::Attach);
    key(&mut a, KeyCode::Tab);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.focus, ComposeFocus::Cancel, "an empty attach line still moves focus");

    key(&mut a, KeyCode::BackTab);
    for c in format!("{}/p", dir.display()).chars() {
        key(&mut a, KeyCode::Char(c));
    }
    key(&mut a, KeyCode::Tab);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.focus, ComposeFocus::Attach);
    assert_eq!(m.attach_path, photo.display().to_string());
    assert!(m.path_matches.is_empty(), "{:?}", m.path_matches);
    assert!(m.attachments.is_empty());
    key(&mut a, KeyCode::Tab);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.focus, ComposeFocus::Cancel, "a second Tab on a finished file moves focus");

    key(&mut a, KeyCode::BackTab);
    let typed = {
        let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
        m.attach_path.chars().count()
    };
    for _ in 0..typed {
        key(&mut a, KeyCode::Backspace);
    }
    for c in format!("{}/", dir.display()).chars() {
        key(&mut a, KeyCode::Char(c));
    }
    key(&mut a, KeyCode::Tab);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.path_matches.len(), 2, "{:?}", m.path_matches);
    assert_eq!(m.focus, ComposeFocus::Attach);
    key(&mut a, KeyCode::Esc);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("Esc stays on the compose box: {:?}", a.modal); };
    assert!(m.path_matches.is_empty());
    assert_eq!(m.focus, ComposeFocus::Attach);
    key(&mut a, KeyCode::Tab);
    key(&mut a, KeyCode::Down);
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert!(m.attachments.is_empty(), "choosing a match does not attach it");
    assert!(m.path_matches.is_empty());
    assert!(m.attach_path.ends_with("photo.jpg"), "{}", m.attach_path);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_r_is_still_research() {
    let mut a = app();
    a.select_only(nick_idx(&a, "bob"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('r'));
    assert!(matches!(a.modal, Some(Modal::Research(_))), "{:?}", a.modal);
}
