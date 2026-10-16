//! Bookmarks in the TUI: the nick-bar row, the view, stars, and the side job.

use super::*;
use crate::tui::commands::Action;
use crate::tui::input::{on_key, on_mouse};
use crate::tui::jobs::JobKind;
use crate::tui::state::{Effect, Hit, SessionState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};

fn render(a: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn click(a: &mut App, hit: Hit) {
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?} drawn"));
    on_mouse(a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
}

#[test]
fn bookmarks_row_opens_only_bookmarked_posts() {
    let mut a = app();
    let s = render(&mut a, 120, 32);
    assert!(s.contains("★ Bookmarks (1)"));
    click(&mut a, Hit::Bookmarks);
    let v = a.view();
    assert!(v.bookmarks);
    assert_eq!(v.posts.iter().map(|p| p.post_id.as_str()).collect::<Vec<_>>(), ["BOB000002"]);
    assert!(a.search_back(), "< goes back like a search");
    on_key(&mut a, KeyEvent::new(KeyCode::Char('B'), KeyModifiers::SHIFT));
    assert!(a.view().bookmarks);
}

#[test]
fn star_click_toggles_at_once_and_queues_a_side_job() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    let s = render(&mut a, 120, 32);
    assert!(s.contains("☆"));
    let first = a.view().posts[0].post_id.clone();
    click(&mut a, Hit::Star(0));
    assert!(a.is_marked(&first));
    let Some(Effect::RunSideJob(JobKind::Bookmark, argv)) = a.effects.pop() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--bookmark".to_string(), first.clone()]);
    assert!(render(&mut a, 120, 32).contains("★"));
    // A side job doesn't need the main job slot (works while a scan runs, and offline).
    a.job = Some(crate::tui::state::JobView { kind: JobKind::Scan, pid: 1, log: "/tmp/x".into() });
    a.act(Action::ToggleBookmark);
    assert!(!a.is_marked(&first));
    assert!(matches!(a.effects.as_slice(), [Effect::RunSideJob(JobKind::Bookmark, v)] if v[1] == "--unbookmark"));
}

#[test]
fn b_toggles_the_open_detail_and_the_detail_has_a_star() {
    let mut a = app();
    a.select_only(nick_idx(&a, "bob"));
    a.open_post(0);
    let id = a.top().unwrap().post.post_id.clone();
    render(&mut a, 120, 32);
    click(&mut a, Hit::StarTop);
    assert!(a.is_marked(&id));
    a.effects.clear();
    on_key(&mut a, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
    assert!(!a.is_marked(&id));
}

#[test]
fn side_job_end_reloads_only_when_all_writes_landed() {
    let mut a = app();
    a.marks.insert("ALICE0001".into());
    a.side_job_finished(true, &[], 1);
    assert!(a.is_marked("ALICE0001"), "another write still pending: keep the optimistic state");
    a.side_job_finished(true, &[], 0);
    assert!(!a.is_marked("ALICE0001"), "re-read from the store");
}

#[test]
fn sync_needs_a_session_check_first() {
    let mut a = app();
    a.act(Action::SyncBookmarks);
    let Some(Effect::CheckSessionThen(JobKind::Sync, argv)) = a.effects.pop() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1], "--sync-bookmarks");
    a.session_checked(Ok(false), JobKind::Sync, argv.clone());
    assert_eq!(a.pending_job, Some((JobKind::Sync, argv)));
    a.session = SessionState::Valid;
}
