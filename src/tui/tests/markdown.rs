//! Markdown rendering of post bodies: one test per construct, then real posts.

use crate::tui::markdown::{normalize, plain, render, render_doc};
use crate::tui::theme::Theme;
use ratatui::style::{Color, Modifier};
use ratatui::text::Line;

fn r(s: &str, w: usize) -> Vec<String> { plain(&render(s, w, &Theme::acp())) }
fn lines(s: &str, w: usize) -> Vec<Line<'static>> { render(s, w, &Theme::acp()) }
/// The style of the first span whose text is `text`.
fn span_style(ls: &[Line], text: &str) -> ratatui::style::Style {
    ls.iter().flat_map(|l| l.spans.iter()).find(|s| s.content == text).unwrap_or_else(|| panic!("no span {text:?}")).style
}
fn has(st: ratatui::style::Style, m: Modifier) -> bool { st.add_modifier.contains(m) }

// ------------------------------------------------------------ paragraphs

#[test]
fn paragraphs_and_line_breaks_are_kept() {
    assert_eq!(r("one\ntwo\n\nthree", 40), ["one", "two", "", "three"]);
    // A hard break (backslash) too.
    assert_eq!(r("one\\\ntwo", 40), ["one", "two"]);
}

#[test]
fn wraps_words_to_the_width() {
    assert_eq!(r("aaa bbb ccc ddd", 8), ["aaa bbb", "ccc ddd"]);
    let long = r(&"x".repeat(25), 10);
    assert!(long.iter().all(|l| l.chars().count() <= 10) && long.len() == 3, "{long:?}");
}

#[test]
fn entities_and_escapes() {
    assert_eq!(r("a &amp; b &copy; &mdash; \\*not italic\\* \\# not a heading", 80), ["a & b © — *not italic* # not a heading"]);
}

// -------------------------------------------------------------- headings

#[test]
fn headings_h1_to_h6_have_distinct_styles_and_h1_h2_a_rule() {
    let t = Theme::acp();
    let ls = lines("# One\n\n## Two\n\n### Three\n\n#### Four\n\n##### Five\n\n###### Six", 40);
    assert_eq!(plain(&ls), [" One ", "━━━━━", "", "Two", "───", "", "Three", "", "Four", "", "Five", "", "Six"]);
    let styles: Vec<_> = ["One", "Two", "Three", "Four", "Five", "Six"].iter().map(|s| span_style(&ls, s)).collect();
    for (i, a) in styles.iter().enumerate() {
        for b in &styles[i + 1..] { assert_ne!(a, b, "every level looks different"); }
    }
    assert_eq!((styles[0].fg, styles[0].bg), (Some(t.on_accent), Some(t.accent)), "H1 is a banner");
    assert_eq!(styles[2].fg, Some(t.accent));
    assert!(has(styles[0], Modifier::BOLD) && has(styles[3], Modifier::ITALIC) && has(styles[5], Modifier::ITALIC));
    assert_eq!(styles[4].fg, Some(t.muted));
    // A long heading wraps; the rule is as wide as its widest line.
    assert_eq!(r("# A heading that wraps", 12), [" A heading ", " that wraps ", "━━━━━━━━━━━━"]);
    assert_eq!(r("## A heading that wraps", 12), ["A heading", "that wraps", "──────────"]);
}

// ---------------------------------------------------------------- inline

#[test]
fn bold_italic_strikethrough_and_code() {
    let t = Theme::acp();
    let ls = lines("**b** *i* ***bi*** ~~s~~ `c`", 40);
    assert_eq!(plain(&ls), ["b i bi s c"]);
    assert!(has(span_style(&ls, "b"), Modifier::BOLD));
    assert!(has(span_style(&ls, "i"), Modifier::ITALIC));
    let bi = span_style(&ls, "bi");
    assert!(has(bi, Modifier::BOLD) && has(bi, Modifier::ITALIC));
    assert!(has(span_style(&ls, "s"), Modifier::CROSSED_OUT));
    assert_eq!(span_style(&ls, "c"), ratatui::style::Style::from(&t.code()));
}

#[test]
fn links_show_their_url_and_autolinks_and_bare_urls_are_links() {
    let t = Theme::acp();
    let link = ratatui::style::Style::from(&t.link());
    let ls = lines("a [site](https://acp.us) and <https://x.org/a> and https://acp.us/rs/post/ABC.", 80);
    assert_eq!(plain(&ls), ["a site <https://acp.us> and https://x.org/a and https://acp.us/rs/post/ABC."]);
    assert_eq!(span_style(&ls, "site"), link);
    assert_eq!(span_style(&ls, "https://x.org/a"), link);
    assert_eq!(span_style(&ls, "https://acp.us/rs/post/ABC"), link, "the full stop isn't part of the URL");
    // A link whose text is its URL doesn't repeat it.
    assert_eq!(r("[https://acp.us](https://acp.us)", 80), ["https://acp.us"]);
}

