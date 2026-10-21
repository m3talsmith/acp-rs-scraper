//! The action-bar branch list. Choosing a row restores that path.

use super::*;
use crossterm::event::KeyCode;

/// Saved action-history paths.
#[derive(Debug)]
pub struct BranchesModal {
    /// One line per path.
    pub rows: Vec<String>,
    /// Highlighted row.
    pub cursor: usize,
}

impl BranchesModal {
    /// A list scrolled to `cursor`.
    pub fn new(rows: Vec<String>, cursor: usize) -> Self {
        let cursor = if rows.is_empty() { 0 } else { cursor.min(rows.len() - 1) };
        Self { rows, cursor }
    }
}

impl ModalState for BranchesModal {
    fn title(&self) -> String { "Action branches".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => Some(ModalResult::ClosedSubmitted),
            KeyCode::Char('?') | KeyCode::F(1) => Some(ModalResult::Help("actions")),
            KeyCode::Up | KeyCode::Char('k') => { self.cursor = self.cursor.saturating_sub(1); None }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.rows.is_empty() { self.cursor = (self.cursor + 1).min(self.rows.len() - 1); }
                None
            }
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Item(i) if i < self.rows.len() => { self.cursor = i; Some(ModalResult::ClosedSubmitted) }
            ModalHit::Button(_) => Some(ModalResult::ClosedCancelled),
            _ => None,
        }
    }
    fn on_scroll(&mut self, d: i32) {
        if self.rows.is_empty() { return; }
        self.cursor = (self.cursor as i64 + d as i64).clamp(0, self.rows.len() as i64 - 1) as usize;
    }
}
