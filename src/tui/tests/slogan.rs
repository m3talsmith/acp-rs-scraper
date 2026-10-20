//! Where the slogan is drawn, and that a narrow footer leaves it off.

use super::*;
use crate::branding::{SLOGAN, SLOGAN_COLS, SLOGAN_GOLD};
use crate::cli::ColorMode;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use std::time::Instant;

fn buffer(a: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    t.backend().buffer().clone()
}

fn row(buf: &ratatui::buffer::Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

/// `(x, y)` of every slogan, left to right, top to bottom.
fn slogans(buf: &ratatui::buffer::Buffer) -> Vec<(u16, u16)> {
    let needle: Vec<char> = SLOGAN.chars().collect();
    let n = needle.len() as u16;
    let mut at = Vec::new();
    for y in 0..buf.area.height {
        let mut x = 0;
        while x + n <= buf.area.width {
            let hit = needle
                .iter()
                .enumerate()
                .all(|(i, ch)| buf[(x + i as u16, y)].symbol() == ch.to_string());
            if hit {
                at.push((x, y));
                x += n;
            } else {
                x += 1;
            }
        }
    }
    at
}

fn plain(st: ratatui::style::Style) -> bool {
    matches!(st.fg, None | Some(Color::Reset)) && !st.add_modifier.contains(Modifier::DIM)
}

fn style_at(buf: &ratatui::buffer::Buffer, x: u16, y: u16) -> ratatui::style::Style {
    buf[(x, y)].style()
}

#[test]
fn welcome_and_empty_database_put_the_slogan_under_the_title() {
    let mut a = app();
    let buf = buffer(&mut a, 110, 30);
    let hits = slogans(&buf);
    assert_eq!(
        hits.len(),
        1,
        "the 110-column footer does not also carry it\n{}",
        row(&buf, 29)
    );
    let (x, y) = hits[0];
    assert!(
        row(&buf, y - 1).contains(TUI_SUBTITLE),
        "title sits above the slogan:\n{}",
        row(&buf, y - 1)
    );
    assert!(
        row(&buf, y + 2).contains("Click a nick"),
        "the hint stays under it"
    );
    assert_eq!(style_at(&buf, x, y).fg, Some(SLOGAN_GOLD));
    assert!(
        !row(&buf, 29).contains(SLOGAN),
        "default hint is not covered"
    );
    assert!(row(&buf, 29).contains("q quit"));

    let mut empty = App::new(
        Box::new(crate::tui::data::MemStore::default()),
        &no_args(),
        Theme::acp(),
        false,
    );
    let buf = buffer(&mut empty, 100, 24);
    let hits = slogans(&buf);
    assert_eq!(hits.len(), 1);
    let (_, y) = hits[0];
    assert!(row(&buf, y - 1).contains(TUI_SUBTITLE));
    assert!(row(&buf, y + 2).contains("database is empty"));
}

#[test]
fn an_empty_search_does_not_repeat_the_welcome_line() {
    let mut a = app();
    a.run_search(crate::tui::data::SearchFilter {
        keyword: "no-such-post".into(),
        ..Default::default()
    });
    let status = a.status.back().cloned().unwrap_or_default();
    let buf = buffer(&mut a, 110, 30);
    let body = slogans(&buf).iter().any(|&(_, y)| y + 1 != buf.area.height);
    assert!(!body, "an empty search is not the welcome screen");
    assert!((0..buf.area.height).any(|y| row(&buf, y).contains("No posts match")));
    let footer = row(&buf, buf.area.height - 1);
    assert!(
        footer.contains(&status) && footer.contains(SLOGAN),
        "a short status line still fits beside it:\n{footer}"
    );
}

#[test]
fn login_shows_the_title_then_the_slogan() {
    let mut a = app();
    a.open_login(None);
    let buf = buffer(&mut a, 110, 30);
    let hits = slogans(&buf);
    assert_eq!(
        hits.len(),
        1,
        "{:?}",
        hits.iter().map(|&(_, y)| row(&buf, y)).collect::<Vec<_>>()
    );
    let (x, y) = hits[0];
    assert!(row(&buf, y - 1).contains(TUI_SUBTITLE));
    assert!(
        row(&buf, y).contains("Log in")
            || (0..buf.area.height).any(|r| row(&buf, r).contains("Log in to acp.us"))
    );
    assert_eq!(style_at(&buf, x, y).fg, Some(SLOGAN_GOLD));
}

#[test]
fn about_header_is_the_slogan() {
    let mut a = app();
    a.open_about();
    let buf = buffer(&mut a, 110, 30);
    let hits = slogans(&buf);
    let header = hits.iter().copied().find(|&(x, y)| {
        row(&buf, y)
            .chars()
            .any(|c| matches!(c, '═' | '╔' | '╗' | '║' | '─' | '┌' | '┐' | '│'))
            && x > 0
    });
    let Some((x, y)) = header else {
        panic!(
            "slogan is not the About header: {hits:?}\n{}",
            (0..buf.area.height)
                .map(|y| row(&buf, y))
                .collect::<Vec<_>>()
                .join("\n")
        );
    };
    assert!(
        (y + 1..buf.area.height).any(|r| row(&buf, r).contains(TUI_SUBTITLE)),
        "the product name is in the modal"
    );
    assert!(
        (y + 1..buf.area.height).any(|r| row(&buf, r).contains(env!("CARGO_PKG_VERSION"))),
        "the version is in the modal"
    );
    assert_eq!(style_at(&buf, x, y).fg, Some(SLOGAN_GOLD));
    assert!(matches!(a.modal, Some(crate::tui::modals::Modal::About(_))));
}

#[test]
fn the_footer_slogan_is_right_aligned_only_when_the_status_line_still_fits() {
    let mut a = app();
    let narrow = buffer(&mut a, 140, 30);
    assert!(
        slogans(&narrow)
            .iter()
            .all(|&(_, y)| y + 1 != narrow.area.height),
        "140 still covers the default hint"
    );
    assert!(row(&narrow, narrow.area.height - 1).contains("? help"));
    assert!(row(&narrow, narrow.area.height - 1).contains("q quit"));

    let wide = buffer(&mut a, 160, 30);
    let footer = row(&wide, wide.area.height - 1);
    assert!(
        footer.contains("? help") && footer.contains("q quit"),
        "{footer}"
    );
    let hits = slogans(&wide);
    let (x, y) = *hits
        .iter()
        .find(|(_, y)| *y + 1 == wide.area.height)
        .expect("slogan on the status row");
    assert_eq!(x + SLOGAN_COLS, wide.area.width, "right-aligned");
    assert!(style_at(&wide, x, y).add_modifier.contains(Modifier::DIM));
    assert_ne!(style_at(&wide, x, y).fg, Some(SLOGAN_GOLD));

    a.status.clear();
    a.push_status("scan finished");
    let buf = buffer(&mut a, 80, 30);
    let (x, y) = *slogans(&buf)
        .iter()
        .find(|(_, y)| *y + 1 == buf.area.height)
        .expect("short status fits at 80");
    assert_eq!(x + SLOGAN_COLS, 80);
    assert!(row(&buf, y).contains("scan finished"));

    a.status.clear();
    a.push_status("x".repeat(50));
    let buf = buffer(&mut a, 80, 30);
    assert!(
        slogans(&buf).iter().all(|&(_, y)| y + 1 != buf.area.height),
        "a 50-column line leaves no room at 80"
    );
    assert!(row(&buf, buf.area.height - 1).contains(&"x".repeat(50)));
}

#[test]
fn colour_off_draws_the_slogan_as_plain_text() {
    let mut a = app();
    a.color_mode = ColorMode::Never;
    let buf = buffer(&mut a, 110, 30);
    let (x, y) = slogans(&buf)[0];
    assert!(
        plain(style_at(&buf, x, y)),
        "welcome slogan is plain under --color never"
    );

    let wide = buffer(&mut a, 160, 30);
    let (x, y) = *slogans(&wide)
        .iter()
        .find(|(_, y)| *y + 1 == wide.area.height)
        .unwrap();
    assert!(
        plain(style_at(&wide, x, y)),
        "footer slogan is plain, not dim: {:?}",
        style_at(&wide, x, y)
    );

    a.color_mode = ColorMode::Auto;
    a.no_color = true;
    a.open_about();
    let buf = buffer(&mut a, 160, 30);
    let (x, y) = slogans(&buf)
        .into_iter()
        .find(|&(_, y)| {
            row(&buf, y)
                .chars()
                .any(|c| matches!(c, '═' | '╔' | '╗' | '║'))
        })
        .expect("About header");
    assert!(
        plain(style_at(&buf, x, y)),
        "About header is plain: {:?}",
        style_at(&buf, x, y)
    );
}
