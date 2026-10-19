use super::*;
use crate::tui::commands::{Action, CommandId};
use crate::tui::data::SearchFilter;
use crate::tui::jobs::JobKind;
use crate::tui::modals::*;
use crate::tui::state::*;

#[test]
fn nick_bar_counts_and_select_loads_newest_first() {
    let mut a = app();
    assert_eq!(a.nicks.iter().map(|n| (n.nick.as_str(), n.post_count)).collect::<Vec<_>>(), [("alice", 3), ("bob", 2)]);
    a.select_only(nick_idx(&a, "alice"));
    let ids: Vec<_> = a.view().posts.iter().map(|p| p.post_id.as_str()).collect();
    assert_eq!(ids, ["ALICE0002", "ALICE0003", "ALICE0001"]);
    assert_eq!(a.menu.menu.text(CommandId::Scrape, "nicks"), "alice", "selection pre-fills Scrape › Nicks");
}

#[test]
fn multi_select_unions_and_toggles() {
    let mut a = app();
    a.select_only(0);
    a.toggle_nick(1);
    assert_eq!(a.selected, ["alice", "bob"]);
    assert_eq!(a.view().posts.len(), 5);
    assert_eq!(a.menu.menu.text(CommandId::Scrape, "nicks"), "alice,bob");
    a.toggle_nick(0);
    assert_eq!(a.selected, ["bob"]);
    assert_eq!(a.view().posts.len(), 2);
    a.select_only(0);
    a.extend_selection(1);
    assert_eq!(a.selected, ["alice", "bob"]);
    a.select_all();
    assert_eq!(a.selected.len(), 2);
}

#[test]
fn detail_stack_push_pop_and_links() {
    let mut a = app();
    a.select_only(nick_idx(&a, "bob"));
    let i = a.view().posts.iter().position(|p| p.post_id == "BOB000001").unwrap();
    a.open_post(i);
    assert_eq!(a.view().stack.len(), 1);
    assert_eq!(a.anim.map(|x| x.kind), Some(AnimKind::Open));
    let f = a.top().unwrap();
    let kinds: Vec<_> = f.links.iter().map(|l| (l.kind, l.id.as_str(), l.target.is_some())).collect();
    assert_eq!(kinds, [(LinkKind::Mentioned, "ALICE0001", true), (LinkKind::Mentioned, "MISSING99", false), (LinkKind::Reply, "ALICE0003", true)]);
    // Missing target: inline notice, no push.
    a.open_link(1);
    assert_eq!(a.view().stack.len(), 1);
    assert!(a.top().unwrap().notice.as_deref().unwrap().contains("not in the database"));
    // Cross-load the linked post, then its reply.
    a.open_link(0);
    assert_eq!(a.view().stack.len(), 2);
    assert_eq!(a.anim.map(|x| x.kind), Some(AnimKind::Push));
    assert_eq!(a.top().unwrap().post.post_id, "ALICE0001");
    assert_eq!(a.top().unwrap().attachments.len(), 1);
    let r = a.top().unwrap().links.iter().position(|l| l.id == "BOB000002").unwrap();
    a.open_link(r);
    assert_eq!(a.view().stack.iter().map(|f| f.post.post_id.as_str()).collect::<Vec<_>>(), ["BOB000001", "ALICE0001", "BOB000002"]);
    assert!(a.pop_detail());
    assert_eq!(a.view().stack.len(), 2);
    a.pop_to(0);
    assert_eq!(a.view().stack.len(), 1);
    assert!(a.pop_detail());
    assert!(!a.pop_detail());
}

#[test]
fn reply_links_reply_to_and_thread() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    let i = a.view().posts.iter().position(|p| p.post_id == "ALICE0003").unwrap();
    a.open_post(i);
    let f = a.top().unwrap();
    assert_eq!(f.links[0].kind, LinkKind::ReplyTo);
    assert_eq!(f.links[0].id, "BOB000001");
    assert!(!f.links.iter().any(|l| l.kind == LinkKind::Thread), "thread == reply-to is listed once");
}

