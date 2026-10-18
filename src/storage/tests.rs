//! Unit tests for the posts schema upgrade.

use crate::prelude::*;

#[test]
fn old_posts_table_gains_in_reply_to_id() {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, post_id TEXT NOT NULL,
            parent_post_id TEXT NOT NULL, nick TEXT NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL,
            posted_at TEXT NOT NULL);
        INSERT INTO posts (post_id,parent_post_id,nick,title,content,posted_at) VALUES
            ('T1','','a','t','x',''), ('R1','T1','b','','y','');").unwrap();
    crate::storage::db::ensure_posts_schema(&c).unwrap();
    crate::storage::db::ensure_posts_schema(&c).unwrap(); // idempotent
    let get = |id: &str| c.query_row("SELECT in_reply_to_id FROM posts WHERE post_id=?1", [id], |r| r.get::<_, String>(0)).unwrap();
    assert_eq!((get("T1"), get("R1")), (String::new(), "T1".to_string()));
    let idx: i64 = c.query_row("SELECT count(*) FROM sqlite_master WHERE name='posts_in_reply_to_idx'", [], |r| r.get(0)).unwrap();
    assert_eq!(idx, 1);
}

#[test]
fn old_posts_table_gains_body_state_unknown() {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, post_id TEXT NOT NULL,
            parent_post_id TEXT NOT NULL, nick TEXT NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL,
            posted_at TEXT NOT NULL);
        INSERT INTO posts (post_id,parent_post_id,nick,title,content,posted_at) VALUES ('T1','','a','t','x','');").unwrap();
    crate::storage::db::ensure_posts_schema(&c).unwrap();
    crate::storage::db::ensure_posts_schema(&c).unwrap();
    let st: String = c.query_row("SELECT body_state FROM posts WHERE post_id='T1'", [], |r| r.get(0)).unwrap();
    assert_eq!(st, "unknown");
}

#[test]
fn a_later_empty_body_does_not_wipe_json_or_tags() {
    let mut c = Connection::open_in_memory().unwrap();
    let json = r##"{"type":"doc","content":[{"type":"mention","attrs":{"mentionSuggestionChar":"#","label":"#Strike"}}]}"##;
    let p = Post {
        post_id: "T1".into(), parent_post_id: String::new(), in_reply_to_id: String::new(),
        nick: "a".into(), title: "t".into(), content: "see #Strike".into(), posted_at: "1".into(),
        media: Vec::new(), truncated: false, body_json: json.into(),
    };
    crate::storage::db::save_posts(&mut c, &[p.clone()]).unwrap();
    let stored: String = c.query_row("SELECT body_json FROM posts WHERE post_id='T1'", [], |r| r.get(0)).unwrap();
    assert_eq!(stored, json);
    let tags_of = |c: &Connection| {
        let mut st = c.prepare("SELECT tag FROM post_tags WHERE post_id='T1' ORDER BY tag").unwrap();
        st.query_map([], |r| r.get::<_, String>(0)).unwrap().map(|r| r.unwrap()).collect::<Vec<_>>()
    };
    assert_eq!(tags_of(&c), ["Strike"]);
    let mut blank = p.clone();
    blank.body_json.clear();
    blank.content = "no hash".into();
    crate::storage::db::save_posts(&mut c, &[blank]).unwrap();
    let stored: String = c.query_row("SELECT body_json FROM posts WHERE post_id='T1'", [], |r| r.get(0)).unwrap();
    assert_eq!(stored, json);
    assert_eq!(tags_of(&c), ["Strike"]);
    let other = r##"{"type":"doc","content":[{"type":"mention","attrs":{"mentionSuggestionChar":"#","label":"#Other"}}]}"##;
    let mut next = p;
    next.body_json = other.into();
    next.content = "plain".into();
    crate::storage::db::save_posts(&mut c, &[next]).unwrap();
    assert_eq!(tags_of(&c), ["Other"]);
}

#[test]
fn posts_by_ids_keeps_the_order_and_reports_missing_ones() {
    let db = std::env::temp_dir().join(format!("acp-ids-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&db);
    let c = Connection::open(&db).unwrap();
    c.execute_batch("CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, post_id TEXT NOT NULL,
            parent_post_id TEXT NOT NULL, nick TEXT NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL,
            posted_at TEXT NOT NULL);
        INSERT INTO posts (post_id,parent_post_id,nick,title,content,posted_at) VALUES
            ('T1','','a','t','x','1'), ('R1','T1','b','','y','2'), ('T2','','a','','z','3');").unwrap();
    drop(c);
    let ids: Vec<String> = ["R1", "NOPE", "T1", "R1"].iter().map(|s| s.to_string()).collect();
    let (posts, atts, missing) = crate::storage::db::posts_by_ids(&db, &ids).unwrap();
    assert_eq!(posts.iter().map(|p| p.post_id.as_str()).collect::<Vec<_>>(), ["R1", "T1"]);
    assert!(atts.is_empty());
    assert_eq!(missing, ["NOPE"]);
    std::fs::remove_file(db).ok();
}
