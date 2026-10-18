//! Post bodies as Markdown → styled, word-wrapped ratatui lines: headings,
//! bold / italic / strikethrough, inline and block code, bullet and numbered
//! lists, block quotes and links. Single line breaks are kept (bodies are
//! stored as plain text whose newlines matter), and paragraphs stay separated
//! by a blank line.

use crate::tui::theme::{Gloss, Theme};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// Styled words of one logical line before wrapping.
#[derive(Default)]
struct Block {
    /// Prefix on the first visual line (bullet, quote bar) and on the rest (indent, quote bar).
    first: Vec<Span<'static>>,
    rest: Vec<Span<'static>>,
    /// (text, style) pieces.
    parts: Vec<(String, Style)>,
}

/// Word-wrap styled `parts` to `width`, with the given prefixes.
fn wrap_parts(b: Block, width: usize, out: &mut Vec<Line<'static>>) {
    let pw = |p: &[Span]| p.iter().map(|s| s.content.width()).sum::<usize>();
    let mut line: Vec<Span<'static>> = b.first.clone();
    let mut w = pw(&b.first);
    let mut started = false;
    let rest_w = pw(&b.rest);
    for (text, st) in b.parts {
        // Split into words keeping single spaces as separators.
        for (k, word) in text.split(' ').enumerate() {
            if k > 0 && started && w < width { line.push(Span::styled(" ", st)); w += 1; }
            if word.is_empty() { continue; }
            let ww = word.width();
            if started && w + ww > width {
                // Trim a trailing space before breaking.
                if line.last().is_some_and(|s| s.content == " ") { line.pop(); }
                out.push(Line::from(std::mem::take(&mut line)));
                line = b.rest.clone();
                w = rest_w;
            }
            if ww > width.saturating_sub(w) && ww > width / 2 {
                // Hard-break very long words (URLs).
                let mut chunk = String::new();
                let mut cw = 0;
                for ch in word.chars() {
                    let c = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                    if w + cw + c > width && (cw > 0 || started) {
                        if !chunk.is_empty() { line.push(Span::styled(std::mem::take(&mut chunk), st)); }
                        out.push(Line::from(std::mem::take(&mut line)));
                        line = b.rest.clone();
                        w = rest_w;
                        cw = 0;
                    }
                    chunk.push(ch);
                    cw += c;
                }
                w += cw;
                line.push(Span::styled(chunk, st));
            } else {
                line.push(Span::styled(word.to_string(), st));
                w += ww;
            }
            started = true;
        }
    }
    out.push(Line::from(line));
}