#[test]
fn link_positions_are_reported_for_osc8() {
    let d = render_doc("x\n\n- see [the site](https://acp.us) now\n\n> https://x.org", 80, &Theme::acp());
    let l: Vec<_> = d.links.iter().map(|l| (l.line, l.col, l.text.as_str(), l.url.as_str())).collect();
    assert_eq!(l, [(2, 6, "the site", "https://acp.us"), (4, 2, "https://x.org", "https://x.org")]);
}

#[test]
fn images_render_as_links() {
    assert_eq!(r("![a star](https://acp.us/star.png)", 80), ["▣ a star <https://acp.us/star.png>"]);
}

#[test]
fn simple_html_is_honoured() {
    let ls = lines("<b>bold</b> <i>it</i> <s>gone</s><br>next <a href=\"https://acp.us\">site</a> <span>x</span>", 80);
    assert_eq!(plain(&ls), ["bold it gone", "next site <https://acp.us> x"]);
    assert!(has(span_style(&ls, "bold"), Modifier::BOLD));
    assert!(has(span_style(&ls, "it"), Modifier::ITALIC));
    assert!(has(span_style(&ls, "gone"), Modifier::CROSSED_OUT));
}

// ----------------------------------------------------------------- lists

#[test]
fn tight_bullet_lists_get_bullets_and_a_hanging_indent() {
    assert_eq!(r("- one\n- two words here\n- three", 12), ["• one", "• two words", "  here", "• three"]);
    // `*` and `+` are bullets too; switching the character starts a new list.
    assert_eq!(r("* a\n* b\n+ c", 12), ["• a", "• b", "", "• c"]);
}

#[test]
fn loose_bullet_lists_get_bullets_and_keep_their_spacing() {
    // Items separated by blank lines are wrapped in paragraphs by the parser.
    assert_eq!(r("- one\n\n- two words here\n\n- three", 12), ["• one", "", "• two words", "  here", "", "• three"]);
    // A second paragraph in an item is indented, without a second bullet.
    assert_eq!(r("- first\n\n  more of it\n\n- second", 20), ["• first", "", "  more of it", "", "• second"]);
}

#[test]
fn ordered_lists_are_numbered_from_their_start_and_aligned() {
    assert_eq!(r("1. one\n2. two words here\n3. three", 12), ["1. one", "2. two words", "   here", "3. three"]);
    assert_eq!(r("3. three\n\n4. four", 20), ["3. three", "", "4. four"]);
    assert_eq!(r("9. nine\n10. ten wraps here", 12), [" 9. nine", "10. ten", "    wraps", "    here"]);
    assert_eq!(r("1) paren\n2) style", 20), ["1. paren", "2. style"]);
}

#[test]
fn nested_lists_are_indented_with_their_own_markers() {
    assert_eq!(r("- outer\n  - inner one\n  - inner two\n    - third\n- next", 30),
        ["• outer", "  ◦ inner one", "  ◦ inner two", "    ▪ third", "• next"]);
    assert_eq!(r("1. step\n   - detail\n2. step", 30), ["1. step", "   ◦ detail", "2. step"]);
    assert_eq!(r("- outer\n\n  1. a\n  2. b\n\n- next", 30), ["• outer", "", "  1. a", "  2. b", "", "• next"]);
    // An item that opens with a sublist keeps its own bullet.
    assert_eq!(r("- - deep", 30), ["• ◦ deep"]);
    // An empty item still shows its bullet.
    assert_eq!(r("- \n- b", 30), ["• ", "• b"]);
}

#[test]
fn items_that_start_with_bold_code_or_a_link_keep_the_marker() {
    let t = Theme::acp();
    let ls = lines("- **Bold.** rest of the item\n- `code` first\n- [link](https://acp.us) first\n- *it* first", 40);
    let txt = plain(&ls);
    assert_eq!(txt[0], "• Bold. rest of the item");
    assert_eq!(txt[1], "• code first");
    assert!(txt[2].starts_with("• link"), "{txt:?}");
    assert_eq!(txt[3], "• it first");
    assert!(has(span_style(&ls, "Bold."), Modifier::BOLD));
    assert_eq!(span_style(&ls, "• ").fg, Some(t.accent), "markers use list_marker");
    assert_eq!(r("- **Counting things.** They count.\n\n- **Next.** More", 20), ["• Counting things.", "  They count.", "", "• Next. More"]);
    assert_eq!(r("1. **Bold** step", 20), ["1. Bold step"]);
}

