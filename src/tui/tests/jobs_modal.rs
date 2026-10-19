//! Job details: state becomes a modal, clicks hit the status bar and the header mark.

use super::*;
use crate::tui::input::{on_key, on_mouse};
use crate::tui::jobs::JobKind;
use crate::tui::modals::job::{job_source, redact_line};
use crate::tui::modals::{Modal, ModalHit};
use crate::tui::state::{Effect, Focus, Hit, JobView};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use std::time::Instant;

fn render(a: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn click(a: &mut App, hit: Hit) {
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?} drawn"));
    on_mouse(a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
}

fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }

fn started(a: &mut App, kind: JobKind, pid: u32, cmd: &str) {
    a.job_started(JobView { kind, pid, log: format!("/tmp/job-{pid}.log").into() }, cmd);
}

#[test]
fn redact_hides_the_key_and_a_password_assignment() {
    let hex = "ab".repeat(32);
    let hidden = "hunter22";
    let line = format!("pragma x'{hex}' and {hex} and ACP_INHERIT_DB_KEY=1 and password={hidden}");
    let out = redact_line(&line);
    assert!(!out.contains(&hex), "{out}");
    assert!(!out.contains(hidden), "{out}");
    assert!(out.contains("ACP_INHERIT_DB_KEY=1"), "{out}");
    assert!(out.contains("[redacted]"), "{out}");
}

#[test]
fn source_names_the_command_including_backup_flags() {
    assert_eq!(job_source(JobKind::Read, "acp-rs-scraper --eb"), "View › Executive Board");
    assert_eq!(job_source(JobKind::Read, "acp-rs-scraper --explore-latest"), "Explore");
    assert_eq!(job_source(JobKind::Read, "acp-rs-scraper --top week"), "Site Top");
    assert_eq!(job_source(JobKind::Sync, "acp-rs-scraper --sync"), "Sync");
    assert_eq!(job_source(JobKind::Scan, "acp-rs-scraper --encrypt-db"), "Encrypt database");
    assert_eq!(job_source(JobKind::Scan, "acp-rs-scraper --backup"), "Backup");
    assert_eq!(job_source(JobKind::Scan, "acp-rs-scraper --restore x.acpbackup"), "Restore");
    assert!(app().tracked.is_empty());
}

#[test]
fn two_jobs_render_as_a_selectable_list() {
    let mut a = app();
    started(&mut a, JobKind::Scan, 11, "acp-rs-scraper --scrape");
    a.track_side(JobKind::Research, 22, "/tmp/research.log".into(), "acp-rs-scraper --research-add Strikes");
    a.open_jobs();
    let Some(Modal::Job(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.rows.len(), 2);
    assert!(m.rows.iter().any(|r| r.name == "Scrape" && r.pid == 11));
    assert!(m.rows.iter().any(|r| r.name == "Research" && r.source == "Research" && r.pid == 22));
    let s = render(&mut a, 120, 36);
    assert!(s.contains("pid 11") && s.contains("pid 22"), "{s}");
    assert!(s.contains("Started by: Scrape") && s.contains("Elapsed"), "{s}");
    click(&mut a, Hit::Modal(ModalHit::Item(1)));
    let Some(Modal::Job(m)) = &a.modal else { panic!("list click keeps the window"); };
    assert_eq!(m.cursor, 1);
    assert_eq!(m.rows[m.cursor].pid, 22);
}

#[test]
fn failed_job_shows_the_error_and_not_the_secret() {
    let mut a = app();
    started(&mut a, JobKind::Import, 9, "acp-rs-scraper --import /tmp/in.json");
    let hidden = "hunter22";
    let line = format!("error: bad archive password={hidden}");
    a.note_job_line(9, &line);
    a.job_finished(false, Some(1), &[line]);
    assert!(a.modal.is_none());
    assert!(a.status.back().unwrap().contains("bad archive"));
    assert!(!a.status.iter().any(|l| l.contains(hidden)));
    a.open_jobs();
    let s = render(&mut a, 120, 36);
    assert!(s.contains("failed"), "{s}");
    assert!(s.contains("bad archive"), "{s}");
    assert!(s.contains("[redacted]"), "{s}");
    assert!(!s.contains(hidden), "{s}");
    assert!(s.contains("Exit: 1"), "{s}");
}

#[test]
fn click_status_or_header_opens_and_esc_closes() {
    let mut a = app();
    started(&mut a, JobKind::Read, 4, "acp-rs-scraper --eb");
    let s = render(&mut a, 140, 32);
    assert!(s.contains("running (pid 4)"), "{s}");
    assert!(a.hits.iter().any(|(_, h)| *h == Hit::Status));
    assert!(a.hits.iter().any(|(_, h)| *h == Hit::Job));
    click(&mut a, Hit::Status);
    assert_eq!(a.focus, Focus::Status);
    let Some(Modal::Job(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.rows[m.cursor].source, "View › Executive Board");
    key(&mut a, KeyCode::Esc);
    assert!(a.modal.is_none());
    render(&mut a, 140, 32);
    click(&mut a, Hit::Job);
    assert!(matches!(a.modal, Some(Modal::Job(_))));
    key(&mut a, KeyCode::Enter);
    assert!(matches!(a.modal, Some(Modal::Job(_))), "Enter does not cancel");
    assert!(a.effects.is_empty());
}

#[test]
fn enter_on_focused_status_opens_the_job() {
    let mut a = app();
    assert_eq!(a.focus, Focus::Nicks);
    key(&mut a, KeyCode::Tab);
    assert_eq!(a.focus, Focus::Body);
    key(&mut a, KeyCode::Tab);
    assert_eq!(a.focus, Focus::Menu);
    key(&mut a, KeyCode::Tab);
    assert_eq!(a.focus, Focus::Status);
    key(&mut a, KeyCode::Enter);
    assert!(a.modal.is_none());
    assert!(a.status.back().unwrap().contains("no job to show"));
    started(&mut a, JobKind::Sync, 7, "acp-rs-scraper --sync");
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Job(m)) = &a.modal else { panic!("{:?}", a.modal); };
    assert_eq!(m.rows[m.cursor].name, "Sync");
    assert!(m.rows[m.cursor].running);
    a.note_job_line(7, "fetched page 2");
    let s = render(&mut a, 120, 36);
    assert!(s.contains("fetched page 2"), "{s}");
    assert!(s.contains("Stage:"), "{s}");
    key(&mut a, KeyCode::Esc);
    assert!(a.modal.is_none());
    key(&mut a, KeyCode::BackTab);
    assert_eq!(a.focus, Focus::Menu);
}

#[test]
fn cancel_main_and_side_from_the_window() {
    let mut a = app();
    started(&mut a, JobKind::Scan, 11, "acp-rs-scraper --scrape");
    a.track_side(JobKind::Bookmark, 22, "/tmp/mark.log".into(), "acp-rs-scraper --bookmark ALICE0001");
    a.open_jobs();
    key(&mut a, KeyCode::Char('c'));
    assert_eq!(a.effects, [Effect::CancelJob]);
    assert!(matches!(a.modal, Some(Modal::Job(_))));
    a.effects.clear();
    key(&mut a, KeyCode::Down);
    key(&mut a, KeyCode::Char('c'));
    assert_eq!(a.effects, [Effect::CancelSide(22)]);
    a.effects.clear();
    render(&mut a, 120, 36);
    click(&mut a, Hit::Modal(ModalHit::Button(0)));
    assert_eq!(a.effects, [Effect::CancelSide(22)]);
}
