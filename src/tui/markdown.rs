//! Post bodies as Markdown → styled, word-wrapped ratatui lines.
//!
//! CommonMark plus the GitHub extensions (tables, task lists, strikethrough,
//! footnotes) parsed by `pulldown-cmark` into a small block tree, then laid
//! out to a width: headings H1–H6 (a `━` / `─` rule under H1 / H2), bold,
//! italic, strikethrough, inline code, boxed code blocks, block quotes with a
//! `┃` gutter, bullet / numbered / task lists (nested, tight or loose) with
//! hanging indents, box-drawn tables with aligned, wrapping columns, rules,
//! links and autolinks (bare `https://` URLs too) with their URL shown,
//! images as links, and footnotes listed at the end. Every style comes from
//! the [`Theme`] (the markdown elements `h1`…`footnote`).
//!
//! Single line breaks are kept: post bodies are stored as plain text whose
//! newlines matter. [`normalize`] first repairs what the site's plain text
//! does to Markdown (typed `•` bullets, a lone indented line used to centre
//! text), and simple inline HTML (`<b>`, `<br>`, `<a href>`, …) is honoured.

use crate::tui::theme::{Gloss, Theme};
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use regex::Regex;
use std::sync::LazyLock;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// ------------------------------------------------------------------ model

/// Inline formatting in effect for a piece of text.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Fmt { bold: bool, italic: bool, strike: bool, code: bool, link: bool, note: bool, muted: bool }

/// Inline content: styled text (with its link target) or a line break.
#[derive(Clone, Debug)]
enum Inl { Text(String, Fmt, Option<String>), Break }

/// A table: column alignments, header cells and body rows.
#[derive(Debug, Default)]
struct TableData { aligns: Vec<Alignment>, head: Vec<Vec<Inl>>, rows: Vec<Vec<Vec<Inl>>>, row: Vec<Vec<Inl>> }

#[derive(Debug)]
enum Blk {
    Para(Vec<Inl>),
    Heading(u8, Vec<Inl>),
    Quote(Vec<Blk>),
    Code(String, String),
    List { start: Option<u64>, loose: bool, items: Vec<Item> },
    Table(TableData),
    Rule,
}

#[derive(Debug, Default)]
struct Item { task: Option<bool>, blocks: Vec<Blk> }

enum Frame {
    Root(Vec<Blk>),
    Quote(Vec<Blk>),
    List { start: Option<u64>, loose: bool, items: Vec<Item> },
    Item(Item),
    Note(String, Vec<Blk>),
    Table(TableData),
}

/// Where the inline content being collected goes when it ends.
#[derive(Clone, Copy, PartialEq)]
enum Sink { Para, Heading(u8), Cell, Implicit }

static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"https?://[^\s<>"]+"#).expect("url regex"));
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?s)<(/?)([A-Za-z][A-Za-z0-9]*)([^>]*)>|[^<]+|<"#).expect("tag regex"));
static HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)href\s*=\s*["']([^"']*)["']"#).expect("href regex"));

/// A bare URL without the punctuation that usually follows it in prose.
fn trim_url(u: &str) -> &str {
    let mut u = u;
    loop {
        let Some(c) = u.chars().last() else { return u };
        let unbalanced = c == ')' && u.matches('(').count() < u.matches(')').count();
        if ".,;:!?'\"*_".contains(c) || unbalanced || c == ']' && !u.contains('[') { u = &u[..u.len() - c.len_utf8()]; } else { return u; }
    }
}

fn entities(s: &str) -> String {
    s.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'").replace("&amp;", "&")
}

struct Builder {
    stack: Vec<Frame>,
    inl: Vec<Inl>,
    sink: Option<Sink>,
    bold: u32,
    italic: u32,
    strike: u32,
    code: u32,
    links: Vec<(String, usize)>,
    block_code: Option<(String, String)>,
    notes: Vec<(String, Vec<Blk>)>,
    note_ids: Vec<String>,
}

impl Builder {
    fn new() -> Self {
        Builder { stack: vec![Frame::Root(Vec::new())], inl: Vec::new(), sink: None, bold: 0, italic: 0, strike: 0, code: 0,
            links: Vec::new(), block_code: None, notes: Vec::new(), note_ids: Vec::new() }
    }

