//! Feed editor and add-to-feed. Both are local. Refresh and delete confirm outside this modal.

use super::*;
use crate::feeds::FeedSummary;
use crate::tui::widgets::text::TextField;
use crossterm::event::KeyCode;

/// Editor, or add the current nick, tag, or search to a feed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedMode {
    /// Name, description, filters, and one new rule.
    Edit,
    /// Pick a feed, or type a new name, and add one source.
    Add,
}

/// What the editor submits. The app turns it into a command or a local view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedSubmit {
    /// Create or update the description and filters.
    Save,
    /// Add the rule line.
    AddRule,
    /// Open the named feed.
    Show,
    /// Add [`FeedModal::source`] to the chosen feed.
    AddTo,
}

/// One thing to add: a nick, a tag, or a search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedSource {
    /// What the row says.
    pub label: String,
    /// `nick`, `tag`, or `search`.
    pub kind: String,
    /// The nick, tag, or query.
    pub value: String,
}

/// Which control has the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedFocus {
    /// Feed list.
    List,
    /// Name.
    Name,
    /// Description.
    Description,
    /// any or all.
    Match,
    /// newest, oldest, or score.
    Sort,
    /// Since.
    Since,
    /// nick, tag, or search.
    Kind,
    /// The rule value.
    Value,
    /// Include or exclude.
    Exclude,
    /// A button. The last one is Cancel.
    Button(usize),
}

const MATCHES: &[&str] = &["any", "all"];
const SORTS: &[&str] = &["newest", "oldest", "score"];
const KINDS: &[&str] = &["nick", "tag", "search"];

/// The feed editor, or add-to-feed.
#[derive(Debug, Clone)]
pub struct FeedModal {
    /// Which modal.
    pub mode: FeedMode,
    /// Saved feeds.
    pub feeds: Vec<FeedSummary>,
    /// Selected row.
    pub sel: usize,
    /// Name.
    pub name: TextField,
    /// Description.
    pub description: TextField,
    /// Match index.
    pub match_mode: usize,
    /// Sort index.
    pub sort: usize,
    /// Since.
    pub since: TextField,
    /// Rule kind index.
    pub kind: usize,
    /// Rule value.
    pub value: TextField,
    /// Exclude this rule.
    pub exclude: bool,
    /// What Add puts on the feed. The editor leaves this empty.
    pub source: Option<FeedSource>,
    /// Focus.
    pub focus: FeedFocus,
    /// Shown when a button was refused.
    pub error: Option<String>,
    /// Set when the modal closes with Submitted.
    pub submit: Option<FeedSubmit>,
}

impl FeedModal {
    /// Editor. `current` selects that feed when it is in the list.
    pub fn edit(feeds: Vec<FeedSummary>, current: Option<&str>) -> Self {
        let mut m = Self::blank(FeedMode::Edit, feeds);
        if let Some(name) = current {
            if let Some(i) = m.feeds.iter().position(|f| f.name.eq_ignore_ascii_case(name)) {
                m.sel = i;
                m.fill_from_sel();
            } else {
                m.name.set(name);
            }
        }
        m.focus = FeedFocus::Name;
        m
    }

    /// Add `source` to a feed. Cancel is focused.
    pub fn add(feeds: Vec<FeedSummary>, source: Option<FeedSource>) -> Self {
        let mut m = Self::blank(FeedMode::Add, feeds);
        if let Some(src) = &source {
            m.kind = KINDS.iter().position(|k| *k == src.kind).unwrap_or(0);
            m.value.set(&src.value);
        }
        m.source = source;
        m.focus = FeedFocus::Button(1);
        m
    }

    fn blank(mode: FeedMode, feeds: Vec<FeedSummary>) -> Self {
        FeedModal {
            mode, feeds, sel: 0, name: TextField::default(), description: TextField::default(),
            match_mode: 0, sort: 0, since: TextField::default(), kind: 0, value: TextField::default(),
            exclude: false, source: None, focus: FeedFocus::List, error: None, submit: None,
        }
    }

    fn fill_from_sel(&mut self) {
        if let Some(f) = self.feeds.get(self.sel) {
            self.name.set(&f.name);
            self.description.set(&f.description);
        }
    }

