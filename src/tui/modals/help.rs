//! Help modal: the real `--help` text (generated from clap, so it cannot drift)
//! plus the TUI key bindings.

use super::*;
use crossterm::event::KeyCode;

/// Scrollable help text.
#[derive(Debug, Clone, PartialEq)]
pub struct HelpModal {
    /// Lines.
    pub lines: Vec<String>,
    /// First visible line.
    pub scroll: usize,
}

/// Help text: key bindings, then `acp-rs-scraper --help`.
pub fn help_text() -> Vec<String> {
    use clap::CommandFactory;
    let mut v = vec!["Terminal UI keys".to_string(), String::new()];
    for (k, d) in crate::config::TUI_KEYBINDINGS { v.push(format!("  {k:<22} {d}")); }
    v.extend(["".into(), "Menu commands map to these flags; Start Scanning runs the same scrape the CLI does.".into(), "".into()]);
    let help = crate::cli::Args::command().render_long_help().to_string();
    v.extend(help.lines().map(str::to_string));
    v
}

impl HelpModal {
    /// Help with the generated text.
    pub fn new() -> Self { HelpModal { lines: help_text(), scroll: 0 } }
    fn scroll_by(&mut self, d: i32) {
        let max = self.lines.len().saturating_sub(1) as i64;
        self.scroll = (self.scroll as i64 + d as i64).clamp(0, max) as usize;
    }
}

impl Default for HelpModal { fn default() -> Self { Self::new() } }

impl ModalState for HelpModal {
    fn title(&self) -> String { "Help".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::F(1) => return Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => return Some(ModalResult::ClosedSubmitted),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_by(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_by(-1),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_by(15),
            KeyCode::PageUp => self.scroll_by(-15),
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll_by(i32::MAX / 2),
            _ => {}
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        matches!(h, ModalHit::Button(_)).then_some(ModalResult::ClosedSubmitted)
    }
    fn on_scroll(&mut self, d: i32) { self.scroll_by(d * 3); }
}
