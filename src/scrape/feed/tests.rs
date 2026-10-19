//! Unit tests for feed paging helpers.

use super::*;

#[test]
fn incremental_page_stop() {
    let known: HashSet<String> = ["a", "b"].into_iter().map(str::to_string).collect();
    let page = vec![json!({"id":"a"}), json!({"id":"b"})];
    assert!(page_all_known(&page, &known));
    let page2 = vec![json!({"id":"a"}), json!({"id":"c"})];
    assert!(!page_all_known(&page2, &known));
    assert!(!page_all_known(&[], &known));
}

#[test]
fn limit_keeps_the_newest_authored_items() {
    let posts = vec![json!({"id": "p1"}), json!({"id": "p2"}), json!({"id": "p3"})];
    let replies = vec![json!({"id": "p1"}), json!({"id": "r1"}), json!({"id": "p2"}), json!({"id": "r2"})];
    let (items, added) = window_feed_items(posts.clone(), Some(replies.clone()), Some(2), ScrapeOrder::Newest);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["p1", "r1"]);
    assert_eq!(added, 1);

    let (items, added) = window_feed_items(posts.clone(), Some(replies), None, ScrapeOrder::Newest);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["p1", "p2", "p3", "r1", "r2"]);
    assert_eq!(added, 2);

    let (items, added) = window_feed_items(posts, None, Some(2), ScrapeOrder::Newest);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["p1", "p2"]);
    assert_eq!(added, 0);
}

#[test]
fn a_short_replies_feed_is_filled_from_posts() {
    let posts = vec![json!({"id": "p1"}), json!({"id": "p2"})];
    let replies = vec![json!({"id": "r1"})];
    let (items, added) = window_feed_items(posts, Some(replies), Some(2), ScrapeOrder::Newest);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["r1", "p1"]);
    assert_eq!(added, 1);
}

#[test]
fn oldest_keeps_the_end_of_the_newest_first_feed() {
    let posts = vec![json!({"id": "p1"}), json!({"id": "p2"}), json!({"id": "p3"})];
    let replies = vec![json!({"id": "p1"}), json!({"id": "r1"}), json!({"id": "p2"}), json!({"id": "r2"})];
    let (items, added) = window_feed_items(posts.clone(), Some(replies.clone()), Some(2), ScrapeOrder::Oldest);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["r2", "p2"]);
    assert_eq!(added, 1);

    let (items, added) = window_feed_items(posts.clone(), Some(replies), None, ScrapeOrder::Oldest);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["r2", "r1", "p3", "p2", "p1"]);
    assert_eq!(added, 2);

    let (items, added) = window_feed_items(posts, None, Some(2), ScrapeOrder::Oldest);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["p3", "p2"]);
    assert_eq!(added, 0);
}

#[test]
fn oldest_short_replies_fill_from_the_tail_of_posts() {
    let posts = vec![json!({"id": "p1"}), json!({"id": "p2"}), json!({"id": "p3"})];
    let replies = vec![json!({"id": "r1"})];
    let (items, added) = window_feed_items(posts, Some(replies), Some(2), ScrapeOrder::Oldest);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["p3", "r1"]);
    assert_eq!(added, 1);
}

#[test]
fn merge_dedupes_by_id() {
    let mut items = vec![json!({"id": "a"}), json!({"id": "b"})];
    let n = merge_feed_items(&mut items, vec![json!({"id": "b"}), json!({"id": "c", "thread_id": "x"}), json!({"id": ""}), json!({"id": "c"})]);
    assert_eq!(n, 1);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["a", "b", "c"]);
}