#[test]
fn detail_stack_depth_is_capped() {
    let mut s = fixture();
    // A chain of replies deeper than the cap.
    for k in 0..(MAX_DETAIL_DEPTH + 3) {
        let prev = if k == 0 { "ALICE0001".to_string() } else { format!("CHAIN{k:04}") };
        s.posts.push(post(&format!("CHAIN{:04}", k + 1), "ALICE0001", &prev, "carol", &format!("2026-10-05T10:{k:02}:00Z"), "deeper"));
    }
    let mut a = App::new(Box::new(s), &no_args(), Theme::acp(), false);
    a.select_only(nick_idx(&a, "carol"));
    let i = a.view().posts.iter().position(|p| p.post_id == "CHAIN0001").unwrap();
    a.open_post(i);
    for _ in 0..(MAX_DETAIL_DEPTH + 2) {
        let r = a.top().unwrap().links.iter().position(|l| l.kind == LinkKind::Reply);
        let Some(r) = r else { break };
        a.open_link(r);
    }
    assert_eq!(a.view().stack.len(), MAX_DETAIL_DEPTH);
    assert!(a.top().unwrap().notice.as_deref().unwrap().contains("nesting limit"));
}

#[test]
fn search_replaces_body_in_place_and_back_restores() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.open_post(0);
    a.run_search(SearchFilter { keyword: "strike".into(), ..Default::default() });
    assert!(a.search.is_some());
    assert_eq!(a.view().posts.len(), 2);
    assert!(a.view().title.starts_with("Search: "));
    assert!(a.view().stack.is_empty());
    a.open_post(0);
    assert!(a.search_back());
    assert!(a.search.is_none());
    assert_eq!(a.view().stack.len(), 1, "the nick view (and its open detail) is kept under the search");
    assert!(!a.search_back());
}

#[test]
fn clear_search_and_empty_filter() {
    let mut a = app();
    a.menu.menu.set_text(CommandId::Search, "query", "strike");
    a.act(Action::RunSearch);
    assert!(a.search.is_some());
    a.act(Action::ClearSearch);
    assert!(a.search.is_none());
    assert_eq!(a.menu.menu.text(CommandId::Search, "query"), "");
    a.run_search(SearchFilter::default());
    assert!(a.search.is_none());
}

#[test]
fn start_scan_needs_nicks_then_checks_session_then_logs_in() {
    let mut a = app();
    a.act(Action::StartScan);
    assert!(matches!(a.modal, Some(Modal::Message(_))));
    assert!(a.effects.is_empty());
    a.modal = None;
    a.select_only(nick_idx(&a, "alice"));
    a.act(Action::StartScan);
    let Some(Effect::CheckSessionThen(JobKind::Scan, argv)) = a.effects.pop() else { panic!("{:?}", a.effects) };
    assert_eq!(a.session, SessionState::Checking);
    // Logged out: Login modal, scan pending.
    a.session_checked(Ok(false), JobKind::Scan, argv.clone());
    assert!(matches!(a.modal, Some(Modal::Login(_))));
    assert_eq!(a.pending_job.as_ref(), Some(&(JobKind::Scan, argv.clone())));
    a.login_need_code();
    let Some(Modal::Login(m)) = &a.modal else { panic!() };
    assert_eq!(m.step, LoginStep::Code);
    a.login_done(Ok(()));
    assert!(a.modal.is_none());
    assert_eq!(a.session, SessionState::Valid);
    assert_eq!(a.effects, [Effect::RunJob(JobKind::Scan, argv)]);
}

#[test]
fn start_scan_with_valid_session_runs_and_cancel_while_running() {
    let mut a = app();
    a.session = SessionState::Valid;
    a.menu.menu.set_text(CommandId::Scrape, "nicks", "@zed");
    a.act(Action::StartScan);
    assert!(matches!(a.effects.as_slice(), [Effect::RunJob(JobKind::Scan, _)]));
    a.effects.clear();
    a.job_started(JobView { kind: JobKind::Scan, pid: 42, log: "/tmp/x.log".into() }, "acp-rs-scraper --nicks zed");
    a.act(Action::StartScan);
    assert_eq!(a.effects, [Effect::CancelJob]);
    a.effects.clear();
    a.job_cancelled();
    assert!(a.job.is_none());
}

