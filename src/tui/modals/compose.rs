//! Compose a new post or a reply. The default button is Cancel.
//!
//! Publish closes the modal only when the body is acceptable. The caller then
//! hits the capture gate and does not open a connection. `@` and `#` complete
//! from local nicks and tags.

use super::*;
use crate::site::{Draft, NickHint, completions, prepare};
use crossterm::event::KeyCode;

/// Which field or button is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposeFocus {
    /// New-post title. A reply skips this.
    Title,
    /// Markdown body.
    Body,
    /// The completion list.
    Complete,
    /// A local file path. Enter adds it. Nothing is uploaded.
    Attach,
    /// Which images or videos go to Red Star. Other types are shown and cannot be checked.
    SitePick,
    /// Default. Enter closes and sends nothing.
    Cancel,
    /// Enter asks the caller to publish. The send stays gated.
    Publish,
}

/// New post or reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeKind {
    /// `/rs/post/new`.
    NewPost,
    /// Reply in `thread`. `in_reply_to` is set when the target is a comment.
    Reply {
        /// Thread root.
        thread: String,
        /// Comment, when this is not a reply to the root.
        in_reply_to: Option<String>,
    },
}

/// The compose modal.
#[derive(Debug, Clone)]
pub struct ComposeModal {
    /// New post or reply.
    pub kind: ComposeKind,
    /// Title. Unused for a reply.
    pub title: String,
    /// Markdown.
    pub body: String,
    /// Selected text, already chosen before the modal opened.
    pub quote: Option<String>,
    /// Post the quote refers to.
    pub quote_post: Option<String>,
    /// Focus. Starts on Cancel.
    pub focus: ComposeFocus,
    /// `@…` or `#…` being completed, including the sigil.
    pub pending: Option<String>,
    /// Highlighted completion.
    pub complete_at: usize,
    /// Why Publish did not close.
    pub error: Option<String>,
    /// Local nicks.
    pub nicks: Vec<NickHint>,
    /// Local tags, without `#`.
    pub tags: Vec<String>,
    /// Path being typed on the attach line.
    pub attach_path: String,
    /// Other real path segments when Tab found more than one.
    pub path_matches: Vec<String>,
    /// Highlighted path match.
    pub path_at: usize,
    /// Files already accepted. Not uploaded.
    pub attachments: Vec<std::path::PathBuf>,
    /// Checked files for the site copy. `None` until Publish opens the picker, and when the list is within the site cap.
    pub site_pick: Option<Vec<bool>>,
    /// Highlighted picker row.
    pub site_at: usize,
}

impl ComposeModal {
    /// A new post. Focus starts on Cancel.
    pub fn new_post(nicks: Vec<NickHint>, tags: Vec<String>) -> Self {
        Self::blank(ComposeKind::NewPost, nicks, tags)
    }

    /// A reply. `quote` is the selected text.
    pub fn reply(thread: &str, in_reply_to: Option<String>, quote: Option<String>, quote_post: Option<String>, nicks: Vec<NickHint>, tags: Vec<String>) -> Self {
        let mut m = Self::blank(ComposeKind::Reply { thread: thread.into(), in_reply_to }, nicks, tags);
        m.quote = quote.filter(|s| !s.trim().is_empty());
        m.quote_post = quote_post;
        m
    }

    fn blank(kind: ComposeKind, nicks: Vec<NickHint>, tags: Vec<String>) -> Self {
        ComposeModal {
            kind,
            title: String::new(),
            body: String::new(),
            quote: None,
            quote_post: None,
            focus: ComposeFocus::Cancel,
            pending: None,
            complete_at: 0,
            error: None,
            nicks,
            tags,
            attach_path: String::new(),
            path_matches: Vec::new(),
            path_at: 0,
            attachments: Vec::new(),
            site_pick: None,
            site_at: 0,
        }
    }

    /// The draft the safety layer checks. The clock is the caller's.
    pub fn draft(&self, now_unix: i64, last_write_unix: Option<i64>, writes_on: bool) -> Draft {
        let (thread, in_reply_to) = match &self.kind {
            ComposeKind::NewPost => (None, None),
            ComposeKind::Reply { thread, in_reply_to } => (Some(thread.clone()), in_reply_to.clone()),
        };
        Draft {
            title: self.title.clone(),
            body: self.body.clone(),
            thread,
            in_reply_to,
            quote: self.quote.clone(),
            quote_post: self.quote_post.clone(),
            attachments: self.attachments.clone(),
            site_attachments: self.site_pick.as_ref().map(|pick| pick.iter().enumerate().filter(|(_, on)| **on).map(|(i, _)| i).collect()),
            html: None,
            accept_bundle: false,
            stored_bundle: None,
            now_unix,
            last_write_unix,
            writes_on,
        }
    }

