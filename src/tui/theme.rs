//! The single source of TUI styling. Colours come from a theme file
//! (`~/.acp/themes/<name>.toml`, see [`crate::themes`]); without one every colour
//! is the terminal's own (`Color::Reset`) and the layout still works.
//!
//! Every text style and every box (border kind, border colour, padding) is a
//! [`ratatui_lipgloss`] [`Gloss`] style built here; widgets only pick one. A theme
//! sets the palette and may override any [`Element`]'s colours and attributes;
//! borders and padding stay here. `&Gloss` / `Gloss` convert into ratatui styles
//! (`Span::styled(s, t.muted())`), and [`frame`] turns a box style into a ratatui
//! `Block` so callers can add a border title (Lip Gloss has no border titles).

use crate::themes::{Element, StyleSpec, ThemeFile, Variant};
use ratatui::style::Color;
use ratatui::widgets::Block;
pub use ratatui_lipgloss::{Border, Style as Gloss};

/// One element's resolved overrides.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Over {
    /// Text colour.
    pub fg: Option<Color>,
    /// Background.
    pub bg: Option<Color>,
    /// Border colour.
    pub border: Option<Color>,
    /// Attributes: bold, italic, underline, reverse, dim.
    pub flags: [Option<bool>; 5],
}

/// Number of [`Element`]s.
pub const ELEMENTS: usize = 27;

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
    /// Base text colour of the whole screen.
    pub fg: Color,
    /// Base background of the whole screen.
    pub bg: Color,
    /// Per-element overrides, by [`Element::index`].
    pub over: [Over; ELEMENTS],
}

/// A colour as RGB bytes (the terminal's own colour reads as light grey).
pub fn rgb_bytes(c: Color) -> [u8; 3] {
    crate::themes::rgb(c).map(|(r, g, b)| [r, g, b]).unwrap_or([200, 200, 200])
}

/// `#rrggbb` (or `rrggbb`) to a colour.
pub fn parse_hex(s: &str) -> Option<Color> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) { return None; }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some(Color::Rgb(p(0)?, p(2)?, p(4)?))
}

/// No theme: every colour is the terminal's own.
impl Default for Theme {
    fn default() -> Self { Theme::reset() }
}

fn over_from(s: &crate::themes::Section, st: &StyleSpec) -> Over {
    let c = |x: &Option<crate::themes::ColorSpec>| x.as_ref().map(|c| s.resolve(c));
    Over { fg: c(&st.fg), bg: c(&st.bg), border: c(&st.border), flags: [st.bold, st.italic, st.underline, st.reverse, st.dim] }
}

impl Theme {
    /// The fallback when no theme file can be used: `Color::Reset` everywhere.
    pub fn reset() -> Self {
        let r = Color::Reset;
        Theme { accent: r, muted: r, border: r, on_accent: r, fg: r, bg: r, over: [Over::default(); ELEMENTS] }
    }

    /// The `v` variant of a theme file (missing entries: the terminal's colour).
    pub fn from_file(f: &ThemeFile, v: Variant) -> Self {
        let s = f.section(v);
        let mut t = Theme { accent: s.color("accent"), muted: s.color("muted"), border: s.color("border"),
            on_accent: s.color("on_accent"), fg: s.color("fg"), bg: s.color("bg"), over: [Over::default(); ELEMENTS] };
        for e in Element::ALL { t.over[e.index()] = over_from(&s, &s.style(*e)); }
        t
    }

    /// A palette colour by key (`accent`, `muted`, `border`, `on_accent`, `fg`, `bg`).
    pub fn palette(&self, k: &str) -> Color {
        match k { "accent" => self.accent, "muted" => self.muted, "border" => self.border, "on_accent" => self.on_accent, "fg" => self.fg, "bg" => self.bg, _ => Color::Reset }
    }

    /// `g` with the theme's overrides for `e` applied.
    pub fn el(&self, e: Element, g: Gloss) -> Gloss {
        let o = &self.over[e.index()];
        let mut g = g;
        if let Some(c) = o.fg { g = g.foreground(c); }
        if let Some(c) = o.bg { g = g.background(c); }
        if let Some(c) = o.border { g = g.border_foreground(c); }
        let [b, i, u, r, d] = o.flags;
        if let Some(v) = b { g = g.bold(v); }
        if let Some(v) = i { g = g.italic(v); }
        if let Some(v) = u { g = g.underline(v); }
        if let Some(v) = r { g = g.reverse(v); }
        if let Some(v) = d { g = g.faint(v); }
        g
    }

    /// The whole screen's base colours (`fg` / `bg`).
    pub fn base(&self) -> Gloss {
        let mut g = Gloss::new();
        if self.fg != Color::Reset { g = g.foreground(self.fg); }
        if self.bg != Color::Reset { g = g.background(self.bg); }
        g
    }

