//! Error / status modal with an OK button.

use super::*;
use crossterm::event::KeyCode;

/// A message to acknowledge.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageModal {
    /// Title.
    pub title: String,
    /// Text lines.
    pub lines: Vec<String>,
    /// Shown in the error colour.
    pub error: bool,
}

impl MessageModal {
    /// An error message.
    pub fn error(title: &str, text: &str) -> Self { MessageModal { title: title.into(), lines: text.lines().map(str::to_string).collect(), error: true } }
    /// A status message.
    pub fn info(title: &str, text: &str) -> Self { MessageModal { error: false, ..Self::error(title, text) } }
}

impl ModalState for MessageModal {
    fn title(&self) -> String { self.title.clone() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Enter | KeyCode::Char(' ') => Some(ModalResult::ClosedSubmitted),
            KeyCode::Esc | KeyCode::Char('q') => Some(ModalResult::ClosedCancelled),
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        matches!(h, ModalHit::Button(_)).then_some(ModalResult::ClosedSubmitted)
    }
}
