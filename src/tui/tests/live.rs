//! View › Live, Live feed, and Site notifications. No new key. No socket in this binary.

use super::*;
use crate::tui::commands::Action;
use crate::tui::jobs::JobKind;
use crate::tui::state::Effect;

#[test]
fn live_follows_the_open_thread_without_a_new_key() {
    let mut a = app();
    a.toggle_nick(nick_idx(&a, "alice"));
    a.act(Action::OpenLive);
    let Some(Effect::RunJob(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1], "--live");
    assert!(argv[2].starts_with("ALICE"), "{argv:?}");
    assert!(!argv.iter().any(|s| s.starts_with("ctrl-")));
    a.effects.clear();
    a.act(Action::OpenLiveFeed);
    let Some(Effect::RunJob(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--live-feed"]);
    a.effects.clear();
    a.act(Action::OpenSiteNotifications);
    let Some(Effect::RunJob(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--site-notifications"]);
}
