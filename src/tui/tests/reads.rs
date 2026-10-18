//! Site reads in the TUI: a saved feed opens in site order, and the menu builds the CLI flag.

use super::*;
use crate::tui::commands::{Action, CommandId};
use crate::tui::jobs::JobKind;
use crate::tui::state::Effect;

#[test]
fn site_top_opens_the_saved_order_and_asks_the_site() {
    let mut store = fixture();
    store.feeds.push(SiteFeed {
        key: "top:week".into(),
        title: "Top · week".into(),
        fetched_at: "t".into(),
        post_ids: vec!["ALICE0001".into(), "BOB000001".into()],
    });
    let mut a = App::new(Box::new(store), &no_args(), Theme::acp(), false);
    a.act(Action::OpenTop);
    assert_eq!(
        a.view()
            .posts
            .iter()
            .map(|p| p.post_id.as_str())
            .collect::<Vec<_>>(),
        ["ALICE0001", "BOB000001"]
    );
    assert_eq!(a.view().title, "Top · week");
    assert_eq!(a.view().site_key.as_deref(), Some("top:week"));
    assert_eq!(a.pending_feed.as_deref(), Some("top:week"));
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else {
        panic!("{:?}", a.effects)
    };
    assert_eq!(argv[1..], ["--top", "week"]);
}

#[test]
fn site_top_period_and_replies_are_flags() {
    let mut a = app();
    a.menu.menu.set_choice(CommandId::View, "top_period", "all");
    a.menu.menu.set_check(CommandId::View, "top_replies", true);
    a.act(Action::OpenTop);
    assert_eq!(a.pending_feed.as_deref(), Some("top:all:replies"));
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else {
        panic!("{:?}", a.effects)
    };
    assert_eq!(argv[1..], ["--top", "all", "--top-replies"]);
    assert!(
        a.view().site_key.is_none(),
        "nothing saved yet, so the Body stays put"
    );
}

#[test]
fn a_finished_read_opens_what_the_job_stored() {
    let mut a = app();
    a.pending_feed = Some("top:day".into());
    a.job = Some(crate::tui::state::JobView {
        kind: JobKind::Read,
        pid: 3,
        log: "/tmp/read.log".into(),
    });
    // The store gains the feed the way the child process would have written it.
    // MemStore is already built; put the row in before the finish by swapping the store's feed through show.
    a.store = Box::new({
        let mut s = fixture();
        s.feeds.push(SiteFeed {
            key: "top:day".into(),
            title: "Top · day".into(),
            fetched_at: "t".into(),
            post_ids: vec!["BOB000001".into()],
        });
        s
    });
    a.job_finished(true, Some(0), &[]);
    assert!(a.pending_feed.is_none());
    assert_eq!(a.view().title, "Top · day");
    assert_eq!(a.view().posts[0].post_id, "BOB000001");
}

#[test]
fn eb_row_opens_the_saved_feed_and_asks_the_site() {
    use crate::tui::input::on_mouse;
    use crate::tui::state::Hit;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{Terminal, backend::TestBackend};
    let mut store = fixture();
    store.feeds.push(SiteFeed {
        key: "eb".into(), title: "EB".into(), fetched_at: "t".into(),
        post_ids: vec!["BOB000001".into(), "ALICE0001".into()],
    });
    let mut a = App::new(Box::new(store), &no_args(), Theme::acp(), false);
    let mut t = Terminal::new(TestBackend::new(120, 32)).unwrap();
    t.draw(|f| crate::tui::widgets::draw(f, &mut a, std::time::Instant::now())).unwrap();
    let (r, _) = *a.hits.iter().rev().find(|(_, h)| *h == Hit::Eb).expect("EB row");
    on_mouse(&mut a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    assert_eq!(a.view().posts.iter().map(|p| p.post_id.as_str()).collect::<Vec<_>>(), ["BOB000001", "ALICE0001"]);
    assert_eq!(a.view().site_key.as_deref(), Some("eb"));
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--eb"]);
}

#[test]
fn eb_replies_checkbox_is_a_flag() {
    let mut a = app();
    a.menu.menu.set_check(CommandId::View, "eb_replies", true);
    a.act(Action::OpenEb);
    assert_eq!(a.pending_feed.as_deref(), Some("eb:replies"));
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--eb", "--eb-replies"]);
}

#[test]
fn explore_latest_is_its_own_flag() {
    let mut a = app();
    a.menu.menu.set_check(CommandId::View, "explore_latest", true);
    a.act(Action::OpenExplore);
    assert_eq!(a.pending_feed.as_deref(), Some("explore:latest"));
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--explore-latest"]);
}

#[test]
fn site_search_uses_the_keyword_and_says_when_other_fields_stay_local() {
    use crate::tui::data::KindFilter;
    use crate::tui::modals::{Modal, ModalResult, ModalState, SearchModal};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut m = SearchModal::new(&Default::default());
    m.site = true;
    assert_eq!(m.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), None);
    assert!(m.error.as_deref().unwrap_or("").contains("keyword"));
    m.fields[0].value = "strike".into();
    m.fields[1].value = "bob".into();
    m.kind = KindFilter::Posts;
    m.error = None;
    let mut a = app();
    a.modal = Some(Modal::Search(m));
    a.close_modal(ModalResult::ClosedSubmitted);
    assert_eq!(a.pending_feed.as_deref(), Some("search:strike"));
    assert!(a.status.iter().any(|s| s.contains("stay local")));
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--site-search", "strike"]);
}
