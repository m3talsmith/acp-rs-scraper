//! Colours: ACP red (`#dc2626`) accent by default, overridable from the
//! `[theme]` table of `~/.acp/preferences.toml`.

use crate::prelude::*;
use ratatui::style::{Color, Modifier, Style};

/// Resolved colours used by every widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Accent: reply links, selection, focused borders, Start Scanning.
    pub accent: Color,
    /// Secondary text.
    pub muted: Color,
    /// Unfocused borders.
    pub border: Color,
    /// Text drawn on an accent background.
    pub on_accent: Color,
}

fn rgb((r, g, b): (u8, u8, u8)) -> Color { Color::Rgb(r, g, b) }

/// A colour as RGB bytes (named colours map to white, black, or grey).
pub fn rgb_bytes(c: Color) -> [u8; 3] {
    match c {
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Black => [0, 0, 0],
        Color::White => [255, 255, 255],
        _ => [200, 200, 200],
    }
}

/// `#rrggbb` (or `rrggbb`) to a colour.
pub fn parse_hex(s: &str) -> Option<Color> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) { return None; }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some(Color::Rgb(p(0)?, p(2)?, p(4)?))
}

impl Default for Theme {
    fn default() -> Self {
        Theme { accent: rgb(ACP_RED), muted: rgb(TUI_MUTED), border: rgb(TUI_BORDER), on_accent: rgb(TUI_ON_ACCENT) }
    }
}

impl Theme {
    /// Defaults with any valid colour from `prefs` applied (invalid values are ignored).
    pub fn from_prefs(prefs: Option<&ThemePrefs>) -> Self {
        let mut t = Theme::default();
        let Some(p) = prefs else { return t };
        let pick = |v: &Option<String>, d: Color| v.as_deref().and_then(parse_hex).unwrap_or(d);
        t.accent = pick(&p.accent, t.accent);
        t.muted = pick(&p.muted, t.muted);
        t.border = pick(&p.border, t.border);
        t.on_accent = pick(&p.on_accent, t.on_accent);
        t
    }
    /// Accent-coloured text.
    pub fn accent(&self) -> Style { Style::default().fg(self.accent) }
    /// Bold accent text.
    pub fn accent_bold(&self) -> Style { self.accent().add_modifier(Modifier::BOLD) }
    /// Muted text.
    pub fn muted(&self) -> Style { Style::default().fg(self.muted) }
    /// Highlighted (selected) row.
    pub fn selected(&self) -> Style { Style::default().bg(self.accent).fg(self.on_accent).add_modifier(Modifier::BOLD) }
    /// Border style for a pane, accent when focused.
    pub fn border(&self, focused: bool) -> Style {
        if focused { Style::default().fg(self.accent) } else { Style::default().fg(self.border) }
    }
}
