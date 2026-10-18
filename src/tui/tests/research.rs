//! Research in the TUI: `r` / ◈ open the project picker, the choice becomes a
//! `--research-add` side job, and Research › Projects… shows a project's items.

use super::*;
use crate::research::{ItemKind, ResearchItem, ResearchProject};
use crate::tui::commands::{Action, CommandId, ResearchSource};
use crate::tui::data::SearchFilter;
use crate::tui::input::on_key;
use crate::tui::jobs::JobKind;
use crate::tui::modals::{Modal, RField, ResearchMode};
use crate::tui::state::{Effect, Focus, Hit, research_post, web_links};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn render(a: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..40).map(|y| (0..120).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}
fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }
fn typ(a: &mut App, s: &str) { for c in s.chars() { key(a, KeyCode::Char(c)); } }

fn web(id: i64, note: &str) -> ResearchItem {
    ResearchItem { id, kind: ItemKind::WebUrl, r#ref: "https://news.example/strike".into(), title: "Rail strike".into(), note: note.into(),
        meta: serde_json::json!({"content_type": "text/html"}), content: "# Walkout\n\nThousands joined.".into(),
        local_path: String::new(), added_at: "2026-10-17T09:00:00-04:00".into() }
}

fn with_projects() -> App {
    let mut s = fixture();
    s.projects = vec![
        ResearchProject { id: 2, name: "Unions".into(), description: "history".into(), created_at: "2026-10-17".into(), items: 0 },
        ResearchProject { id: 1, name: "Strikes".into(), description: "rail".into(), created_at: "2026-10-16".into(), items: 2 },
    ];
    let mut post = web(5, "");
    post.kind = ItemKind::Post;
    post.r#ref = "ALICE0002".into();
    s.research = vec![("Strikes".into(), post), ("Strikes".into(), web(6, "key source"))];
    App::new(Box::new(s), &no_args(), Theme::acp(), false)
}

fn modal(a: &App) -> &crate::tui::modals::ResearchModal {
    match &a.modal { Some(Modal::Research(m)) => m, m => panic!("no research modal: {m:?}") }
}

#[test]
fn r_on_a_card_offers_the_post_and_the_view() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    let id = a.current_post_id().unwrap();
    key(&mut a, KeyCode::Char('r'));
    let m = modal(&a);
    assert_eq!(m.mode, ResearchMode::Add);
    assert!(m.what[0].0.starts_with(&format!("This post {id}")), "{:?}", m.what);
    assert_eq!(m.what[0].1, ResearchSource::Posts(vec![id]));
    assert!(m.what.last().unwrap().0.starts_with("All 3 posts in this view"), "{:?}", m.what);
    assert_eq!(m.focus, RField::Name, "no projects yet: straight to the new name");
    assert!(render(&mut a).contains("none yet"));
}

#[test]
fn a_new_project_name_runs_research_add_with_its_description() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    let id = a.current_post_id().unwrap();
    key(&mut a, KeyCode::Char('r'));
    typ(&mut a, "Strikes");
    key(&mut a, KeyCode::Tab);
    typ(&mut a, "rail and port");
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::BackTab);
    typ(&mut a, "key");
    key(&mut a, KeyCode::Enter);
    assert!(a.modal.is_none());
    let Some(Effect::RunSideJob(JobKind::Research, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--research-add", "Strikes", "--research-description", "rail and port", "--post-ids", id.as_str(), "--note", "key"]);
    assert_eq!(a.research_current.as_deref(), Some("Strikes"));
    assert_eq!(a.menu.menu.text(CommandId::Research, "project"), "Strikes");
}

#[test]
fn an_existing_project_is_picked_from_the_list() {
    let mut a = with_projects();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('r'));
    assert_eq!(modal(&a).focus, RField::List);
    let out = render(&mut a);
    assert!(out.contains("Strikes  (2 items) — rail"), "{out}");
    key(&mut a, KeyCode::Down);
    key(&mut a, KeyCode::Enter);
    let Some(Effect::RunSideJob(JobKind::Research, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..3], ["--research-add", "Strikes"]);
    assert!(!argv.contains(&"--research-description".to_string()));
    // the last project is preselected next time
    key(&mut a, KeyCode::Char('r'));
    assert_eq!(modal(&a).sel, 1);
}

