//! About, and the credits hidden inside it.
//!
//! The public page's border title is the slogan.

use super::*;
use crate::branding::SLOGAN;
use crate::eggs;
use crossterm::event::KeyCode;
use std::time::Instant;

/// The About page. `credits` is the hidden page.
#[derive(Debug)]
pub struct AboutModal {
    /// The credits are showing.
    pub credits: bool,
    /// Star presses on this page.
    pub taps: u8,
    /// When the star on this page was last pressed.
    pub last_tap: Option<Instant>,
    /// First visible credits line.
    pub scroll: usize,
}

impl AboutModal {
    /// The public page.
    pub fn page() -> Self {
        Self { credits: false, taps: 0, last_tap: None, scroll: 0 }
    }
    /// The credits, already open.
    pub fn credits() -> Self {
        Self { credits: true, taps: 0, last_tap: None, scroll: 0 }
    }
    /// One press on this page's star.
    pub fn tap(&mut self, now: Instant) -> bool {
        if eggs::note_tap(&mut self.taps, &mut self.last_tap, now) {
            self.credits = true;
            self.scroll = 0;
            true
        } else {
            false
        }
    }
}

impl ModalState for AboutModal {
    fn title(&self) -> String {
        if self.credits { "Credits".into() } else { SLOGAN.to_string() }
    }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(ModalResult::ClosedCancelled),
            KeyCode::Char('~') => { self.credits = true; self.scroll = 0; None }
            KeyCode::Enter | KeyCode::Char(' ') if !self.credits => { self.tap(Instant::now()); None }
            KeyCode::Enter => Some(ModalResult::ClosedSubmitted),
            KeyCode::Up | KeyCode::Char('k') => { self.scroll = self.scroll.saturating_sub(1); None }
            KeyCode::Down | KeyCode::Char('j') => { self.scroll = self.scroll.saturating_add(1); None }
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Item(_) if !self.credits => { self.tap(Instant::now()); None }
            ModalHit::Button(_) => Some(ModalResult::ClosedSubmitted),
            _ => None,
        }
    }
    fn on_scroll(&mut self, d: i32) {
        self.scroll = (self.scroll as i64 + d as i64).max(0) as usize;
    }
}
