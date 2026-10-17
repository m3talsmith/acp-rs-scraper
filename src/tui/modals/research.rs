//! Research modal: pick a project (or create one), then add the chosen thing
//! to it (`r` on a post, the Research menu's Add buttons). In browse mode
//! (Research › Projects…) it only picks or creates a project to show.

use super::*;
use crate::tui::commands::ResearchSource;
use crate::tui::widgets::text::TextField;
use crossterm::event::KeyCode;

/// What the modal is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchMode {
    /// Add one of `what` to the chosen project.
    Add,
    /// Choose a project to show (or create one).
    Browse,
}

/// A focusable part of the form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RField {
    /// What to add (‹ › choice).
    What,
    /// Note.
    Note,
    /// The project list.
    List,
    /// New project name.
    Name,
    /// New project description.
    Desc,
    /// The n-th button.
    Button(usize),
}

/// The modal.
#[derive(Clone, Debug, PartialEq)]
pub struct ResearchModal {
    /// Add or browse.
    pub mode: ResearchMode,
    /// Things that can be added: (label, source).
    pub what: Vec<(String, ResearchSource)>,
    /// Chosen `what`.
    pub what_idx: usize,
    /// Projects: (name, items, description).
    pub projects: Vec<(String, usize, String)>,
    /// Highlighted project.
    pub sel: usize,
    /// Note for the added item.
    pub note: TextField,
    /// New project name.
    pub name: TextField,
    /// New project description.
    pub desc: TextField,
    /// Focus.
    pub focus: RField,
    /// Validation error.
    pub error: Option<String>,
    /// Set on submit: (project, description when new, created).
    pub chosen: Option<(String, String, bool)>,
}

impl ResearchModal {
    /// An add modal over `projects`; `current` is preselected.
    pub fn add(what: Vec<(String, ResearchSource)>, projects: Vec<(String, usize, String)>, current: Option<&str>) -> Self {
        let sel = current.and_then(|c| projects.iter().position(|p| p.0.eq_ignore_ascii_case(c))).unwrap_or(0);
        let focus = if projects.is_empty() { RField::Name } else { RField::List };
        ResearchModal { mode: ResearchMode::Add, what, what_idx: 0, projects, sel, note: TextField::new(""), name: TextField::new(""),
            desc: TextField::new(""), focus, error: None, chosen: None }
    }
    /// A browse modal.
    pub fn browse(projects: Vec<(String, usize, String)>, current: Option<&str>) -> Self {
        ResearchModal { mode: ResearchMode::Browse, ..Self::add(Vec::new(), projects, current) }
    }
    /// Button labels.
    pub fn buttons(&self) -> Vec<String> {
        match self.mode { ResearchMode::Add => vec!["Add".into(), "Cancel".into()], ResearchMode::Browse => vec!["Open".into(), "Cancel".into()] }
    }
    /// Focus order.
    pub fn order(&self) -> Vec<RField> {
        let mut v = Vec::new();
        if self.mode == ResearchMode::Add { v.extend([RField::What, RField::Note]); }
        if !self.projects.is_empty() { v.push(RField::List); }
        v.extend([RField::Name, RField::Desc, RField::Button(0), RField::Button(1)]);
        v
    }
    /// The source to add.
    pub fn source(&self) -> Option<&ResearchSource> { self.what.get(self.what_idx).map(|w| &w.1) }
    fn step(&mut self, d: i32) {
        let o = self.order();
        let i = o.iter().position(|f| *f == self.focus).unwrap_or(0);
        self.focus = o[cycle(i, o.len(), d)];
    }
    /// Submit: a typed new name wins, else the highlighted project.
    pub fn submit(&mut self) -> Option<ModalResult> {
        let new = self.name.value.trim().to_string();
        if self.mode == ResearchMode::Add && self.what.is_empty() { self.error = Some("nothing to add here".into()); return None; }
        if !new.is_empty() {
            if self.projects.iter().any(|p| p.0.eq_ignore_ascii_case(&new)) {
                self.chosen = Some((self.projects.iter().find(|p| p.0.eq_ignore_ascii_case(&new)).map(|p| p.0.clone()).unwrap_or(new), String::new(), false));
            } else {
                self.chosen = Some((new, self.desc.value.trim().to_string(), true));
            }
            return Some(ModalResult::ClosedSubmitted);
        }
        match self.projects.get(self.sel) {
            Some(p) => { self.chosen = Some((p.0.clone(), String::new(), false)); Some(ModalResult::ClosedSubmitted) }
            None => { self.error = Some("type a name for the new project".into()); self.focus = RField::Name; None }
        }
    }
}

impl ModalState for ResearchModal {
    fn title(&self) -> String {
        match self.mode { ResearchMode::Add => "Add to research".into(), ResearchMode::Browse => "Research projects".into() }
    }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match (k.code, self.focus) {
            (KeyCode::Esc, _) => return Some(ModalResult::ClosedCancelled),
            (KeyCode::Tab, _) => self.step(1),
            (KeyCode::BackTab, _) => self.step(-1),
            (KeyCode::Enter, RField::Button(1)) => return Some(ModalResult::ClosedCancelled),
            (KeyCode::Enter, _) => return self.submit(),
            (KeyCode::Left | KeyCode::Right | KeyCode::Char(' '), RField::What) if !self.what.is_empty() => {
                self.what_idx = cycle(self.what_idx, self.what.len(), if k.code == KeyCode::Left { -1 } else { 1 });
            }
            (KeyCode::Up, RField::List) => self.sel = self.sel.saturating_sub(1),
            (KeyCode::Down, RField::List) => self.sel = (self.sel + 1).min(self.projects.len().saturating_sub(1)),
            (KeyCode::Up, _) => self.step(-1),
            (KeyCode::Down, _) => self.step(1),
            (KeyCode::Left, RField::Button(_)) => self.focus = RField::Button(0),
            (KeyCode::Right, RField::Button(_)) => self.focus = RField::Button(1),
            (_, RField::Note) => { self.note.on_key(k); self.error = None; }
            (_, RField::Name) => { self.name.on_key(k); self.error = None; }
            (_, RField::Desc) => { self.desc.on_key(k); self.error = None; }
            _ => {}
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Item(i) => {
                if self.focus == RField::List && self.sel == i { return self.submit(); }
                self.sel = i;
                self.focus = RField::List;
                None
            }
            ModalHit::Field(0) => { self.focus = RField::What; if !self.what.is_empty() { self.what_idx = cycle(self.what_idx, self.what.len(), 1); } None }
            ModalHit::Field(1) => { self.focus = RField::Note; None }
            ModalHit::Field(2) => { self.focus = RField::Name; None }
            ModalHit::Field(_) => { self.focus = RField::Desc; None }
            ModalHit::Button(0) => self.submit(),
            ModalHit::Button(_) => Some(ModalResult::ClosedCancelled),
        }
    }
    fn on_scroll(&mut self, d: i32) {
        self.sel = (self.sel as i64 + d as i64).clamp(0, self.projects.len().saturating_sub(1) as i64) as usize;
    }
}