    /// The style of an element as drawn (for the theme editor's swatches).
    pub fn element(&self, e: Element) -> Gloss {
        match e {
            Element::Text => self.plain(), Element::Bold => self.bold(), Element::Accent => self.accent(),
            Element::AccentBold => self.accent_bold(), Element::Link => self.link(), Element::Muted => self.muted(),
            Element::Gone => self.gone(), Element::Code => self.code(), Element::Selected => self.selected(),
            Element::Field => self.field(), Element::Label => self.label(false), Element::LabelFocused => self.label(true),
            Element::Bar => self.bar(), Element::Button => self.button(), Element::PostTag => self.post_tag(),
            Element::ReplyTag => self.reply_tag(), Element::Dimmed => self.dimmed(), Element::Border => self.border(false),
            Element::BorderFocused => self.border(true), Element::Pane => self.pane(false), Element::PaneFocused => self.pane(true),
            Element::Card => self.card(false, false), Element::CardSelected => self.card(true, false), Element::CardFocused => self.card(true, true),
            Element::Detail => self.detail(false), Element::DetailFocused => self.detail(true), Element::Dialog => self.dialog(),
        }
    }

    // ----- text ------------------------------------------------------------
    /// Unstyled text (the terminal's own colours).
    pub fn plain(&self) -> Gloss { self.el(Element::Text, Gloss::new()) }
    /// Bold text (headings, titles, nicks).
    pub fn bold(&self) -> Gloss { self.el(Element::Bold, Gloss::new().bold(true)) }
    /// Accent-coloured text.
    pub fn accent(&self) -> Gloss { self.el(Element::Accent, Gloss::new().foreground(self.accent)) }
    /// Bold accent text.
    pub fn accent_bold(&self) -> Gloss { self.el(Element::AccentBold, Gloss::new().foreground(self.accent).bold(true)) }
    /// A link: underlined accent.
    pub fn link(&self) -> Gloss { self.el(Element::Link, Gloss::new().foreground(self.accent).underline(true)) }
    /// Muted text.
    pub fn muted(&self) -> Gloss { self.el(Element::Muted, Gloss::new().foreground(self.muted)) }
    /// Something that is gone (a missing file): muted, struck through.
    pub fn gone(&self) -> Gloss { self.el(Element::Gone, Gloss::new().foreground(self.muted).strikethrough(true)) }
    /// Inline code: bold muted.
    pub fn code(&self) -> Gloss { self.el(Element::Code, Gloss::new().foreground(self.muted).bold(true)) }
    /// Highlighted (selected) row.
    pub fn selected(&self) -> Gloss { self.el(Element::Selected, Gloss::new().background(self.accent).foreground(self.on_accent).bold(true)) }
    /// A text field's value.
    pub fn field(&self) -> Gloss { self.el(Element::Field, Gloss::new().underline(true)) }
    /// A form label, accent when its field has focus.
    pub fn label(&self, focused: bool) -> Gloss {
        if focused { self.el(Element::LabelFocused, self.accent_bold()) } else { self.el(Element::Label, self.plain()) }
    }
    /// The header bar and its text: on-accent over accent.
    pub fn bar(&self) -> Gloss { self.el(Element::Bar, Gloss::new().background(self.accent).foreground(self.on_accent)) }
    /// A filled button (Start Scanning).
    pub fn button(&self) -> Gloss { self.el(Element::Button, self.bar().bold(true)) }
    /// The "Post" badge of a card.
    pub fn post_tag(&self) -> Gloss { self.el(Element::PostTag, Gloss::new().foreground(self.on_accent).background(self.border).bold(true)) }
    /// The "Reply" badge of a card.
    pub fn reply_tag(&self) -> Gloss { self.el(Element::ReplyTag, Gloss::new().foreground(self.on_accent).background(self.accent).bold(true)) }
    /// The background under an open modal.
    pub fn dimmed(&self) -> Gloss { self.el(Element::Dimmed, Gloss::new().faint(true)) }

    // ----- boxes -----------------------------------------------------------
    /// Border colour of a pane: accent when focused.
    pub fn border(&self, focused: bool) -> Gloss {
        self.el(if focused { Element::BorderFocused } else { Element::Border }, Gloss::new().foreground(if focused { self.accent } else { self.border }))
    }
    /// A pane (Nicks, Body, carousel sides): plain border, accent when focused.
    pub fn pane(&self, focused: bool) -> Gloss {
        self.el(if focused { Element::PaneFocused } else { Element::Pane },
            Gloss::new().border(Border::normal()).border_foreground(if focused { self.accent } else { self.border }))
    }
    /// A post card with one column of padding: thick when selected (accent
    /// when the Body has focus), plain and muted otherwise.
    pub fn card(&self, selected: bool, focused: bool) -> Gloss {
        let g = Gloss::new().border(if selected { Border::thick() } else { Border::normal() }).padding((0, 1));
        match (selected, focused) {
            (true, true) => self.el(Element::CardFocused, g.border_foreground(self.accent)),
            (true, false) => self.el(Element::CardSelected, g),
            _ => self.el(Element::Card, g.border_foreground(self.border)),
        }
    }
    /// A detail / content pane: double border, accent when focused.
    pub fn detail(&self, focused: bool) -> Gloss {
        self.el(if focused { Element::DetailFocused } else { Element::Detail },
            Gloss::new().border(Border::double()).border_foreground(if focused { self.accent } else { self.border }))
    }
    /// Dialogs, dropdown menus and nested details: double accent border.
    pub fn dialog(&self) -> Gloss { self.el(Element::Dialog, Gloss::new().border(Border::double()).border_foreground(self.accent)) }
}

/// The ratatui `Block` for a box style (borders, border colour, padding), to
/// render or to give a title.
pub fn frame(g: &Gloss) -> Block<'static> { g.to_block() }