#[test]
fn login_cancel_drops_pending_scan() {
    let mut a = app();
    a.open_login(Some((JobKind::Scan, vec!["x".into()])));
    a.close_modal(ModalResult::ClosedCancelled);
    assert!(a.pending_job.is_none());
    assert_eq!(a.effects, [Effect::LoginCancel]);
}

#[test]
fn login_failure_stays_open_with_error() {
    let mut a = app();
    a.open_login(None);
    a.login_done(Err("sign-in POST failed".into()));
    let Some(Modal::Login(m)) = &a.modal else { panic!() };
    assert_eq!(m.error.as_deref(), Some("sign-in POST failed"));
    assert_eq!(m.step, LoginStep::Email);
}

#[test]
fn logout_routes_to_login() {
    let mut a = app();
    a.session = SessionState::Valid;
    a.act(Action::Logout);
    assert_eq!(a.effects, [Effect::Logout]);
    a.logged_out(true);
    assert_eq!(a.session, SessionState::LoggedOut);
    assert!(matches!(a.modal, Some(Modal::Login(_))));
}

#[test]
fn quit_detaches_or_cancels_running_job() {
    let mut a = app();
    a.act(Action::Quit);
    assert_eq!(a.effects, [Effect::Quit { cancel_job: false }]);
    a.effects.clear();
    a.job_started(JobView { kind: JobKind::Scan, pid: 7, log: "/tmp/s.log".into() }, "x");
    a.act(Action::Quit);
    let Some(Modal::Confirm(c)) = &a.modal else { panic!() };
    assert_eq!(c.buttons.len(), 3);
    assert_eq!(c.focus, 0, "default is keep running in the background");
    a.close_modal(ModalResult::ClosedSubmitted);
    assert_eq!(a.effects, [Effect::Quit { cancel_job: false }]);
    a.effects.clear();
    a.act(Action::Quit);
    if let Some(Modal::Confirm(c)) = &mut a.modal { c.chosen = 1; }
    a.close_modal(ModalResult::ClosedSubmitted);
    assert_eq!(a.effects, [Effect::Quit { cancel_job: true }]);
}

#[test]
fn remove_nicks_confirms_then_runs_with_yes() {
    let mut a = app();
    a.select_only(nick_idx(&a, "bob"));
    a.act(Action::RemoveNicks);
    let Some(Modal::Confirm(c)) = &a.modal else { panic!("{:?}", a.modal) };
    assert!(c.lines[0].contains("@bob"));
    a.close_modal(ModalResult::ClosedSubmitted);
    let Some(Effect::RunJob(JobKind::Maintain, argv)) = a.effects.pop() else { panic!() };
    assert_eq!(argv[1..], ["--remove-nick", "bob", "--yes"]);
}

#[test]
fn clear_all_is_strict() {
    let mut a = app();
    a.act(Action::ClearAll);
    let Some(Modal::Confirm(c)) = &mut a.modal else { panic!() };
    assert!(c.strict);
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let k = |c| KeyEvent::new(c, KeyModifiers::NONE);
    assert_eq!(c.on_key(k(KeyCode::Enter)), None, "Enter without typing yes is refused");
    for ch in "yes".chars() { c.on_key(k(KeyCode::Char(ch))); }
    assert_eq!(c.on_key(k(KeyCode::Enter)), Some(ModalResult::ClosedSubmitted));
    a.close_modal(ModalResult::ClosedSubmitted);
    let Some(Effect::RunJob(JobKind::Maintain, argv)) = a.effects.pop() else { panic!() };
    assert_eq!(argv[1..], ["--clear-all", "--yes"]);
}

