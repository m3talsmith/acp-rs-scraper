//! SQLite queries: saving posts, profiles, mentions, attachments; listing and searching.

use crate::prelude::*;
use rusqlite::OpenFlags;

/// Stored `post_id`s that have a body (or attachments); blank rows and rows
/// whose body is known to be truncated are left out so the next scrape
/// re-fetches and repairs them. Empty when there is no database yet.
pub fn db_known_ids(db: &std::path::Path) -> Result<HashSet<String>> {
    let mut out = HashSet::new();
    if !db.exists() { return Ok(out); }
    let c = open_db(db)?;
    let ok: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='posts'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if !ok { return Ok(out); }
    let has_atts: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='attachments'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    let has_state: bool = c.query_row(
        "SELECT count(*) FROM pragma_table_info('posts') WHERE name = 'body_state'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    let base = if has_atts {
        "(content <> '' OR title <> '' OR post_id IN (SELECT post_id FROM attachments))"
    } else {
        "(content <> '' OR title <> '')"
    };
    let extra = if has_state { " AND body_state <> 'truncated'" } else { "" };
    let mut st = c.prepare(&format!("SELECT post_id FROM posts WHERE {base}{extra}"))?;
    for id in st.query_map([], |r| r.get::<_, String>(0))? { out.insert(id?); }
    Ok(out)
}

/// Stored reply count per thread root, used to skip unchanged threads.
pub fn db_reply_counts(db: &std::path::Path) -> Result<HashMap<String, usize>> {
    let mut out = HashMap::new();
    if !db.exists() { return Ok(out); }
    let c = open_db(db)?;
    let ok: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='posts'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if !ok { return Ok(out); }
    let mut st = c.prepare(
        "SELECT parent_post_id, COUNT(*) FROM posts WHERE parent_post_id != '' GROUP BY parent_post_id")?;
    for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize)))? {
        let (k, v) = row?;
        out.insert(k, v);
    }
    Ok(out)
}

#[derive(Debug)]
/// A `--list-nicks` row.
pub struct NickInfo {
    /// Nick (username, without `@`).
    pub nick: String,
    /// Display name.
    pub display_name: Option<String>,
    /// Stored posts by the nick.
    pub post_count: i64,
    /// Newest stored post date.
    pub latest: Option<String>,
    /// Whether a profile row exists.
    pub profile_scraped: bool,
    /// When it was scraped (RFC 3339).
    pub scraped_at: Option<String>,
}

/// Every nick with posts or a profile, with counts and dates (for `--list-nicks`).
pub fn list_nicks_db(db: &std::path::Path) -> Result<Vec<NickInfo>> {
    if !db.exists() { return Ok(Vec::new()); }
    list_nicks_conn(&open_db(db)?)
}

/// [`list_nicks_db`] on an open connection (only reads; works read-only).
pub fn list_nicks_conn(c: &Connection) -> Result<Vec<NickInfo>> {
    let mut map: HashMap<String, NickInfo> = HashMap::new();
    let has_posts: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='posts'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if has_posts {
        let mut st = c.prepare(
            "SELECT nick, COUNT(*), MAX(posted_at) FROM posts GROUP BY lower(nick)")?;
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?)))? {
            let (nick, cnt, latest) = row?;
            let key = nick.to_lowercase();
            map.insert(key, NickInfo {
                nick, display_name: None, post_count: cnt, latest,
                profile_scraped: false, scraped_at: None,
            });
        }
    }
    let has_p: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='profiles'", [],
        |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if has_p {
        let mut st = c.prepare("SELECT nick, display_name, scraped_at FROM profiles")?;
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?)))? {
            let (nick, dn, sa) = row?;
            let key = nick.to_lowercase();
            let e = map.entry(key).or_insert_with(|| NickInfo {
                nick: nick.clone(), display_name: None, post_count: 0, latest: None,
                profile_scraped: false, scraped_at: None,
            });
            e.nick = nick;
            e.display_name = dn.or(e.display_name.clone());
            e.profile_scraped = true;
            e.scraped_at = sa;
        }
    }
    let mut v: Vec<_> = map.into_values().collect();
    v.sort_by_key(|a| a.nick.to_lowercase());
    Ok(v)
}

