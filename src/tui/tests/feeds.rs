//! Feed editor, add-to-feed, and the confirm step before a refresh or a delete.

use super::*;
use crate::tui::commands::{Action, CommandId};
use crate::tui::jobs::JobKind;
use crate::tui::modals::{ConfirmAction, FeedFocus, FeedMode, Modal, ModalState};
use crate::tui::state::Effect;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn add_to_feed_starts_on_cancel_and_a_nick_builds_the_add_command() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.open_post(0);
    a.act(Action::AddToFeed);
    let Some(Modal::Feed(m)) = &mut a.modal else { panic!("{:?}", a.modal) };
    assert_eq!(m.mode, FeedMode::Add);
    assert_eq!(m.focus, FeedFocus::Button(1));
    assert_eq!(m.value.value, "alice");
    m.name.set("Theory");
    m.focus = FeedFocus::Button(0);
    assert_eq!(m.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), Some(crate::tui::modals::ModalResult::ClosedSubmitted));
    a.close_modal(crate::tui::modals::ModalResult::ClosedSubmitted);
    let Some(Effect::RunSideJob(JobKind::Feed, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert!(argv.windows(2).any(|w| w == ["--feed-add", "Theory"]), "{argv:?}");
    assert!(argv.windows(2).any(|w| w == ["--feed-nick", "alice"]), "{argv:?}");
}

#[test]
fn the_editor_save_and_a_refresh_confirm_use_the_feed_flags() {
    let mut a = app();
    a.menu.menu.set_text(CommandId::Feeds, "name", "Theory");
    a.menu.menu.set_text(CommandId::Feeds, "description", "Books");
    a.menu.menu.set_text(CommandId::Feeds, "nick", "alice");
    let argv = a.menu.menu.feed_rule_argv().unwrap();
    assert_eq!(&argv[1..], ["--feed-add", "Theory", "--feed-nick", "alice"]);
    a.act(Action::RefreshFeed);
    let Some(Modal::Confirm(c)) = &a.modal else { panic!("{:?}", a.modal) };
    assert_eq!(c.focus, 1, "Cancel is the default");
    assert!(matches!(&c.action, ConfirmAction::Feed { online: true, argv } if argv.windows(2).any(|w| w == ["--feed-refresh", "Theory"])));
    a.act(Action::DeleteFeed);
    let Some(Modal::Confirm(c)) = &a.modal else { panic!("{:?}", a.modal) };
    assert_eq!(c.focus, 1);
    assert!(matches!(&c.action, ConfirmAction::Feed { online: false, .. }));
}

#[test]
fn a_confirmed_refresh_checks_the_session_before_it_connects() {
    let mut a = app();
    a.menu.menu.set_text(CommandId::Feeds, "name", "Theory");
    a.act(Action::RefreshFeed);
    let Some(Modal::Confirm(c)) = a.modal.take() else { panic!("confirm") };
    let ConfirmAction::Feed { online: true, argv } = c.action else { panic!("refresh") };
    a.modal = Some(Modal::Confirm(crate::tui::modals::ConfirmModal::new("Refresh feed", vec!["Refresh Theory from the site?".into()], "Refresh", ConfirmAction::Feed { online: true, argv: argv.clone() })));
    // Submit the focused Cancel does not run. Press the Refresh button.
    if let Some(Modal::Confirm(m)) = &mut a.modal { m.focus = 0; }
    a.close_modal(crate::tui::modals::ModalResult::ClosedSubmitted);
    let Some(Effect::CheckSessionThen(JobKind::Feed, got)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert!(got.windows(2).any(|w| w == ["--feed-refresh", "Theory"]));
}