    fn fmt(&self) -> Fmt {
        Fmt { bold: self.bold > 0, italic: self.italic > 0, strike: self.strike > 0, code: self.code > 0, link: !self.links.is_empty(), ..Fmt::default() }
    }
    fn url(&self) -> Option<String> { self.links.last().map(|(u, _)| u.clone()) }

    fn ensure_inline(&mut self) { if self.sink.is_none() { self.sink = Some(Sink::Implicit); } }

    /// End text that came straight inside a container (a tight list item).
    fn close_implicit(&mut self) {
        if self.sink == Some(Sink::Implicit) {
            self.sink = None;
            let inl = std::mem::take(&mut self.inl);
            self.push_block(Blk::Para(inl));
        }
    }

    fn push_block(&mut self, b: Blk) {
        if let Blk::Para(i) = &b && i.is_empty() { return; }
        for f in self.stack.iter_mut().rev() {
            match f {
                Frame::Root(v) | Frame::Quote(v) | Frame::Note(_, v) => { v.push(b); return; }
                Frame::Item(it) => { it.blocks.push(b); return; }
                _ => {}
            }
        }
    }

    fn text(&mut self, s: &str) {
        self.ensure_inline();
        let f = self.fmt();
        if f.link || f.code {
            self.inl.push(Inl::Text(s.to_string(), f, self.url()));
            return;
        }
        // Bare URLs become links.
        let mut at = 0;
        for m in URL.find_iter(s) {
            let u = trim_url(m.as_str());
            if u.len() < 10 { continue; }
            if m.start() > at { self.inl.push(Inl::Text(s[at..m.start()].to_string(), f, None)); }
            self.inl.push(Inl::Text(u.to_string(), Fmt { link: true, ..f }, Some(u.to_string())));
            at = m.start() + u.len();
        }
        if at < s.len() { self.inl.push(Inl::Text(s[at..].to_string(), f, None)); }
    }

    fn end_link(&mut self) {
        let Some((url, start)) = self.links.pop() else { return };
        let shown: String = self.inl[start.min(self.inl.len())..].iter().filter_map(|i| match i { Inl::Text(s, ..) => Some(s.as_str()), Inl::Break => None }).collect();
        if !url.is_empty() && !shown.contains(url.as_str()) && !url.starts_with('#') {
            self.inl.push(Inl::Text(format!(" <{url}>"), Fmt { muted: true, ..Fmt::default() }, None));
        }
    }

    /// Simple HTML as the formatting it stands for; other tags are dropped.
    fn html(&mut self, s: &str) {
        for m in TAG.captures_iter(s) {
            let whole = m.get(0).map_or("", |x| x.as_str());
            let Some(name) = m.get(2) else {
                let t = entities(whole);
                if !t.trim().is_empty() || self.sink.is_some() { self.text(&t.replace('\n', " ")); }
                continue;
            };
            let close = m.get(1).is_some_and(|c| !c.as_str().is_empty());
            let dec = |n: &mut u32| *n = n.saturating_sub(1);
            match (name.as_str().to_ascii_lowercase().as_str(), close) {
                ("br", _) => { self.ensure_inline(); self.inl.push(Inl::Break); }
                ("p" | "div", true) => { self.ensure_inline(); self.inl.push(Inl::Break); }
                ("li", false) => { self.ensure_inline(); if !self.inl.is_empty() { self.inl.push(Inl::Break); } self.text("• "); }
                ("b" | "strong", false) => self.bold += 1,
                ("b" | "strong", true) => dec(&mut self.bold),
                ("i" | "em", false) => self.italic += 1,
                ("i" | "em", true) => dec(&mut self.italic),
                ("s" | "del" | "strike", false) => self.strike += 1,
                ("s" | "del" | "strike", true) => dec(&mut self.strike),
                ("code" | "kbd" | "tt", false) => self.code += 1,
                ("code" | "kbd" | "tt", true) => dec(&mut self.code),
                ("a", false) => {
                    let href = m.get(3).and_then(|a| HREF.captures(a.as_str())).and_then(|c| c.get(1)).map(|h| entities(h.as_str())).unwrap_or_default();
                    self.ensure_inline();
                    let at = self.inl.len();
                    self.links.push((href, at));
                }
                ("a", true) => self.end_link(),
                _ => {}
            }
        }
    }

