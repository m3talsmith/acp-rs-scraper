//! Offline maintenance: `--remove-nick` and `--clear-all`.
//!
//! Removal runs in a single SQLite transaction. The same code path computes the
//! preview (`--dry-run` and the confirmation counts) by rolling the transaction
//! back instead of committing, so the preview always matches what would happen.
//! Files on disk are only touched after a successful commit, and only files that
//! live inside the attachments directory and are no longer referenced by any
//! remaining `attachments` row or profile avatar/banner path.

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

/// What a `--remove-nick` run removed (or would remove, for a preview).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RemoveReport {
    /// Posts authored by the removed nicks.
    pub own_posts: usize,
    /// Other people's replies in the removed nicks' threads (0 with `--keep-replies`).
    pub child_replies: usize,
    /// `attachments` rows deleted.
    pub attachment_rows: usize,
    /// `mentions` rows deleted (mentioned_nick matches, or the post was removed).
    pub mention_rows: usize,
    /// `profiles` rows deleted.
    pub profile_rows: usize,
    /// Files no longer referenced by anything, which are (or would be) deleted.
    pub files: Vec<PathBuf>,
    /// Nicks that had nothing at all in the database.
    pub unknown_nicks: Vec<String>,
}

impl RemoveReport {
    /// True if nothing would be removed.
    pub fn is_empty(&self) -> bool {
        self.own_posts + self.child_replies + self.attachment_rows + self.mention_rows + self.profile_rows == 0
            && self.files.is_empty()
    }

    /// Human-readable multi-line summary.
    pub fn summary(&self, verb: &str) -> String {
        let mut s = format!(
            "{verb}: {} posts by the nick(s), {} replies in their threads, {} attachment rows, \
             {} mention rows, {} profiles, {} files",
            self.own_posts, self.child_replies, self.attachment_rows, self.mention_rows,
            self.profile_rows, self.files.len());
        if !self.unknown_nicks.is_empty() {
            s.push_str(&format!("\nnot in database: {}", self.unknown_nicks.join(", ")));
        }
        s
    }
}

fn table_exists(c: &Connection, t: &str) -> Result<bool> {
    Ok(c.query_row("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1", [t], |_| Ok(()))
        .optional()?.is_some())
}