    fn matches(&self) -> Vec<crate::site::Completion> {
        match &self.pending {
            Some(q) => completions(q, &self.nicks, &self.tags),
            None => Vec::new(),
        }
    }

    fn order(&self) -> Vec<ComposeFocus> {
        let mut v = Vec::new();
        if matches!(self.kind, ComposeKind::NewPost) {
            v.push(ComposeFocus::Title);
        }
        v.push(ComposeFocus::Body);
        v.push(ComposeFocus::Attach);
        if self.site_pick.is_some() {
            v.push(ComposeFocus::SitePick);
        }
        if self.pending.is_some() {
            v.push(ComposeFocus::Complete);
        }
        v.push(ComposeFocus::Cancel);
        v.push(ComposeFocus::Publish);
        v
    }

    fn move_focus(&mut self, d: i32) {
        let order = self.order();
        let i = order.iter().position(|f| *f == self.focus).unwrap_or(0);
        self.focus = order[cycle(i, order.len(), d)];
    }

    fn field_mut(&mut self) -> Option<&mut String> {
        match self.focus {
            ComposeFocus::Title => Some(&mut self.title),
            ComposeFocus::Body => Some(&mut self.body),
            ComposeFocus::Attach => Some(&mut self.attach_path),
            ComposeFocus::Complete => self.pending.as_mut(),
            ComposeFocus::Cancel | ComposeFocus::Publish | ComposeFocus::SitePick => None,
        }
    }

    fn apply_completion(&mut self) {
        let Some(q) = self.pending.clone() else { return };
        let Some(row) = self.matches().get(self.complete_at).cloned() else { return };
        if let Some(body) = self.body.strip_suffix(&q) {
            self.body = format!("{body}{} ", row.label);
        }
        self.pending = None;
        self.complete_at = 0;
        self.focus = ComposeFocus::Body;
    }

    fn clear_path_matches(&mut self) {
        self.path_matches.clear();
        self.path_at = 0;
    }

    fn accept_path_match(&mut self) {
        if let Some(text) = self.path_matches.get(self.path_at).cloned() {
            self.attach_path = text;
        }
        self.clear_path_matches();
    }

    fn add_attachment(&mut self) {
        self.clear_path_matches();
        let raw = self.attach_path.trim();
        if raw.is_empty() {
            self.error = Some("type a file path, then Enter".into());
            return;
        }
        let planned = match crate::site::plan_attachment(std::path::Path::new(raw)) {
            Ok(planned) => planned,
            Err(message) => {
                self.error = Some(message);
                return;
            }
        };
        if self.attachments.iter().any(|p| p == &planned.path) {
            self.error = Some(format!("{} is already attached", planned.path.display()));
            return;
        }
        self.attachments.push(planned.path);
        self.attach_path.clear();
        self.error = None;
        self.note_attachments_changed();
    }

    fn eligible_site_count(&self) -> usize {
        self.attachments.iter().filter(|p| crate::config::site_accepts_path(p)).count()
    }

    fn note_attachments_changed(&mut self) {
        let n = self.attachments.len();
        let max = crate::config::SITE_MAX_ATTACHMENTS;
        if self.eligible_site_count() <= max {
            self.site_pick = None;
            if self.focus == ComposeFocus::SitePick {
                self.focus = ComposeFocus::Attach;
            }
            return;
        }
        if let Some(pick) = &mut self.site_pick {
            pick.resize(n, false);
            if self.site_at >= n {
                self.site_at = n - 1;
            }
        }
    }

    fn open_site_picker(&mut self) {
        let max = crate::config::SITE_MAX_ATTACHMENTS;
        let mut pick = vec![false; self.attachments.len()];
        let mut left = max;
        for (on, path) in pick.iter_mut().zip(self.attachments.iter()) {
            if left == 0 {
                break;
            }
            if crate::config::site_accepts_path(path) {
                *on = true;
                left -= 1;
            }
        }
        self.site_at = 0;
        self.site_pick = Some(pick);
        self.focus = ComposeFocus::SitePick;
    }

