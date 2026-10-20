//! The nick-bar filter: matching, keys, the clear button, selection on the filtered list.

use super::*;
use crate::tui::data::{NickEntry, filter_nicks};
use crate::tui::input::{on_key, on_mouse};
use crate::tui::state::{Focus, Hit, ViewMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};

fn render(a: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 32)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..32).map(|y| (0..120).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}
fn click(a: &mut App, hit: Hit, mods: KeyModifiers) {
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == hit).unwrap_or_else(|| panic!("no {hit:?} drawn"));
    on_mouse(a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: mods });
}
fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }
fn typ(a: &mut App, s: &str) { for c in s.chars() { key(a, KeyCode::Char(c)); } }
fn shown(a: &App) -> Vec<&str> { a.nicks.iter().map(|n| n.nick.as_str()).collect() }
fn entry(n: &str, d: Option<&str>) -> NickEntry { NickEntry { nick: n.into(), display_name: d.map(String::from), post_count: 1, latest: None } }

#[test]
fn matching_is_case_insensitive_substring_first_then_fuzzy() {
    let all = [entry("Hazaldin", None), entry("zed", Some("Haz Example")), entry("hzdx", None), entry("bob", None)];
    let names = |q: &str| filter_nicks(&all, q).into_iter().map(|n| n.nick).collect::<Vec<_>>();
    assert_eq!(names("HAZ"), ["Hazaldin", "zed"], "nick or display name, any case");
    assert_eq!(names("hzd"), ["hzdx", "Hazaldin"], "substring before fuzzy");
    assert_eq!(names("@bob"), ["bob"]);
    assert_eq!(names("  ").len(), 4);
    assert!(names("qqq").is_empty());
}

#[test]
fn slash_types_into_the_filter_and_shows_the_count() {
    let mut a = app();
    a.focus = Focus::Body;
    key(&mut a, KeyCode::Char('/'));
    assert!(a.filter_editing);
    assert_eq!(a.focus, Focus::Nicks);
    typ(&mut a, "BO");
    assert_eq!(shown(&a), ["bob"]);
    assert!(!a.show_log && a.view_mode == ViewMode::Nested, "letters went to the filter, not to l / v");
    let s = render(&mut a);
    assert!(s.contains("1/2 x"), "{s}");
    assert!(s.contains("★ Bookmarks (1)"), "the bookmarks row stays");
    key(&mut a, KeyCode::Enter);
    assert!(!a.filter_editing);
    key(&mut a, KeyCode::Enter);
    assert_eq!(a.selected, ["bob"], "Enter selects on the filtered list");
}

#[test]
fn esc_and_the_x_clear_it() {
    let mut a = app();
    a.focus_nick_filter();
    typ(&mut a, "ali");
    key(&mut a, KeyCode::Esc);
    assert!(!a.filter_editing && a.nicks.len() == 2);
    a.focus_nick_filter();
    typ(&mut a, "ali");
    key(&mut a, KeyCode::Down);
    assert_eq!(shown(&a), ["alice"]);
    key(&mut a, KeyCode::Esc);
    assert_eq!(a.nicks.len(), 2, "Esc in the list clears an active filter");
    a.focus_nick_filter();
    typ(&mut a, "ali");
    render(&mut a);
    click(&mut a, Hit::NickFilterClear, KeyModifiers::NONE);
    assert_eq!(a.nicks.len(), 2);
    render(&mut a);
    click(&mut a, Hit::NickFilter, KeyModifiers::NONE);
    assert!(a.filter_editing);
}

#[test]
fn multi_select_works_on_the_filtered_list_and_keeps_hidden_picks() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus_nick_filter();
    typ(&mut a, "bob");
    key(&mut a, KeyCode::Enter);
    render(&mut a);
    click(&mut a, Hit::Nick(0), KeyModifiers::CONTROL);
    assert_eq!(a.selected, ["alice", "bob"], "alice stays selected while hidden");
    assert_eq!(a.view().posts.len(), 5);
    a.selected.clear();
    a.select_all();
    assert_eq!(a.selected, ["bob"], "Ctrl-A selects the shown nicks");
    a.refresh_nicks();
    assert_eq!(shown(&a), ["bob"], "a refresh keeps the filter");
}

#[test]
fn filter_survives_view_mode_switches_and_search() {
    let mut a = app();
    a.focus_nick_filter();
    typ(&mut a, "bob");
    key(&mut a, KeyCode::Enter);
    for _ in 0..ViewMode::ALL.len() { key(&mut a, KeyCode::Char('v')); render(&mut a); }
    assert_eq!(a.nick_filter.value, "bob");
    a.open_bookmarks();
    a.search_back();
    assert_eq!(shown(&a), ["bob"]);
    let s = render(&mut a);
    assert!(s.contains("Nicks (1/2)"));
}

#[test]
fn no_match_has_a_hint() {
    let mut a = app();
    a.focus_nick_filter();
    typ(&mut a, "zzz");
    assert!(render(&mut a).contains("No nick matches"));
}
