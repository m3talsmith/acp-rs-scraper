//! Account › Profile and the account writes. Cancel is the default.

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
    let Some(Effect::RunJob(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--show-profile", "alice"]);
    assert!(argv.iter().all(|s| !s.starts_with("ctrl-")));
    a.effects.clear();
    a.act(Action::ShowSiteSettings);
    let Some(Effect::RunJob(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--site-settings"]);
}

#[test]
fn a_confirmed_profile_edit_calls_the_service() {
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
    let Some(Effect::RunJob(JobKind::Write, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--edit-profile", "--display-name", "Ada", "--yes"]);
    assert!(!argv.iter().any(|s| s.contains("https://acp.us")));
}

#[test]
fn report_from_the_post_menu_asks_then_calls() {
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
    let Some(Effect::RunJob(JobKind::Write, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    let id = argv.windows(2).find(|w| w[0] == "--report-post").map(|w| w[1].as_str()).unwrap_or("");
    assert!(id.starts_with("ALICE"), "{argv:?}");
    assert!(argv.windows(2).any(|w| w == ["--reason", "constitution"]), "{argv:?}");
    assert!(argv.iter().any(|s| s == "--yes"));
    assert!(!argv.iter().any(|s| s.contains("https://acp.us")));
}
