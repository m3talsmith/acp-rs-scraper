//! Order and limit on lists and filters. The window is [`apply_order_limit`].

use super::*;
use crate::prefs::{ListOrderPrefs, save_list_order_at};
use crate::lists::apply_order_limit;
use crate::tui::commands::{Action, CommandId};
use crate::tui::data::SearchFilter;
use crate::tui::input::on_key;
use crate::tui::modals::{
    HelpModal, Modal, ModalResult, ModalState, SEARCH_LIMIT, SEARCH_ORDER, SearchModal,
};
use crate::tui::order::{arrange_nicks, arrange_posts, result_status};
use crate::tui::state::{Focus, Hit};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn ids(posts: &[Post]) -> Vec<&str> {
    posts.iter().map(|p| p.post_id.as_str()).collect()
}
fn key(a: &mut App, c: KeyCode) {
    on_key(a, KeyEvent::new(c, KeyModifiers::NONE));
}

fn render(a: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(140, 32)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..32)
        .map(|y| {
            (0..140)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn order_limit_keeps_the_head_or_the_tail() {
    let items = vec!["new", "mid", "old"];
    assert_eq!(
        apply_order_limit(items.clone(), Some(2), ScrapeOrder::Newest),
        ["new", "mid"]
    );
    assert_eq!(
        apply_order_limit(items.clone(), Some(2), ScrapeOrder::Oldest),
        ["old", "mid"]
    );
    assert_eq!(
        apply_order_limit(items.clone(), None, ScrapeOrder::Oldest),
        ["old", "mid", "new"]
    );
    assert_eq!(
        apply_order_limit(items, None, ScrapeOrder::Newest),
        ["new", "mid", "old"]
    );
}

#[test]
fn nested_orders_roots_and_keeps_replies_in_thread_order() {
    let posts = vec![
        post("root-new", "", "", "a", "2026-10-03T00:00:00Z", "new"),
        post("root-old", "", "", "a", "2026-10-01T00:00:00Z", "old"),
        post(
            "reply-late",
            "root-old",
            "root-old",
            "b",
            "2026-10-02T00:00:00Z",
            "late",
        ),
        post(
            "reply-early",
            "root-old",
            "root-old",
            "b",
            "2026-10-01T12:00:00Z",
            "early",
        ),
        post(
            "orphan",
            "missing",
            "missing",
            "a",
            "2026-10-02T12:00:00Z",
            "orphan",
        ),
    ];
    let nested = arrange_posts(&posts, ScrapeOrder::Newest, None, true, false, false);
    assert_eq!(
        ids(&nested),
        [
            "root-new",
            "orphan",
            "root-old",
            "reply-early",
            "reply-late"
        ]
    );
    let limited = arrange_posts(&posts, ScrapeOrder::Oldest, Some(1), true, false, false);
    assert_eq!(
        ids(&limited),
        ["root-old", "reply-early", "reply-late"],
        "a limit counts roots"
    );
    let flat = arrange_posts(&posts, ScrapeOrder::Newest, Some(2), false, false, false);
    assert_eq!(
        ids(&flat),
        ["root-new", "orphan"],
        "a flat limit counts cards"
    );
}

#[test]
fn site_order_is_not_resorted_by_time() {
    let posts = vec![
        post("older-but-first", "", "", "a", "2026-10-01T00:00:00Z", "a"),
        post("newer-but-second", "", "", "a", "2026-10-03T00:00:00Z", "b"),
    ];
    assert_eq!(
        ids(&arrange_posts(
            &posts,
            ScrapeOrder::Newest,
            None,
            false,
            true,
            false
        )),
        ["older-but-first", "newer-but-second"]
    );
    assert_eq!(
        ids(&arrange_posts(
            &posts,
            ScrapeOrder::Oldest,
            None,
            false,
            true,
            false
        )),
        ["newer-but-second", "older-but-first"]
    );
    let edits = arrange_posts(&posts, ScrapeOrder::Oldest, Some(1), false, false, true);
    assert_eq!(
        ids(&edits),
        ["older-but-first", "newer-but-second"],
        "edit history stays in version order"
    );
}

#[test]
fn o_toggles_the_open_list_and_the_header_names_it() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    assert_eq!(
        ids(&a.view().posts),
        ["ALICE0002", "ALICE0003", "ALICE0001"]
    );
    let s = render(&mut a);
    assert!(s.contains("[Order: newest ▸]"), "{s}");
    assert!(s.contains("[View: Nested ▸]"), "{s}");
    key(&mut a, KeyCode::Char('o'));
    assert_eq!(
        ids(&a.view().posts),
        ["ALICE0001", "ALICE0003", "ALICE0002"]
    );
    assert!(a.status.back().unwrap().contains("Help: order"));
    assert!(render(&mut a).contains("[Order: oldest ▸]"));
    a.focus = Focus::Nicks;
    key(&mut a, KeyCode::Char('o'));
    assert_eq!(
        a.nicks.iter().map(|n| n.nick.as_str()).collect::<Vec<_>>(),
        ["bob", "alice"]
    );
    assert!(a.status.back().unwrap().contains("Help: nick-order"));
}

#[test]
fn search_limit_uses_the_same_window_and_says_so() {
    let mut a = app();
    a.set_view_mode(crate::tui::state::ViewMode::Stack);
    a.run_search(SearchFilter {
        order: ScrapeOrder::Oldest,
        limit: Some(2),
        ..SearchFilter::default()
    });
    assert_eq!(a.view().posts.len(), 2, "a flat limit counts cards");
    assert_eq!(a.view().source.len(), 5);
    assert_eq!(a.view().posts[0].post_id, "ALICE0001");
    let line = a.status.back().unwrap();
    assert_eq!(
        line,
        &result_status("search everything", 2, 5, ScrapeOrder::Oldest, true)
    );
    assert!(line.contains("2 of 5 posts · oldest"));
    a.set_view_mode(crate::tui::state::ViewMode::Nested);
    a.run_search(SearchFilter {
        order: ScrapeOrder::Oldest,
        limit: Some(2),
        ..SearchFilter::default()
    });
    assert_eq!(
        ids(&a.view().posts),
        ["ALICE0001", "BOB000002", "BOB000001", "ALICE0003"],
        "a nested limit counts roots"
    );
    a.menu.menu.set_text(CommandId::Search, "query", "strike");
    a.menu.menu.set_choice(CommandId::Search, "order", "Oldest");
    a.menu.menu.set_text(CommandId::Search, "limit", "1");
    a.act(Action::RunSearch);
    assert_eq!(a.view().posts.len(), 1);
    assert!(a.status.back().unwrap().contains("1 of 2 posts · oldest"));
}

#[test]
fn nick_limit_keeps_the_chosen_end_after_the_filter() {
    let mut a = app();
    a.nick_limit = Some(1);
    a.nick_order = ScrapeOrder::Oldest;
    a.apply_nick_filter();
    assert_eq!(a.nicks.len(), 1);
    assert_eq!(a.nicks[0].nick, "bob", "oldest keeps the earliest nick");
    let arranged = arrange_nicks(a.all_nicks.clone(), ScrapeOrder::Newest, Some(1));
    assert_eq!(arranged[0].nick, "alice");
}

#[test]
fn help_opens_the_highlighted_topics() {
    let h = HelpModal::topic("search-limit");
    assert!(h.lines[h.scroll].contains("[[search-limit]]"));
    assert!(h.lines.iter().any(|l| l.contains("100 of 236")));
    let mut a = app();
    let search = a
        .menu
        .menu
        .items
        .iter()
        .position(|i| i.id == CommandId::Search)
        .unwrap();
    a.open_menu(search);
    let limit = a.menu.menu.items[search]
        .opts
        .iter()
        .position(|o| o.key == "limit")
        .unwrap();
    a.menu.opt = limit;
    key(&mut a, KeyCode::Char('?'));
    let Some(Modal::Help(h)) = &a.modal else {
        panic!("{:?}", a.modal)
    };
    assert!(h.lines[h.scroll].contains("[[search-limit]]"));
    let mut s = SearchModal::new(&SearchFilter::default());
    s.focus = SEARCH_ORDER;
    assert_eq!(
        s.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
        Some(ModalResult::Help("search-order"))
    );
    s.focus = SEARCH_LIMIT;
    assert_eq!(
        s.on_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
        Some(ModalResult::Help("search-limit"))
    );
}

#[test]
fn list_order_prefs_round_trip_without_touching_other_keys() {
    let path = std::env::temp_dir().join(format!(
        "acp-list-order-{}-{}.toml",
        std::process::id(),
        crate::tui::order::ListOrders::default().timeline
    ));
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, "email = \"me@example.test\"\n").unwrap();
    let prefs = ListOrderPrefs {
        timeline: Some("oldest".into()),
        nick_limit: Some(4),
        ..ListOrderPrefs::default()
    };
    save_list_order_at(&path, &prefs).unwrap();
    let loaded: crate::prefs::Prefs =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(loaded.email.as_deref(), Some("me@example.test"));
    let got = loaded.ui.unwrap().list_order.unwrap();
    assert_eq!(got.timeline.as_deref(), Some("oldest"));
    assert_eq!(got.nick_limit, Some(4));
    assert!(got.bookmarks.is_none());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn header_click_toggles_order() {
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    render(&mut a);
    let (r, _) = *a
        .hits
        .iter()
        .rev()
        .find(|(_, h)| *h == Hit::ListOrder)
        .expect("order control");
    crate::tui::input::on_mouse(
        &mut a,
        crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: r.x,
            row: r.y,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(a.orders.timeline, ScrapeOrder::Oldest);
}
