//! Markdown to the site editor's ProseMirror document.
//!
//! The schema is the one recorded for 3.4.0: paragraph, hardBreak, blockquote,
//! bulletList, orderedList, listItem, taskList, taskItem, and the marks bold,
//! italic, underline, and link. Inline nodes are mention (`@nick`, `#tag`) and
//! postRef. A construct the schema cannot express is rejected before a write
//! is prepared.

use crate::prelude::*;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Site limit on a post body (characters).
pub const BODY_CHAR_LIMIT: usize = 40_000;

/// Why a body cannot become a ProseMirror document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmError {
    /// What to show the user.
    pub message: String,
}

impl std::fmt::Display for PmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PmError {}

fn err(message: impl Into<String>) -> PmError {
    PmError { message: message.into() }
}

/// A `postRef` node. `id` on a mention is a different field; this one is the post.
pub fn post_ref(post_id: &str, thread_id: &str) -> Value {
    json!({"type": "postRef", "attrs": {"postId": post_id, "threadId": thread_id}})
}

/// Compact JSON in the editor's key order: `type`, `attrs`, `content`, `text`,
/// `marks`, then `postId` and `threadId`, then any other key alphabetically.
///
/// `serde_json` would sort every key. The captured `form[body_input]` does not.
pub fn editor_json(v: &Value) -> String {
    let mut out = String::new();
    write_editor(v, &mut out);
    out
}

fn write_editor(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_editor(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            let mut keys: Vec<&String> = map.keys().collect();
            const PREFERRED: &[&str] = &["type", "attrs", "content", "text", "marks", "postId", "threadId"];
            keys.sort_by(|a, b| {
                let rank = |k: &str| PREFERRED.iter().position(|p| *p == k).unwrap_or(PREFERRED.len());
                rank(a).cmp(&rank(b)).then(a.cmp(b))
            });
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(*k).unwrap_or_default());
                out.push(':');
                write_editor(&map[*k], out);
            }
            out.push('}');
        }
    }
}

/// Turn `markdown` into a `{type: doc}` document.
pub fn markdown_to_doc(markdown: &str) -> Result<Value, PmError> {
    check_len(markdown)?;
    if markdown.trim().is_empty() {
        return Err(err("the body is empty"));
    }
    parse_blocks(markdown)
}

/// Quote text is the selection, not a second Markdown document: lines stay
/// paragraphs, and a leading `#` does not become a heading.
pub fn plain_blocks(text: &str) -> Result<Vec<Value>, PmError> {
    check_len(text)?;
    if text.trim().is_empty() {
        return Err(err("the quote is empty"));
    }
    let mut blocks = Vec::new();
    for para in text.split("\n\n") {
        if para.trim().is_empty() {
            continue;
        }
        let mut content = Vec::new();
        for (i, line) in para.split('\n').enumerate() {
            if i > 0 {
                content.push(hard_break());
            }
            content.extend(text_nodes(line, &[]));
        }
        if !content.is_empty() {
            blocks.push(json!({"type": "paragraph", "content": content}));
        }
    }
    if blocks.is_empty() {
        return Err(err("the quote is empty"));
    }
    Ok(blocks)
}

/// Body plus an optional quote and a reply reference.
///
/// `post_ref_ids` is `(post id, thread id)`. The reference is the first node,
/// then the blockquote, then the body. A quote with no extra body is allowed.
pub fn assemble(body: &str, quote: Option<&str>, post_ref_ids: Option<(&str, &str)>) -> Result<Value, PmError> {
    let mut budget = body.chars().count();
    if let Some(q) = quote {
        budget += q.chars().count();
    }
    if budget > BODY_CHAR_LIMIT {
        return Err(err(format!("the body is {budget} characters; the site limit is {BODY_CHAR_LIMIT}")));
    }
    let mut content = Vec::new();
    if let Some((post, thread)) = post_ref_ids {
        content.push(post_ref(post, thread));
    }
    if let Some(q) = quote.filter(|q| !q.trim().is_empty()) {
        content.push(json!({"type": "blockquote", "content": plain_blocks(q)?}));
    }
    if !body.trim().is_empty() {
        let doc = parse_blocks(body)?;
        if let Some(blocks) = doc.get("content").and_then(Value::as_array) {
            content.extend(blocks.clone());
        }
    }
    if content.is_empty() || content.iter().all(|n| n.get("type").and_then(Value::as_str) == Some("postRef")) {
        return Err(err("the body is empty"));
    }
    Ok(json!({"type": "doc", "content": content}))
}

fn check_len(text: &str) -> Result<(), PmError> {
    let chars = text.chars().count();
    if chars > BODY_CHAR_LIMIT {
        return Err(err(format!("the body is {chars} characters; the site limit is {BODY_CHAR_LIMIT}")));
    }
    Ok(())
}

fn parse_blocks(markdown: &str) -> Result<Value, PmError> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_TASKLISTS);
    let mut b = Builder::new();
    for ev in Parser::new_ext(markdown, opts) {
        b.on(ev)?;
    }
    b.finish()
}