#[test]
fn task_lists_show_boxes() {
    let ls = lines("- [x] done\n- [ ] open\n- plain", 30);
    assert_eq!(plain(&ls), ["☑ done", "☐ open", "• plain"]);
    assert!(has(span_style(&ls, "☑ "), Modifier::BOLD));
    assert_eq!(r("1. [ ] first", 30), ["1. ☐ first"]);
}

// ---------------------------------------------------------------- blocks

#[test]
fn block_quotes_have_a_gutter_and_nest() {
    let t = Theme::acp();
    let ls = lines("> quoted *text* that wraps\n>\n> > nested", 14);
    assert_eq!(plain(&ls), ["┃ quoted text", "┃ that wraps", "┃", "┃ ┃ nested"].map(|s| if s == "┃" { "┃ " } else { s }));
    assert_eq!(span_style(&ls, "quoted").fg, Some(t.muted));
    assert!(has(span_style(&ls, "quoted"), Modifier::ITALIC));
    assert_eq!(span_style(&ls, "┃ ").fg, Some(t.accent));
}

#[test]
fn fenced_and_indented_code_blocks_are_boxed() {
    let out = r("```rust\nlet x = 1;\n\tindented\n```", 24);
    assert_eq!(out, [
        "╭─ rust ───────────────╮",
        "│ let x = 1;           │",
        "│     indented         │",
        "╰──────────────────────╯",
    ]);
    let out = r("text\n\n    code one\n    code two", 20);
    assert_eq!(out, ["text", "", "╭──────────────────╮", "│ code one         │", "│ code two         │", "╰──────────────────╯"]);
    // Long lines wrap inside the box; markdown inside code stays literal.
    let out = r("```\n**not bold** and a long line\n```", 16);
    assert_eq!(out, ["╭──────────────╮", "│ **not bold** │", "│  and a long  │", "│ line         │", "╰──────────────╯"]);
}

#[test]
fn tables_are_box_drawn_aligned_and_wrap() {
    let t = Theme::acp();
    let src = "| Item | Votes | State |\n|:--|--:|:-:|\n| Strike fund | 12 | **open** |\n| Reading | 7 | x |";
    let ls = lines(src, 60);
    assert_eq!(plain(&ls), [
        "┌─────────────┬───────┬───────┐",
        "│ Item        │ Votes │ State │",
        "├─────────────┼───────┼───────┤",
        "│ Strike fund │    12 │ open  │",
        "│ Reading     │     7 │   x   │",
        "└─────────────┴───────┴───────┘",
    ]);
    let head = span_style(&ls, "Item");
    assert!(has(head, Modifier::BOLD) && head.fg == Some(t.accent), "table_header");
    assert!(has(span_style(&ls, "open"), Modifier::BOLD));
    // Too wide: the widest column shrinks and its cells wrap.
    let out = r("| a | b |\n|---|---|\n| short | a much longer cell of text |", 24);
    assert!(out.iter().all(|l| l.chars().count() <= 24), "{out:#?}");
    assert_eq!(out[3], "│ short │ a much       │", "{out:#?}");
    assert_eq!(out[4], "│       │ longer cell  │", "{out:#?}");
}

#[test]
fn rules_footnotes_and_their_order() {
    let out = r("a[^b] c[^a]\n\n---\n\n[^a]: First defined.\n[^b]: Referenced first.", 30);
    assert_eq!(out, ["a[1] c[2]", "", "──────────────────────────────", "", "────────────────────────", "[1] Referenced first.", "[2] First defined."]);
}

// ------------------------------------------------------------- normalise

#[test]
fn typed_bullets_become_list_items_and_centred_lines_stay_text() {
    assert_eq!(normalize("Intro:\n• one\n\t•\ttwo"), "Intro:\n- one\n- two");
    assert_eq!(r("Intro:\n• one\n• two", 30), ["Intro:", "", "• one", "• two"]);
    // A lone indented line is centring, not code; two or more lines are code.
    assert_eq!(r("top\n\n              (centred)\n\nbottom", 40), ["top", "", "(centred)", "", "bottom"]);
    assert!(r("top\n\n    a\n    b", 20)[2].starts_with('╭'));
    // List continuations and fenced code are left alone.
    assert_eq!(normalize("- a\n\n    more\n"), "- a\n\n    more\n");
    assert_eq!(normalize("```\n• x\n```"), "```\n• x\n```");
}