    fn event(&mut self, ev: Event) {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    self.close_implicit();
                    self.sink = Some(Sink::Para);
                    // A paragraph directly inside an item makes the list loose.
                    let n = self.stack.len();
                    if n >= 2 && matches!(self.stack[n - 1], Frame::Item(_))
                        && let Frame::List { loose, .. } = &mut self.stack[n - 2] { *loose = true; }
                }
                Tag::Heading { level, .. } => {
                    self.close_implicit();
                    let n = match level { HeadingLevel::H1 => 1, HeadingLevel::H2 => 2, HeadingLevel::H3 => 3, HeadingLevel::H4 => 4, HeadingLevel::H5 => 5, HeadingLevel::H6 => 6 };
                    self.sink = Some(Sink::Heading(n));
                }
                Tag::BlockQuote(_) => { self.close_implicit(); self.stack.push(Frame::Quote(Vec::new())); }
                Tag::CodeBlock(kind) => {
                    self.close_implicit();
                    let lang = match kind { CodeBlockKind::Fenced(l) => l.split_whitespace().next().unwrap_or("").to_string(), CodeBlockKind::Indented => String::new() };
                    self.block_code = Some((lang, String::new()));
                }
                Tag::HtmlBlock => { self.close_implicit(); self.sink = Some(Sink::Para); }
                Tag::List(start) => { self.close_implicit(); self.stack.push(Frame::List { start, loose: false, items: Vec::new() }); }
                Tag::Item => { self.close_implicit(); self.stack.push(Frame::Item(Item::default())); }
                Tag::Table(aligns) => { self.close_implicit(); self.stack.push(Frame::Table(TableData { aligns, ..TableData::default() })); }
                Tag::TableHead | Tag::TableRow => { if let Some(Frame::Table(t)) = self.stack.last_mut() { t.row.clear(); } }
                Tag::TableCell => { self.inl.clear(); self.sink = Some(Sink::Cell); }
                Tag::FootnoteDefinition(label) => { self.close_implicit(); self.stack.push(Frame::Note(label.to_string(), Vec::new())); }
                Tag::Emphasis => self.italic += 1,
                Tag::Strong => self.bold += 1,
                Tag::Strikethrough => self.strike += 1,
                Tag::Link { dest_url, .. } => { self.ensure_inline(); let at = self.inl.len(); self.links.push((dest_url.to_string(), at)); }
                Tag::Image { dest_url, .. } => {
                    self.ensure_inline();
                    let at = self.inl.len();
                    self.links.push((dest_url.to_string(), at));
                    // ▣, not 🖼: unicode-width counts 🖼 as one cell, tmux and many terminals as two.
                    self.inl.push(Inl::Text("▣ ".into(), self.fmt(), self.url()));
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph | TagEnd::HtmlBlock => {
                    self.sink = None;
                    while matches!(self.inl.last(), Some(Inl::Break)) { self.inl.pop(); }
                    let inl = std::mem::take(&mut self.inl);
                    self.push_block(Blk::Para(inl));
                }
                TagEnd::Heading(_) => {
                    let n = match self.sink { Some(Sink::Heading(n)) => n, _ => 1 };
                    self.sink = None;
                    let inl = std::mem::take(&mut self.inl);
                    self.push_block(Blk::Heading(n, inl));
                }
                TagEnd::BlockQuote(_) => {
                    self.close_implicit();
                    if let Some(Frame::Quote(v)) = self.stack.pop() { self.push_block(Blk::Quote(v)); }
                }
                TagEnd::CodeBlock => {
                    if let Some((lang, mut text)) = self.block_code.take() {
                        if text.ends_with('\n') { text.pop(); }
                        self.push_block(Blk::Code(lang, text));
                    }
                }
                TagEnd::List(_) => {
                    self.close_implicit();
                    if let Some(Frame::List { start, loose, items }) = self.stack.pop() { self.push_block(Blk::List { start, loose, items }); }
                }
                TagEnd::Item => {
                    self.close_implicit();
                    if let Some(Frame::Item(it)) = self.stack.pop()
                        && let Some(Frame::List { items, .. }) = self.stack.last_mut() { items.push(it); }
                }
                TagEnd::TableCell => {
                    self.sink = None;
                    let inl = std::mem::take(&mut self.inl);
                    if let Some(Frame::Table(t)) = self.stack.last_mut() { t.row.push(inl); }
                }
                TagEnd::TableHead => { if let Some(Frame::Table(t)) = self.stack.last_mut() { t.head = std::mem::take(&mut t.row); } }
                TagEnd::TableRow => { if let Some(Frame::Table(t)) = self.stack.last_mut() { let r = std::mem::take(&mut t.row); t.rows.push(r); } }
                TagEnd::Table => { if let Some(Frame::Table(t)) = self.stack.pop() { self.push_block(Blk::Table(t)); } }
                TagEnd::FootnoteDefinition => {
                    self.close_implicit();
                    if let Some(Frame::Note(l, v)) = self.stack.pop() { self.notes.push((l, v)); }
                }
                TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
                TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
                TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
                TagEnd::Link | TagEnd::Image => self.end_link(),
                _ => {}
            },
            Event::Text(s) => match &mut self.block_code { Some((_, c)) => c.push_str(&s), None => self.text(&s) },
            Event::Code(s) => {
                self.ensure_inline();
                let f = Fmt { code: true, ..self.fmt() };
                self.inl.push(Inl::Text(s.to_string(), f, self.url()));
            }
            Event::SoftBreak | Event::HardBreak => { self.ensure_inline(); self.inl.push(Inl::Break); }
            Event::Rule => { self.close_implicit(); self.push_block(Blk::Rule); }
            Event::TaskListMarker(done) => {
                for f in self.stack.iter_mut().rev() { if let Frame::Item(it) = f { it.task = Some(done); break; } }
            }
            Event::FootnoteReference(label) => {
                self.ensure_inline();
                let n = match self.note_ids.iter().position(|l| l.as_str() == label.as_ref()) {
                    Some(i) => i + 1,
                    None => { self.note_ids.push(label.to_string()); self.note_ids.len() }
                };
                self.inl.push(Inl::Text(format!("[{n}]"), Fmt { note: true, ..Fmt::default() }, None));
            }
            Event::Html(s) | Event::InlineHtml(s) => self.html(&s),
            _ => {}
        }
    }

    fn finish(mut self) -> (Vec<Blk>, Vec<(usize, Vec<Blk>)>) {
        self.close_implicit();
        while self.stack.len() > 1 {
            // Unbalanced input: fold open containers into their parents.
            match self.stack.pop() {
                Some(Frame::Quote(v)) => self.push_block(Blk::Quote(v)),
                Some(Frame::Item(it)) => { if let Some(Frame::List { items, .. }) = self.stack.last_mut() { items.push(it); } }
                Some(Frame::List { start, loose, items }) => self.push_block(Blk::List { start, loose, items }),
                Some(Frame::Table(t)) => self.push_block(Blk::Table(t)),
                Some(Frame::Note(l, v)) => self.notes.push((l, v)),
                _ => {}
            }
        }
        let root = match self.stack.pop() { Some(Frame::Root(v)) => v, _ => Vec::new() };
        // Notes in the order they are first referenced, then any never referenced.
        let mut notes = Vec::new();
        let mut defs = self.notes;
        for (i, id) in self.note_ids.iter().enumerate() {
            if let Some(p) = defs.iter().position(|(l, _)| l == id) { notes.push((i + 1, defs.remove(p).1)); }
        }
        let mut n = self.note_ids.len();
        for (_, v) in defs { n += 1; notes.push((n, v)); }
        (root, notes)
    }
}

