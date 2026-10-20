//! The notice log, do-not-disturb, and the per-type switches.

use super::*;
use crossterm::event::KeyCode;

/// Cursor over the controls and the log. Toggles are applied by the app.
#[derive(Debug)]
pub struct NoticesModal {
    /// Highlighted row. 0 is do-not-disturb, then the kinds, then the log.
    pub cursor: usize,
    /// The last key asked to flip the highlighted control.
    toggle: bool,
}

impl NoticesModal {
    /// The window, with the first row highlighted.
    pub fn new() -> Self { Self { cursor: 0, toggle: false } }

    /// Take a pending toggle, if the last key asked for one.
    pub fn take_toggle(&mut self) -> bool { std::mem::take(&mut self.toggle) }
}

impl Default for NoticesModal {
    fn default() -> Self { Self::new() }
}

impl ModalState for NoticesModal {
    fn title(&self) -> String { "Notices".into() }

    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        let rows = crate::notify::control_rows();
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(ModalResult::ClosedCancelled),
            KeyCode::Up | KeyCode::Char('k') => { self.cursor = self.cursor.saturating_sub(1); None }
            KeyCode::Down | KeyCode::Char('j') => { self.cursor = self.cursor.saturating_add(1); None }
            KeyCode::Char(' ') | KeyCode::Enter if self.cursor < rows => { self.toggle = true; None }
            _ => None,
        }
    }

    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Item(i) => {
                self.cursor = i;
                if i < crate::notify::control_rows() { self.toggle = true; }
                None
            }
            ModalHit::Button(_) => Some(ModalResult::ClosedCancelled),
            _ => None,
        }
    }

    fn on_scroll(&mut self, delta: i32) {
        if delta > 0 { self.cursor = self.cursor.saturating_add(delta as usize); }
        else { self.cursor = self.cursor.saturating_sub((-delta) as usize); }
    }
}
