//! The single source of TUI styling. Colours: ACP red (`#dc2626`) accent by
//! default, overridable from the `[theme]` table of `~/.acp/preferences.toml`.
//!
//! Every text style and every box (border kind, border colour, padding) is a
//! [`ratatui_lipgloss`] [`Gloss`] style built here; widgets only pick one.
//! `&Gloss` / `Gloss` convert into ratatui styles (`Span::styled(s, t.muted())`),
//! and [`frame`] turns a box style into a ratatui `Block` so callers can add a
//! border title (Lip Gloss has no border titles).

use crate::prelude::*;
use ratatui::style::Color;
use ratatui::widgets::Block;
pub use ratatui_lipgloss::{Border, Style as Gloss};

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

    // ----- text ------------------------------------------------------------
    /// Unstyled text (the terminal's own colours).
    pub fn plain(&self) -> Gloss { Gloss::new() }
    /// Bold text (headings, titles, nicks).
    pub fn bold(&self) -> Gloss { Gloss::new().bold(true) }
    /// Accent-coloured text.
    pub fn accent(&self) -> Gloss { Gloss::new().foreground(self.accent) }
    /// Bold accent text.
    pub fn accent_bold(&self) -> Gloss { self.accent().bold(true) }
    /// A link: underlined accent.
    pub fn link(&self) -> Gloss { self.accent().underline(true) }
    /// Muted text.
    pub fn muted(&self) -> Gloss { Gloss::new().foreground(self.muted) }
    /// Something that is gone (a missing file): muted, struck through.
    pub fn gone(&self) -> Gloss { self.muted().strikethrough(true) }
    /// Inline code: bold muted.
    pub fn code(&self) -> Gloss { self.muted().bold(true) }
    /// Highlighted (selected) row.
    pub fn selected(&self) -> Gloss { Gloss::new().background(self.accent).foreground(self.on_accent).bold(true) }
    /// A text field's value.
    pub fn field(&self) -> Gloss { Gloss::new().underline(true) }
    /// A form label, accent when its field has focus.
    pub fn label(&self, focused: bool) -> Gloss { if focused { self.accent_bold() } else { self.plain() } }
    /// The header bar and its text: on-accent over accent.
    pub fn bar(&self) -> Gloss { Gloss::new().background(self.accent).foreground(self.on_accent) }
    /// A filled button (Start Scanning).
    pub fn button(&self) -> Gloss { self.bar().bold(true) }
    /// The "Post" badge of a card.
    pub fn post_tag(&self) -> Gloss { Gloss::new().foreground(self.on_accent).background(self.border).bold(true) }
    /// The "Reply" badge of a card.
    pub fn reply_tag(&self) -> Gloss { Gloss::new().foreground(self.on_accent).background(self.accent).bold(true) }
    /// The background under an open modal.
    pub fn dimmed(&self) -> Gloss { Gloss::new().faint(true) }

    // ----- boxes -----------------------------------------------------------
    /// Border colour of a pane: accent when focused.
    pub fn border(&self, focused: bool) -> Gloss { Gloss::new().foreground(if focused { self.accent } else { self.border }) }
    /// A pane (Nicks, Body, carousel sides): plain border, accent when focused.
    pub fn pane(&self, focused: bool) -> Gloss {
        Gloss::new().border(Border::normal()).border_foreground(if focused { self.accent } else { self.border })
    }
    /// A post card with one column of padding: thick when selected (accent
    /// when the Body has focus), plain and muted otherwise.
    pub fn card(&self, selected: bool, focused: bool) -> Gloss {
        let g = Gloss::new().border(if selected { Border::thick() } else { Border::normal() }).padding((0, 1));
        match (selected, focused) {
            (true, true) => g.border_foreground(self.accent),
            (true, false) => g,
            _ => g.border_foreground(self.border),
        }
    }
    /// A detail / content pane: double border, accent when focused.
    pub fn detail(&self, focused: bool) -> Gloss {
        Gloss::new().border(Border::double()).border_foreground(if focused { self.accent } else { self.border })
    }
    /// Dialogs, dropdown menus and nested details: double accent border.
    pub fn dialog(&self) -> Gloss { Gloss::new().border(Border::double()).border_foreground(self.accent) }
}

/// The ratatui `Block` for a box style (borders, border colour, padding), to
/// render or to give a title.
pub fn frame(g: &Gloss) -> Block<'static> { g.to_block() }
