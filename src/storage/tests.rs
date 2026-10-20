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
        media: Vec::new(), truncated: false, body_json: json.into(), score: None,
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
fn a_missing_score_does_not_wipe_one_already_stored() {
    let mut c = Connection::open_in_memory().unwrap();
    let mut p = Post {
        post_id: "T1".into(), parent_post_id: String::new(), in_reply_to_id: String::new(),
        nick: "a".into(), title: "t".into(), content: "x".into(), posted_at: "1".into(),
        media: Vec::new(), truncated: false, body_json: String::new(), score: Some(7),
    };
    crate::storage::db::save_posts(&mut c, &[p.clone()]).unwrap();
    p.score = None;
    p.content = "x".into();
    crate::storage::db::save_posts(&mut c, &[p.clone()]).unwrap();
    let score: Option<i64> = c.query_row("SELECT score FROM posts WHERE post_id='T1'", [], |r| r.get(0)).unwrap();
    assert_eq!(score, Some(7));
    p.score = Some(2);
    crate::storage::db::save_posts(&mut c, &[p]).unwrap();
    let score: Option<i64> = c.query_row("SELECT score FROM posts WHERE post_id='T1'", [], |r| r.get(0)).unwrap();
    assert_eq!(score, Some(2));
}

#[test]
fn post_versions_round_trip_and_reject_an_unknown_source() {
    let mut c = Connection::open_in_memory().unwrap();
    let row = |hash: &str, at: &str, body: &str| PostVersion {
        post_id: "P1".into(), version_hash: hash.into(), body_json: body.into(),
        edited_at: at.into(), observed_at: "o1".into(), source: "site".into(),
    };
    crate::storage::db::save_post_versions(&mut c, &[row("b", "2026-02-01", "new"), row("a", "2026-01-01", "old")]).unwrap();
    let mut again = row("a", "2026-01-01", "old");
    again.observed_at = "o2".into();
    crate::storage::db::save_post_versions(&mut c, &[again]).unwrap();
    let loaded = crate::storage::db::load_post_versions(&c, "P1").unwrap();
    assert_eq!(loaded.iter().map(|v| v.version_hash.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(loaded[0].observed_at, "o2");
    assert_eq!(loaded[0].body_json, "old");
    let mut bad = row("c", "2026-03-01", "x");
    bad.source = "nope".into();
    assert!(crate::storage::db::save_post_versions(&mut c, &[bad]).is_err());
    assert!(c.execute(
        "INSERT INTO post_versions (post_id, version_hash, body_json, edited_at, observed_at, source) VALUES ('P1','z','','','','nope')",
        []).is_err());
    c.execute("INSERT INTO posts (post_id, parent_post_id, nick, title, content, posted_at) VALUES ('P1','','a','','','')", []).unwrap();
    c.execute("INSERT INTO posts (post_id, parent_post_id, nick, title, content, posted_at) VALUES ('R1','P1','b','','','')", []).unwrap();
    assert_eq!(crate::storage::db::thread_of_post(&c, "P1").unwrap().as_deref(), Some("P1"));
    assert_eq!(crate::storage::db::thread_of_post(&c, "R1").unwrap().as_deref(), Some("P1"));
    assert_eq!(crate::storage::db::thread_of_post(&c, "NO").unwrap(), None);
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

#[test]
fn site_subset_round_trips_and_old_rows_count_as_on_the_site() {
    let dir = std::env::temp_dir().join(format!("acp-on-site-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rs.database");
    let a = dir.join("a.jpg");
    let b = dir.join("b.jpg");
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();
    let c = Connection::open(&db).unwrap();
    c.execute_batch("CREATE TABLE attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL,
            url TEXT NOT NULL,
            local_path TEXT NOT NULL,
            mime_type TEXT NOT NULL
        );").unwrap();
    c.execute("INSERT INTO attachments (post_id, url, local_path, mime_type) VALUES ('P1', 'https://media.example/a.jpg', ?1, 'image/jpeg')", [a.to_string_lossy()]).unwrap();
    c.execute("INSERT INTO attachments (post_id, url, local_path, mime_type) VALUES ('P1', 'https://media.example/b.jpg', ?1, 'image/jpeg')", [b.to_string_lossy()]).unwrap();
    drop(c);
    use crate::tui::data::Store;
    let store = crate::tui::data::SqliteStore::new(db.clone());
    let got = store.attachments("P1").unwrap();
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|x| x.on_site));
    let c = Connection::open(&db).unwrap();
    assert!(!crate::storage::db::attachments_have_on_site(&c));
    drop(c);
    crate::storage::db::set_post_site_attachments(&db, "P1", &[a.clone()]).unwrap();
    let got = store.attachments("P1").unwrap();
    let on = |name: &str| got.iter().find(|x| x.local_path.ends_with(name)).unwrap().on_site;
    assert!(on("a.jpg"));
    assert!(!on("b.jpg"));
    let c = Connection::open(&db).unwrap();
    c.execute_batch("CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, post_id TEXT NOT NULL,
            parent_post_id TEXT NOT NULL, nick TEXT NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL, posted_at TEXT NOT NULL);
        INSERT INTO posts (post_id,parent_post_id,nick,title,content,posted_at) VALUES ('P1','','ada','t','c','1');").unwrap();
    drop(c);
    let (posts, atts, _) = crate::storage::db::posts_by_ids(&db, &["P1".into()]).unwrap();
    assert_eq!(atts.len(), 2);
    let doc = build_json("ada", &posts, &atts, &[], &[], &|x| x.local_path.display().to_string());
    let files = doc["posts"][0]["attachments"].as_array().unwrap();
    assert!(files.iter().any(|f| f["on_site"] == false));
    assert!(files.iter().any(|f| f["on_site"] == true));
    let json_path = dir.join("out.json");
    let body = serde_json::json!({
        "format": EXPORT_FORMAT,
        "format_version": EXPORT_FORMAT_VERSION,
        "posts": [{
            "post_id": "P1", "parent_post_id": "", "nick": "ada", "title": "t", "content": "c", "posted_at": "1",
            "attachments": [
                {"url": "https://media.example/a.jpg", "kind": "image", "mime_type": "image/jpeg", "local_path": "a.jpg", "on_site": false},
                {"url": "https://media.example/c.jpg", "kind": "image", "mime_type": "image/jpeg", "local_path": "b.jpg"}
            ]
        }]
    });
    std::fs::write(&json_path, body.to_string()).unwrap();
    let att_dir = dir.join("imported");
    let (_posts, imported, _, _) = import(&json_path, None, &att_dir, &dir).unwrap();
    let flag = |url: &str| imported.iter().find(|x| x.url == url).unwrap().on_site;
    assert!(!flag("https://media.example/a.jpg"));
    assert!(flag("https://media.example/c.jpg"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_audio_attachment_is_stored_viewed_and_exported() {
    let dir = std::env::temp_dir().join(format!("acp-audio-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rs.database");
    let audio = dir.join("talk.mp3");
    std::fs::write(&audio, b"ID3").unwrap();
    let att = Attachment {
        post_id: "P1".into(), url: "https://media.example/talk.mp3".into(), kind: "audio".into(),
        local_path: audio.clone(), mime_type: "audio/mpeg".into(), on_site: false,
    };
    crate::storage::db::save_attachments(&db, &[att]).unwrap();
    use crate::tui::data::Store;
    let store = crate::tui::data::SqliteStore::new(db.clone());
    let got = store.attachments("P1").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].mime_type, "audio/mpeg");
    assert_eq!(got[0].local_path, audio);
    assert!(!got[0].on_site);
    let c = Connection::open(&db).unwrap();
    c.execute_batch("CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, post_id TEXT NOT NULL,
            parent_post_id TEXT NOT NULL, nick TEXT NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL, posted_at TEXT NOT NULL);
        INSERT INTO posts (post_id,parent_post_id,nick,title,content,posted_at) VALUES ('P1','','ada','t','c','1');").unwrap();
    drop(c);
    let (posts, atts, _) = crate::storage::db::posts_by_ids(&db, &["P1".into()]).unwrap();
    assert_eq!(atts.len(), 1, "export still loads the audio file");
    assert_eq!(atts[0].mime_type, "audio/mpeg");
    let doc = build_json("ada", &posts, &atts, &[], &[], &|x| x.local_path.file_name().unwrap().to_string_lossy().into_owned());
    let files = doc["posts"][0]["attachments"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["mime_type"], "audio/mpeg");
    assert_eq!(files[0]["local_path"], "talk.mp3");
    assert_eq!(files[0]["on_site"], false);
    let json_path = dir.join("out.json");
    std::fs::write(&json_path, doc.to_string()).unwrap();
    let att_dir = dir.join("imported");
    let (_posts, imported, _, _) = import(&json_path, None, &att_dir, &dir).unwrap();
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].mime_type, "audio/mpeg");
    assert!(imported[0].local_path.ends_with("talk.mp3"));
    assert!(imported[0].local_path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
