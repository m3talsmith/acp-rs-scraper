use crate::tui::data::KindFilter;
use crate::tui::modals::*;
use crate::tui::widgets::text::TextField;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn k(c: KeyCode) -> KeyEvent { KeyEvent::new(c, KeyModifiers::NONE) }
fn typ(m: &mut dyn ModalState, s: &str) { for ch in s.chars() { m.on_key(k(KeyCode::Char(ch))); } }

#[test]
fn login_email_then_code() {
    let mut m = LoginModal::new(None, None);
    m.on_key(k(KeyCode::Enter));
    assert!(m.error.is_some() && m.request.is_none(), "needs an email");
    typ(&mut m, "me@example.test");
    assert_eq!(m.on_key(k(KeyCode::Enter)), None);
    assert_eq!(m.request.take(), Some(LoginRequest::SendEmail("me@example.test".into())));
    assert!(matches!(m.step, LoginStep::Busy(_)));
    typ(&mut m, "ignored while busy");
    m.need_code();
    typ(&mut m, "123456");
    m.on_key(k(KeyCode::Enter));
    assert_eq!(m.request.take(), Some(LoginRequest::SubmitCode("123456".into())));
    assert_eq!(m.on_key(k(KeyCode::Esc)), Some(ModalResult::ClosedCancelled));
}

#[test]
fn login_code_step_resends_and_a_failed_verify_stays_there() {
    let mut m = LoginModal::new(Some("a@b.c".into()), None);
    m.need_code();
    typ(&mut m, "111111");
    m.on_key(k(KeyCode::F(2)));
    assert_eq!(m.request.take(), Some(LoginRequest::Resend));
    assert!(matches!(m.step, LoginStep::Busy(ref s) if s.starts_with("Sending another")));
    assert!(m.code.value.is_empty());
    m.need_code();
    assert_eq!(m.on_click(ModalHit::Button(1)), None);
    assert_eq!(m.request.take(), Some(LoginRequest::Resend));
    m.need_code();
    typ(&mut m, "222222");
    m.held("websocket closed");
    assert_eq!(m.step, LoginStep::Code);
    assert!(m.code.value.is_empty());
    assert_eq!(m.error.as_deref(), Some("websocket closed"));
    assert_eq!(m.on_key(k(KeyCode::F(1))), Some(ModalResult::Help("login")));
    m.need_code();
    assert_eq!(m.on_click(ModalHit::Button(2)), Some(ModalResult::ClosedCancelled));
}

#[test]
fn login_prefills_email_and_clicks() {
    let mut m = LoginModal::new(Some("a@b.c".into()), Some("why".into()));
    assert_eq!(m.email.value, "a@b.c");
    assert_eq!(m.on_click(ModalHit::Button(0)), None);
    assert!(matches!(m.request, Some(LoginRequest::SendEmail(_))));
    assert_eq!(m.on_click(ModalHit::Button(1)), Some(ModalResult::ClosedCancelled));
}

#[test]
fn search_modal_fields_kind_and_buttons() {
    let mut m = SearchModal::new(&Default::default());
    typ(&mut m, "strike");
    m.on_key(k(KeyCode::Tab));
    typ(&mut m, "@bob");
    for _ in 0..3 { m.on_key(k(KeyCode::Tab)); }
    typ(&mut m, "2026-10-15");
    m.on_key(k(KeyCode::Tab));
    m.on_key(k(KeyCode::Right));
    assert_eq!(m.kind, KindFilter::Posts);
    let f = m.filter().unwrap();
    assert_eq!((f.keyword.as_str(), f.nicks.as_slice(), f.to.as_str()), ("strike", &["bob".to_string()][..], "2026-10-15"));
    assert_eq!(m.on_key(k(KeyCode::Enter)), Some(ModalResult::ClosedSubmitted));
    assert_eq!(m.on_click(ModalHit::Button(1)), Some(ModalResult::ClosedSubmitted));
    assert!(m.cleared);
    assert_eq!(m.on_click(ModalHit::Button(2)), Some(ModalResult::ClosedCancelled));
}

#[test]
fn help_is_generated_from_clap() {
    let h = HelpModal::new();
    let all = h.lines.join("\n");
    assert!(all.contains("--nicks") && all.contains("--tui") && all.contains("Shift-S"));
    assert!(all.contains("[[login]]") && all.contains("Resend code"));
    assert!(all.contains("not the master key"));
    assert!(all.contains("--show-phrase"));
    assert!(all.contains("Any 3 of those 5"));
    let mut h = h;
    h.on_scroll(1);
    assert_eq!(h.scroll, 3);
}

#[test]
fn text_field_editing() {
    let mut t = TextField::new("ab");
    t.on_key(k(KeyCode::Left));
    t.on_key(k(KeyCode::Char('é')));
    assert_eq!(t.value, "aéb");
    t.on_key(k(KeyCode::Backspace));
    t.on_key(k(KeyCode::Delete));
    assert_eq!(t.value, "a");
    t.on_key(k(KeyCode::Home));
    t.on_key(k(KeyCode::Char('x')));
    assert_eq!(t.value, "xa");
    t.masked = true;
    assert_eq!(t.display(), "••");
    t.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert_eq!(t.value, "");
}

#[test]
fn cycle_wraps() {
    assert_eq!(cycle(0, 3, -1), 2);
    assert_eq!(cycle(2, 3, 1), 0);
    assert_eq!(cycle(0, 0, 1), 0);
}
