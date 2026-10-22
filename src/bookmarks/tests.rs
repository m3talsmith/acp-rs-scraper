//! Unit tests for the local bookmark store.

use super::*;

fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

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