#[test]
fn job_finished_failure_shows_tail_and_refreshes() {
    let mut a = app();
    a.job_started(JobView { kind: JobKind::Import, pid: 9, log: "/tmp/i.log".into() }, "x");
    a.needs_redraw = false;
    a.job_finished(false, Some(1), &[" ".into(), "error: bad archive".into()]);
    assert!(a.modal.is_none(), "a failed job stays in the status bar");
    let line = a.status.back().unwrap();
    assert!(line.starts_with("error: import failed (exit 1):"), "{line}");
    assert!(line.contains("bad archive"), "{line}");
    assert!(a.needs_redraw);
    assert!(a.job.is_none());
}

#[test]
fn switching_menus_marks_a_full_redraw() {
    let mut a = app();
    assert!(crate::tui::state::status_line_is_problem("Error: this database needs a password"));
    assert!(crate::tui::state::status_line_is_problem("warning: kept"));
    assert!(!crate::tui::state::status_line_is_problem("started scan"));
    assert!(!a.needs_redraw);
    a.open_menu(0);
    assert!(a.needs_redraw);
    a.needs_redraw = false;
    a.close_menu();
    assert!(a.needs_redraw);
    a.needs_redraw = false;
    a.close_menu_dropdown();
    assert!(a.needs_redraw);
}

#[test]
fn palette_lists_and_runs_commands() {
    let mut a = app();
    let e = a.palette_entries();
    assert!(e.iter().any(|(l, _)| l.starts_with("Start Scanning")));
    assert!(e.iter().any(|(l, _)| l == "Maintain › Clear all"));
    let mut p = PaletteModal::new(e);
    for ch in "help".chars() { p.query.insert(ch); }
    assert_eq!(p.chosen(), Some(PaletteCmd::OpenMenu(a.menu.menu.items.iter().position(|i| i.label == "Help").unwrap())));
    a.modal = Some(Modal::Palette(p));
    a.close_modal(ModalResult::ClosedSubmitted);
    assert!(matches!(a.modal, Some(Modal::Help(_))));
}

#[test]
fn menu_open_activate_options() {
    let mut a = app();
    a.open_menu(0);
    assert_eq!(a.menu.open, Some(0));
    let mentions = a.menu.menu.items[0].opts.iter().position(|o| o.key == "mentions").unwrap();
    a.activate_opt(mentions);
    assert!(!a.menu.menu.check(CommandId::Scrape, "mentions"));
    a.activate_opt(0);
    assert_eq!(a.menu.menu.choice(CommandId::Scrape, "mode"), "Trending");
    let nicks = a.menu.menu.items[0].opts.iter().position(|o| o.key == "nicks").unwrap();
    a.activate_opt(nicks);
    assert!(a.menu.editing);
    // Help has no options: opening runs it.
    let h = a.menu.menu.items.iter().position(|i| i.label == "Help").unwrap();
    a.open_menu(h);
    assert!(matches!(a.modal, Some(Modal::Help(_))));
}

#[test]
fn args_nicks_preselect() {
    use clap::Parser;
    let args = Args::parse_from(["acp-rs-scraper", "--tui", "--nicks", "bob,ghost"]);
    let a = App::new(Box::new(fixture()), &args, Theme::acp(), true);
    assert_eq!(a.selected, ["bob"]);
    assert_eq!(a.session, SessionState::Saved);
    assert_eq!(a.view().posts.len(), 2);
}

#[test]
fn animation_progress_and_finish() {
    use std::time::{Duration, Instant};
    let t0 = Instant::now();
    let an = Anim { kind: AnimKind::Open, start: t0 };
    assert_eq!(an.progress(t0), 0.0);
    let mid = an.progress(t0 + Duration::from_millis(TUI_ANIM_MS / 2));
    assert!(mid > 0.5 && mid < 1.0, "eased: {mid}");
    assert_eq!(an.progress(t0 + Duration::from_millis(TUI_ANIM_MS * 2)), 1.0);
    let mut a = app();
    a.anim = Some(an);
    a.tick(t0 + Duration::from_millis(TUI_ANIM_MS));
    assert!(a.anim.is_none());
}