/// Render `text` as Markdown wrapped to `width` columns.
pub fn render(text: &str, width: usize, t: &Theme) -> Vec<Line<'static>> {
    let width = width.max(8);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let mut style_stack: Vec<Gloss> = vec![t.plain()];
    let mut cur = Block::default();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut quote = 0usize;
    let mut in_code = false;
    let mut link: Option<String> = None;
    let mut pending_item = false;
    let blank = |out: &mut Vec<Line<'static>>| if out.last().is_some_and(|l: &Line| !l.spans.is_empty() && l.width() > 0) { out.push(Line::raw("")); };
    let quote_prefix = |q: usize, t: &Theme| -> Vec<Span<'static>> { (0..q).map(|_| Span::styled("│ ", t.muted())).collect() };
    let flush = |cur: &mut Block, out: &mut Vec<Line<'static>>| {
        let b = std::mem::take(cur);
        if b.parts.iter().any(|(s, _)| !s.trim().is_empty()) { wrap_parts(b, width, out); }
    };
    let start_block = |cur: &mut Block, lists: &[Option<u64>], quote: usize, pending: &mut bool, t: &Theme| {
        let indent = "  ".repeat(lists.len().saturating_sub(1));
        let mut first = quote_prefix(quote, t);
        let mut rest = quote_prefix(quote, t);
        if let Some(l) = lists.last() {
            let bullet = if *pending { match l { Some(n) => format!("{indent}{n}. "), None => format!("{indent}• ") } } else { format!("{indent}  ") };
            let pad = " ".repeat(bullet.width());
            first.push(Span::styled(bullet, t.accent()));
            rest.push(Span::raw(pad));
            *pending = false;
        }
        cur.first = first;
        cur.rest = rest;
    };
    // Open a block (with the item's bullet when one is due) before the first inline content.
    let ensure_block = |cur: &mut Block, lists: &[Option<u64>], quote: usize, pending: &mut bool| {
        if *pending || (cur.first.is_empty() && cur.rest.is_empty() && cur.parts.is_empty()) { start_block(cur, lists, quote, pending, t); }
    };
    for ev in Parser::new_ext(text, opts) {
        let g = style_stack.last().cloned().unwrap_or_else(|| t.plain());
        let st = Style::from(&g);
        match ev {
            Event::Start(tag) => match tag {
                Tag::Paragraph => { if lists.is_empty() { blank(&mut out); } start_block(&mut cur, &lists, quote, &mut pending_item, t); }
                Tag::Heading { level, .. } => {
                    blank(&mut out);
                    start_block(&mut cur, &lists, quote, &mut pending_item, t);
                    style_stack.push(if level == HeadingLevel::H1 { t.accent_bold().underline(true) } else { t.accent_bold() });
                }
                Tag::BlockQuote(_) => { blank(&mut out); quote += 1; }
                Tag::CodeBlock(_) => { blank(&mut out); in_code = true; }
                Tag::List(start) => {
                    if lists.is_empty() { blank(&mut out); }
                    // An item that opens straight with a sublist still shows its own bullet.
                    if pending_item { start_block(&mut cur, &lists, quote, &mut pending_item, t); cur.parts.push((String::new(), st)); wrap_parts(std::mem::take(&mut cur), width, &mut out); }
                    flush(&mut cur, &mut out);
                    lists.push(start);
                }
                // The bullet goes on whatever block opens the item: its text in a tight list, its first
                // paragraph in a loose one (so the paragraph's own start can't replace it).
                Tag::Item => { flush(&mut cur, &mut out); pending_item = true; }
                Tag::Emphasis => { ensure_block(&mut cur, &lists, quote, &mut pending_item); style_stack.push(g.italic(true)) }
                Tag::Strong => { ensure_block(&mut cur, &lists, quote, &mut pending_item); style_stack.push(g.bold(true)) }
                Tag::Strikethrough => { ensure_block(&mut cur, &lists, quote, &mut pending_item); style_stack.push(g.strikethrough(true)) }
                Tag::Link { dest_url, .. } => { ensure_block(&mut cur, &lists, quote, &mut pending_item); link = Some(dest_url.to_string()); style_stack.push(t.link()); }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => flush(&mut cur, &mut out),
                TagEnd::Heading(_) => { flush(&mut cur, &mut out); style_stack.pop(); }
                TagEnd::BlockQuote(_) => { flush(&mut cur, &mut out); quote = quote.saturating_sub(1); }
                TagEnd::CodeBlock => in_code = false,
                TagEnd::List(_) => { flush(&mut cur, &mut out); lists.pop(); }
                TagEnd::Item => {
                    // An empty item still shows its bullet.
                    if pending_item { start_block(&mut cur, &lists, quote, &mut pending_item, t); cur.parts.push((String::new(), st)); wrap_parts(std::mem::take(&mut cur), width, &mut out); }
                    flush(&mut cur, &mut out);
                    if let Some(Some(n)) = lists.last_mut() { *n += 1; }
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => { style_stack.pop(); }
                TagEnd::Link => {
                    style_stack.pop();
                    if let Some(u) = link.take() {
                        let shown: String = cur.parts.iter().rev().take(1).map(|(s, _)| s.clone()).collect();
                        if !u.is_empty() && !shown.contains(u.as_str()) { cur.parts.push((format!(" <{u}>"), t.muted().into())); }
                    }
                }
                _ => {}
            },
            Event::Text(s) if in_code => {
                for l in s.trim_end_matches('\n').split('\n') {
                    let mut first = quote_prefix(quote, t);
                    first.push(Span::styled("  ", t.muted()));
                    wrap_parts(Block { rest: first.clone(), first, parts: vec![(l.to_string(), t.muted().into())] }, width, &mut out);
                }
            }
            Event::Text(s) => {
                ensure_block(&mut cur, &lists, quote, &mut pending_item);
                cur.parts.push((s.to_string(), st));
            }
            Event::Code(s) => { ensure_block(&mut cur, &lists, quote, &mut pending_item); cur.parts.push((s.to_string(), t.code().into())) }
            Event::SoftBreak | Event::HardBreak => {
                // Keep the author's line breaks.
                let (first, rest) = (cur.first.clone(), cur.rest.clone());
                flush(&mut cur, &mut out);
                cur.first = rest.clone();
                cur.rest = rest;
                let _ = first;
            }
            Event::Rule => { blank(&mut out); out.push(Line::styled("─".repeat(width.min(40)), t.muted())); }
            Event::Html(s) | Event::InlineHtml(s) => { ensure_block(&mut cur, &lists, quote, &mut pending_item); cur.parts.push((s.to_string(), st)) }
            _ => {}
        }
    }
    flush(&mut cur, &mut out);
    while out.first().is_some_and(|l| l.width() == 0) { out.remove(0); }
    while out.last().is_some_and(|l| l.width() == 0) { out.pop(); }
    out
}

/// Plain text of rendered lines (tests, card snippets).
pub fn plain(lines: &[Line]) -> Vec<String> {
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect()
}
