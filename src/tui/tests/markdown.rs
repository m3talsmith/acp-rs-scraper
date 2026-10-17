//! Markdown rendering of post bodies.

use crate::tui::markdown::{plain, render};
use crate::tui::theme::Theme;
use ratatui::style::Modifier;

fn r(s: &str, w: usize) -> Vec<String> { plain(&render(s, w, &Theme::default())) }

#[test]
fn paragraphs_and_line_breaks_are_kept() {
    assert_eq!(r("one\ntwo\n\nthree", 40), ["one", "two", "", "three"]);
}

#[test]
fn wraps_words_to_the_width() {
    assert_eq!(r("aaa bbb ccc ddd", 8), ["aaa bbb", "ccc ddd"]);
    let long = r(&"x".repeat(25), 10);
    assert!(long.iter().all(|l| l.chars().count() <= 10) && long.len() == 3, "{long:?}");
}

#[test]
fn headings_emphasis_code_and_links_are_styled() {
    let t = Theme::default();
    let lines = render("# Title\n\nsome **bold** and *it* and `code` [site](https://acp.us)", 80, &t);
    let txt = plain(&lines);
    assert_eq!(txt[0], "Title");
    assert!(lines[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(txt[2], "some bold and it and code site <https://acp.us>");
    let bold = lines[2].spans.iter().find(|s| s.content == "bold").unwrap();
    assert!(bold.style.add_modifier.contains(Modifier::BOLD));
    let it = lines[2].spans.iter().find(|s| s.content == "it").unwrap();
    assert!(it.style.add_modifier.contains(Modifier::ITALIC));
    let site = lines[2].spans.iter().find(|s| s.content == "site").unwrap();
    assert!(site.style.add_modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn lists_quotes_and_code_blocks() {
    let out = r("- one\n- two long item text\n\n1. first\n2. second\n\n> quoted\n\n```\nlet x = 1;\n```", 14);
    assert!(out.contains(&"• one".to_string()), "{out:?}");
    assert!(out.contains(&"• two long".to_string()) && out.contains(&"  item text".to_string()), "hanging indent: {out:?}");
    assert!(out.contains(&"1. first".to_string()) && out.contains(&"2. second".to_string()));
    assert!(out.contains(&"│ quoted".to_string()));
    assert!(out.contains(&"  let x = 1;".to_string()));
}

#[test]
fn bare_urls_are_left_alone() {
    assert_eq!(r("see https://acp.us/rs/post/ABC", 80), ["see https://acp.us/rs/post/ABC"]);
}
