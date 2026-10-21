//! Job details: state becomes a modal, clicks hit the status bar and the header mark.

use super::*;
use crate::tui::input::{on_key, on_mouse};
use crate::tui::jobs::JobKind;
use crate::tui::modals::job::{job_source, redact_line};
use crate::tui::modals::{Modal, ModalHit};
use crate::tui::state::{Effect, Focus, Hit, JobView};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};

fn render_at(a: &mut App, w: u16, h: u16, now: Instant) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn render(a: &mut App, w: u16, h: u16) -> String {
    render_at(a, w, h, Instant::now() + Duration::from_secs(5))
}

fn elapsed_of(screen: &str) -> String {
    screen.lines().find_map(|l| {
        let t = l.trim();
        let i = t.find("Elapsed:")?;
        let rest = t[i + "Elapsed:".len()..].trim();
        let value = rest.split_whitespace().next().unwrap_or("");
        Some(format!("Elapsed: {value}"))
    }).unwrap_or_else(|| panic!("no Elapsed line in {screen}"))
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
    assert_eq!(job_source(JobKind::Read, "acp-red-star --eb"), "View › Executive Board");
    assert_eq!(job_source(JobKind::Read, "acp-red-star --explore-latest"), "Explore");
    assert_eq!(job_source(JobKind::Read, "acp-red-star --spaces"), "View › Spaces");
    assert_eq!(job_source(JobKind::Read, "acp-red-star --top week"), "Site Top");
    assert_eq!(job_source(JobKind::Sync, "acp-red-star --sync"), "Sync");
    assert_eq!(job_source(JobKind::Scan, "acp-red-star --encrypt-db"), "Encrypt database");
    assert_eq!(job_source(JobKind::Scan, "acp-red-star --backup"), "Backup");
    assert_eq!(job_source(JobKind::Scan, "acp-red-star --restore x.acpbackup"), "Restore");
    assert!(app().tracked.is_empty());
}

#[test]
fn two_jobs_render_as_a_selectable_list() {
    let mut a = app();
    started(&mut a, JobKind::Scan, 11, "acp-red-star --scrape");
    a.track_side(JobKind::Research, 22, "/tmp/research.log".into(), "acp-red-star --research-add Strikes");
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
    started(&mut a, JobKind::Import, 9, "acp-red-star --import /tmp/in.json");
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
    started(&mut a, JobKind::Read, 4, "acp-red-star --eb");
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
    started(&mut a, JobKind::Sync, 7, "acp-red-star --sync");
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
    started(&mut a, JobKind::Scan, 11, "acp-red-star --scrape");
    a.track_side(JobKind::Bookmark, 22, "/tmp/mark.log".into(), "acp-red-star --bookmark ALICE0001");
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

#[test]
fn finished_elapsed_does_not_change_between_renders() {
    let early = Instant::now();
    let late = early + Duration::from_secs(7200);

    let mut running = app();
    started(&mut running, JobKind::Sync, 1, "acp-red-star --sync");
    running.open_jobs();
    let run_early = elapsed_of(&render_at(&mut running, 120, 36, early));
    let run_late = elapsed_of(&render_at(&mut running, 120, 36, late));
    assert_ne!(run_early, run_late, "a running job should keep counting");

    let mut done = app();
    started(&mut done, JobKind::Sync, 258651, "acp-red-star --sync");
    done.job_finished(true, Some(0), &[]);
    done.open_jobs();
    let _ = render_at(&mut done, 120, 36, early);
    let Some(Modal::Job(m)) = &done.modal else { panic!("window stayed open"); };
    let frozen: Vec<String> = m.rows.iter().map(|r| r.elapsed.clone()).collect();
    let first = elapsed_of(&render_at(&mut done, 120, 36, early));
    let second = elapsed_of(&render_at(&mut done, 120, 36, late));
    assert_eq!(first, second, "finished elapsed changed with the clock");
    assert!(first.starts_with("Elapsed: "), "{first}");
    assert!(!first.contains("unknown"), "{first}");
    let Some(Modal::Job(m)) = &done.modal else { panic!("window stayed open"); };
    assert_eq!(frozen, m.rows.iter().map(|r| r.elapsed.clone()).collect::<Vec<_>>());

    done.tracked[0].ended = None;
    let unknown_early = elapsed_of(&render_at(&mut done, 120, 36, early));
    let unknown_late = elapsed_of(&render_at(&mut done, 120, 36, late));
    assert_eq!(unknown_early, unknown_late);
    assert!(unknown_early.contains("unknown"), "{unknown_early}");

    let mut failed = app();
    started(&mut failed, JobKind::Import, 9, "acp-red-star --import /tmp/in.json");
    let boom = "boom".to_string();
    failed.job_finished(false, Some(1), std::slice::from_ref(&boom));
    failed.open_jobs();
    assert_eq!(
        elapsed_of(&render_at(&mut failed, 120, 36, early)),
        elapsed_of(&render_at(&mut failed, 120, 36, late)),
    );

    let mut cancelled = app();
    started(&mut cancelled, JobKind::Scan, 4, "acp-red-star --scrape");
    cancelled.job_cancelled();
    cancelled.open_jobs();
    assert_eq!(
        elapsed_of(&render_at(&mut cancelled, 120, 36, early)),
        elapsed_of(&render_at(&mut cancelled, 120, 36, late)),
    );

    let mut both = app();
    started(&mut both, JobKind::Sync, 11, "acp-red-star --sync");
    both.track_side(JobKind::Research, 22, "/tmp/research.log".into(), "acp-red-star --research-add Strikes");
    both.job_finished(true, Some(0), &[]);
    both.open_jobs();
    let _ = render_at(&mut both, 120, 36, early);
    let Some(Modal::Job(m)) = &both.modal else { panic!("list stayed open"); };
    let snap: Vec<(u32, String, bool)> = m.rows.iter().map(|r| (r.pid, r.elapsed.clone(), r.running)).collect();
    let _ = render_at(&mut both, 120, 36, late);
    let Some(Modal::Job(m)) = &both.modal else { panic!("list stayed open"); };
    assert_eq!(snap.len(), 2);
    for (pid, elapsed, running) in &snap {
        let row = m.rows.iter().find(|r| r.pid == *pid).unwrap();
        if *running { assert_ne!(&row.elapsed, elapsed, "running list row pid {pid}"); }
        else { assert_eq!(&row.elapsed, elapsed, "finished list row pid {pid}"); }
    }
}
