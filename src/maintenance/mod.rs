//! Offline maintenance: `--remove-nick` and `--clear-all`.

use crate::prelude::*;

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

/// Whether table `t` exists in the database.
pub fn table_exists(c: &Connection, t: &str) -> Result<bool> {
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
                log_error(format_args!("warning: could not delete {}: {e}", f.display()));
            }
        }
        // Drop the profile-images folder once it is empty.
        let _ = std::fs::remove_dir(attach_dir.join(PROFILE_IMAGES_DIR));
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
    let a = prompt(&format!("{question} [{hint}]: "))?.to_lowercase();
    Ok(if strict { a == "yes" } else { a == "y" || a == "yes" })
}

#[cfg(test)]
mod tests;
