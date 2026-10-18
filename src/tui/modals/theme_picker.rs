//! The theme picker (F5, or the header's `‹theme› ▾`): every theme file (or, when
//! there are none, "Download the default theme" and "Create starter theme"), then
//! "Edit themes…". Moving the highlight previews the theme; Esc puts the old one back.

use super::*;
use crate::themes::Entry;
use crate::tui::theme::Theme;
use crossterm::event::KeyCode;

/// What the picker chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerChoice {
    /// Use this theme.
    Theme(String),
    /// Open the theme editor.
    Edit,
    /// Download the default theme (offered when there are no themes).
    Download,
    /// Write the minimal starter theme and use it (offered when there are no themes).
    Starter,
    /// A theme that can't be used (or nothing).
    None,
}

/// Picker state.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemePickerModal {
    /// Theme files.
    pub entries: Vec<Entry>,
    /// Highlighted row (`entries.len()` is "Edit themes…").
    pub cursor: usize,
    /// The theme in use when it opened (put back on Esc).
    pub restore: Theme,
    /// Its name.
    pub current: String,
    /// Rows after the themes, before "Edit themes…" (Download / Starter when there are no themes).
    pub extras: Vec<PickerChoice>,
}

impl ThemePickerModal {
    /// Picker over `entries`, highlighting `current`.
    pub fn new(entries: Vec<Entry>, current: &str, restore: Theme) -> Self {
        let cursor = entries.iter().position(|e| e.name == current).unwrap_or(0);
        let extras = if entries.is_empty() { vec![PickerChoice::Download, PickerChoice::Starter] } else { Vec::new() };
        ThemePickerModal { entries, cursor, restore, current: current.into(), extras }
    }
    /// Rows: the themes, the extras and "Edit themes…".
    pub fn rows(&self) -> usize { self.entries.len() + self.extras.len() + 1 }
    /// Row `i` when it is past the themes: an extra or Edit.
    pub fn action_row(&self, i: usize) -> Option<PickerChoice> {
        let k = i.checked_sub(self.entries.len())?;
        self.extras.get(k).cloned().or((k == self.extras.len()).then_some(PickerChoice::Edit))
    }
    /// The highlighted theme, when it can be used.
    pub fn highlighted(&self) -> Option<&str> { self.entries.get(self.cursor).filter(|e| e.error.is_none()).map(|e| e.name.as_str()) }
    /// What Enter does.
    pub fn chosen(&self) -> PickerChoice {
        if let Some(c) = self.action_row(self.cursor) { return c; }
        self.highlighted().map(|n| PickerChoice::Theme(n.into())).unwrap_or(PickerChoice::None)
    }
}

impl ModalState for ThemePickerModal {
    fn title(&self) -> String { "Theme · ↑↓ preview · Enter uses · Esc keeps".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        let n = self.rows();
        match k.code {
            KeyCode::Esc | KeyCode::F(5) => return Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => return (self.chosen() != PickerChoice::None).then_some(ModalResult::ClosedSubmitted),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.cursor = cycle(self.cursor, n, 1),
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => self.cursor = cycle(self.cursor, n, -1),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = n - 1,
            KeyCode::Char('e') => { self.cursor = n - 1; return Some(ModalResult::ClosedSubmitted); }
            _ => {}
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Item(i) if i < self.rows() => { self.cursor = i; (self.chosen() != PickerChoice::None).then_some(ModalResult::ClosedSubmitted) }
            _ => None,
        }
    }
    fn on_scroll(&mut self, d: i32) { self.cursor = cycle(self.cursor, self.rows(), d.signum()); }
}
