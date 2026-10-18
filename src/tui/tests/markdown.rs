//! Markdown rendering of post bodies.

use crate::tui::markdown::{plain, render};
use crate::tui::theme::Theme;
use ratatui::style::Modifier;

fn r(s: &str, w: usize) -> Vec<String> { plain(&render(s, w, &Theme::acp())) }

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
    let t = Theme::acp();
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

#[test]
fn tight_bullet_lists_get_bullets_and_a_hanging_indent() {
    assert_eq!(r("- one\n- two words here\n* three", 12), ["• one", "• two words", "  here", "", "• three"]);
}

#[test]
fn loose_bullet_lists_get_bullets_too() {
    // Items separated by blank lines are wrapped in paragraphs by the parser.
    assert_eq!(r("- one\n\n- two words here\n\n- three", 12), ["• one", "• two words", "  here", "• three"]);
    // A second paragraph in an item is indented, without a second bullet.
    assert_eq!(r("- first\n\n  more of it\n\n- second", 20), ["• first", "  more of it", "• second"]);
}

#[test]
fn ordered_lists_are_numbered_tight_and_loose() {
    assert_eq!(r("1. one\n2. two words here\n3. three", 12), ["1. one", "2. two words", "   here", "3. three"]);
    assert_eq!(r("3. three\n\n4. four", 20), ["3. three", "4. four"]);
    let ten = r("9. nine\n10. ten wraps here", 12);
    assert_eq!(ten, ["9. nine", "10. ten", "    wraps", "    here"]);
}

#[test]
fn nested_lists_are_indented_with_their_own_markers() {
    assert_eq!(r("- outer\n  - inner one\n  - inner two\n- next", 30), ["• outer", "  • inner one", "  • inner two", "• next"]);
    assert_eq!(r("1. step\n   - detail\n2. step", 30), ["1. step", "  • detail", "2. step"]);
    assert_eq!(r("- outer\n\n  1. a\n  2. b\n\n- next", 30), ["• outer", "  1. a", "  2. b", "• next"]);
    // An item that opens with a sublist keeps its own bullet.
    assert_eq!(r("- - deep", 30), ["• ", "  • deep"]);
}

#[test]
fn items_that_start_with_bold_code_or_a_link_keep_the_bullet() {
    let t = Theme::acp();
    let lines = render("- **Bold.** rest of the item\n- `code` first\n- [link](https://acp.us) first\n- *it* first", 40, &t);
    let txt = plain(&lines);
    assert_eq!(txt[0], "• Bold. rest of the item");
    assert_eq!(txt[1], "• code first");
    assert!(txt[2].starts_with("• link"), "{txt:?}");
    assert_eq!(txt[3], "• it first");
    let bold = lines[0].spans.iter().find(|s| s.content == "Bold.").unwrap();
    assert!(bold.style.add_modifier.contains(Modifier::BOLD));
    // Loose and bold-led, wrapped: the hanging indent lines up under the text.
    assert_eq!(r("- **Counting things.** They count.\n\n- **Next.** More", 20), ["• Counting things.", "  They count.", "• Next. More"]);
}

#[test]
fn the_audit_post_shows_a_bullet_on_every_item() {
    // A real post (loose list, every item bold-led) where the bullets used to be missing.
    let body = include_str!("fixtures/audit-post.md");
    let out = r(body, 120);
    for item in ["Checking documents.", "Confirming with outsiders.", "Counting things.", "Recalculating.", "Testing the system."] {
        let l = out.iter().find(|l| l.contains(item)).unwrap_or_else(|| panic!("{item} missing: {out:#?}"));
        assert!(l.starts_with(&format!("• {item}")), "no bullet: {l:?}");
    }
    assert_eq!(out.iter().filter(|l| l.starts_with("• ")).count(), 5);
    // Wrapped item lines hang under the item text.
    let i = out.iter().position(|l| l.starts_with("• Checking documents.")).unwrap();
    assert!(out[i + 1].starts_with("  ") && !out[i + 1].starts_with("   "), "{:?}", out[i + 1]);
}
