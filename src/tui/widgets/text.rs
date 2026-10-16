//! Single-line text field state (cursor editing), shared by menu options and modals.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// An editable line of text with a cursor (in chars).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextField {
    /// Current text.
    pub value: String,
    /// Cursor position, in chars.
    pub cursor: usize,
    /// Show `•` instead of the characters.
    pub masked: bool,
}

impl TextField {
    /// A field holding `v`, cursor at the end.
    pub fn new(v: &str) -> Self { TextField { value: v.into(), cursor: v.chars().count(), masked: false } }
    /// Replace the text, cursor at the end.
    pub fn set(&mut self, v: &str) { *self = TextField { masked: self.masked, ..TextField::new(v) }; }
    fn byte(&self, i: usize) -> usize { self.value.char_indices().nth(i).map(|(b, _)| b).unwrap_or(self.value.len()) }
    /// Insert a char at the cursor.
    pub fn insert(&mut self, c: char) { let b = self.byte(self.cursor); self.value.insert(b, c); self.cursor += 1; }
    /// Apply an editing key; returns whether it was consumed.
    pub fn on_key(&mut self, k: KeyEvent) -> bool {
        let n = self.value.chars().count();
        match k.code {
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => { self.set(""); }
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) && !k.modifiers.contains(KeyModifiers::ALT) => self.insert(c),
            KeyCode::Backspace if self.cursor > 0 => {
                let b = self.byte(self.cursor - 1);
                self.value.remove(b);
                self.cursor -= 1;
            }
            KeyCode::Delete if self.cursor < n => { let b = self.byte(self.cursor); self.value.remove(b); }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(n),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = n,
            KeyCode::Backspace | KeyCode::Delete => {}
            _ => return false,
        }
        true
    }
    /// Text as displayed (masked or not).
    pub fn display(&self) -> String {
        if self.masked { "•".repeat(self.value.chars().count()) } else { self.value.clone() }
    }
}