    /// Button labels. The last one cancels.
    pub fn buttons(&self) -> Vec<&'static str> {
        match self.mode {
            FeedMode::Edit => vec!["Show", "Save", "Add rule", "Cancel"],
            FeedMode::Add => vec!["Add", "Cancel"],
        }
    }

    /// `any` or `all`.
    pub fn match_mode(&self) -> &'static str { MATCHES[self.match_mode % MATCHES.len()] }
    /// `newest`, `oldest`, or `score`.
    pub fn sort_mode(&self) -> &'static str { SORTS[self.sort % SORTS.len()] }
    /// `nick`, `tag`, or `search`.
    pub fn kind_mode(&self) -> &'static str { KINDS[self.kind % KINDS.len()] }

    fn fields(&self) -> Vec<FeedFocus> {
        match self.mode {
            FeedMode::Edit => vec![FeedFocus::List, FeedFocus::Name, FeedFocus::Description, FeedFocus::Match, FeedFocus::Sort, FeedFocus::Since, FeedFocus::Kind, FeedFocus::Value, FeedFocus::Exclude],
            FeedMode::Add => vec![FeedFocus::List, FeedFocus::Name, FeedFocus::Kind, FeedFocus::Value],
        }
    }

    fn cycle_focus(&mut self, d: i32) {
        let fields = self.fields();
        let n = fields.len() + self.buttons().len();
        let cur = match self.focus {
            FeedFocus::Button(i) => fields.len() + i,
            other => fields.iter().position(|f| *f == other).unwrap_or(0),
        };
        let next = cycle(cur, n, d);
        self.focus = if next < fields.len() { fields[next] } else { FeedFocus::Button(next - fields.len()) };
    }

    fn press(&mut self, i: usize) -> Option<ModalResult> {
        let labels = self.buttons();
        if i + 1 >= labels.len() { return Some(ModalResult::ClosedCancelled); }
        self.error = None;
        let name = self.name.value.trim();
        if name.is_empty() && !matches!(self.focus, FeedFocus::Button(_)) {
            // A selected row still names the feed.
        }
        let named = if name.is_empty() { self.feeds.get(self.sel).map(|f| f.name.clone()).unwrap_or_default() } else { name.to_string() };
        if named.trim().is_empty() {
            self.error = Some("type a feed name, or pick one".into());
            return None;
        }
        if self.name.value.trim().is_empty() { self.name.set(&named); }
        self.submit = Some(match self.mode {
            FeedMode::Edit => match i {
                0 => FeedSubmit::Show,
                1 => FeedSubmit::Save,
                _ => FeedSubmit::AddRule,
            },
            FeedMode::Add => FeedSubmit::AddTo,
        });
        if self.submit == Some(FeedSubmit::AddRule) && self.value.value.trim().is_empty() {
            self.submit = None;
            self.error = Some("type a nick, a tag, or a search".into());
            return None;
        }
        if self.submit == Some(FeedSubmit::AddTo) && self.value.value.trim().is_empty() {
            self.submit = None;
            self.error = Some("nothing to add".into());
            return None;
        }
        Some(ModalResult::ClosedSubmitted)
    }
}

impl ModalState for FeedModal {
    fn title(&self) -> String {
        match self.mode {
            FeedMode::Edit => "Feed".into(),
            FeedMode::Add => "Add to feed".into(),
        }
    }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        let text = matches!(self.focus, FeedFocus::Name | FeedFocus::Description | FeedFocus::Since | FeedFocus::Value);
        match k.code {
            KeyCode::Esc => return Some(ModalResult::ClosedCancelled),
            KeyCode::Tab => self.cycle_focus(1),
            KeyCode::BackTab => self.cycle_focus(-1),
            KeyCode::Enter => {
                if let FeedFocus::Button(i) = self.focus { return self.press(i); }
                if self.focus == FeedFocus::List { self.fill_from_sel(); }
            }
            KeyCode::Up | KeyCode::Char('k') if self.focus == FeedFocus::List => {
                self.sel = self.sel.saturating_sub(1);
                self.fill_from_sel();
            }
            KeyCode::Down | KeyCode::Char('j') if self.focus == FeedFocus::List => {
                if self.sel + 1 < self.feeds.len() { self.sel += 1; self.fill_from_sel(); }
            }
            KeyCode::Left | KeyCode::Right if self.focus == FeedFocus::Match => {
                self.match_mode = cycle(self.match_mode, MATCHES.len(), if k.code == KeyCode::Left { -1 } else { 1 });
            }
            KeyCode::Left | KeyCode::Right if self.focus == FeedFocus::Sort => {
                self.sort = cycle(self.sort, SORTS.len(), if k.code == KeyCode::Left { -1 } else { 1 });
            }
            KeyCode::Left | KeyCode::Right if self.focus == FeedFocus::Kind => {
                self.kind = cycle(self.kind, KINDS.len(), if k.code == KeyCode::Left { -1 } else { 1 });
            }
            KeyCode::Char(' ') if self.focus == FeedFocus::Exclude => self.exclude = !self.exclude,
            KeyCode::Left | KeyCode::Right if matches!(self.focus, FeedFocus::Button(_)) => self.cycle_focus(if k.code == KeyCode::Left { -1 } else { 1 }),
            _ if text => {
                if let FeedFocus::Name = self.focus { self.name.on_key(k); }
                if let FeedFocus::Description = self.focus { self.description.on_key(k); }
                if let FeedFocus::Since = self.focus { self.since.on_key(k); }
                if let FeedFocus::Value = self.focus { self.value.on_key(k); }
            }
            _ => {}
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Button(i) => self.press(i),
            ModalHit::Item(i) if i < self.feeds.len() => { self.sel = i; self.focus = FeedFocus::List; self.fill_from_sel(); None }
            ModalHit::Field(i) => {
                let fields = self.fields();
                if let Some(f) = fields.get(i) { self.focus = *f; }
                None
            }
            _ => None,
        }
    }
}
