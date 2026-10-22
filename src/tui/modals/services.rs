//! Preferences › Services. Enable a service, read a quarantine reason, reload plugins.

use super::*;
use crate::services::ServiceRow;
use crossterm::event::{KeyCode, KeyModifiers};

/// The Services list.
#[derive(Debug, Clone)]
pub struct ServicesModal {
    /// Rows, including headers.
    pub rows: Vec<ServiceRow>,
    /// Highlighted row.
    pub cursor: usize,
}

impl ServicesModal {
    /// Build from the current registry and quarantine list.
    pub fn from_rows(rows: Vec<ServiceRow>) -> Self {
        Self { rows, cursor: 0 }
    }

    fn reload_rows(&mut self) {
        let home = crate::config::home().unwrap_or_default();
        self.rows = crate::services::services_rows(&home);
        if self.cursor >= self.rows.len() { self.cursor = self.rows.len().saturating_sub(1); }
    }
}

impl ModalState for ServicesModal {
    fn title(&self) -> String { "Services".into() }

    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        if k.modifiers.contains(KeyModifiers::CONTROL) { return None; }
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(ModalResult::ClosedCancelled),
            KeyCode::Char('?') | KeyCode::F(1) => Some(ModalResult::Help("services")),
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor > 0 { self.cursor -= 1; }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.cursor + 1 < self.rows.len() { self.cursor += 1; }
                None
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                self.activate();
                None
            }
            KeyCode::Char('u') => {
                self.restore_current();
                None
            }
            KeyCode::Char('d') => {
                self.delete_current();
                None
            }
            _ => None,
        }
    }

    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        if let ModalHit::Item(i) = h {
            self.cursor = i;
            self.activate();
        }
        None
    }
}

impl ServicesModal {
    fn activate(&mut self) {
        let home = match crate::config::home() {
            Ok(h) => h,
            Err(e) => {
                self.rows.push(ServiceRow::Note(format!("{e:#}")));
                return;
            }
        };
        match self.rows.get(self.cursor).cloned() {
            Some(ServiceRow::Service { id, enabled, .. }) => {
                let _ = crate::services::set_enabled(&home, &id, !enabled);
                self.reload_rows();
            }
            Some(ServiceRow::Reload) => {
                let _ = crate::services::reload_plugins();
                self.reload_rows();
            }
            _ => {}
        }
    }

    fn restore_current(&mut self) {
        let Some(ServiceRow::Quarantine { file, .. }) = self.rows.get(self.cursor).cloned() else { return };
        let home = crate::config::home().unwrap_or_default();
        if crate::plugins::restore(&home, &file).is_ok() {
            let _ = crate::services::reload_plugins();
        }
        self.reload_rows();
    }

    fn delete_current(&mut self) {
        let Some(ServiceRow::Quarantine { file, .. }) = self.rows.get(self.cursor).cloned() else { return };
        let home = crate::config::home().unwrap_or_default();
        let _ = crate::plugins::delete(&home, &file);
        self.reload_rows();
    }
}
