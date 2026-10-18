//! Top periods, feed order and the configure-feed query.

use super::*;

#[test]
fn periods_are_the_five_site_windows() {
    assert_eq!(
        TopPeriod::ALL.map(TopPeriod::as_str),
        ["week", "day", "month", "year", "all"]
    );
    assert_eq!(TopPeriod::parse("Week").unwrap(), TopPeriod::Week);
    assert_eq!(TopPeriod::parse(" all ").unwrap(), TopPeriod::All);
    assert!(TopPeriod::parse("hour").unwrap_err().contains("day, week"));
}

#[test]
fn top_urls_and_fields() {
    assert_eq!(
        feed_top_url(TopPeriod::Week, false),
        format!("{}/rs/top?t=week", base_url())
    );
    assert_eq!(
        feed_top_url(TopPeriod::All, true),
        format!("{}/rs/top?t=all&replies=1", base_url())
    );
    assert_eq!(feed_top_page_url(), format!("{}/rs/top", base_url()));
    assert_eq!(top_fields(TopPeriod::Day, false), [("t", "day")]);
    assert_eq!(
        top_fields(TopPeriod::Month, true),
        [("t", "month"), ("replies", "1")]
    );
    assert_eq!(
        form_query(&top_fields(TopPeriod::Year, true)),
        "t=year&replies=1"
    );
    assert_eq!(
        (
            top_key(TopPeriod::Week, false),
            top_title(TopPeriod::Week, true)
        ),
        ("top:week".into(), "Top · week · posts and replies".into())
    );
}

#[test]
fn feed_posts_keep_site_order_and_drop_duplicate_ids() {
    let items = vec![
        json!({"id": "B", "thread_id": "B", "author": {"username": "bob"}, "inserted_at": "2", "title": "second"}),
        json!({"id": "A", "thread_id": "A", "author": {"username": "ann"}, "inserted_at": "9", "title": "first"}),
        json!({"id": "B", "thread_id": "B", "author": {"username": "bob"}, "inserted_at": "2"}),
        json!({"id": "", "author": {"username": "x"}}),
    ];
    let posts = feed_posts(&items);
    assert_eq!(
        posts.iter().map(|p| p.post_id.as_str()).collect::<Vec<_>>(),
        ["B", "A"]
    );
    assert_eq!(posts[0].nick, "bob");
}

#[test]
fn site_feed_round_trip_keeps_order() {
    let c = Connection::open_in_memory().unwrap();
    let feed = SiteFeed {
        key: "top:week".into(),
        title: "Top · week".into(),
        fetched_at: "t".into(),
        post_ids: vec!["B".into(), "A".into()],
    };
    save_site_feed(&c, &feed).unwrap();
    save_site_feed(
        &c,
        &SiteFeed {
            title: "Top · week · again".into(),
            ..feed.clone()
        },
    )
    .unwrap();
    assert_eq!(
        load_site_feed(&c, "top:week").unwrap().unwrap().title,
        "Top · week · again"
    );
    assert_eq!(
        load_site_feed(&c, "top:week").unwrap().unwrap().post_ids,
        ["B", "A"]
    );
    assert!(load_site_feed(&c, "missing").unwrap().is_none());
}

#[test]
fn eb_urls_keys_and_fields() {
    assert_eq!(feed_eb_url(false), format!("{}/rs/eb", base_url()));
    assert_eq!(feed_eb_url(true), format!("{}/rs/eb?replies=1", base_url()));
    assert_eq!(feed_eb_page_url(), format!("{}/rs/eb", base_url()));
    assert!(eb_fields(false).is_empty());
    assert_eq!(eb_fields(true), [("replies", "1")]);
    assert_eq!(form_query(&eb_fields(true)), "replies=1");
    assert_eq!((eb_key(false), eb_key(true)), ("eb".into(), "eb:replies".into()));
    assert_eq!(eb_title(true), "EB · posts and replies");
}

#[test]
fn explore_and_site_search_urls() {
    assert_eq!(feed_explore_url(false), format!("{}/rs/explore", base_url()));
    assert_eq!(feed_explore_url(true), format!("{}/rs/explore/latest", base_url()));
    assert_eq!((explore_key(false), explore_key(true)), ("explore".into(), "explore:latest".into()));
    assert_eq!(explore_title(true), "Explore · latest");
    assert_eq!(site_search_url("a b"), format!("{}/rs/explore?q=a+b", base_url()));
    assert_eq!(search_key("  strike "), "search:strike");
    assert_eq!(search_title("strike"), "Site search · strike");
}
