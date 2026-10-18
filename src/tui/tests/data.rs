use super::*;
use crate::tui::data::*;

#[test]
fn linked_ids_from_urls_and_anchors() {
    let t = "see https://acp.us/rs/post/ROOT0001/REPLY0001 and /rs/post/ROOT0002, #post-ANCHOR01, /rs/post/ROOT0002 again";
    assert_eq!(linked_ids(t), ["REPLY0001", "ROOT0002", "ANCHOR01"]);
    assert!(linked_ids("no links").is_empty());
}

#[test]
fn filter_combines_every_criterion() {
    let s = fixture();
    let q = |f: SearchFilter| { let mut v: Vec<String> = s.search(&f).unwrap().into_iter().map(|p| p.post_id).collect(); v.sort(); v };
    assert_eq!(q(SearchFilter { keyword: "STRIKE".into(), ..Default::default() }), ["ALICE0001", "ALICE0002"]);
    assert_eq!(q(SearchFilter { keyword: "strike".into(), from: "2026-10-02".into(), ..Default::default() }), ["ALICE0002"]);
    assert_eq!(q(SearchFilter { to: "2026-10-01".into(), ..Default::default() }), ["ALICE0001", "BOB000002"]);
    assert_eq!(q(SearchFilter { nicks: vec!["Bob".into()], kind: KindFilter::Replies, ..Default::default() }), ["BOB000002"]);
    assert_eq!(q(SearchFilter { post_id: "BOB000001".into(), ..Default::default() }), ["ALICE0003", "BOB000001"]);
    assert_eq!(q(SearchFilter { kind: KindFilter::Posts, nicks: vec!["alice".into()], ..Default::default() }), ["ALICE0001", "ALICE0002"]);
    assert!(SearchFilter::default().is_empty());
    assert_eq!(SearchFilter { keyword: "x".into(), nicks: vec!["a".into()], ..Default::default() }.describe(), "\"x\" · @a");
}

fn temp_db(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("acp-tui-store-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("rs.database");
    let c = Connection::open(&p).unwrap();
    c.execute_batch(POSTS_SCHEMA).unwrap();
    c.execute_batch(ATTACHMENTS_SCHEMA).unwrap();
    for x in fixture().posts {
        c.execute("INSERT INTO posts (post_id,parent_post_id,in_reply_to_id,nick,title,content,posted_at,body_state) VALUES (?1,?2,?3,?4,?5,?6,?7,'full')",
            params![x.post_id, x.parent_post_id, x.in_reply_to_id, x.nick, x.title, x.content, x.posted_at]).unwrap();
    }
    // The same file under two presigned URLs: listed once.
    for q in ["a=1", "a=2"] {
        c.execute("INSERT INTO attachments (post_id,url,local_path,mime_type) VALUES ('ALICE0001',?1,'/x/y.jpg','image/jpeg')",
            params![format!("https://media.acp.us/y.jpg?{q}")]).unwrap();
    }
    p
}

#[test]
fn sqlite_store_reads_without_writing() {
    let p = temp_db("ro");
    let before = std::fs::read(&p).unwrap();
    let s = SqliteStore::new(p.clone());
    let n = s.nicks().unwrap();
    assert_eq!(n.iter().map(|e| (e.nick.as_str(), e.post_count)).collect::<Vec<_>>(), [("alice", 3), ("bob", 2)]);
    let ids: Vec<String> = s.posts_for(&["ALICE".into()]).unwrap().into_iter().map(|p| p.post_id).collect();
    assert_eq!(ids, ["ALICE0002", "ALICE0003", "ALICE0001"]);
    assert_eq!(s.search(&SearchFilter { keyword: "strike".into(), ..Default::default() }).unwrap().len(), 2);
    assert_eq!(s.post("BOB000001").unwrap().unwrap().nick, "bob");
    assert!(s.post("NOPE").unwrap().is_none());
    assert_eq!(s.replies("ALICE0001").unwrap().len(), 1);
    assert_eq!(s.attachments("ALICE0001").unwrap().len(), 1);
    assert_eq!(std::fs::read(&p).unwrap(), before, "the store never writes");
    assert!(!p.with_extension("database-journal").exists());
}

#[test]
fn sqlite_store_missing_db_is_empty() {
    let s = SqliteStore::new(std::env::temp_dir().join("acp-tui-no-such-dir/rs.database"));
    assert!(s.nicks().unwrap().is_empty());
    assert!(s.posts_for(&["a".into()]).unwrap().is_empty());
    assert!(!std::env::temp_dir().join("acp-tui-no-such-dir").exists(), "nothing is created");
}

#[test]
fn old_theme_prefs_convert_like_before() {
    use crate::tui::theme::*;
    use ratatui::style::Color;
    assert_eq!(parse_hex("#dc2626"), Some(Color::Rgb(0xdc, 0x26, 0x26)));
    assert_eq!(parse_hex("zzzzzz"), None);
    assert_eq!(Theme::acp().accent, Color::Rgb(0xdc, 0x26, 0x26));
    let base = crate::themes::parse(include_str!("../../../themes/default.toml"), "default").unwrap().0;
    let (f, warn) = crate::themes::from_legacy("custom", &ThemePrefs { accent: Some("00ff00".into()), muted: Some("bad".into()), ..Default::default() }, Some(&base));
    assert_eq!(warn.len(), 1, "{warn:?}");
    let t = Theme::from_file(&f, crate::themes::Variant::Dark);
    assert_eq!(t.accent, Color::Rgb(0, 255, 0));
    assert_eq!(t.muted, Theme::acp().muted);
    let p: Prefs = toml::from_str("email = \"a@b\"\n[theme]\naccent = \"#112233\"\n").unwrap();
    assert_eq!(p.theme.unwrap().accent.as_deref(), Some("#112233"));
}
