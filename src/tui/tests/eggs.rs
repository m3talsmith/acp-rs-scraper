//! The four surprises: the star burst, the roar's phrase in search, and the credits.

use super::*;
use crate::eggs::{BURST_LINE, COSMOS_QUERY};
use crate::tui::data::SearchFilter;
use crate::tui::input::{on_key, on_mouse};
use crate::tui::modals::{AboutModal, ComposeFocus, ComposeModal, Modal, SearchModal};
use crate::tui::state::{Effect, Hit};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};

fn render(a: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| crate::tui::widgets::draw(f, a, Instant::now())).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }

fn konami(a: &mut App) {
    for c in [KeyCode::Up, KeyCode::Up, KeyCode::Down, KeyCode::Down, KeyCode::Left, KeyCode::Right, KeyCode::Left, KeyCode::Right, KeyCode::Char('b'), KeyCode::Char('a')] {
        key(a, c);
    }
}

fn click(a: &mut App, hit: Hit) {
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?}"));
    on_mouse(a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
}

fn credits_open(a: &App) -> bool {
    matches!(&a.modal, Some(Modal::About(m)) if m.credits)
}

#[test]
fn konami_plays_the_line_and_any_key_skips_it() {
    let mut a = app();
    konami(&mut a);
    assert!(a.egg.is_some());
    let s = render(&mut a, 80, 24);
    assert!(s.contains(BURST_LINE), "{s}");
    key(&mut a, KeyCode::Char('q'));
    assert!(a.egg.is_none());
    assert!(!a.effects.iter().any(|e| matches!(e, Effect::Quit { .. })), "the skipping q does not quit");
}

#[test]
fn konami_does_not_fire_while_typing() {
    let mut a = app();
    let mut compose = ComposeModal::new_post(vec![], vec![]);
    compose.focus = ComposeFocus::Title;
    a.modal = Some(Modal::Compose(compose));
    konami(&mut a);
    assert!(a.egg.is_none());

    let mut a = app();
    a.modal = Some(Modal::Search(SearchModal::new(&SearchFilter::default())));
    konami(&mut a);
    assert!(a.egg.is_none(), "the keyword row is a text field");

    let mut a = app();
    a.filter_editing = true;
    konami(&mut a);
    assert!(a.egg.is_none());

    let mut a = app();
    let mut compose = ComposeModal::new_post(vec![], vec![]);
    compose.focus = ComposeFocus::Cancel;
    a.modal = Some(Modal::Compose(compose));
    konami(&mut a);
    assert!(a.egg.is_some(), "Cancel is not a text field");
}

#[test]
fn the_phrase_flashes_the_starfield_then_the_results() {
    let mut a = app();
    let f = SearchFilter {
        keyword: format!("  {}  ", COSMOS_QUERY.to_uppercase()),
        ..SearchFilter::default()
    };
    a.run_search(f);
    assert!(a.egg.is_some());
    let flash = render(&mut a, 80, 24);
    assert!(flash.contains('*'), "{flash}");
    assert!(!flash.contains(BURST_LINE), "{flash}");
    key(&mut a, KeyCode::Esc);
    assert!(a.egg.is_none());
    let results = render(&mut a, 100, 30);
    assert!(results.to_lowercase().contains(COSMOS_QUERY), "{results}");

    let mut a = app();
    let other = SearchFilter { keyword: "strike".into(), ..SearchFilter::default() };
    a.run_search(other);
    assert!(a.egg.is_none());
    assert!(!render(&mut a, 80, 24).contains(BURST_LINE));
}

#[test]
fn tilde_and_five_stars_open_the_credits() {
    let mut a = app();
    key(&mut a, KeyCode::Char('~'));
    let s = render(&mut a, 100, 30);
    for needle in ["Neon Lust", "Haz al Din (@hazaldin)", "https://www.youtube.com/@IGG-QZBSH", "Contributors", "For the working class,", "~ @m3talsmith"] {
        assert!(s.contains(needle), "{needle}\n{s}");
    }

    let mut a = app();
    let _ = render(&mut a, 100, 30);
    for _ in 0..4 { click(&mut a, Hit::Logo); }
    assert!(!credits_open(&a));
    click(&mut a, Hit::Logo);
    assert!(credits_open(&a));

    let mut a = app();
    let _ = render(&mut a, 100, 30);
    click(&mut a, Hit::Logo);
    a.logo_tap_at = Some(Instant::now() - Duration::from_secs(5));
    for _ in 0..4 { click(&mut a, Hit::Logo); }
    assert!(!credits_open(&a), "a long gap starts the count over");

    let mut a = app();
    a.act(crate::tui::commands::Action::About);
    assert!(matches!(&a.modal, Some(Modal::About(AboutModal { credits: false, .. }))));
    for _ in 0..4 { key(&mut a, KeyCode::Enter); }
    assert!(!credits_open(&a));
    key(&mut a, KeyCode::Char(' '));
    let s = render(&mut a, 100, 30);
    assert!(s.contains("Neon Lust") && s.contains("~ @m3talsmith"), "{s}");
}

#[test]
fn plain_and_small_degrade() {
    let mut a = app();
    konami(&mut a);
    let play = a.egg.as_mut().unwrap();
    play.plain = true;
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| crate::tui::widgets::draw(f, &mut a, Instant::now())).unwrap();
    let buf = term.backend().buffer().clone();
    let coloured = buf.content().iter().any(|c| c.fg != ratatui::style::Color::Reset && c.symbol() != " ");
    assert!(!coloured, "plain draws no forced colour");

    let mut a = app();
    konami(&mut a);
    let s = render(&mut a, 40, 12);
    assert!(s.contains('*') || s.contains(BURST_LINE), "{s}");
}

#[test]
fn public_pages_do_not_tell() {
    let readme = include_str!("../../../README.md");
    let usage = include_str!("../../../docs/USAGE.md");
    let notes = include_str!("../../../CHANGELOG.md");
    let plan = include_str!("../../../architecture/PLAN-3.0-CLIENT.md");
    for (name, doc) in [("readme", readme), ("usage", usage), ("changelog", notes)] {
        assert!(!doc.contains("--roar"), "{name}");
        assert!(!doc.contains(BURST_LINE), "{name}");
        assert!(!doc.contains("Neon Lust"), "{name}");
    }
    assert!(notes.contains("A few surprises for the curious."));
    assert!(plan.contains("3.30.0") && !plan.contains(BURST_LINE) && !plan.contains("--roar"));
    let help = crate::tui::modals::help_text().join("\n");
    assert!(!help.contains("--roar") && !help.contains(BURST_LINE) && !help.contains("Neon Lust"));
    let secret = include_str!("../../../docs/.secrets/EASTER_EGGS.md");
    assert!(secret.contains(BURST_LINE) && secret.contains("--roar") && secret.contains(COSMOS_QUERY) && secret.contains("Neon Lust"));
}
