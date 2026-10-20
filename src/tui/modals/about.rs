//! About: the product name, with the slogan as the header line.

use super::*;
use crate::branding::SLOGAN;
use crossterm::event::KeyCode;

/// The About modal. Its border title is [`SLOGAN`].
#[derive(Debug, Default)]
pub struct AboutModal;

impl ModalState for AboutModal {
    fn title(&self) -> String {
        SLOGAN.to_string()
    }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char(' ') => {
                Some(ModalResult::ClosedCancelled)
            }
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        matches!(h, ModalHit::Button(_)).then_some(ModalResult::ClosedCancelled)
    }
}