// ------------------------------------------------------------- normalise

fn bullet_char(t: &str) -> Option<&str> {
    let c = t.chars().next()?;
    if !"•●▪◦‣".contains(c) { return None; }
    let rest = &t[c.len_utf8()..];
    rest.starts_with([' ', '\t']).then_some(rest)
}

fn is_list_line(t: &str) -> bool {
    let t = t.trim_start();
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") { return true; }
    let d = t.chars().take_while(|c| c.is_ascii_digit()).count();
    d > 0 && (t[d..].starts_with(". ") || t[d..].starts_with(") "))
}

/// Repairs for Markdown as the site stores it (plain text, newlines kept):
/// lines starting with a typed bullet (`•`, `●`, `▪`, `◦`, `‣`) become list
/// items, and a lone indented line between blank lines (text centred with
/// spaces) is un-indented instead of turning into a code block. Fenced code
/// is left alone.
pub fn normalize(src: &str) -> String {
    let src = src.replace("\r\n", "\n");
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut fence: Option<&str> = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        if let Some(f) = fence {
            if t.starts_with(f) { fence = None; }
            out.push(l.to_string());
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(&t[..3]);
            out.push(l.to_string());
            continue;
        }
        if let Some(rest) = bullet_char(t) {
            out.push(format!("- {}", rest.trim_start()));
            continue;
        }
        let indented = l.starts_with("    ") || l.starts_with('\t');
        if indented && !t.is_empty() {
            let prev_blank = i == 0 || lines[i - 1].trim().is_empty();
            let next_blank = i + 1 >= lines.len() || lines[i + 1].trim().is_empty();
            let prev = lines[..i].iter().rev().find(|p| !p.trim().is_empty());
            let after_list = prev.is_some_and(|p| is_list_line(p) || p.starts_with("    ") || p.starts_with('\t') || bullet_char(p.trim_start()).is_some());
            if prev_blank && next_blank && !after_list { out.push(t.to_string()); continue; }
        }
        out.push(l.to_string());
    }
    out.join("\n")
}