#[test]
fn typing_an_existing_name_does_not_create_it_again() {
    let mut a = with_projects();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('r'));
    if let Some(Modal::Research(m)) = &mut a.modal { m.focus = RField::Name; }
    typ(&mut a, "unions");
    if let Some(Modal::Research(m)) = &mut a.modal { m.desc.value = "ignored".into(); }
    key(&mut a, KeyCode::Enter);
    let Some(Effect::RunSideJob(_, argv)) = a.effects.last() else { panic!() };
    assert_eq!(argv[1..3], ["--research-add", "Unions"]);
    assert!(!argv.contains(&"ignored".to_string()));
}

#[test]
fn an_open_post_offers_its_links_files_and_web_pages() {
    let mut a = app();
    a.select_only(nick_idx(&a, "bob"));
    a.focus = Focus::Body;
    let i = a.view().posts.iter().position(|p| p.post_id == "BOB000001").unwrap();
    a.open_post(i);
    if let Some(f) = a.view_mut().stack.last_mut() { f.post.content.push_str(" and https://news.example/strike."); }
    let w = a.research_candidates();
    let labels: Vec<&str> = w.iter().map(|x| x.0.as_str()).collect();
    assert!(labels[0].starts_with("This post BOB000001"), "{labels:?}");
    assert!(w.iter().any(|x| x.1 == ResearchSource::Posts(vec!["ALICE0001".into()])), "linked post: {labels:?}");
    assert!(!labels.iter().any(|l| l.contains("MISSING99")), "{labels:?}");
    assert!(w.iter().any(|x| x.1 == ResearchSource::Url("https://news.example/strike".into())), "{labels:?}");
}

#[test]
fn a_keyword_search_adds_all_results_as_a_search() {
    let mut a = app();
    a.run_search(SearchFilter { keyword: "strike".into(), nicks: vec!["alice".into()], ..SearchFilter::default() });
    let w = a.research_candidates();
    assert!(w[0].0.starts_with("This search result"), "{:?}", w);
    assert_eq!(w.last().unwrap().1, ResearchSource::Search { query: "strike".into(), nicks: vec!["alice".into()] });
    a.act(Action::AddViewToResearch);
    assert_eq!(modal(&a).what.len(), 1);
}

#[test]
fn file_and_url_buttons_need_their_fields() {
    let mut a = app();
    a.act(Action::AddFileToResearch);
    assert!(matches!(a.modal, Some(Modal::Message(_))));
    a.modal = None;
    a.menu.menu.set_text(CommandId::Research, "file", "~/notes.txt");
    a.act(Action::AddFileToResearch);
    assert_eq!(modal(&a).what[0].1, ResearchSource::File("~/notes.txt".into()));
    a.modal = None;
    a.menu.menu.set_text(CommandId::Research, "url", "ftp://x");
    a.act(Action::AddUrlToResearch);
    assert!(matches!(a.modal, Some(Modal::Message(_))));
    a.modal = None;
    a.menu.menu.set_text(CommandId::Research, "url", "https://news.example/strike");
    a.act(Action::AddUrlToResearch);
    typ(&mut a, "Web");
    key(&mut a, KeyCode::Enter);
    let Some(Effect::RunSideJob(JobKind::Research, argv)) = a.effects.last() else { panic!() };
    assert!(argv.join(" ").ends_with("--research-add Web --url https://news.example/strike"), "{argv:?}");
}

#[test]
fn google_drive_and_telegram_are_pinned_for_later() {
    let mut a = app();
    a.act(Action::ComingLater(0));
    assert!(a.status.back().unwrap().contains("Google Drive as a research source is pinned for later"));
    a.act(Action::ComingLater(1));
    assert!(a.status.back().unwrap().contains("Telegram"));
    assert!(a.effects.is_empty());
}