fn hard_break() -> Value {
    json!({"type": "hardBreak"})
}

fn text_node(text: String, marks: &[Value]) -> Value {
    let mut n = json!({"type": "text", "text": text});
    if !marks.is_empty() {
        n["marks"] = Value::Array(marks.to_vec());
    }
    n
}

fn mention_node(label: &str, sig: char) -> Value {
    let mut attrs = json!({"id": "", "label": label});
    if sig == '#' {
        attrs["mentionSuggestionChar"] = json!("#");
    }
    json!({"type": "mention", "attrs": attrs})
}

/// Split `@nick` and `#tag` out of plain text. The site user id is left empty:
/// `mention-query` has not been captured. `#post-…` stays text.
fn text_nodes(text: &str, marks: &[Value]) -> Vec<Value> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut i = 0;
    while i < chars.len() {
        let boundary = i == 0 || chars[i - 1].is_whitespace();
        if boundary && (chars[i] == '@' || chars[i] == '#') {
            let sig = chars[i];
            let start = i + 1;
            if start < chars.len() && chars[start].is_ascii_alphabetic() {
                let mut j = start + 1;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_' || chars[j] == '-') {
                    j += 1;
                }
                let name: String = chars[start..j].iter().collect();
                let anchor = sig == '#' && name.to_ascii_lowercase().starts_with("post-");
                if !anchor {
                    if !buf.is_empty() {
                        out.push(text_node(std::mem::take(&mut buf), marks));
                    }
                    out.push(mention_node(&format!("{sig}{name}"), sig));
                    i = j;
                    continue;
                }
            }
        }
        buf.push(chars[i]);
        i += 1;
    }
    if !buf.is_empty() {
        out.push(text_node(buf, marks));
    }
    out
}

#[derive(Default)]
struct Item {
    task: Option<bool>,
    blocks: Vec<Value>,
}

enum Frame {
    Doc(Vec<Value>),
    Quote(Vec<Value>),
    List { start: Option<u64>, items: Vec<Item> },
    Item(Item),
}

struct Builder {
    stack: Vec<Frame>,
    inl: Vec<Value>,
    para: bool,
    implicit: bool,
    bold: u32,
    italic: u32,
    underline: u32,
    links: Vec<String>,
}

impl Builder {
    fn new() -> Self {
        Builder {
            stack: vec![Frame::Doc(Vec::new())],
            inl: Vec::new(),
            para: false,
            implicit: false,
            bold: 0,
            italic: 0,
            underline: 0,
            links: Vec::new(),
        }
    }

    fn marks(&self) -> Vec<Value> {
        let mut m = Vec::new();
        if self.bold > 0 {
            m.push(json!({"type": "bold"}));
        }
        if self.italic > 0 {
            m.push(json!({"type": "italic"}));
        }
        if self.underline > 0 {
            m.push(json!({"type": "underline"}));
        }
        if let Some(href) = self.links.last() {
            m.push(json!({"type": "link", "attrs": {"href": href}}));
        }
        m
    }

    fn on(&mut self, ev: Event<'_>) -> Result<(), PmError> {
        match ev {
            Event::Start(tag) => self.start(tag)?,
            Event::End(tag) => self.end(tag)?,
            Event::Text(s) => {
                let marks = self.marks();
                let nodes = text_nodes(&s, &marks);
                for n in nodes {
                    self.push_inline(n);
                }
            }
            Event::SoftBreak | Event::HardBreak => self.push_inline(hard_break()),
            Event::Code(_) => return Err(err("inline code cannot be sent; the editor schema has no code")),
            Event::Html(s) | Event::InlineHtml(s) => self.html(&s)?,
            Event::TaskListMarker(done) => self.mark_task(done),
            Event::Rule => return Err(err("a horizontal rule cannot be sent")),
            Event::FootnoteReference(_) => return Err(err("footnotes cannot be sent")),
            _ => {}
        }
        Ok(())
    }

    fn start(&mut self, tag: Tag<'_>) -> Result<(), PmError> {
        match tag {
            Tag::Paragraph => {
                self.close_implicit();
                self.para = true;
                self.implicit = false;
                self.inl.clear();
            }
            Tag::Heading { .. } => return Err(err("headings cannot be sent; the editor schema has no heading")),
            Tag::BlockQuote(_) => {
                self.close_implicit();
                self.stack.push(Frame::Quote(Vec::new()));
            }
            Tag::CodeBlock(_) => return Err(err("a code block cannot be sent; the editor schema has no code block")),
            Tag::List(start) => {
                self.close_implicit();
                self.stack.push(Frame::List { start, items: Vec::new() });
            }
            Tag::Item => {
                self.close_implicit();
                self.stack.push(Frame::Item(Item::default()));
            }
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => return Err(err("strikethrough cannot be sent; the editor schema has no strikethrough")),
            Tag::Link { dest_url, .. } => self.links.push(dest_url.to_string()),
            Tag::Image { .. } => return Err(err("an image in the markdown cannot be sent; attach the file on the attachment line")),
            Tag::Table(_) | Tag::TableHead | Tag::TableRow | Tag::TableCell => {
                return Err(err("tables cannot be sent"));
            }
            Tag::FootnoteDefinition(_) => return Err(err("footnotes cannot be sent")),
            Tag::HtmlBlock => return Err(err("html cannot be sent")),
            Tag::MetadataBlock(_) => return Err(err("metadata cannot be sent")),
            _ => return Err(err("that block cannot be sent")),
        }
        Ok(())
    }