// ---------------------------------------------------------------- layout

/// A laid-out line and the links on it: (column, text, URL).
#[derive(Clone, Default)]
struct RLine { spans: Vec<Span<'static>>, links: Vec<(usize, String, String)> }

impl RLine {
    fn width(&self) -> usize { self.spans.iter().map(|s| s.content.width()).sum() }
    fn styled(s: String, st: Style) -> RLine { RLine { spans: vec![Span::styled(s, st)], links: Vec::new() } }
    fn prefixed(mut self, p: &[Span<'static>]) -> RLine {
        let pw: usize = p.iter().map(|s| s.content.width()).sum();
        let mut spans = p.to_vec();
        spans.append(&mut self.spans);
        RLine { spans, links: self.links.into_iter().map(|(c, t, u)| (c + pw, t, u)).collect() }
    }
    fn push(&mut self, s: String, st: Style, url: Option<&String>, at: usize) {
        if let Some(u) = url {
            match self.links.last_mut() {
                Some((c, t, lu)) if lu == u && *c + t.width() + 1 >= at => { while *c + t.width() < at { t.push(' '); } t.push_str(&s); }
                _ => self.links.push((at, s.clone(), u.clone())),
            }
        }
        self.spans.push(Span::styled(s, st));
    }
}

fn pad(n: usize) -> Span<'static> { Span::raw(" ".repeat(n)) }

/// Display width of inline content on one line (the widest line when it has breaks).
fn inl_width(inl: &[Inl]) -> usize {
    let mut best = 0;
    let mut w = 0;
    for i in inl {
        match i { Inl::Text(s, ..) => w += s.width(), Inl::Break => { best = best.max(w); w = 0; } }
    }
    best.max(w)
}

struct Layout<'a> { t: &'a Theme, depth: usize }

impl Layout<'_> {
    fn style(&self, base: &Gloss, f: Fmt) -> Style {
        let t = self.t;
        let mut g = if f.code { t.code() } else if f.link { t.link() } else if f.note { t.footnote() } else if f.muted { t.muted() } else { base.clone() };
        if f.bold { g = g.bold(true); }
        if f.italic { g = g.italic(true); }
        if f.strike { g = g.strikethrough(true); }
        Style::from(&g)
    }

