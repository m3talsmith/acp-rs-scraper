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
