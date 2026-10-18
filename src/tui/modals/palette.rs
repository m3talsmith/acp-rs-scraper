//! Ctrl+Space command palette: type to filter every command, Enter runs it.

use super::*;
use crate::tui::commands::Action;
use crate::tui::widgets::text::TextField;
use crossterm::event::KeyCode;

/// What a palette entry does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteCmd {
    /// Run an action.
    Action(Action),
    /// Open the n-th menu item's dropdown.
    OpenMenu(usize),
}

/// Palette state.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteModal {
    /// Every entry.
    pub entries: Vec<(String, PaletteCmd)>,
    /// Filter text.
    pub query: TextField,
    /// Selected row among the filtered ones.
    pub cursor: usize,
}

impl PaletteModal {
    /// Palette over `entries`.
    pub fn new(entries: Vec<(String, PaletteCmd)>) -> Self { PaletteModal { entries, query: TextField::default(), cursor: 0 } }
    /// Entries matching the filter (every word, case-insensitive). An entry whose label
    /// ends in `[name]` takes one extra word after the words that match it: its argument
    /// (`theme starter mine`).
    pub fn filtered(&self) -> Vec<&(String, PaletteCmd)> {
        let q = self.query.value.to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        self.entries.iter().filter(|(l, _)| {
            let l = l.to_lowercase();
            if words.iter().all(|w| l.contains(w)) { return true; }
            l.ends_with("[name]") && words.len() >= 2 && words[..words.len() - 1].iter().all(|w| l.contains(w))
        }).collect()
    }
    /// The argument typed after an `[name]` entry's words (`theme starter mine` → `mine`).
    pub fn argument(&self) -> Option<String> {
        let (label, _) = self.filtered().get(self.cursor).copied()?;
        let l = label.to_lowercase();
        if !l.ends_with("[name]") { return None; }
        let words: Vec<&str> = self.query.value.split_whitespace().collect();
        let last = words.last()?;
        (words.len() >= 2 && !l.contains(&last.to_lowercase())).then(|| last.to_string())
    }
    /// The entry Enter would run.
    pub fn chosen(&self) -> Option<PaletteCmd> { self.filtered().get(self.cursor).map(|e| e.1) }
}

impl ModalState for PaletteModal {
    fn title(&self) -> String { "Commands".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        let n = self.filtered().len();
        match k.code {
            KeyCode::Esc => return Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => return (n > 0).then_some(ModalResult::ClosedSubmitted),
            KeyCode::Down | KeyCode::Tab => self.cursor = cycle(self.cursor, n, 1),
            KeyCode::Up | KeyCode::BackTab => self.cursor = cycle(self.cursor, n, -1),
            _ => { if self.query.on_key(k) { self.cursor = 0; } }
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h { ModalHit::Item(i) => { self.cursor = i; Some(ModalResult::ClosedSubmitted) } _ => None }
    }
    fn on_scroll(&mut self, d: i32) { let n = self.filtered().len(); self.cursor = cycle(self.cursor, n, d.signum()); }
}
