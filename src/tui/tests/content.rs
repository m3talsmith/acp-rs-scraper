//! Post content in the TUI: Markdown, padding, attachments as openable links, OSC 8.

use super::*;
use crate::tui::state::{Effect, Hit, LinkKind};
use ratatui::{Terminal, backend::TestBackend};

fn render(a: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..40).map(|y| (0..120).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn open_alice1(a: &mut App) {
    a.select_only(nick_idx(a, "alice"));
    let i = a.view().posts.iter().position(|p| p.post_id == "ALICE0001").unwrap();
    a.open_post(i);
}

#[test]
fn attachments_are_links_that_open_or_say_missing() {
    let mut a = app();
    open_alice1(&mut a);
    let k = a.top().unwrap().links.iter().position(|l| l.kind == LinkKind::Attachment).expect("attachment link");
    let s = render(&mut a);
    assert!(s.contains("▣ Open image/jpeg") && s.contains("(file missing)"), "{s}");
    a.open_link(k);
    assert!(a.effects.is_empty());
    assert!(a.top().unwrap().notice.as_deref().unwrap().contains("missing"));
}

#[test]
fn an_existing_attachment_opens_and_is_an_osc8_link() {
    let f = std::env::temp_dir().join(format!("acp-att-{}.jpg", std::process::id()));
    std::fs::write(&f, b"x").unwrap();
    let mut store = fixture();
    store.atts[0].local_path = f.clone();
    let mut a = App::new(Box::new(store), &no_args(), crate::tui::theme::Theme::default(), false);
    open_alice1(&mut a);
    render(&mut a);
    let k = a.top().unwrap().links.iter().position(|l| l.kind == LinkKind::Attachment).unwrap();
    assert!(a.hits.iter().any(|(_, h)| *h == Hit::Link(k)), "clickable");
    let name = f.file_name().unwrap().to_string_lossy().to_string();
    assert!(a.osc_links.iter().any(|(_, _, t, u)| *t == name && *u == format!("file://{}", f.display())), "{:?}", a.osc_links);
    a.open_link(k);
    assert_eq!(a.effects, [Effect::OpenFile(f.clone())]);
    std::fs::remove_file(f).ok();
}

#[test]
fn osc8_sequence_strips_control_chars() {
    assert_eq!(crate::tui::osc8("a\x07b", "file:///x\x1b"), "\x1b]8;;file:///x\x1b\\ab\x1b]8;;\x1b\\");
}

#[test]
fn detail_renders_markdown_with_padding() {
    let mut store = fixture();
    store.posts[1].content = "## Plan\n\n- **march** at noon".into();
    let mut a = App::new(Box::new(store), &no_args(), crate::tui::theme::Theme::default(), false);
    a.select_only(nick_idx(&a, "alice"));
    let s = render(&mut a);
    assert!(s.contains("│ Plan") || s.contains("┃ Plan"), "card text is padded one column from the border:\n{s}");
    let i = a.view().posts.iter().position(|p| p.post_id == "ALICE0002").unwrap();
    a.open_post(i);
    let s = render(&mut a);
    assert!(s.contains("• march at noon"), "{s}");
    assert!(!s.contains("**march**"));
}
