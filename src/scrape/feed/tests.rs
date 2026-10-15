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
fn merge_dedupes_by_id() {
    let mut items = vec![json!({"id": "a"}), json!({"id": "b"})];
    let n = merge_feed_items(&mut items, vec![json!({"id": "b"}), json!({"id": "c", "thread_id": "x"}), json!({"id": ""}), json!({"id": "c"})]);
    assert_eq!(n, 1);
    let ids: Vec<_> = items.iter().map(|x| s(x, "id")).collect();
    assert_eq!(ids, ["a", "b", "c"]);
}