    /// Word-wrap inline content to `width`.
    fn wrap(&self, inl: &[Inl], width: usize, base: &Gloss) -> Vec<RLine> {
        let width = width.max(1);
        let mut out = Vec::new();
        let mut line = RLine::default();
        let mut w = 0usize;
        let mut started = false;
        for piece in inl {
            let (text, f, url) = match piece {
                Inl::Break => { out.push(std::mem::take(&mut line)); w = 0; started = false; continue; }
                Inl::Text(s, f, u) => (s, *f, u.as_ref()),
            };
            let st = self.style(base, f);
            for (k, word) in text.split(' ').enumerate() {
                if k > 0 && started && w < width {
                    // The space keeps the link's underline across words of one link.
                    line.spans.push(Span::styled(" ", if url.is_some() { st } else { self.style(base, Fmt { strike: false, ..f }) }));
                    w += 1;
                }
                if word.is_empty() { continue; }
                let ww = word.width();
                if started && w + ww > width {
                    if line.spans.last().is_some_and(|s| s.content == " ") { line.spans.pop(); }
                    out.push(std::mem::take(&mut line));
                    w = 0;
                }
                if ww > width.saturating_sub(w) {
                    // Hard-break words longer than the line (URLs).
                    let mut chunk = String::new();
                    let mut cw = 0;
                    let mut at = w;
                    for ch in word.chars() {
                        let c = ch.width().unwrap_or(0);
                        if w + cw + c > width && cw > 0 {
                            line.push(std::mem::take(&mut chunk), st, url, at);
                            out.push(std::mem::take(&mut line));
                            w = 0;
                            cw = 0;
                            at = 0;
                        }
                        chunk.push(ch);
                        cw += c;
                    }
                    line.push(chunk, st, url, at);
                    w += cw;
                } else {
                    line.push(word.to_string(), st, url, w);
                    w += ww;
                }
                started = true;
            }
        }
        out.push(line);
        out
    }

    fn blocks(&mut self, bs: &[Blk], width: usize, base: &Gloss, tight: bool) -> Vec<RLine> {
        let mut out = Vec::new();
        for (i, b) in bs.iter().enumerate() {
            if i > 0 && !tight { out.push(RLine::default()); }
            out.extend(self.block(b, width, base));
        }
        out
    }

    fn block(&mut self, b: &Blk, width: usize, base: &Gloss) -> Vec<RLine> {
        let t = self.t;
        match b {
            Blk::Para(inl) => self.wrap(inl, width, base),
            Blk::Heading(n, inl) => {
                let g = t.heading(*n);
                let mut v = if *n == 1 {
                    // H1 is a banner: one cell of its background either side.
                    let st = Style::from(&g);
                    self.wrap(inl, width.saturating_sub(2).max(1), &g).into_iter().map(|l| {
                        let mut l = l.prefixed(&[Span::styled(" ", st)]);
                        l.spans.push(Span::styled(" ", st));
                        l
                    }).collect()
                } else { self.wrap(inl, width, &g) };
                if *n <= 2 {
                    let w = v.iter().map(RLine::width).max().unwrap_or(0).max(3);
                    v.push(RLine::styled((if *n == 1 { "━" } else { "─" }).repeat(w), Style::from(&t.heading_rule(*n))));
                }
                v
            }
            Blk::Quote(bs) => {
                let bar = Span::styled("┃ ", Style::from(&t.quote_bar()));
                self.blocks(bs, width.saturating_sub(2).max(1), &t.quote(), false).into_iter().map(|l| l.prefixed(std::slice::from_ref(&bar))).collect()
            }
            Blk::Code(lang, text) => self.code(lang, text, width),
            Blk::List { start, loose, items } => self.list(*start, *loose, items, width, base),
            Blk::Table(td) => self.table(td, width),
            Blk::Rule => vec![RLine::styled("─".repeat(width), Style::from(&t.rule()))],
        }
    }