    fn toggle_site(&mut self) {
        let max = crate::config::SITE_MAX_ATTACHMENTS;
        let Some(pick) = self.site_pick.as_mut() else { return };
        if pick.is_empty() {
            return;
        }
        let i = self.site_at.min(pick.len() - 1);
        if pick[i] {
            pick[i] = false;
            self.error = None;
            return;
        }
        let name = self.attachments.get(i).and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if self.attachments.get(i).is_some_and(|p| !crate::config::site_accepts_path(p)) {
            self.error = Some(crate::config::site_attachment_refusal(&name));
            return;
        }
        let n = pick.iter().filter(|on| **on).count();
        if n >= max {
            self.error = Some(format!("Red Star allows {max}. Uncheck one first."));
            return;
        }
        pick[i] = true;
        self.error = None;
    }

    fn complete_attach(&mut self) {
        if !self.path_matches.is_empty() {
            let n = self.path_matches.len();
            self.path_at = cycle(self.path_at, n, 1);
            return;
        }
        if self.attach_path.trim().is_empty() {
            self.move_focus(1);
            return;
        }
        let home = crate::config::home().ok();
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let done = crate::site::complete_path(&self.attach_path, home.as_deref(), &cwd);
        if done.matches.is_empty() && done.text == self.attach_path {
            self.move_focus(1);
            return;
        }
        self.attach_path = done.text;
        self.path_matches = done.matches;
        self.path_at = 0;
    }

    fn try_publish(&mut self) -> Option<ModalResult> {
        if self.eligible_site_count() > crate::config::SITE_MAX_ATTACHMENTS && self.site_pick.is_none() {
            self.open_site_picker();
            return None;
        }
        if let Some(pick) = &self.site_pick {
            let n = pick.iter().filter(|on| **on).count();
            let max = crate::config::SITE_MAX_ATTACHMENTS;
            if n > max {
                self.error = Some(format!("Red Star allows {max} attachments on the site copy"));
                self.focus = ComposeFocus::SitePick;
                return None;
            }
        }
        let draft = self.draft(0, None, true);
        if let Err(e) = prepare(&draft) {
            self.error = Some(e.message);
            self.focus = if self.site_pick.is_some() { ComposeFocus::SitePick } else { ComposeFocus::Body };
            return None;
        }
        Some(ModalResult::ClosedSubmitted)
    }
}

