//! The product line shared by the terminal UI.
//!
//! Welcome, login, About, and the status bar all read [`SLOGAN`] from here.

use crate::cli::{ColorMode, colors_suppressed};
use ratatui::style::{Color, Modifier, Style};

/// The line under the title, the About header, and the wide status bar.
pub const SLOGAN: &str = "The future belongs to the working class";

/// Display columns of [`SLOGAN`]. It is ASCII, so this is also its byte length.
pub const SLOGAN_COLS: u16 = 39;

/// Blank columns kept between status text and the slogan.
pub const SLOGAN_STATUS_GAP: u16 = 2;

/// Room the status bar keeps for real text before the slogan may appear.
pub const SLOGAN_STATUS_RESERVE: u16 = 24;

/// Narrowest status bar that may show the slogan.
pub const SLOGAN_STATUS_MIN: u16 = SLOGAN_COLS + SLOGAN_STATUS_GAP + SLOGAN_STATUS_RESERVE;

/// Muted gold for the slogan when colours are on.
pub const SLOGAN_GOLD: Color = Color::Rgb(0xC9, 0xA2, 0x27);

const _: () = {
    let b = SLOGAN.as_bytes();
    let mut i = 0;
    while i < b.len() {
        assert!(b[i].is_ascii());
        i += 1;
    }
    assert!(b.len() == SLOGAN_COLS as usize);
    assert!(SLOGAN_STATUS_MIN == 65);
};

/// The footer can show the slogan beside `status_cols` of text without covering it.
///
/// Hidden when the bar is narrower than [`SLOGAN_STATUS_MIN`], and hidden when the
/// text, the gap, and the slogan do not all fit. The status text is never cut to
/// make room.
pub fn slogan_in_status(width: u16, status_cols: u16) -> bool {
    width >= SLOGAN_STATUS_MIN
        && status_cols
            .saturating_add(SLOGAN_STATUS_GAP)
            .saturating_add(SLOGAN_COLS)
            <= width
}

/// Muted gold, or faint text when `dim` is set. Plain when colours are off
/// (`--color never`, or `NO_COLOR` while the mode is `auto`).
pub fn slogan_style(mode: ColorMode, no_color: bool, dim: bool) -> Style {
    // Reset, not an unset colour: a border title would otherwise keep the border colour.
    if colors_suppressed(mode, no_color) {
        Style::default().fg(Color::Reset)
    } else if dim {
        Style::default()
            .fg(Color::Reset)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(SLOGAN_GOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slogan_width_matches_the_constant() {
        assert_eq!(
            unicode_width::UnicodeWidthStr::width(SLOGAN) as u16,
            SLOGAN_COLS
        );
    }

    #[test]
    fn status_slogan_hides_below_the_threshold_and_when_text_would_be_covered() {
        assert!(!slogan_in_status(64, 0), "below the width threshold");
        assert!(
            slogan_in_status(SLOGAN_STATUS_MIN, 0),
            "empty bottom row at the threshold"
        );
        assert!(slogan_in_status(160, 108), "the default hint fits at 160");
        assert!(!slogan_in_status(148, 108), "108 + gap + slogan is 149");
        assert!(
            !slogan_in_status(110, 108),
            "the default footer is too narrow"
        );
        assert!(
            !slogan_in_status(80, 40),
            "wide enough, but this line would be covered"
        );
        assert!(slogan_in_status(100, 20));
    }

    #[test]
    fn slogan_is_plain_when_colour_is_off_and_gold_or_dim_when_on() {
        let plain = slogan_style(ColorMode::Never, false, false);
        assert_eq!(plain.fg, Some(Color::Reset));
        assert!(!plain.add_modifier.contains(Modifier::DIM));
        let no_color = slogan_style(ColorMode::Auto, true, true);
        assert_eq!(no_color.fg, Some(Color::Reset));
        assert!(!no_color.add_modifier.contains(Modifier::DIM));
        let forced = slogan_style(ColorMode::Always, true, false);
        assert_eq!(forced.fg, Some(SLOGAN_GOLD));
        assert_eq!(
            slogan_style(ColorMode::Auto, false, false).fg,
            Some(SLOGAN_GOLD)
        );
        let dim = slogan_style(ColorMode::Auto, false, true);
        assert_eq!(dim.fg, Some(Color::Reset));
        assert!(dim.add_modifier.contains(Modifier::DIM));
    }
}
