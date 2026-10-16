//! Unit tests for bookmark storage and sync planning.

use super::*;

fn bm(id: &str, remote: bool, deleted: bool) -> Bookmark { Bookmark { post_id: id.into(), created_at: "t".into(), remote, deleted } }
fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

#[test]
fn plan_covers_every_case() {
    let local = [bm("LOCAL", false, false), bm("BOTH", true, false), bm("SEEN", false, false),
                 bm("GONE", true, false), bm("UNMARK", true, true), bm("STALE", true, true)];
    let p = plan_sync(&local, &ids(&["BOTH", "SEEN", "UNMARK", "NEW"]), true);
    assert_eq!(p.push_add, ids(&["LOCAL"]));
    assert_eq!(p.push_remove, ids(&["UNMARK"]));
    assert_eq!(p.set_remote, ids(&["SEEN"]));
    assert_eq!(p.insert_remote, ids(&["NEW"]));
    assert_eq!(p.delete_rows, ids(&["GONE", "STALE"]));
    let partial = plan_sync(&local, &ids(&["BOTH"]), false);
    assert!(!partial.delete_rows.contains(&"GONE".to_string()), "an incomplete site list never deletes");
}

#[test]
fn local_add_remove_round_trip() {
    let d = std::env::temp_dir().join(format!("acp-bm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let c = bookmarks_conn(&d.join("rs.database")).unwrap();
    assert_eq!(add_bookmarks(&c, &ids(&["A1", "B1"]), "2026-10-16T10:00:00").unwrap(), 2);
    assert_eq!(add_bookmarks(&c, &ids(&["A1"]), "later").unwrap(), 0, "already there");
    c.execute("UPDATE bookmarks SET remote = 1 WHERE post_id = 'B1'", []).unwrap();
    assert_eq!(remove_bookmarks(&c, &ids(&["A1", "B1"])).unwrap(), 2);
    let v = load_bookmarks(&c).unwrap();
    assert_eq!(v, vec![Bookmark { post_id: "B1".into(), created_at: "2026-10-16T10:00:00".into(), remote: true, deleted: true }]);
    assert_eq!(add_bookmarks(&c, &ids(&["B1"]), "x").unwrap(), 1, "re-adding undoes the pending removal");
    assert!(!load_bookmarks(&c).unwrap()[0].deleted);
    assert!(add_bookmarks(&c, &ids(&["bad id"]), "x").is_err());
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn post_id_lists() {
    assert_eq!(parse_post_ids(" A1, post-B2 ,A1").unwrap(), ["A1", "B2"]);
    assert!(parse_post_ids(",").is_err());
    assert!(parse_post_ids("a/b").is_err());
}

#[test]
fn feed_items_become_posts() {
    let r = feed_item_post(&json!({"id": "R1", "thread_id": "A1", "author": {"username": "bob"}, "body": "hi", "inserted_at": "t"}));
    assert_eq!((r.parent_post_id.as_str(), r.nick.as_str()), ("A1", "bob"));
    let a = feed_item_post(&json!({"id": "A1", "thread_id": "A1", "author": {"username": "alice"}, "inserted_at": "t"}));
    assert!(a.parent_post_id.is_empty());
}
