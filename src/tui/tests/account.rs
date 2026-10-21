//! Account › Profile and the gated account writes.

use super::*;
use crate::tui::commands::{Action, CommandId};
use crate::tui::input::on_key;
use crate::tui::jobs::JobKind;
use crate::tui::modals::{Modal, PostAction};
use crate::tui::state::{Effect, Focus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(a: &mut App, c: KeyCode) {
    on_key(a, KeyEvent::new(c, KeyModifiers::NONE));
}

#[test]
fn profile_and_settings_are_reads_without_a_new_key() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.act(Action::ShowProfile);
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--show-profile", "alice"]);
    assert!(argv.iter().all(|s| !s.starts_with("ctrl-")));
    a.effects.clear();
    a.act(Action::ShowSiteSettings);
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--site-settings"]);
}

#[test]
fn a_confirmed_profile_edit_is_not_sent() {
    let mut a = app();
    a.menu.menu.set_text(CommandId::Session, "display_name", "Ada");
    a.act(Action::EditProfile);
    let Some(Modal::Confirm(m)) = &a.modal else { panic!("{:?}", a.modal) };
    assert_eq!(m.focus, 1, "Cancel is the default");
    assert!(m.strict);
    key(&mut a, KeyCode::Char('y'));
    key(&mut a, KeyCode::Char('e'));
    key(&mut a, KeyCode::Char('s'));
    key(&mut a, KeyCode::Enter);
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("are not sent"), "{status}");
    assert!(status.contains("form[display_name]=Ada"), "{status}");
    assert!(status.contains("capture still needed"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
}

#[test]
fn report_from_the_post_menu_is_blocked() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('p'));
    let Some(Modal::Post(m)) = &mut a.modal else { panic!("{:?}", a.modal) };
    let i = m.items.iter().position(|item| *item == PostAction::Report).unwrap();
    m.cursor = i;
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Confirm(m)) = &a.modal else { panic!("{:?}", a.modal) };
    assert_eq!(m.focus, 1, "Cancel is the default");
    assert!(m.strict);
    for c in ['y', 'e', 's'] {
        key(&mut a, KeyCode::Char(c));
    }
    key(&mut a, KeyCode::Enter);
    let status = a.status.iter().cloned().collect::<Vec<_>>().join("\n");
    assert!(status.contains("are not sent"), "{status}");
    assert!(status.contains("pushEventTo=#js-reportPostActions"), "{status}");
    assert!(status.contains("form[reason]=constitution"), "{status}");
    assert!(!status.contains("https://acp.us"), "{status}");
}
