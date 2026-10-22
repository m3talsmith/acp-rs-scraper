use crate::tui::data::KindFilter;
use crate::tui::modals::*;
use crate::tui::widgets::text::TextField;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn k(c: KeyCode) -> KeyEvent { KeyEvent::new(c, KeyModifiers::NONE) }
fn typ(m: &mut dyn ModalState, s: &str) { for ch in s.chars() { m.on_key(k(KeyCode::Char(ch))); } }

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