/// Remove nicks from the database at `db`. With `commit == false` nothing is
/// changed (the transaction is rolled back and no files are deleted), which is
/// how `--dry-run` and the confirmation preview work.
pub fn remove_nicks(db: &Path, attach_dir: &Path, nicks: &[String], keep_replies: bool, commit: bool) -> Result<RemoveReport> {
    let mut rep = RemoveReport::default();
    if !db.exists() {
        rep.unknown_nicks = nicks.to_vec();
        return Ok(rep);
    }
    let mut c = Connection::open(db).with_context(|| format!("open {}", db.display()))?;
    let has_posts = table_exists(&c, "posts")?;
    let has_atts = table_exists(&c, "attachments")?;
    let has_mentions = table_exists(&c, "mentions")?;
    let has_profiles = table_exists(&c, "profiles")?;
    let tx = c.transaction()?;
    tx.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS rm_nicks (nick TEXT PRIMARY KEY);
         CREATE TEMP TABLE IF NOT EXISTS rm_posts (post_id TEXT PRIMARY KEY, own INTEGER NOT NULL);
         CREATE TEMP TABLE IF NOT EXISTS rm_files (path TEXT PRIMARY KEY);
         DELETE FROM rm_nicks; DELETE FROM rm_posts; DELETE FROM rm_files;")?;
    for n in nicks {
        tx.execute("INSERT OR IGNORE INTO rm_nicks VALUES (lower(?1))", [n])?;
    }
    for n in nicks {
        let mut found = false;
        if has_posts {
            found |= tx.query_row("SELECT 1 FROM posts WHERE lower(nick)=lower(?1) LIMIT 1", [n], |_| Ok(())).optional()?.is_some();
        }
        if has_profiles {
            found |= tx.query_row("SELECT 1 FROM profiles WHERE lower(nick)=lower(?1)", [n], |_| Ok(())).optional()?.is_some();
        }
        if has_mentions {
            found |= tx.query_row("SELECT 1 FROM mentions WHERE lower(mentioned_nick)=lower(?1) LIMIT 1", [n], |_| Ok(())).optional()?.is_some();
        }
        if !found { rep.unknown_nicks.push(n.clone()); }
    }
    if has_posts {
        rep.own_posts = tx.execute(
            "INSERT OR IGNORE INTO rm_posts SELECT post_id, 1 FROM posts WHERE lower(nick) IN (SELECT nick FROM rm_nicks)", [])?;
        if !keep_replies {
            rep.child_replies = tx.execute(
                "INSERT OR IGNORE INTO rm_posts SELECT post_id, 0 FROM posts
                 WHERE parent_post_id <> '' AND parent_post_id IN
                   (SELECT p.post_id FROM posts p JOIN rm_posts r ON r.post_id = p.post_id AND r.own = 1
                    WHERE p.parent_post_id = '')", [])?;
        }
    }
    if has_atts {
        tx.execute("INSERT OR IGNORE INTO rm_files SELECT local_path FROM attachments WHERE post_id IN (SELECT post_id FROM rm_posts)", [])?;
        rep.attachment_rows = tx.execute("DELETE FROM attachments WHERE post_id IN (SELECT post_id FROM rm_posts)", [])?;
    }
    if has_mentions {
        rep.mention_rows = tx.execute(
            "DELETE FROM mentions WHERE lower(mentioned_nick) IN (SELECT nick FROM rm_nicks)
                OR post_id IN (SELECT post_id FROM rm_posts)", [])?;
    }
    if has_profiles {
        tx.execute(
            "INSERT OR IGNORE INTO rm_files SELECT avatar_path FROM profiles
               WHERE lower(nick) IN (SELECT nick FROM rm_nicks) AND avatar_path IS NOT NULL AND avatar_path <> ''", [])?;
        tx.execute(
            "INSERT OR IGNORE INTO rm_files SELECT banner_path FROM profiles
               WHERE lower(nick) IN (SELECT nick FROM rm_nicks) AND banner_path IS NOT NULL AND banner_path <> ''", [])?;
        rep.profile_rows = tx.execute("DELETE FROM profiles WHERE lower(nick) IN (SELECT nick FROM rm_nicks)", [])?;
    }
    if has_posts {
        tx.execute("DELETE FROM posts WHERE post_id IN (SELECT post_id FROM rm_posts)", [])?;
    }
    // Keep only files nothing else still references.
    if has_atts {
        tx.execute("DELETE FROM rm_files WHERE path IN (SELECT local_path FROM attachments)", [])?;
    }
    if has_profiles {
        tx.execute("DELETE FROM rm_files WHERE path IN (SELECT avatar_path FROM profiles WHERE avatar_path IS NOT NULL)
                    OR path IN (SELECT banner_path FROM profiles WHERE banner_path IS NOT NULL)", [])?;
    }
    let candidates: Vec<String> = {
        let mut st = tx.prepare("SELECT path FROM rm_files ORDER BY path")?;
        st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
    };
    let root = attach_dir.canonicalize().ok();
    let files: BTreeSet<PathBuf> = candidates.iter().map(PathBuf::from)
        .filter(|p| p.is_file())
        .filter(|p| match (&root, p.canonicalize()) {
            (Some(r), Ok(cp)) => cp.starts_with(r),
            _ => false,
        })
        .collect();
    rep.files = files.into_iter().collect();
    if commit {
        tx.commit()?;
        for f in &rep.files {
            if let Err(e) = std::fs::remove_file(f) {
                crate::log_error(format_args!("warning: could not delete {}: {e}", f.display()));
            }
        }
    } else {
        tx.rollback()?;
    }
    Ok(rep)
}

/// Paths `--clear-all` deletes, filtered to those that exist: the database
/// (plus any `-wal`/`-shm`/`-journal` sidecars) and the attachments directory.
pub fn clear_all_targets(db: &Path, attach_dir: &Path) -> Vec<PathBuf> {
    let mut v = vec![db.to_path_buf()];
    for ext in ["-wal", "-shm", "-journal"] {
        let mut s = db.as_os_str().to_owned();
        s.push(ext);
        v.push(PathBuf::from(s));
    }
    v.push(attach_dir.to_path_buf());
    v.into_iter().filter(|p| p.exists()).collect()
}

/// Delete everything from [`clear_all_targets`]. Returns what was removed.
pub fn clear_all(db: &Path, attach_dir: &Path) -> Result<Vec<PathBuf>> {
    let t = clear_all_targets(db, attach_dir);
    for p in &t {
        if p.is_dir() {
            std::fs::remove_dir_all(p).with_context(|| format!("remove {}", p.display()))?;
        } else {
            std::fs::remove_file(p).with_context(|| format!("remove {}", p.display()))?;
        }
    }
    Ok(t)
}

/// Count files (recursively) and their total size under `dir`.
pub fn dir_usage(dir: &Path) -> (usize, u64) {
    let mut n = 0;
    let mut bytes = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                let (a, b) = dir_usage(&p);
                n += a;
                bytes += b;
            } else if let Ok(m) = e.metadata() {
                n += 1;
                bytes += m.len();
            }
        }
    }
    (n, bytes)
}

/// Ask for confirmation on stderr. `--yes` skips the prompt; a non-interactive
/// stdin without `--yes` is refused. With `strict`, only the word `yes` counts.
pub fn confirm(question: &str, strict: bool, assume_yes: bool) -> Result<bool> {
    use std::io::IsTerminal;
    if assume_yes { return Ok(true); }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("refusing to run without confirmation: stdin is not a terminal (pass --yes)");
    }
    let hint = if strict { "type 'yes' to continue" } else { "y/N" };
    let a = crate::prompt(&format!("{question} [{hint}]: "))?.to_lowercase();
    Ok(if strict { a == "yes" } else { a == "y" || a == "yes" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    /// A throwaway HOME-like directory with the real layout under it.
    struct TempHome(PathBuf);
    impl TempHome {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("acp-maint-{tag}-{}-{:?}", std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            std::fs::create_dir_all(p.join(crate::ATTACH_REL_PATH)).unwrap();
            TempHome(p)
        }
        fn db(&self) -> PathBuf { self.0.join(crate::DB_REL_PATH) }
        fn att(&self) -> PathBuf { self.0.join(crate::ATTACH_REL_PATH) }
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
        let prefs = h.0.join(crate::PREFS_REL_PATH);
        let exports = h.0.join(crate::EXPORTS_REL_PATH);
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
}