/// Print `--list-nicks` as an aligned table on stdout.
pub fn print_nicks_table(infos: &[NickInfo]) {
    println!("{:<20} {:<24} {:>6}  {:<20} {:<8} SCRAPED_AT",
        "NICK", "DISPLAY", "POSTS", "LATEST", "PROFILE");
    for i in infos {
        println!("{:<20} {:<24} {:>6}  {:<20} {:<8} {}",
            format!("@{}", i.nick),
            i.display_name.as_deref().unwrap_or("-").chars().take(24).collect::<String>(),
            i.post_count,
            i.latest.as_deref().unwrap_or("-").chars().take(20).collect::<String>(),
            if i.profile_scraped { "yes" } else { "no" },
            i.scraped_at.as_deref().unwrap_or("-"));
    }
}

/// Insert posts into `~/.acp/ai/data/rs.database` (creating the table); existing `post_id`s are kept, except blank rows, whose title/content are repaired. Returns rows inserted or repaired.
pub fn save(posts: &[Post]) -> Result<usize> {
    let path = home()?.join(DB_REL_PATH);
    let mut db = open_db(&path)?;
    let n = save_posts(&mut db, posts)?;
    info!("DB: {}", path.display());
    Ok(n)
}

/// Insert `posts` into an open database. Same rules as [`save`]: a blank
/// `body_json` does not erase one already stored, and tags are replaced only
/// when this write carries a JSON body or at least one tag.
pub fn save_posts(db: &mut Connection, posts: &[Post]) -> Result<usize> {
    ensure_posts_schema(db)?;
    let tx = db.transaction()?;
    let mut n = 0;
    {
        let mut st = tx.prepare(
            "INSERT INTO posts (post_id,parent_post_id,in_reply_to_id,nick,title,content,posted_at,body_state,body_json,score)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(post_id) DO UPDATE SET
               title = CASE WHEN (posts.content = '' AND posts.title = '') OR (posts.body_state <> 'full' AND excluded.body_state = 'full' AND excluded.title <> '') THEN excluded.title ELSE posts.title END,
               content = CASE WHEN (posts.content = '' AND posts.title = '') OR (posts.body_state <> 'full' AND excluded.body_state = 'full' AND excluded.content <> '') THEN excluded.content ELSE posts.content END,
               body_state = CASE WHEN excluded.body_state = 'full' AND (excluded.content <> '' OR posts.content = '') THEN 'full' ELSE posts.body_state END,
               body_json = CASE WHEN excluded.body_json <> '' THEN excluded.body_json ELSE posts.body_json END,
               score = COALESCE(excluded.score, posts.score),
               in_reply_to_id = CASE WHEN excluded.in_reply_to_id <> '' AND excluded.in_reply_to_id <> excluded.parent_post_id
                                THEN excluded.in_reply_to_id ELSE posts.in_reply_to_id END
             WHERE (posts.content = '' AND posts.title = '' AND (excluded.content <> '' OR excluded.title <> ''))
                OR (posts.body_state <> 'full' AND excluded.body_state = 'full' AND (excluded.content <> '' OR posts.content = ''))
                OR (excluded.body_json <> '' AND excluded.body_json <> posts.body_json)
                OR (excluded.score IS NOT NULL AND excluded.score IS NOT posts.score)
                OR (excluded.in_reply_to_id <> '' AND excluded.in_reply_to_id <> excluded.parent_post_id
                    AND excluded.in_reply_to_id <> posts.in_reply_to_id)")?;
        let mut clear_tags = tx.prepare("DELETE FROM post_tags WHERE post_id = ?1")?;
        let mut ins_tag = tx.prepare("INSERT OR IGNORE INTO post_tags (post_id, tag) VALUES (?1, ?2)")?;
        for p in posts {
            let state = if p.truncated { "truncated" } else { "full" };
            n += st.execute(params![p.post_id, p.parent_post_id, p.in_reply_to_id, p.nick, p.title, p.content, p.posted_at, state, p.body_json, p.score])?;
            let tags = extract_tags(&p.body_json, &p.content);
            if !p.body_json.is_empty() || !tags.is_empty() {
                clear_tags.execute(params![p.post_id])?;
                for t in &tags { ins_tag.execute(params![p.post_id, t])?; }
            }
        }
    }
    tx.commit()?;
    Ok(n)
}

/// Create `posts` if missing and upgrade older tables in place: adds
/// `in_reply_to_id` (backfilled from `parent_post_id`) and the relationship indexes.
pub fn ensure_posts_schema(c: &Connection) -> Result<()> {
    c.execute_batch(POSTS_SCHEMA)?;
    let has: bool = c.query_row(
        "SELECT count(*) FROM pragma_table_info('posts') WHERE name = 'in_reply_to_id'", [],
        |r| r.get::<_, i64>(0))? > 0;
    if !has { c.execute_batch(&format!("BEGIN; {POSTS_ADD_IN_REPLY_TO} COMMIT;"))?; }
    let has_state: bool = c.query_row(
        "SELECT count(*) FROM pragma_table_info('posts') WHERE name = 'body_state'", [],
        |r| r.get::<_, i64>(0))? > 0;
    if !has_state { c.execute_batch(POSTS_ADD_BODY_STATE)?; }
    let has_json: bool = c.query_row(
        "SELECT count(*) FROM pragma_table_info('posts') WHERE name = 'body_json'", [],
        |r| r.get::<_, i64>(0))? > 0;
    if !has_json { c.execute_batch(POSTS_ADD_BODY_JSON)?; }
    let has_score: bool = c.query_row(
        "SELECT count(*) FROM pragma_table_info('posts') WHERE name = 'score'", [],
        |r| r.get::<_, i64>(0))? > 0;
    if !has_score { c.execute_batch(POSTS_ADD_SCORE)?; }
    c.execute_batch(POSTS_MIGRATIONS)?;
    c.execute_batch(POST_TAGS_SCHEMA)?;
    c.execute_batch(crate::storage::history::SEARCH_HISTORY_SCHEMA)?;
    c.execute_batch(crate::storage::history::ACTION_TREE_SCHEMA)?;
    c.execute_batch(crate::storage::notices::NOTICES_SCHEMA)?;
    c.execute_batch(POST_VERSIONS_SCHEMA)?;
    c.execute_batch(SITE_STATE_SCHEMA)?;
    Ok(())
}

/// Read one `site_state` value. The table is created if this database is already open.
pub fn site_state_get(c: &Connection, key: &str) -> Result<Option<String>> {
    c.execute_batch(SITE_STATE_SCHEMA)?;
    c.query_row("SELECT value FROM site_state WHERE key = ?1", params![key], |r| r.get(0)).optional().map_err(Into::into)
}

/// Write one `site_state` value.
pub fn site_state_set(c: &Connection, key: &str, value: &str) -> Result<()> {
    c.execute_batch(SITE_STATE_SCHEMA)?;
    c.execute(
        "INSERT INTO site_state (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Insert or update a nick's profile row.
pub fn save_profile(db: &std::path::Path, p: &Profile) -> Result<()> {
    let c = open_db(db)?;
    c.execute_batch(
        PROFILES_SCHEMA)?;
    c.execute(
        "INSERT INTO profiles (nick,user_id,display_name,bio,avatar_url,avatar_path,banner_url,banner_path,
            location,role,joined_at,followers,following,posts_count,raw_json,scraped_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
         ON CONFLICT(nick) DO UPDATE SET
            user_id=excluded.user_id, display_name=excluded.display_name, bio=excluded.bio,
            avatar_url=excluded.avatar_url, avatar_path=excluded.avatar_path,
            banner_url=excluded.banner_url, banner_path=excluded.banner_path,
            location=excluded.location, role=excluded.role, joined_at=excluded.joined_at,
            followers=excluded.followers, following=excluded.following, posts_count=excluded.posts_count,
            raw_json=excluded.raw_json, scraped_at=excluded.scraped_at",
        rusqlite::params![
            p.nick, p.user_id, p.display_name, p.bio, p.avatar_url, p.avatar_path,
            p.banner_url, p.banner_path, p.location, p.role, p.joined_at,
            p.followers, p.following, p.posts_count, p.raw_json, p.scraped_at
        ],
    )?;
    Ok(())
}

/// Insert mention hits (duplicates ignored).
pub fn save_mentions(db: &std::path::Path, hits: &[MentionHit]) -> Result<()> {
    let mut c = open_db(db)?;
    c.execute_batch(
        MENTIONS_SCHEMA)?;
    let tx = c.transaction()?;
    {
        let mut st = tx.prepare(
            "INSERT OR IGNORE INTO mentions (post_id, mentioned_nick, match_type) VALUES (?1,?2,?3)")?;
        for h in hits {
            st.execute(rusqlite::params![h.post_id, h.mentioned_nick, h.match_type.as_str()])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Load profiles and mentions, optionally only for `nicks`.
pub fn load_profiles_mentions(db: &std::path::Path, nicks: &[String]) -> Result<(Vec<Profile>, Vec<MentionHit>)> {
    if !db.exists() { return Ok((Vec::new(), Vec::new())); }
    let c = open_db(db)?;
    let mut profiles = Vec::new();
    let has_p: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='profiles'", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0) > 0;
    if has_p {
        let mut st = c.prepare("SELECT nick,user_id,display_name,bio,avatar_url,avatar_path,banner_url,banner_path,location,role,joined_at,followers,following,posts_count,raw_json,scraped_at FROM profiles")?;
        for row in st.query_map([], |r| Ok(Profile {
            nick: r.get(0)?, user_id: r.get(1)?, display_name: r.get(2)?, bio: r.get(3)?,
            avatar_url: r.get(4)?, avatar_path: r.get(5)?, banner_url: r.get(6)?, banner_path: r.get(7)?,
            location: r.get(8)?, role: r.get(9)?, joined_at: r.get(10)?, followers: r.get(11)?,
            following: r.get(12)?, posts_count: r.get(13)?, raw_json: r.get(14)?, scraped_at: r.get(15)?,
        }))? {
            let p = row?;
            if nicks.is_empty() || nicks.iter().any(|n| n.eq_ignore_ascii_case(&p.nick)) {
                profiles.push(p);
            }
        }
    }
    let mut hits = Vec::new();
    let has_m: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='mentions'", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0) > 0;
    if has_m {
        let mut st = c.prepare("SELECT post_id, mentioned_nick, match_type FROM mentions")?;
        for row in st.query_map([], |r| {
            let mt: String = r.get(2)?;
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, mt))
        })? {
            let (post_id, mentioned_nick, mt) = row?;
            if !nicks.is_empty() && !nicks.iter().any(|n| n.eq_ignore_ascii_case(&mentioned_nick)) { continue; }
            if let Some(match_type) = MatchType::parse(&mt) {
                hits.push(MentionHit { post_id, mentioned_nick, match_type });
            }
        }
    }
    Ok((profiles, hits))
}

/// Create `attachments` if missing and add `on_site` on tables from before the site subset.
pub fn ensure_attachments_schema(c: &Connection) -> Result<()> {
    c.execute_batch(ATTACHMENTS_SCHEMA)?;
    if !attachments_have_on_site(c) {
        c.execute_batch(ATTACHMENTS_ADD_ON_SITE)?;
    }
    Ok(())
}

/// `true` when `attachments.on_site` is already a column.
pub fn attachments_have_on_site(c: &Connection) -> bool {
    c.query_row("SELECT count(*) FROM pragma_table_info('attachments') WHERE name = 'on_site'", [], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0
}

/// One attachment row. Column 5 is `on_site` (0 or 1).
pub fn attachment_from_row(r: &rusqlite::Row) -> rusqlite::Result<Attachment> {
    Ok(Attachment {
        post_id: r.get(0)?, url: r.get(1)?, kind: "file".into(),
        local_path: PathBuf::from(r.get::<_, String>(2)?), mime_type: r.get(3)?,
        on_site: r.get::<_, i64>(4)? != 0,
    })
}

/// Mark which stored files of `post_id` are on the site copy.
/// Paths in `on_site` get `on_site = 1`. Every other attachment of that post gets `0`.
pub fn set_post_site_attachments(db: &Path, post_id: &str, on_site: &[PathBuf]) -> Result<()> {
    let mut c = open_db(db)?;
    ensure_attachments_schema(&c)?;
    let tx = c.transaction()?;
    tx.execute("UPDATE attachments SET on_site = 0 WHERE post_id = ?1", params![post_id])?;
    {
        let mut st = tx.prepare("UPDATE attachments SET on_site = 1 WHERE post_id = ?1 AND local_path = ?2")?;
        for p in on_site {
            st.execute(params![post_id, p.to_string_lossy()])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Insert or update attachment rows. A row whose URL differs from a stored one
/// only in its query string (a fresh presigned link to the same file) replaces it.
pub fn save_attachments(db: &Path, atts: &[Attachment]) -> Result<()> {
    let mut c = open_db(db)?;
    ensure_attachments_schema(&c)?;
    let tx = c.transaction()?;
    {
        let mut st = tx.prepare(
            "INSERT INTO attachments (post_id,url,local_path,mime_type,on_site) VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(post_id,url) DO UPDATE SET local_path=excluded.local_path, mime_type=excluded.mime_type, on_site=excluded.on_site")?;
        let mut old = tx.prepare("SELECT id,url FROM attachments WHERE post_id = ?1 AND url <> ?2")?;
        let mut del = tx.prepare("DELETE FROM attachments WHERE id = ?1")?;
        for a in atts {
            let stale: Vec<i64> = old.query_map(params![a.post_id, a.url], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                .filter_map(|r| r.ok())
                .filter(|(_, u)| crate::media::canonical_url(u) == crate::media::canonical_url(&a.url))
                .map(|(id, _)| id).collect();
            for id in stale { del.execute(params![id])?; }
            st.execute(params![a.post_id, a.url, a.local_path.to_string_lossy(), a.mime_type, i64::from(a.on_site)])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Load posts matching `query` (case-insensitive LIKE on title + content),
/// optionally restricted to `nick`, plus context: each match's parent and
/// its direct children. Attachments come from the attachments table.
pub fn search_db(db: &Path, query: Option<&str>, nicks: &[String]) -> Result<(Vec<Post>, Vec<Attachment>, usize)> {
    let c = open_db(db).with_context(|| format!("open {}", db.display()))?;
    ensure_posts_schema(&c)?;
    let like = query.map(|q| format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let cols = "post_id,parent_post_id,nick,title,content,posted_at,in_reply_to_id,body_state,body_json,score";
    let row = |r: &rusqlite::Row| -> rusqlite::Result<Post> {
        Ok(Post { post_id: r.get(0)?, parent_post_id: r.get(1)?, nick: r.get(2)?, title: r.get(3)?,
                  content: r.get(4)?, posted_at: r.get(5)?, in_reply_to_id: r.get(6)?, media: Vec::new(),
                  truncated: r.get::<_, String>(7)? == "truncated", body_json: r.get(8)?, score: r.get(9)? })
    };
    let mut st = c.prepare(&format!(
        "SELECT {cols} FROM posts WHERE (?1 IS NULL OR title LIKE ?1 ESCAPE '\\' OR content LIKE ?1 ESCAPE '\\')
         ORDER BY posted_at"))?;
    let want: HashSet<String> = nicks.iter().map(|n| n.to_lowercase()).collect();
    let matches: Vec<Post> = st.query_map(params![like], row)?
        .filter(|p| p.as_ref().map_or(true, |p| want.is_empty() || want.contains(&p.nick.to_lowercase())))
        .collect::<rusqlite::Result<_>>()?;
    let n = matches.len();
    let mut seen: HashSet<String> = matches.iter().map(|p| p.post_id.clone()).collect();
    let mut out = matches;
    let mut by_id = c.prepare(&format!("SELECT {cols} FROM posts WHERE post_id = ?1"))?;
    let mut kids = c.prepare(&format!("SELECT {cols} FROM posts WHERE parent_post_id = ?1 ORDER BY posted_at"))?;
    // Context (parent + children) only for text searches; a full/nick dump has it already.
    let base: Vec<(String, String, String)> = if query.is_none() { Vec::new() } else { out.iter().map(|p| (p.post_id.clone(), p.parent_post_id.clone(), p.in_reply_to_id.clone())).collect() };
    for (id, parent, direct) in base {
        for up in [parent, direct] {
            if up.is_empty() { continue; }
            for p in by_id.query_map(params![up], row)? { let p = p?; if seen.insert(p.post_id.clone()) { out.push(p); } }
        }
        for p in kids.query_map(params![id], row)? { let p = p?; if seen.insert(p.post_id.clone()) { out.push(p); } }
    }
    let atts = attachments_for(&c, &out)?;
    Ok((out, atts, n))
}

/// Stored attachment files of `posts` (one per file, newest row first; missing files are warned about and skipped).
fn attachments_for(c: &Connection, out: &[Post]) -> Result<Vec<Attachment>> {
    let mut atts = Vec::new();
    let has_att: bool = c.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='attachments'", [], |r| r.get::<_, i64>(0))? > 0;
    if has_att {
        ensure_attachments_schema(c)?;
        // Newest row first; older rows for the same file (stale presigned links
        // stored before 1.26.1) are skipped so each file is exported once.
        let mut st = c.prepare("SELECT post_id,url,local_path,mime_type,on_site FROM attachments WHERE post_id = ?1 ORDER BY id DESC")?;
        let mut seen_att: HashSet<(String, String)> = HashSet::new();
        for p in out {
            for a in st.query_map(params![p.post_id], attachment_from_row)? {
                let a = a?;
                if !seen_att.insert((a.post_id.clone(), crate::media::canonical_url(&a.url).to_string())) { continue; }
                if a.local_path.exists() { atts.push(a); } else { log_error(format_args!("warning: missing attachment file {}", a.local_path.display())); }
            }
        }
    }
    Ok(atts)
}

/// Exactly these posts (in the order given, duplicates once) with their
/// attachments, and the ids that are not in the database.
pub fn posts_by_ids(db: &Path, ids: &[String]) -> Result<(Vec<Post>, Vec<Attachment>, Vec<String>)> {
    let c = open_db(db).with_context(|| format!("open {}", db.display()))?;
    ensure_posts_schema(&c)?;
    let cols = "post_id,parent_post_id,nick,title,content,posted_at,in_reply_to_id,body_state,body_json,score";
    let mut by_id = c.prepare(&format!("SELECT {cols} FROM posts WHERE post_id = ?1"))?;
    let (mut out, mut missing, mut seen) = (Vec::new(), Vec::new(), HashSet::new());
    for id in ids {
        if !seen.insert(id.clone()) { continue; }
        let p = by_id.query_row(params![id], |r| Ok(Post { post_id: r.get(0)?, parent_post_id: r.get(1)?, nick: r.get(2)?, title: r.get(3)?,
            content: r.get(4)?, posted_at: r.get(5)?, in_reply_to_id: r.get(6)?, media: Vec::new(),
            truncated: r.get::<_, String>(7)? == "truncated", body_json: r.get(8)?, score: r.get(9)? })).optional()?;
        match p { Some(p) => out.push(p), None => missing.push(id.clone()) }
    }
    let atts = attachments_for(&c, &out)?;
    Ok((out, atts, missing))
}

/// One stored edit of a post. `source` is `site`, `peer`, `local` or `draft`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostVersion {
    /// The post these versions belong to.
    pub post_id: String,
    /// Stable hash of `body_json` and `edited_at` (FNV-1a 64, hex).
    pub version_hash: String,
    /// ProseMirror JSON when the site sent a document, otherwise the text body.
    pub body_json: String,
    /// When this version was written (`edited_at`, else `inserted_at` / `updated_at`).
    /// A page that sent no timestamp is `~NNNN`, the order on that page.
    pub edited_at: String,
    /// When this client stored the row (RFC 3339).
    pub observed_at: String,
    /// `site` (read from the site), or later `peer` / `local` / `draft`.
    pub source: String,
}

/// Insert versions. The same `(post_id, version_hash)` only refreshes `observed_at`.
/// Creates the table. `source` must be `site`, `peer`, `local` or `draft`.
pub fn save_post_versions(c: &mut Connection, rows: &[PostVersion]) -> Result<usize> {
    ensure_posts_schema(c)?;
    let tx = c.transaction()?;
    {
        let mut st = tx.prepare(
            "INSERT INTO post_versions (post_id, version_hash, body_json, edited_at, observed_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(post_id, version_hash) DO UPDATE SET observed_at = excluded.observed_at")?;
        for v in rows {
            if !matches!(v.source.as_str(), "site" | "peer" | "local" | "draft") {
                bail!("post version source {:?} is not site, peer, local or draft", v.source);
            }
            st.execute(params![v.post_id, v.version_hash, v.body_json, v.edited_at, v.observed_at, v.source])?;
        }
    }
    tx.commit()?;
    Ok(rows.len())
}

/// Versions of `post_id`, oldest `edited_at` first. Empty when the table is absent.
pub fn load_post_versions(c: &Connection, post_id: &str) -> Result<Vec<PostVersion>> {
    if !table_exists(c, "post_versions")? { return Ok(Vec::new()); }
    let mut st = c.prepare(
        "SELECT post_id, version_hash, body_json, edited_at, observed_at, source
         FROM post_versions WHERE post_id = ?1 ORDER BY edited_at, version_hash")?;
    let v = st.query_map(params![post_id], |r| Ok(PostVersion {
        post_id: r.get(0)?, version_hash: r.get(1)?, body_json: r.get(2)?,
        edited_at: r.get(3)?, observed_at: r.get(4)?, source: r.get(5)?,
    }))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Thread id of a stored post: `parent_post_id` when it is a reply, else its own id.
/// `None` when the post is not stored.
pub fn thread_of_post(c: &Connection, post_id: &str) -> Result<Option<String>> {
    if !table_exists(c, "posts")? { return Ok(None); }
    let parent: Option<String> = c.query_row(
        "SELECT parent_post_id FROM posts WHERE post_id = ?1",
        params![post_id], |r| r.get(0)).optional()?;
    Ok(parent.map(|p| if p.is_empty() { post_id.to_string() } else { p }))
}

/// One saved site read: the posts in the order the site returned them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteFeed {
    /// Stable key (`top:week`, `top:all:replies`, …).
    pub key: String,
    /// Title shown in the Body.
    pub title: String,
    /// When this view was last read from the site (RFC 3339).
    pub fetched_at: String,
    /// Post ids, in site order.
    pub post_ids: Vec<String>,
}

/// Insert or replace a site read. Creates the table.
pub fn save_site_feed(c: &Connection, feed: &SiteFeed) -> Result<()> {
    c.execute_batch(SITE_FEEDS_SCHEMA)?;
    let ids = serde_json::to_string(&feed.post_ids)?;
    c.execute(
        "INSERT INTO site_feeds (key, title, fetched_at, post_ids) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET title = excluded.title, fetched_at = excluded.fetched_at, post_ids = excluded.post_ids",
        params![feed.key, feed.title, feed.fetched_at, ids])?;
    Ok(())
}

/// The saved site read `key`, if that table and row exist.
pub fn load_site_feed(c: &Connection, key: &str) -> Result<Option<SiteFeed>> {
    let has: bool = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='site_feeds'",
        [], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    if !has { return Ok(None); }
    let row: Option<(String, String, String)> = c.query_row(
        "SELECT title, fetched_at, post_ids FROM site_feeds WHERE key = ?1",
        params![key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    let Some((title, fetched_at, ids)) = row else { return Ok(None) };
    let post_ids: Vec<String> = serde_json::from_str(&ids)
        .with_context(|| format!("site feed {key} has a broken post list"))?;
    Ok(Some(SiteFeed { key: key.into(), title, fetched_at, post_ids }))
}

/// Open (creating) the database at `db`, creating its directory first so the
/// very first write on a fresh `$HOME` works whichever table is written first.
pub fn open_db(db: &Path) -> Result<Connection> {
    open_db_flags(db, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX)
}

/// Open `db` with `flags`. An encrypted database gets its key before any other statement.
pub fn open_db_flags(db: &Path, flags: OpenFlags) -> Result<Connection> {
    if let Some(dir) = db.parent() {
        if flags.contains(OpenFlags::SQLITE_OPEN_CREATE) {
            std::fs::create_dir_all(dir)?;
        }
    }
    let c = Connection::open_with_flags(db, flags).with_context(|| format!("open {}", db.display()))?;
    // SQLCipher wants the key before any other statement, including busy_timeout.
    crate::crypto::atrest::apply_if_needed(&c, db)?;
    c.busy_timeout(std::time::Duration::from_secs(10))?;
    Ok(c)
}
