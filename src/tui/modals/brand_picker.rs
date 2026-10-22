//! The brand picker (the header's `«brand» ▾`, and Brand: use… in the palette).
//!
//! Moving the highlight previews the brand. Esc puts the old one back.

use super::*;
use crate::brands::{Brand, Entry};
use crossterm::event::KeyCode;

/// What the picker chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrandChoice {
    /// Use this brand.
    Brand(String),
    /// Open the brand editor.
    Edit,
    /// A brand that can't be used (or nothing).
    None,
}

/// Picker state.
#[derive(Debug, Clone, PartialEq)]
pub struct BrandPickerModal {
    /// Built-ins and brand files.
    pub entries: Vec<Entry>,
    /// Highlighted row (`entries.len()` is "Edit brands…").
    pub cursor: usize,
    /// The brand in use when it opened (put back on Esc).
    pub restore: Brand,
    /// Its name.
    pub current: String,
}

impl BrandPickerModal {
    /// Picker over `entries`, highlighting `current`.
    pub fn new(entries: Vec<Entry>, current: &str, restore: Brand) -> Self {
        let cursor = entries.iter().position(|e| e.name == current).unwrap_or(0);
        BrandPickerModal { entries, cursor, restore, current: current.into() }
    }
    /// Rows: the brands and "Edit brands…".
    pub fn rows(&self) -> usize { self.entries.len() + 1 }
    /// The highlighted brand, when it can be used.
    pub fn highlighted(&self) -> Option<&str> {
        self.entries.get(self.cursor).filter(|e| e.error.is_none()).map(|e| e.name.as_str())
    }
    /// What Enter does.
    pub fn chosen(&self) -> BrandChoice {
        if self.cursor == self.entries.len() { return BrandChoice::Edit; }
        self.highlighted().map(|n| BrandChoice::Brand(n.into())).unwrap_or(BrandChoice::None)
    }
}

impl ModalState for BrandPickerModal {
    fn title(&self) -> String { "Brand · ↑↓ preview · Enter uses · Esc keeps".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        let n = self.rows();
        match k.code {
            KeyCode::Esc => return Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => return (self.chosen() != BrandChoice::None).then_some(ModalResult::ClosedSubmitted),
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
            ModalHit::Item(i) if i < self.rows() => {
                self.cursor = i;
                (self.chosen() != BrandChoice::None).then_some(ModalResult::ClosedSubmitted)
            }
            _ => None,
        }
    }
    fn on_scroll(&mut self, d: i32) { self.cursor = cycle(self.cursor, self.rows(), d.signum()); }
}