#[test]
fn styles_come_from_the_theme_elements() {
    let (mut f, _) = crate::themes::parse(include_str!("../../../themes/default.toml"), "default").unwrap();
    use crate::themes::{ColorSpec, Element};
    for (e, c) in [(Element::H3, 1u8), (Element::ListMarker, 2), (Element::TableHeader, 3), (Element::Footnote, 4), (Element::Rule, 5)] {
        let mut st = f.dark.style(e);
        st.fg = Some(ColorSpec::Value(Color::Indexed(c)));
        f.dark.set_style(e, st);
    }
    let mut st = f.dark.style(Element::Quote);
    st.border = Some(ColorSpec::Value(Color::Indexed(6)));
    f.dark.set_style(Element::Quote, st);
    let t = Theme::from_file(&f, crate::themes::Variant::Dark);
    let ls = render("### h\n\n- x\n\n| a |\n|---|\n| b |\n\n> q\n\n---\n\nn[^1]\n\n[^1]: note", 40, &t);
    assert_eq!(span_style(&ls, "h").fg, Some(Color::Indexed(1)));
    assert_eq!(span_style(&ls, "• ").fg, Some(Color::Indexed(2)));
    assert_eq!(span_style(&ls, "a").fg, Some(Color::Indexed(3)));
    assert_eq!(span_style(&ls, "[1]").fg, Some(Color::Indexed(4)));
    assert_eq!(span_style(&ls, &"─".repeat(40)).fg, Some(Color::Indexed(5)));
    assert_eq!(span_style(&ls, "┃ ").fg, Some(Color::Indexed(6)));
}

// ------------------------------------------------------------ real posts

#[test]
fn the_audit_post_shows_a_bullet_on_every_item() {
    // A real post (loose list, every item bold-led) where the bullets used to be missing.
    let out = r(include_str!("fixtures/audit-post.md"), 120);
    for item in ["Checking documents.", "Confirming with outsiders.", "Counting things.", "Recalculating.", "Testing the system."] {
        let l = out.iter().find(|l| l.contains(item)).unwrap_or_else(|| panic!("{item} missing: {out:#?}"));
        assert!(l.starts_with(&format!("• {item}")), "no bullet: {l:?}");
    }
    assert_eq!(out.iter().filter(|l| l.starts_with("• ")).count(), 5);
    let i = out.iter().position(|l| l.starts_with("• Checking documents.")).unwrap();
    assert!(out[i + 1].starts_with("  ") && !out[i + 1].starts_with("   "), "{:?}", out[i + 1]);
}

#[test]
fn real_posts_with_typed_bullets_star_lists_tables_and_centring() {
    let out = r(include_str!("fixtures/typed-bullets.md"), 80);
    assert_eq!(out.iter().filter(|l| l.starts_with("• ")).count(), 7, "{out:#?}");
    assert_eq!(out[0], "You’ll get practical training for:");
    let out = r(include_str!("fixtures/tab-bullets.md"), 60);
    assert_eq!(out.iter().filter(|l| l.starts_with("• ")).count(), 2, "{out:#?}");
    assert!(out.iter().all(|l| !l.contains('\t') && !l.starts_with('╭')), "not code: {out:#?}");
    let out = r(include_str!("fixtures/star-loose-list.md"), 80);
    assert_eq!(out.iter().filter(|l| l.starts_with("• ")).count(), 4, "{out:#?}");
    let out = r(include_str!("fixtures/headings-and-table.md"), 100);
    assert_eq!(out[0], "PHASE 2 — SCORING & RANKING");
    assert!(out[1].starts_with('─'));
    assert!(out.iter().any(|l| l.starts_with("┌")) && out.iter().all(|l| l.chars().count() <= 100), "{out:#?}");
    assert!(out.iter().any(|l| l.contains("│ Criterion ") && l.contains("│ Weight ")), "{out:#?}");
    let out = r(include_str!("fixtures/centred-line.md"), 80);
    assert!(out.contains(&"(like me😭)".to_string()) && !out.iter().any(|l| l.starts_with('╭')), "{out:#?}");
}

#[test]
fn the_showcase_renders_every_construct() {
    let out = r(include_str!("fixtures/markdown-showcase.md"), 70);
    let all = out.join("\n");
    for want in ["Markdown showcase", "━━━", "• Tight bullet item", "  ◦ Nested bullet", "    ▪ Third level", "1. First step",
        "☑ Task done", "☐ Task open", "╭─ rust ", "│ indented code block", "┃ A block quote", "┃ ┃ A nested quote",
        "┌", "│ Strike fund   │    12 │  open  │", "Heading six", "a break.", "▣ The red star", "[1] Footnotes",
        "entities & © —", "*star*", "https://example.org/docs"] {
        assert!(all.contains(want), "{want:?} missing:\n{all}");
    }
    assert!(out.iter().all(|l| unicode_width::UnicodeWidthStr::width(l.as_str()) <= 70), "{all}");
}

#[test]
#[ignore]
fn print_showcase() {
    for l in r(include_str!("fixtures/markdown-showcase.md"), 70) { println!("|{l}|"); }
}
