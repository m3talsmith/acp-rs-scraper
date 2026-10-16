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