    fn code(&self, lang: &str, text: &str, width: usize) -> Vec<RLine> {
        let t = self.t;
        let bs = Style::from(&t.code_block_border());
        let ts = Style::from(&t.code_block());
        let text = text.replace('\t', "    ");
        if width < 6 {
            return text.split('\n').map(|l| RLine::styled(l.to_string(), ts)).collect();
        }
        let inner = width - 4;
        let mut out = Vec::new();
        let label = if lang.is_empty() || lang.width() + 4 > width - 2 { String::new() } else { format!(" {lang} ") };
        let mut top = RLine::styled("╭─".into(), bs);
        if !label.is_empty() { top.spans.push(Span::styled(label.clone(), Style::from(&t.muted()))); }
        top.spans.push(Span::styled(format!("{}╮", "─".repeat(width - 3 - label.width())), bs));
        out.push(top);
        for l in text.split('\n') {
            let mut chunks: Vec<(String, usize)> = vec![(String::new(), 0)];
            for ch in l.chars() {
                let c = ch.width().unwrap_or(0);
                if chunks.last().is_some_and(|(_, w)| w + c > inner) { chunks.push((String::new(), 0)); }
                let last = chunks.last_mut().expect("one chunk");
                last.0.push(ch);
                last.1 += c;
            }
            for (s, w) in chunks {
                out.push(RLine { spans: vec![Span::styled("│ ", bs), Span::styled(s, ts), pad(inner - w), Span::styled(" │", bs)], links: Vec::new() });
            }
        }
        out.push(RLine::styled(format!("╰{}╯", "─".repeat(width - 2)), bs));
        out
    }

    fn list(&mut self, start: Option<u64>, loose: bool, items: &[Item], width: usize, base: &Gloss) -> Vec<RLine> {
        let t = self.t;
        let ms = Style::from(&t.list_marker());
        let ts = Style::from(&t.task());
        let numw = start.map(|s| format!("{}.", s + items.len().saturating_sub(1) as u64).len()).unwrap_or(0);
        let bullet = ["•", "◦", "▪"][self.depth % 3];
        let mut out = Vec::new();
        self.depth += 1;
        for (i, it) in items.iter().enumerate() {
            if loose && i > 0 { out.push(RLine::default()); }
            let mut marker: Vec<Span<'static>> = Vec::new();
            match start {
                Some(s) => marker.push(Span::styled(format!("{:>numw$} ", format!("{}.", s + i as u64)), ms)),
                None if it.task.is_none() => marker.push(Span::styled(format!("{bullet} "), ms)),
                None => {}
            }
            if let Some(done) = it.task { marker.push(Span::styled(if done { "☑ " } else { "☐ " }, ts)); }
            let mw: usize = marker.iter().map(|s| s.content.width()).sum();
            let mut body = self.blocks(&it.blocks, width.saturating_sub(mw).max(1), base, !loose);
            if body.is_empty() { body.push(RLine::default()); }
            let indent = [pad(mw)];
            for (k, l) in body.into_iter().enumerate() {
                out.push(if k == 0 { l.prefixed(&marker) } else if l.spans.is_empty() { l } else { l.prefixed(&indent) });
            }
        }
        self.depth -= 1;
        out
    }

