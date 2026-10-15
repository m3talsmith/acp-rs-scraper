//! Unit tests for `--remove-nick` / `--clear-all` against a temporary HOME layout.

use super::*;

/// A throwaway HOME-like directory with the real layout under it.
struct TempHome(PathBuf);
impl TempHome {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("acp-maint-{tag}-{}-{:?}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(p.join(ATTACH_REL_PATH)).unwrap();
        TempHome(p)
    }
    fn db(&self) -> PathBuf { self.0.join(DB_REL_PATH) }
    fn att(&self) -> PathBuf { self.0.join(ATTACH_REL_PATH) }
    fn file(&self, name: &str) -> String {
        let p = self.att().join(name);
        std::fs::write(&p, b"x").unwrap();
        p.to_string_lossy().into_owned()
    }
}
impl Drop for TempHome {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn seed(h: &TempHome) -> Connection {
    let c = Connection::open(h.db()).unwrap();
    c.execute_batch(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, post_id TEXT NOT NULL, parent_post_id TEXT NOT NULL,
           nick TEXT NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL, posted_at TEXT NOT NULL);
         CREATE TABLE attachments (id INTEGER PRIMARY KEY AUTOINCREMENT, post_id TEXT NOT NULL, url TEXT NOT NULL,
           local_path TEXT NOT NULL, mime_type TEXT NOT NULL);
         CREATE TABLE mentions (id INTEGER PRIMARY KEY AUTOINCREMENT, post_id TEXT NOT NULL,
           mentioned_nick TEXT NOT NULL, match_type TEXT NOT NULL);
         CREATE TABLE profiles (nick TEXT PRIMARY KEY, avatar_path TEXT, banner_path TEXT,
           raw_json TEXT NOT NULL DEFAULT '{}', scraped_at TEXT NOT NULL DEFAULT '');").unwrap();
    let shared = h.file("shared.png");
    let own = h.file("alice-only.png");
    let reply_img = h.file("bob-reply.png");
    let avatar = h.file("alice-avatar.png");
    let bob_avatar = h.file("bob-avatar.png");
    for (id, parent, nick) in [("A1", "", "Alice"), ("A2", "B1", "alice"), ("R1", "A1", "bob"),
                               ("B1", "", "bob"), ("R2", "B1", "carol")] {
        c.execute("INSERT INTO posts (post_id,parent_post_id,nick,title,content,posted_at) VALUES (?1,?2,?3,'','','')",
            params![id, parent, nick]).unwrap();
    }
    for (pid, path) in [("A1", &own), ("A1", &shared), ("B1", &shared), ("R1", &reply_img)] {
        c.execute("INSERT INTO attachments (post_id,url,local_path,mime_type) VALUES (?1,?2,?3,'image/png')",
            params![pid, format!("u/{pid}/{path}"), path]).unwrap();
    }
    for (pid, n) in [("B1", "alice"), ("R2", "bob"), ("A1", "bob")] {
        c.execute("INSERT INTO mentions (post_id,mentioned_nick,match_type) VALUES (?1,?2,'nick')", params![pid, n]).unwrap();
    }
    c.execute("INSERT INTO profiles (nick,avatar_path) VALUES ('alice',?1)", [&avatar]).unwrap();
    c.execute("INSERT INTO profiles (nick,avatar_path) VALUES ('bob',?1)", [&bob_avatar]).unwrap();
    c
}

fn count(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }

#[test]
fn dry_run_changes_nothing() {
    let h = TempHome::new("dry");
    let c = seed(&h);
    let r = remove_nicks(&h.db(), &h.att(), &["alice".into()], false, false).unwrap();
    assert_eq!((r.own_posts, r.child_replies, r.attachment_rows, r.mention_rows, r.profile_rows), (2, 1, 3, 2, 1));
    assert_eq!(count(&c, "SELECT count(*) FROM posts"), 5);
    assert_eq!(count(&c, "SELECT count(*) FROM attachments"), 4);
    assert!(r.files.iter().all(|f| f.exists()));
}

#[test]
fn removes_nick_replies_and_unreferenced_files() {
    let h = TempHome::new("rm");
    let c = seed(&h);
    let r = remove_nicks(&h.db(), &h.att(), &["ALICE".into(), "ghost".into()], false, true).unwrap();
    assert_eq!(r.unknown_nicks, vec!["ghost".to_string()]);
    // Alice's two posts plus bob's reply R1 under her root A1. B1/R2 stay.
    let left: Vec<String> = c.prepare("SELECT post_id FROM posts ORDER BY post_id").unwrap()
        .query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    assert_eq!(left, ["B1", "R2"]);
    assert_eq!(count(&c, "SELECT count(*) FROM attachments"), 1);
    // mentions of alice and mentions attached to removed posts are gone; R2->bob stays.
    assert_eq!(count(&c, "SELECT count(*) FROM mentions"), 1);
    assert_eq!(count(&c, "SELECT count(*) FROM profiles"), 1);
    // shared.png is still used by B1; bob's avatar untouched.
    assert!(h.att().join("shared.png").exists());
    assert!(h.att().join("bob-avatar.png").exists());
    for gone in ["alice-only.png", "bob-reply.png", "alice-avatar.png"] {
        assert!(!h.att().join(gone).exists(), "{gone} should be deleted");
    }
    assert_eq!(r.files.len(), 3);
}

#[test]
fn keep_replies_keeps_other_peoples_replies() {
    let h = TempHome::new("keep");
    let c = seed(&h);
    let r = remove_nicks(&h.db(), &h.att(), &["alice".into()], true, true).unwrap();
    assert_eq!((r.own_posts, r.child_replies), (2, 0));
    assert_eq!(count(&c, "SELECT count(*) FROM posts WHERE post_id='R1'"), 1);
    assert!(h.att().join("bob-reply.png").exists());
}

#[test]
fn files_outside_attachments_dir_are_never_deleted() {
    let h = TempHome::new("outside");
    let c = seed(&h);
    let outside = h.0.join("precious.png");
    std::fs::write(&outside, b"x").unwrap();
    c.execute("UPDATE profiles SET banner_path=?1 WHERE nick='alice'", [outside.to_string_lossy()]).unwrap();
    let r = remove_nicks(&h.db(), &h.att(), &["alice".into()], false, true).unwrap();
    assert!(outside.exists());
    assert!(!r.files.contains(&outside));
}

#[test]
fn missing_db_reports_unknown() {
    let h = TempHome::new("nodb");
    let r = remove_nicks(&h.db(), &h.att(), &["x".into()], false, true).unwrap();
    assert!(r.is_empty());
    assert_eq!(r.unknown_nicks, vec!["x".to_string()]);
}

#[test]
fn clear_all_spares_prefs_and_exports() {
    let h = TempHome::new("clear");
    drop(seed(&h));
    let prefs = h.0.join(PREFS_REL_PATH);
    let exports = h.0.join(EXPORTS_REL_PATH);
    std::fs::create_dir_all(&exports).unwrap();
    std::fs::write(&prefs, "email = \"a@b.c\"\n").unwrap();
    std::fs::write(exports.join("x.json"), "{}").unwrap();
    assert_eq!(clear_all_targets(&h.db(), &h.att()).len(), 2);
    let gone = clear_all(&h.db(), &h.att()).unwrap();
    assert_eq!(gone.len(), 2);
    assert!(!h.db().exists() && !h.att().exists());
    assert!(prefs.exists() && exports.join("x.json").exists());
    assert!(clear_all(&h.db(), &h.att()).unwrap().is_empty());
}

#[test]
fn confirm_refuses_without_tty_unless_yes() {
    assert!(confirm("x", true, true).unwrap());
    // Under `cargo test` stdin is not a terminal.
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        assert!(confirm("x", false, false).is_err());
    }
}