impl ModalState for ComposeModal {
    fn title(&self) -> String {
        match self.kind {
            ComposeKind::NewPost => "New post".into(),
            ComposeKind::Reply { .. } => "Reply".into(),
        }
    }

    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        self.error = None;
        match k.code {
            KeyCode::Esc if self.focus == ComposeFocus::Attach && !self.path_matches.is_empty() => {
                self.clear_path_matches();
                None
            }
            KeyCode::Esc if self.focus == ComposeFocus::SitePick => {
                self.site_pick = None;
                self.focus = ComposeFocus::Publish;
                None
            }
            KeyCode::Char('?') | KeyCode::F(1) if self.focus == ComposeFocus::SitePick => {
                Some(ModalResult::Help("site-attachments"))
            }
            KeyCode::Esc if self.pending.is_some() => {
                self.pending = None;
                self.focus = ComposeFocus::Body;
                None
            }
            KeyCode::Esc => Some(ModalResult::ClosedCancelled),
            KeyCode::Tab if self.focus == ComposeFocus::Attach => {
                self.complete_attach();
                None
            }
            KeyCode::Tab => {
                self.move_focus(1);
                None
            }
            KeyCode::BackTab => {
                self.clear_path_matches();
                self.move_focus(-1);
                None
            }
            KeyCode::Up | KeyCode::Down if self.focus == ComposeFocus::SitePick => {
                let n = self.site_pick.as_ref().map(|p| p.len()).unwrap_or(1).max(1);
                let d = if k.code == KeyCode::Up { -1 } else { 1 };
                self.site_at = cycle(self.site_at, n, d);
                None
            }
            KeyCode::Up | KeyCode::Down
                if self.focus == ComposeFocus::Attach && !self.path_matches.is_empty() =>
            {
                let d = if k.code == KeyCode::Up { -1 } else { 1 };
                self.path_at = cycle(self.path_at, self.path_matches.len(), d);
                None
            }
            KeyCode::Up if self.pending.is_some() => {
                let n = self.matches().len().max(1);
                self.complete_at = cycle(self.complete_at, n, -1);
                self.focus = ComposeFocus::Complete;
                None
            }
            KeyCode::Down if self.pending.is_some() => {
                let n = self.matches().len().max(1);
                self.complete_at = cycle(self.complete_at, n, 1);
                self.focus = ComposeFocus::Complete;
                None
            }
            KeyCode::Enter => match self.focus {
                ComposeFocus::Cancel => Some(ModalResult::ClosedCancelled),
                ComposeFocus::Publish | ComposeFocus::SitePick => self.try_publish(),
                ComposeFocus::Complete => {
                    self.apply_completion();
                    None
                }
                ComposeFocus::Body => {
                    if self.pending.is_some() {
                        self.apply_completion();
                    } else {
                        self.body.push('\n');
                    }
                    None
                }
                ComposeFocus::Title => {
                    self.focus = ComposeFocus::Body;
                    None
                }
                ComposeFocus::Attach => {
                    if self.path_matches.is_empty() {
                        self.add_attachment();
                    } else {
                        self.accept_path_match();
                    }
                    None
                }
            },
            KeyCode::Backspace => {
                if self.focus == ComposeFocus::Complete {
                    self.focus = ComposeFocus::Body;
                }
                if self.focus == ComposeFocus::Attach {
                    self.clear_path_matches();
                }
                if self.focus == ComposeFocus::Attach && self.attach_path.is_empty() {
                    self.attachments.pop();
                    self.note_attachments_changed();
                } else if let Some(field) = self.field_mut() {
                    field.pop();
                }
                if self.focus == ComposeFocus::Body && self.pending.as_ref().is_some_and(|q| q.len() <= 1) {
                    self.pending = None;
                } else if self.focus == ComposeFocus::Body
                    && let Some(q) = self.pending.as_mut()
                {
                    q.pop();
                }
                None
            }
            KeyCode::Char(c) => {
                if self.focus == ComposeFocus::SitePick && c == ' ' {
                    self.toggle_site();
                } else if self.focus == ComposeFocus::Attach {
                    self.clear_path_matches();
                    self.attach_path.push(c);
                } else if self.focus != ComposeFocus::Cancel && self.focus != ComposeFocus::Publish && self.focus != ComposeFocus::SitePick {
                    if self.focus == ComposeFocus::Complete {
                        self.focus = ComposeFocus::Body;
                    }
                    let boundary = self.body.chars().next_back().is_none_or(char::is_whitespace);
                    let starting = self.focus == ComposeFocus::Body && (c == '@' || c == '#') && boundary;
                    if let Some(field) = self.field_mut() {
                        field.push(c);
                    }
                    if starting {
                        self.pending = Some(c.to_string());
                        self.complete_at = 0;
                    } else if self.focus == ComposeFocus::Body
                        && let Some(q) = self.pending.as_mut()
                        && (c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    {
                        q.push(c);
                        self.complete_at = 0;
                    } else if self.focus == ComposeFocus::Body && self.pending.is_some() && c.is_whitespace() {
                        self.pending = None;
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Button(0) => Some(ModalResult::ClosedCancelled),
            ModalHit::Button(1) => self.try_publish(),
            ModalHit::Field(0) if matches!(self.kind, ComposeKind::NewPost) => {
                self.focus = ComposeFocus::Title;
                None
            }
            ModalHit::Field(2) => {
                self.focus = ComposeFocus::Attach;
                None
            }
            ModalHit::Field(_) => {
                self.focus = ComposeFocus::Body;
                None
            }
            ModalHit::Button(_) => None,
            ModalHit::Item(i) if self.focus == ComposeFocus::SitePick => {
                self.site_at = i;
                self.toggle_site();
                None
            }
            ModalHit::Item(i) if self.focus == ComposeFocus::Attach && !self.path_matches.is_empty() => {
                self.path_at = i;
                self.accept_path_match();
                None
            }
            ModalHit::Item(i) => {
                self.complete_at = i;
                self.focus = ComposeFocus::Complete;
                self.apply_completion();
                None
            }
        }
    }
}
