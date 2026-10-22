//! About, and the credits hidden inside it.
//!
//! The public page's border title is the slogan.

use super::*;
use crate::eggs;
use crossterm::event::KeyCode;
use std::time::Instant;

/// The About page. `credits` is the hidden page. `section` is 0, 1, or 2.
#[derive(Debug)]
pub struct AboutModal {
    /// The hidden credits are showing.
    pub credits: bool,
    /// 0 Credits, 1 Why this is necessary, 2 ACP platform.
    pub section: u8,
    /// Star presses on this page.
    pub taps: u8,
    /// When the star on this page was last pressed.
    pub last_tap: Option<Instant>,
    /// First visible line.
    pub scroll: usize,
}

const PROGRAM: &str = include_str!("../../../docs/acp-program-2024.md");
const JOIN: &str = "https://acp.us/join";
const PROGRAM_URL: &str = "https://acp.us/program";
const PROGRAM_PDF: &str = "https://acp.us/docs/2024-program-a51b2b90421960a720072cc1731dfac0.pdf";

impl AboutModal {
    /// The public page, on Credits.
    pub fn page() -> Self { Self::section(0) }
    /// The same modal, focused on `section` (0, 1, or 2).
    pub fn section(section: u8) -> Self {
        let max = if crate::brands::current().shows_acp() { 2 } else { 1 };
        Self { credits: false, section: section.min(max), taps: 0, last_tap: None, scroll: 0 }
    }
    /// The hidden credits, already open.
    pub fn credits() -> Self {
        Self { credits: true, section: 0, taps: 0, last_tap: None, scroll: 0 }
    }
    /// Whether the open brand has the platform section.
    pub fn shows_platform() -> bool { crate::brands::current().shows_acp() }
    /// Lines for the open page. A neutral brand has two sections.
    pub fn body_lines(&self) -> Vec<String> {
        if self.credits { return crate::eggs::credits_lines(); }
        let brand = crate::brands::current();
        let tabs = if brand.shows_acp() {
            "1 Credits    2 Why this is necessary    3 ACP platform"
        } else {
            "1 Credits    2 Why this is necessary"
        };
        let mut lines = vec![
            brand.title.clone(),
            format!("v{}", env!("CARGO_PKG_VERSION")),
            String::new(),
            tabs.into(),
            String::new(),
        ];
        match self.section {
            0 => lines.extend(public_credits()),
            1 => lines.extend(why_lines(&brand.title)),
            _ if brand.shows_acp() => lines.extend(platform_lines()),
            _ => lines.extend(why_lines(&brand.title)),
        }
        lines.push(String::new());
        lines.push("★".into());
        lines.push("Esc closes. ← → moves sections.".into());
        lines
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

fn public_credits() -> Vec<String> {
    vec![
        "Credits".into(),
        "The people who built this client.".into(),
        "The repository is m3talsmith/acp-rs-scraper.".into(),
    ]
}

fn why_lines(title: &str) -> Vec<String> {
    vec![
        "Why this is necessary".into(),
        format!("{title} keeps a copy of the Red Star forum on this machine."),
        "Reading, search, and export work from that copy when the site is slow or gone.".into(),
        "The database is encrypted. The password stays on this computer.".into(),
        "Publishing a post still waits on a confirmed capture. This window does not send one.".into(),
    ]
}

fn platform_lines() -> Vec<String> {
    let mut lines = vec![
        "ACP platform".into(),
        JOIN.into(),
        format!("Program: {PROGRAM_URL}"),
        format!("PDF: {PROGRAM_PDF}"),
        String::new(),
    ];
    lines.extend(PROGRAM.lines().map(str::to_string));
    lines.push(String::new());
    lines.push(JOIN.into());
    lines
}

impl ModalState for AboutModal {
    fn title(&self) -> String {
        if self.credits {
            "Credits".into()
        } else {
            let brand = crate::brands::current();
            if brand.motto.is_empty() { brand.title } else { brand.motto }
        }
    }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(ModalResult::ClosedCancelled),
            KeyCode::Char('~') => { self.credits = true; self.scroll = 0; None }
            KeyCode::Enter | KeyCode::Char(' ') if !self.credits => { self.tap(Instant::now()); None }
            KeyCode::Enter => Some(ModalResult::ClosedSubmitted),
            KeyCode::Char('1') if !self.credits => { self.section = 0; self.scroll = 0; None }
            KeyCode::Char('2') if !self.credits => { self.section = 1; self.scroll = 0; None }
            KeyCode::Char('3') if !self.credits && Self::shows_platform() => { self.section = 2; self.scroll = 0; None }
            KeyCode::Left if !self.credits => { self.section = self.section.saturating_sub(1); self.scroll = 0; None }
            KeyCode::Right if !self.credits => {
                let max = if Self::shows_platform() { 2 } else { 1 };
                self.section = (self.section + 1).min(max);
                self.scroll = 0;
                None
            }
            KeyCode::Up | KeyCode::Char('k') => { self.scroll = self.scroll.saturating_sub(1); None }
            KeyCode::Down | KeyCode::Char('j') => { self.scroll = self.scroll.saturating_add(1); None }
            KeyCode::PageUp => { self.scroll = self.scroll.saturating_sub(15); None }
            KeyCode::PageDown => { self.scroll = self.scroll.saturating_add(15); None }
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
