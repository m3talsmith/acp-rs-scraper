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

#[test]
fn clicking_a_hashtag_opens_its_tag_feed() {
    use crate::tui::input::on_mouse;
    use crate::tui::state::{Hit, LinkKind};
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{Terminal, backend::TestBackend};
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    let i = a.view().posts.iter().position(|p| p.post_id == "ALICE0001").unwrap();
    a.view_mut().posts[i].content = "Join the #strike today. ## Notes and #post-abc stay put.".into();
    a.open_post(i);
    let k = a.top().unwrap().links.iter().position(|l| l.kind == LinkKind::Tag && l.id == "strike").expect("tag link");
    assert_eq!(a.top().unwrap().links.iter().filter(|l| l.kind == LinkKind::Tag).count(), 1);
    let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, &mut a, now)).unwrap();
    let (r, _) = *a.hits.iter().rev().find(|(r, h)| *h == Hit::Link(k) && r.width < 16).unwrap_or_else(|| {
        let hits: Vec<_> = a.hits.iter().map(|(r, h)| (r.x, r.y, r.width, format!("{h:?}"))).collect();
        panic!("no word hit; hits={hits:?}");
    });
    on_mouse(&mut a, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    assert_eq!(a.pending_feed.as_deref(), Some("tag:strike"));
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--tag", "strike"]);
}

#[test]
fn s_cycles_reply_order_and_asks_the_site_for_oldest_and_top() {
    use crate::tui::input::on_key;
    use crate::tui::state::LinkKind;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut store = fixture();
    store.posts.push(post("ALICE0004", "ALICE0001", "ALICE0001", "carol", "2026-10-04T10:00:00Z", "later"));
    store.posts.push(post("ALICE0005", "ALICE0001", "ALICE0001", "dave", "2026-10-01T11:00:00Z", "earlier"));
    store.posts.iter_mut().find(|p| p.post_id == "ALICE0004").unwrap().score = Some(1);
    store.posts.iter_mut().find(|p| p.post_id == "ALICE0005").unwrap().score = Some(9);
    let mut a = App::new(Box::new(store), &no_args(), Theme::acp(), false);
    a.select_only(nick_idx(&a, "alice"));
    let i = a.view().posts.iter().position(|p| p.post_id == "ALICE0001").unwrap();
    a.open_post(i);
    let ids = |a: &App| a.top().unwrap().links.iter().filter(|l| l.kind == LinkKind::Reply).map(|l| l.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&a), vec!["ALICE0004".to_string(), "BOB000002".to_string(), "ALICE0005".to_string()]);
    on_key(&mut a, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert_eq!(ids(&a), vec!["ALICE0005".to_string(), "BOB000002".to_string(), "ALICE0004".to_string()]);
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--sort-thread", "ALICE0001", "--thread-sort", "oldest"]);
    a.effects.clear();
    a.job = None;
    on_key(&mut a, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert_eq!(ids(&a), vec!["ALICE0005".to_string(), "ALICE0004".to_string(), "BOB000002".to_string()]);
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--sort-thread", "ALICE0001", "--thread-sort", "top"]);
    let n = a.effects.len();
    a.job = None;
    on_key(&mut a, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert_eq!(ids(&a), vec!["ALICE0004".to_string(), "BOB000002".to_string(), "ALICE0005".to_string()]);
    assert_eq!(a.effects.len(), n, "newest is the site default and sends no hook");
}

#[test]
fn p_views_reposts_of_the_highlighted_post() {
    use crate::tui::input::on_key;
    use crate::tui::modals::Modal;
    use crate::tui::state::Focus;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    a.select_only(nick_idx(&a, "alice"));
    a.focus = Focus::Body;
    let id = a.view().posts[a.view().cursor].post_id.clone();
    on_key(&mut a, KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    assert!(matches!(a.modal, Some(Modal::Post(_))), "post menu");
    on_key(&mut a, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(a.pending_feed.as_deref(), Some(reposts_key(&id).as_str()));
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--reposts", id.as_str()]);
}

#[test]
fn p_views_edits_as_a_diff_of_the_open_post() {
    use crate::tui::input::on_key;
    use crate::tui::state::Focus;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut store = fixture();
    store.versions.push(PostVersion {
        post_id: "ALICE0001".into(), version_hash: "h1".into(), body_json: "alpha\nbeta".into(),
        edited_at: "2026-01-01T00:00:00Z".into(), observed_at: "o".into(), source: "site".into(),
    });
    store.versions.push(PostVersion {
        post_id: "ALICE0001".into(), version_hash: "h2".into(), body_json: "alpha\ngamma".into(),
        edited_at: "2026-02-01T00:00:00Z".into(), observed_at: "o".into(), source: "site".into(),
    });
    let mut a = App::new(Box::new(store), &no_args(), Theme::acp(), false);
    a.select_only(nick_idx(&a, "alice"));
    let i = a.view().posts.iter().position(|p| p.post_id == "ALICE0001").unwrap();
    a.open_post(i);
    a.focus = Focus::Body;
    on_key(&mut a, KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    on_key(&mut a, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    on_key(&mut a, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(a.pending_feed.as_deref(), Some(edits_key("ALICE0001", "ALICE0001").as_str()));
    assert_eq!(a.view().title, "Edits · ALICE0001");
    assert_eq!(a.view().posts.len(), 2);
    assert!(a.view().posts[1].content.contains("- beta\n+ gamma\n"), "{}", a.view().posts[1].content);
    let Some(Effect::CheckSessionThen(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--edits", "ALICE0001/ALICE0001"]);
}
