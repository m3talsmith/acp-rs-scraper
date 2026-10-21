//! Search history: pick a saved query and re-run it.

use super::*;
use crate::storage::history::SavedQuery;
use crossterm::event::KeyCode;

/// A list of saved queries, newest first.
#[derive(Debug)]
pub struct HistoryModal {
    /// Saved queries.
    pub rows: Vec<SavedQuery>,
    /// Highlighted row.
    pub cursor: usize,
}

impl HistoryModal {
    /// History with `rows` (newest first).
    pub fn new(rows: Vec<SavedQuery>) -> Self { Self { rows, cursor: 0 } }
    /// The highlighted query.
    pub fn chosen(&self) -> Option<&SavedQuery> { self.rows.get(self.cursor) }
}

impl ModalState for HistoryModal {
    fn title(&self) -> String { "Search history".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => Some(ModalResult::ClosedSubmitted),
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