    fn table(&self, td: &TableData, width: usize) -> Vec<RLine> {
        let t = self.t;
        let bs = Style::from(&t.table_border());
        let ncols = td.rows.iter().map(Vec::len).chain(std::iter::once(td.head.len())).max().unwrap_or(0);
        if ncols == 0 { return Vec::new(); }
        fn cell(r: &[Vec<Inl>], c: usize) -> &[Inl] { r.get(c).map(Vec::as_slice).unwrap_or(&[]) }
        let mut w: Vec<usize> = (0..ncols).map(|c| {
            std::iter::once(&td.head).chain(td.rows.iter()).map(|r| inl_width(cell(r, c))).max().unwrap_or(0).max(1)
        }).collect();
        let avail = width.saturating_sub(3 * ncols + 1).max(ncols);
        while w.iter().sum::<usize>() > avail {
            let (c, _) = w.iter().enumerate().max_by_key(|(_, x)| **x).expect("columns");
            if w[c] <= 1 { break; }
            w[c] -= 1;
        }
        let rule = |l: &str, m: &str, r: &str| RLine::styled(format!("{l}{}{r}", w.iter().map(|x| "─".repeat(x + 2)).collect::<Vec<_>>().join(m)), bs);
        let row_lines = |r: &Vec<Vec<Inl>>, base: &Gloss| -> Vec<RLine> {
            let cells: Vec<Vec<RLine>> = (0..ncols).map(|c| self.wrap(cell(r, c), w[c], base)).collect();
            let h = cells.iter().map(Vec::len).max().unwrap_or(1);
            (0..h).map(|y| {
                let mut line = RLine::styled("│".into(), bs);
                let mut col = 1;
                for c in 0..ncols {
                    let content = cells[c].get(y).cloned().unwrap_or_default();
                    let cw = content.width().min(w[c]);
                    let left = match td.aligns.get(c) { Some(Alignment::Right) => w[c] - cw, Some(Alignment::Center) => (w[c] - cw) / 2, _ => 0 };
                    line.spans.push(pad(1 + left));
                    let shifted = content.prefixed(&[]);
                    for (lc, lt, lu) in shifted.links { line.links.push((col + 1 + left + lc, lt, lu)); }
                    line.spans.extend(shifted.spans);
                    line.spans.push(pad(w[c] - cw - left + 1));
                    line.spans.push(Span::styled("│", bs));
                    col += w[c] + 3;
                }
                line
            }).collect()
        };
        let mut out = vec![rule("┌", "┬", "┐")];
        if !td.head.is_empty() {
            out.extend(row_lines(&td.head, &t.table_header()));
            out.push(rule("├", "┼", "┤"));
        }
        for r in &td.rows { out.extend(row_lines(r, &t.plain())); }
        out.push(rule("└", "┴", "┘"));
        out
    }
}

// ---------------------------------------------------------------- output

/// A link drawn at `line`, starting at display column `col`.
#[derive(Clone, Debug, PartialEq)]
pub struct MdLink {
    /// Line index in [`Rendered::lines`].
    pub line: usize,
    /// Display column where the link text starts.
    pub col: usize,
    /// The link text as shown on that line.
    pub text: String,
    /// Its target.
    pub url: String,
}

/// Rendered Markdown: the lines and where their links are (for OSC 8 hyperlinks).
#[derive(Clone, Debug, Default)]
pub struct Rendered {
    /// Styled, wrapped lines.
    pub lines: Vec<Line<'static>>,
    /// Links on those lines.
    pub links: Vec<MdLink>,
}

/// Render `text` as Markdown wrapped to `width` columns, with link positions.
pub fn render_doc(text: &str, width: usize, t: &Theme) -> Rendered {
    let width = width.max(8);
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    let src = normalize(text);
    let mut b = Builder::new();
    for ev in Parser::new_ext(&src, opts) { b.event(ev); }
    let (root, notes) = b.finish();
    let mut lay = Layout { t, depth: 0 };
    let mut out = lay.blocks(&root, width, &t.plain(), false);
    if !notes.is_empty() {
        let fs = Style::from(&t.footnote());
        out.push(RLine::default());
        out.push(RLine::styled("─".repeat(width.min(24)), fs));
        let mw = notes.iter().map(|(n, _)| format!("[{n}] ").len()).max().unwrap_or(4);
        for (n, bs) in &notes {
            let body = lay.blocks(bs, width.saturating_sub(mw).max(1), &t.footnote(), true);
            let marker = [Span::styled(format!("{:<mw$}", format!("[{n}]")), fs)];
            let indent = [pad(mw)];
            for (k, l) in body.into_iter().enumerate() { out.push(if k == 0 { l.prefixed(&marker) } else { l.prefixed(&indent) }); }
        }
    }
    while out.first().is_some_and(|l| l.width() == 0) { out.remove(0); }
    while out.last().is_some_and(|l| l.width() == 0) { out.pop(); }
    let mut r = Rendered::default();
    for (i, l) in out.into_iter().enumerate() {
        for (col, text, url) in l.links { r.links.push(MdLink { line: i, col, text, url }); }
        r.lines.push(Line::from(l.spans));
    }
    r
}

/// Render `text` as Markdown wrapped to `width` columns.
pub fn render(text: &str, width: usize, t: &Theme) -> Vec<Line<'static>> { render_doc(text, width, t).lines }

/// Plain text of rendered lines (tests, card snippets).
pub fn plain(lines: &[Line]) -> Vec<String> {
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect()
}
