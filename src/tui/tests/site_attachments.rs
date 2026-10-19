//! Site attachment picker: more than four files ask which ones go to Red Star.

use super::*;
use crate::tui::input::on_key;
use crate::tui::modals::{ComposeFocus, Modal, ModalResult, ModalState};
use crate::tui::state::{DetailFrame, Link, LinkKind};
use crate::tui::widgets::detail::detail_layout;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};

fn key(a: &mut App, c: KeyCode) {
    on_key(a, KeyEvent::new(c, KeyModifiers::NONE));
}

fn render(a: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(100, 40)).unwrap();
    t.draw(|f| crate::tui::widgets::draw(f, a, Instant::now() + Duration::from_secs(5))).unwrap();
    let buf = t.backend().buffer().clone();
    (0..40).map(|y| (0..100).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn files(label: &str, names: &[&str]) -> (PathBuf, Vec<PathBuf>) {
    let dir = std::env::temp_dir().join(format!("acp-pick-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let paths = names.iter().map(|n| {
        let p = dir.join(n);
        std::fs::write(&p, b"x").unwrap();
        p
    }).collect();
    (dir, paths)
}

fn composing(paths: Vec<PathBuf>) -> App {
    let mut a = app();
    key(&mut a, KeyCode::Char('n'));
    let Some(Modal::Compose(m)) = &mut a.modal else { panic!("{:?}", a.modal) };
    m.title = "T".into();
    m.body = "hi".into();
    m.attachments = paths;
    m.focus = ComposeFocus::Publish;
    a
}

fn line_text(lines: &[ratatui::text::Line]) -> String {
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

#[test]
fn more_than_four_opens_a_picker_and_four_or_fewer_does_not() {
    let (dir, paths) = files("six", &["one.jpg", "two.png", "three.gif", "four.webp", "notes.pdf", "extra.png"]);
    let mut a = composing(paths.clone());
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal) };
    assert_eq!(m.focus, ComposeFocus::SitePick);
    assert_eq!(m.site_pick.as_deref(), Some(&[true, true, true, true, false, false][..]));
    let screen = render(&mut a);
    assert!(screen.contains("4/4 selected for Red Star"), "{screen}");
    assert!(screen.contains("? site-attachments"), "{screen}");
    for _ in 0..4 { key(&mut a, KeyCode::Down); }
    key(&mut a, KeyCode::Char(' '));
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal) };
    assert!(!m.site_pick.as_ref().unwrap()[4]);
    assert!(m.error.as_deref().unwrap_or("").contains("Uncheck one first"), "{:?}", m.error);
    let Some(Modal::Compose(m)) = &mut a.modal else { panic!() };
    m.site_pick = Some(vec![true; 6]);
    m.focus = ComposeFocus::SitePick;
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("confirm with 6 checked should stay open: {:?}", a.modal) };
    assert!(m.error.as_deref().unwrap_or("").contains("allows 4"), "{:?}", m.error);
    let Some(Modal::Compose(m)) = &mut a.modal else { panic!() };
    m.site_pick = Some(vec![true, true, true, false, false, false]);
    m.site_at = 0;
    m.focus = ComposeFocus::SitePick;
    m.error = None;
    key(&mut a, KeyCode::Char(' '));
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Message(m)) = &a.modal else { panic!("{:?}", a.modal) };
    let text = m.lines.join("\n");
    assert!(text.contains("nothing was published"), "{text}");
    assert!(text.contains("local-only media one.jpg"), "{text}");
    assert!(!text.contains("first 4 of 6"), "{text}");
    key(&mut a, KeyCode::Enter);
    let _ = std::fs::remove_dir_all(&dir);

    let (dir, paths) = files("four", &["one.jpg", "two.png", "three.gif", "four.webp"]);
    let mut a = composing(paths);
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Message(m)) = &a.modal else { panic!("4 files skip the picker: {:?}", a.modal) };
    let text = m.lines.join("\n");
    assert!(text.contains("nothing was published"), "{text}");
    assert!(text.contains("attachments (local only, not uploaded)"), "{text}");
    assert!(text.contains("one.jpg") && text.contains("four.webp"), "{text}");
    assert!(!text.contains("selected for Red Star"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_picker_links_to_its_help_topic_and_clears_when_the_list_shrinks() {
    let (dir, paths) = files("help", &["one.jpg", "two.png", "three.gif", "four.webp", "notes.pdf"]);
    let mut a = composing(paths);
    key(&mut a, KeyCode::Enter);
    key(&mut a, KeyCode::Char('?'));
    let Some(Modal::Help(h)) = &a.modal else { panic!("{:?}", a.modal) };
    assert!(h.lines[h.scroll].contains("[[site-attachments]]"), "{}", h.lines[h.scroll]);
    let mut a = composing(files("f1", &["one.jpg", "two.png", "three.gif", "four.webp", "notes.pdf"]).1);
    key(&mut a, KeyCode::Enter);
    let Some(Modal::Compose(m)) = &mut a.modal else { panic!() };
    assert_eq!(m.on_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)), Some(ModalResult::Help("site-attachments")));
    let mut a = composing(files("esc", &["one.jpg", "two.png", "three.gif", "four.webp", "notes.pdf"]).1);
    key(&mut a, KeyCode::Enter);
    key(&mut a, KeyCode::Esc);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal) };
    assert!(m.site_pick.is_none());
    assert_eq!(m.focus, ComposeFocus::Publish);
    let mut a = composing(files("drop", &["one.jpg", "two.png", "three.gif", "four.webp", "notes.pdf"]).1);
    key(&mut a, KeyCode::Enter);
    key(&mut a, KeyCode::BackTab);
    key(&mut a, KeyCode::Backspace);
    let Some(Modal::Compose(m)) = &a.modal else { panic!("{:?}", a.modal) };
    assert_eq!(m.attachments.len(), 4);
    assert!(m.site_pick.is_none(), "4 files do not keep the picker");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn detail_names_the_site_subset_and_skips_research_files() {
    let mut p = post("ALICE0001", "", "", "alice", "2026-10-01T10:00:00Z", "body");
    let atts: Vec<Attachment> = (0..6).map(|i| Attachment {
        post_id: "ALICE0001".into(), url: format!("https://media.acp.us/{i}.jpg"), kind: "image".into(),
        local_path: PathBuf::from(format!("/nonexistent/{i}.jpg")), mime_type: "image/jpeg".into(), on_site: i < 4,
    }).collect();
    let links: Vec<Link> = atts.iter().map(|a| Link { kind: LinkKind::Attachment, id: a.local_path.display().to_string(), target: None }).collect();
    let frame = DetailFrame { post: p.clone(), attachments: atts, links, link_cursor: 0, sort: ThreadSort::Newest, scroll: 0, follow_link: false, notice: None };
    let (lines, _, _, _) = detail_layout(&frame, 80, &Theme::acp(), 0);
    let text = line_text(&lines);
    assert!(text.contains("On Red Star: 4 of 6"), "{text}");
    p.post_id = "research:1".into();
    let atts = vec![Attachment {
        post_id: "research:1".into(), url: "file://x".into(), kind: "file".into(),
        local_path: PathBuf::from("/nonexistent/x.jpg"), mime_type: "image/jpeg".into(), on_site: false,
    }];
    let links = vec![Link { kind: LinkKind::Attachment, id: "/nonexistent/x.jpg".into(), target: None }];
    let frame = DetailFrame { post: p, attachments: atts, links, link_cursor: 0, sort: ThreadSort::Newest, scroll: 0, follow_link: false, notice: None };
    let (lines, _, _, _) = detail_layout(&frame, 80, &Theme::acp(), 0);
    let text = line_text(&lines);
    assert!(!text.contains("On Red Star"), "{text}");
}