#[test]
fn projects_browse_opens_a_project_view() {
    let mut a = with_projects();
    a.act(Action::ResearchProjects);
    assert_eq!(modal(&a).mode, ResearchMode::Browse);
    key(&mut a, KeyCode::Down);
    key(&mut a, KeyCode::Enter);
    let v = a.view();
    assert_eq!(v.research.as_deref(), Some("Strikes"));
    assert!(v.title.contains("Research: Strikes (2 items)"));
    assert_eq!(v.posts[0].post_id, "ALICE0002", "posts come from the database");
    assert_eq!(v.posts[1].post_id, "research:6");
    assert!(v.posts[1].content.starts_with("> key source"));
    assert!(a.effects.is_empty(), "opening an existing project writes nothing");
    let out = render(&mut a);
    assert!(out.contains("Research: Strikes"), "{out}");
    assert!(a.research_candidates().iter().all(|w| !w.0.starts_with("All ")), "no 'all posts' inside a project");
}

#[test]
fn browse_with_a_new_name_creates_the_project() {
    let mut a = app();
    a.act(Action::ResearchProjects);
    typ(&mut a, "Fresh");
    key(&mut a, KeyCode::Tab);
    typ(&mut a, "new one");
    key(&mut a, KeyCode::Enter);
    let Some(Effect::RunSideJob(JobKind::Research, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--research-create", "Fresh", "--research-description", "new one"]);
    assert_eq!(a.view().research.as_deref(), Some("Fresh"));
}

#[test]
fn export_research_needs_a_project_and_runs_an_export_job() {
    let mut a = app();
    a.act(Action::ExportResearch);
    assert!(matches!(a.modal, Some(Modal::Message(_))));
    a.modal = None;
    a.menu.menu.set_text(CommandId::Research, "project", "Strikes");
    a.menu.menu.set_text(CommandId::Research, "export_path", "/tmp/strikes.json");
    a.act(Action::ExportResearch);
    let Some(Effect::RunJob(JobKind::Export, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert!(argv.join(" ").contains("--research-export Strikes --export /tmp/strikes.json"), "{argv:?}");
}

#[test]
fn cards_and_details_have_a_research_button() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    render(&mut a);
    let (r, _) = *a.hits.iter().find(|(_, h)| *h == Hit::Research(1)).expect("card button");
    crate::tui::input::on_mouse(&mut a, crossterm::event::MouseEvent { kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    let id = a.view().posts[1].post_id.clone();
    assert_eq!(modal(&a).what[0].1, ResearchSource::Posts(vec![id]));
    a.modal = None;
    a.open_post(0);
    render(&mut a);
    assert!(a.hits.iter().any(|(_, h)| *h == Hit::ResearchTop));
}

#[test]
fn side_job_end_refreshes_an_open_project() {
    let mut a = with_projects();
    a.open_research("Strikes");
    a.side_job_finished(true, &["2026-10-17 INFO research \"Strikes\": 1 added, 0 already there".into()], 0);
    assert!(a.status.back().unwrap().contains("◈ research \"Strikes\": 1 added"));
    assert_eq!(a.view().research.as_deref(), Some("Strikes"));
    a.side_job_finished(false, &["boom".into()], 0);
    assert!(a.status.back().unwrap().contains("change not saved: boom"));
}

#[test]
fn web_links_skip_post_links_and_trailing_punctuation() {
    let v = web_links("see https://news.example/a, and (https://x.org/b). Also https://acp.us/rs/post/A1 and https://news.example/a");
    assert_eq!(v, ["https://news.example/a", "https://x.org/b"]);
}

#[test]
fn research_items_become_cards() {
    let p = research_post(&web(9, "why it matters"));
    assert_eq!(p.post_id, "research:9");
    assert_eq!(p.nick, "web");
    assert_eq!(p.title, "Rail strike");
    assert!(p.content.starts_with("> why it matters\n\nhttps://news.example/strike · text/html\n\n# Walkout"), "{}", p.content);
    let mut gone = web(10, "");
    gone.kind = ItemKind::Post;
    gone.r#ref = "GONE1".into();
    assert!(research_post(&gone).content.starts_with("GONE1 is not in the database any more."));
}