    fn end(&mut self, tag: TagEnd) -> Result<(), PmError> {
        match tag {
            TagEnd::Paragraph => self.end_para(),
            TagEnd::BlockQuote(_) => self.end_quote(),
            TagEnd::List(_) => self.end_list()?,
            TagEnd::Item => self.end_item()?,
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Link => {
                self.links.pop();
            }
            TagEnd::Strikethrough | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Image | TagEnd::Table
            | TagEnd::TableHead | TagEnd::TableRow | TagEnd::TableCell | TagEnd::FootnoteDefinition
            | TagEnd::HtmlBlock | TagEnd::MetadataBlock(_) => {}
            _ => {}
        }
        Ok(())
    }

    fn html(&mut self, raw: &str) -> Result<(), PmError> {
        let t = raw.trim().to_ascii_lowercase();
        if t.starts_with("</u") {
            self.underline = self.underline.saturating_sub(1);
            return Ok(());
        }
        if t.starts_with("<u") {
            self.underline += 1;
            return Ok(());
        }
        if t.starts_with("<br") {
            self.push_inline(hard_break());
            return Ok(());
        }
        if t.is_empty() {
            return Ok(());
        }
        Err(err("html cannot be sent; the editor schema has no html"))
    }

    fn mark_task(&mut self, done: bool) {
        if let Some(Frame::Item(item)) = self.stack.last_mut() {
            item.task = Some(done);
        }
    }

    fn push_inline(&mut self, node: Value) {
        if !self.para {
            self.para = true;
            self.implicit = true;
            self.inl.clear();
        }
        self.inl.push(node);
    }

    fn close_implicit(&mut self) {
        if self.implicit {
            self.end_para();
        }
    }

    fn end_para(&mut self) {
        self.para = false;
        self.implicit = false;
        let inl = std::mem::take(&mut self.inl);
        if inl.is_empty() {
            return;
        }
        self.push_block(json!({"type": "paragraph", "content": inl}));
    }

    fn end_quote(&mut self) {
        self.close_implicit();
        let Some(Frame::Quote(blocks)) = self.stack.pop() else { return };
        self.push_block(json!({"type": "blockquote", "content": blocks}));
    }

    fn end_item(&mut self) -> Result<(), PmError> {
        self.close_implicit();
        let Some(Frame::Item(item)) = self.stack.pop() else {
            return Err(err("a list item ended outside a list"));
        };
        let Some(Frame::List { items, .. }) = self.stack.last_mut() else {
            return Err(err("a list item ended outside a list"));
        };
        items.push(item);
        Ok(())
    }

    fn end_list(&mut self) -> Result<(), PmError> {
        self.close_implicit();
        let Some(Frame::List { start, items }) = self.stack.pop() else {
            return Err(err("a list ended outside a list"));
        };
        let tasks = items.iter().any(|i| i.task.is_some());
        let item_nodes: Vec<Value> = items
            .into_iter()
            .map(|item| {
                if tasks {
                    json!({"type": "taskItem", "attrs": {"checked": item.task.unwrap_or(false)}, "content": item.blocks})
                } else {
                    json!({"type": "listItem", "content": item.blocks})
                }
            })
            .collect();
        let node = if tasks {
            json!({"type": "taskList", "content": item_nodes})
        } else if let Some(order) = start {
            json!({"type": "orderedList", "attrs": {"order": order}, "content": item_nodes})
        } else {
            json!({"type": "bulletList", "content": item_nodes})
        };
        self.push_block(node);
        Ok(())
    }

    fn push_block(&mut self, node: Value) {
        match self.stack.last_mut() {
            Some(Frame::Doc(blocks) | Frame::Quote(blocks)) => blocks.push(node),
            Some(Frame::Item(item)) => item.blocks.push(node),
            Some(Frame::List { .. }) | None => {}
        }
    }

    fn finish(mut self) -> Result<Value, PmError> {
        self.close_implicit();
        if self.stack.len() != 1 {
            return Err(err("the body ended inside an open block"));
        }
        let Some(Frame::Doc(blocks)) = self.stack.pop() else {
            return Err(err("the body ended inside an open block"));
        };
        if blocks.is_empty() {
            return Err(err("the body is empty"));
        }
        Ok(json!({"type": "doc", "content": blocks}))
    }
}
