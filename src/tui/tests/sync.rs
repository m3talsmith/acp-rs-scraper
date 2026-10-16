//! The Sync menu: argv, interval, the automatic due-check and its quiet failures.

use super::*;
use crate::tui::commands::{Action, CommandId};
use crate::tui::jobs::JobKind;
use crate::tui::state::{Effect, JobView, SessionState};
use std::time::{Duration, Instant};

#[test]
fn sync_argv_uses_the_field_else_the_selection() {
    let mut a = app();
    let m = &mut a.menu.menu;
    assert_eq!(m.sync_argv(&[]).unwrap()[1..], ["--sync"]);
    assert_eq!(m.sync_argv(&["alice".into()]).unwrap()[1..], ["--sync", "--nicks", "alice", "--no-mentions"]);
    m.set_text(CommandId::Sync, "nicks", "@bob, carol");
    m.set_check(CommandId::Sync, "mentions", true);
    m.set_check(CommandId::Sync, "bookmarks", false);
    assert_eq!(m.sync_argv(&["alice".into()]).unwrap()[1..], ["--sync", "--nicks", "bob,carol", "--no-bookmarks"]);
    m.set_text(CommandId::Sync, "nicks", "");
    assert!(m.sync_argv(&[]).is_err(), "nothing to sync");
}

#[test]
fn interval_and_prefs_round_trip() {
    let mut a = app();
    let m = &mut a.menu.menu;
    assert_eq!(m.sync_interval().unwrap(), Duration::from_secs(15 * 60));
    m.set_text(CommandId::Sync, "interval", "0");
    assert!(m.sync_interval().is_err());
    m.set_text(CommandId::Sync, "interval", "5");
    m.set_check(CommandId::Sync, "auto", true);
    let p = m.sync_prefs();
    assert_eq!((p.auto, p.interval_minutes, p.bookmarks), (Some(true), Some(5), Some(true)));
    let mut b = app();
    b.menu.menu.apply_sync_prefs(&p);
    assert_eq!(b.menu.menu.sync_prefs(), p);
    b.act(Action::SaveSyncPrefs);
    assert!(matches!(b.effects.as_slice(), [Effect::SaveSyncPrefs(q)] if *q == p));
}

#[test]
fn auto_sync_runs_when_due_and_reschedules() {
    let mut a = app();
    let t0 = Instant::now();
    a.sync_tick(t0);
    assert!(a.effects.is_empty(), "off by default");
    a.menu.menu.set_check(CommandId::Sync, "auto", true);
    a.session = SessionState::Saved;
    a.sync_tick(t0);
    let Some(Effect::AutoSync(argv)) = a.effects.pop() else { panic!("{:?}", a.effects) };
    a.sync_tick(t0 + Duration::from_secs(60));
    assert!(a.effects.is_empty(), "not due again yet");
    a.auto_sync_checked(Ok(true), argv.clone());
    assert_eq!(a.effects, [Effect::RunJob(JobKind::Sync, argv)]);
    assert!(a.auto_running);
    // A failing automatic sync only reports in the status line.
    a.effects.clear();
    a.job_started(JobView { kind: JobKind::Sync, pid: 9, log: "/tmp/s.log".into() }, "x");
    a.job_finished(false, Some(1), &["boom".into()]);
    assert!(a.modal.is_none());
    assert!(a.status.back().unwrap().contains("auto sync failed"));
    a.sync_tick(t0 + Duration::from_secs(15 * 60 + 1));
    assert!(matches!(a.effects.as_slice(), [Effect::AutoSync(_)]));
}

#[test]
fn auto_sync_skips_quietly_when_logged_out_offline_or_busy() {
    let mut a = app();
    a.menu.menu.set_check(CommandId::Sync, "auto", true);
    a.sync_tick(Instant::now());
    assert!(a.effects.is_empty() && a.modal.is_none(), "logged out: no login modal");
    a.session = SessionState::Saved;
    a.auto_sync_checked(Err("connection refused".into()), vec![]);
    assert!(a.modal.is_none() && a.effects.is_empty());
    assert!(a.status.back().unwrap().contains("offline"));
    a.auto_sync_checked(Ok(false), vec![]);
    assert_eq!(a.session, SessionState::LoggedOut);
    assert!(a.modal.is_none());
    a.session = SessionState::Valid;
    a.sync_next = None;
    a.job = Some(JobView { kind: JobKind::Scan, pid: 1, log: "/tmp/x".into() });
    a.sync_tick(Instant::now());
    assert!(a.effects.is_empty());
}

#[test]
fn sync_now_checks_the_session_like_a_scan() {
    let mut a = app();
    a.act(Action::SyncNow);
    assert!(matches!(a.effects.as_slice(), [Effect::CheckSessionThen(JobKind::Sync, v)] if v[1] == "--sync"));
}
