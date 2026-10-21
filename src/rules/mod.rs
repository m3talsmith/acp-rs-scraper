//! Rule engine (plan stage 3.32.0).
//!
//! Triggers, conditions, and actions, stored in the SQLCipher database.
//! A sync runs the enabled rules on the rows that sync just wrote.
//! [`preview`] runs the same rules on posts already stored and applies nothing.
//! System rules are read-only. Clone one to get an ordinary user rule.
//!
//! 3.32.0 stays the plan number. This module does not take that version early.
//! A notify action records the firing and prints a line. It does not raise a toast.

mod cli;
mod file;

#[cfg(test)]
mod tests;

pub use cli::run_cli;
pub use file::{export_file, import_file};

use crate::prelude::*;

/// Stable id of the shipped "Synced post" rule.
pub const SYSTEM_SYNCED_POST: &str = "synced-post";

/// Where a rule came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Shipped with the app. The editor does not rewrite it.
    System,
    /// Created by the user, including a clone and an import.
    User,
}

impl Origin {
    /// `system` or `user`, as stored.
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::System => "system",
            Origin::User => "user",
        }
    }
    fn parse(s: &str) -> Result<Self> {
        match s {
            "system" => Ok(Origin::System),
            "user" => Ok(Origin::User),
            other => bail!("rule origin {other:?} is not system or user"),
        }
    }
}

/// When a rule is considered. The only trigger is a post that just synced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// A post in the sync that just committed, or a stored post during a preview.
    PostSynced,
}

/// A filter. Every condition on a rule has to match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Condition {
    /// The post's nick, without `@`. Compared case-insensitively.
    Author {
        /// Nick, without `@`.
        nick: String,
    },
    /// Case-insensitive text in the title or the body.
    Keyword {
        /// Text to find.
        text: String,
    },
    /// A regular expression over the title and the body. An invalid pattern is an error.
    Regex {
        /// The pattern, as the `regex` crate reads it.
        pattern: String,
    },
    /// `site` and `redstar` match a Red Star sync. `node` does not, until peer sync exists.
    Source {
        /// `site`, `redstar`, or `node`.
        source: String,
    },
    /// A tag, stored without `#`.
    Tag {
        /// Tag name, without `#`.
        tag: String,
    },
    /// A keyword, and optional nicks, kept on the rule. This is not the search-history table.
    SavedSearch {
        /// Case-insensitive text in the title or the body. Empty matches any text.
        keyword: String,
        /// Nicks to keep. Empty keeps every nick.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        nicks: Vec<String>,
    },
    /// A reply whose parent was written by this nick.
    RepliesTo {
        /// The parent's nick, without `@`.
        nick: String,
    },
}

/// What a matching rule does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleAction {
    /// Record the match and print a line the job log can show.
    Notify,
    /// Bookmark the post locally.
    Bookmark,
    /// Add a tag, without `#`.
    Tag {
        /// Tag name, without `#`.
        tag: String,
    },
    /// Add the post to a research project, creating the project when it is missing.
    Research {
        /// Project name.
        name: String,
    },
    /// Write that one post as JSON under the database's `exports/rules/` directory.
    Export,
}

/// One stored rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Row id.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// System or user.
    pub origin: Origin,
    /// Disabled rules are skipped.
    pub enabled: bool,
    /// Lower runs first.
    pub priority: i64,
    /// Set on a shipped rule. Null on a user rule.
    pub system_key: Option<String>,
    /// `system_key` of the rule this clone came from.
    pub cloned_from: Option<String>,
    /// Set when a later preset created this rule.
    pub template_key: Option<String>,
    /// When the rule runs.
    pub trigger: Trigger,
    /// Filters. Empty matches every post of the trigger.
    pub conditions: Vec<Condition>,
    /// Side effects. Evaluated in order.
    pub actions: Vec<RuleAction>,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub updated_at: String,
}

/// One row in `rule_firings`, also returned to the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Firing {
    /// The rule.
    pub rule_id: i64,
    /// The rule's name, for logs.
    pub rule_name: String,
    /// The post, when the firing is about one.
    pub post_id: String,
    /// `site` until node sync exists.
    pub source: String,
    /// The action kind, or `rule` when the post was skipped before any action.
    pub action: String,
    /// `fired`, `skipped`, or `error`.
    pub outcome: String,
    /// Why, or what the action did.
    pub detail: String,
}

/// A user rule to insert. Import and the editor build this. `system_key` stays null.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRule {
    /// Display name.
    pub name: String,
    /// Starts enabled unless the file said otherwise.
    pub enabled: bool,
    /// Lower runs first.
    pub priority: i64,
    /// Set when this row clones a system rule.
    pub cloned_from: Option<String>,
    /// Set when a preset created this row.
    pub template_key: Option<String>,
    /// When it runs.
    pub trigger: Trigger,
    /// Filters.
    pub conditions: Vec<Condition>,
    /// Side effects.
    pub actions: Vec<RuleAction>,
}

/// Fields a user may change. A system row accepts only [`set_priority`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleEdit {
    /// Display name.
    pub name: String,
    /// Enabled.
    pub enabled: bool,
    /// Lower runs first.
    pub priority: i64,
    /// When it runs.
    pub trigger: Trigger,
    /// Filters.
    pub conditions: Vec<Condition>,
    /// Side effects.
    pub actions: Vec<RuleAction>,
}

fn now_stamp() -> String { chrono::Local::now().to_rfc3339() }

fn json_of<T: Serialize>(v: &T) -> Result<String> {
    Ok(serde_json::to_string(v)?)
}

/// Open the database for rule writes, create the tables, and insert any missing system rule.
pub fn open_rules(db: &Path) -> Result<Connection> {
    let c = open_db(db)?;
    c.execute_batch("PRAGMA foreign_keys = ON;")?;
    ensure_posts_schema(&c)?;
    ensure_system_rules(&c)?;
    Ok(c)
}

/// Insert the shipped rules when their `system_key` is missing. An existing row is left alone,
/// so a priority change sticks.
pub fn ensure_system_rules(c: &Connection) -> Result<()> {
    let have: bool = c.query_row(
        "SELECT count(*) FROM rules WHERE system_key = ?1",
        params![SYSTEM_SYNCED_POST],
        |r| r.get::<_, i64>(0),
    )? > 0;
    if have { return Ok(()); }
    let stamp = now_stamp();
    let trigger = json_of(&Trigger::PostSynced)?;
    let actions = json_of(&vec![RuleAction::Notify])?;
    c.execute(
        "INSERT INTO rules (name, origin, enabled, priority, system_key, cloned_from, template_key, trigger, conditions, actions, created_at, updated_at)
         VALUES (?1, 'system', 0, 100, ?2, NULL, NULL, ?3, '[]', ?4, ?5, ?5)",
        params!["Synced post", SYSTEM_SYNCED_POST, trigger, actions, stamp],
    )?;
    Ok(())
}

/// Every rule, lower priority first.
pub fn list_rules(c: &Connection) -> Result<Vec<Rule>> {
    ensure_system_rules(c)?;
    let mut st = c.prepare(
        "SELECT id, name, origin, enabled, priority, system_key, cloned_from, template_key, trigger, conditions, actions, created_at, updated_at
         FROM rules ORDER BY priority, id",
    )?;
    let rows = st.query_map([], raw_rule)?.collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().map(decode_rule).collect()
}

struct RawRule {
    id: i64,
    name: String,
    origin: String,
    enabled: i64,
    priority: i64,
    system_key: Option<String>,
    cloned_from: Option<String>,
    template_key: Option<String>,
    trigger: String,
    conditions: String,
    actions: String,
    created_at: String,
    updated_at: String,
}

fn raw_rule(r: &rusqlite::Row) -> rusqlite::Result<RawRule> {
    Ok(RawRule {
        id: r.get(0)?,
        name: r.get(1)?,
        origin: r.get(2)?,
        enabled: r.get(3)?,
        priority: r.get(4)?,
        system_key: r.get(5)?,
        cloned_from: r.get(6)?,
        template_key: r.get(7)?,
        trigger: r.get(8)?,
        conditions: r.get(9)?,
        actions: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
    })
}

fn decode_rule(raw: RawRule) -> Result<Rule> {
    Ok(Rule {
        id: raw.id,
        name: raw.name,
        origin: Origin::parse(&raw.origin)?,
        enabled: raw.enabled != 0,
        priority: raw.priority,
        system_key: raw.system_key,
        cloned_from: raw.cloned_from,
        template_key: raw.template_key,
        trigger: serde_json::from_str(&raw.trigger).with_context(|| format!("rule {} has a broken trigger", raw.id))?,
        conditions: serde_json::from_str(&raw.conditions).with_context(|| format!("rule {} has broken conditions", raw.id))?,
        actions: serde_json::from_str(&raw.actions).with_context(|| format!("rule {} has broken actions", raw.id))?,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    })
}

fn by_name(c: &Connection, name: &str) -> Result<Rule> {
    let name = name.trim();
    let found: Vec<Rule> = list_rules(c)?.into_iter().filter(|r| r.name.eq_ignore_ascii_case(name)).collect();
    match found.len() {
        0 => bail!("no rule named {name:?}"),
        1 => Ok(found.into_iter().next().expect("one rule")),
        n => bail!("{n} rules are named {name:?}"),
    }
}

/// Insert a user rule. The row has no `system_key`.
pub fn insert_user(c: &Connection, rule: &NewRule) -> Result<i64> {
    let name = check_name(&rule.name)?;
    if rule.actions.is_empty() { bail!("a rule needs an action"); }
    let stamp = now_stamp();
    c.execute(
        "INSERT INTO rules (name, origin, enabled, priority, system_key, cloned_from, template_key, trigger, conditions, actions, created_at, updated_at)
         VALUES (?1, 'user', ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![
            name,
            if rule.enabled { 1 } else { 0 },
            rule.priority,
            rule.cloned_from,
            rule.template_key,
            json_of(&rule.trigger)?,
            json_of(&rule.conditions)?,
            json_of(&rule.actions)?,
            stamp,
        ],
    )?;
    Ok(c.last_insert_rowid())
}

fn check_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 80 || n.chars().any(char::is_control) {
        bail!("rule names need 1-80 printable characters (got {name:?})");
    }
    Ok(n.to_string())
}

/// Replace a user rule's body. A system row is refused.
pub fn update_user(c: &Connection, id: i64, edit: &RuleEdit) -> Result<()> {
    let rule = rule_by_id(c, id)?;
    if rule.origin == Origin::System { bail!("a system rule is read-only; clone it"); }
    let name = check_name(&edit.name)?;
    if edit.actions.is_empty() { bail!("a rule needs an action"); }
    let n = c.execute(
        "UPDATE rules SET name = ?1, enabled = ?2, priority = ?3, trigger = ?4, conditions = ?5, actions = ?6, updated_at = ?7
         WHERE id = ?8 AND origin = 'user'",
        params![
            name,
            if edit.enabled { 1 } else { 0 },
            edit.priority,
            json_of(&edit.trigger)?,
            json_of(&edit.conditions)?,
            json_of(&edit.actions)?,
            now_stamp(),
            id,
        ],
    )?;
    if n == 0 { bail!("no user rule {id}"); }
    Ok(())
}

fn rule_by_id(c: &Connection, id: i64) -> Result<Rule> {
    list_rules(c)?.into_iter().find(|r| r.id == id).ok_or_else(|| anyhow!("no rule {id}"))
}

/// Enable or disable a user rule. A system row is refused.
pub fn set_enabled(c: &Connection, name: &str, enabled: bool) -> Result<()> {
    let rule = by_name(c, name)?;
    if rule.origin == Origin::System { bail!("a system rule is read-only; clone it"); }
    c.execute("UPDATE rules SET enabled = ?1, updated_at = ?2 WHERE id = ?3", params![if enabled { 1 } else { 0 }, now_stamp(), rule.id])?;
    Ok(())
}

/// Delete a user rule and its firings. A system row is refused.
pub fn delete_user(c: &Connection, name: &str) -> Result<()> {
    let rule = by_name(c, name)?;
    if rule.origin == Origin::System { bail!("a system rule is read-only; clone it"); }
    c.execute_batch("PRAGMA foreign_keys = ON;")?;
    c.execute("DELETE FROM rules WHERE id = ?1 AND origin = 'user'", params![rule.id])?;
    Ok(())
}

/// Set priority on a system or user rule. This is ordering, not a body edit.
pub fn set_priority(c: &Connection, name: &str, priority: i64) -> Result<()> {
    let rule = by_name(c, name)?;
    c.execute("UPDATE rules SET priority = ?1, updated_at = ?2 WHERE id = ?3", params![priority, now_stamp(), rule.id])?;
    Ok(())
}

/// Copy a system rule into a user rule. The original stays. The clone starts enabled.
pub fn clone_system(c: &Connection, system_key: &str) -> Result<Rule> {
    let key = system_key.trim();
    let src = list_rules(c)?.into_iter().find(|r| r.system_key.as_deref() == Some(key))
        .ok_or_else(|| anyhow!("no system rule {key:?}"))?;
    if src.origin != Origin::System { bail!("{key:?} is not a system rule"); }
    let name = fresh_copy_name(c, &src.name)?;
    let id = insert_user(c, &NewRule {
        name,
        enabled: true,
        priority: src.priority,
        cloned_from: src.system_key.clone(),
        template_key: None,
        trigger: src.trigger,
        conditions: src.conditions,
        actions: src.actions,
    })?;
    rule_by_id(c, id)
}

fn fresh_copy_name(c: &Connection, name: &str) -> Result<String> {
    let taken = list_rules(c)?;
    let mut candidate = format!("{name} copy");
    let mut n = 2i64;
    while taken.iter().any(|r| r.name.eq_ignore_ascii_case(&candidate)) {
        candidate = format!("{name} copy {n}");
        n += 1;
    }
    Ok(candidate)
}

/// Run enabled rules on `posts` after a sync. Failures of one action are recorded; the sync still finishes.
pub fn on_sync(db: &Path, posts: &[Post]) -> Result<Vec<Firing>> {
    let c = open_rules(db)?;
    evaluate(&c, db, posts, "site", false)
}

/// Preview every enabled rule against posts already stored. Writes no bookmark, tag, research item, or export.
pub fn preview(db: &Path) -> Result<Vec<Firing>> {
    let c = open_rules(db)?;
    let posts = load_posts(&c)?;
    evaluate(&c, db, &posts, "site", true)
}

pub(crate) fn load_posts(c: &Connection) -> Result<Vec<Post>> {
    let mut st = c.prepare(
        "SELECT post_id, parent_post_id, nick, title, content, posted_at, in_reply_to_id, body_state, body_json, score
         FROM posts ORDER BY posted_at, id",
    )?;
    let v = st.query_map([], |r| Ok(Post {
        post_id: r.get(0)?,
        parent_post_id: r.get(1)?,
        nick: r.get(2)?,
        title: r.get(3)?,
        content: r.get(4)?,
        posted_at: r.get(5)?,
        in_reply_to_id: r.get(6)?,
        media: Vec::new(),
        truncated: r.get::<_, String>(7)? == "truncated",
        body_json: r.get(8)?,
        score: r.get(9)?,
    }))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Evaluate `posts` in order. `dry_run` records `skipped` / `dry-run` and applies nothing.
pub fn evaluate(c: &Connection, db: &Path, posts: &[Post], source: &str, dry_run: bool) -> Result<Vec<Firing>> {
    let mut pass = Pass { conn: c, db, posts, source, dry_run, out: Vec::new() };
    let rules: Vec<Rule> = list_rules(c)?.into_iter().filter(|r| r.enabled).collect();
    for rule in &rules {
        pass.one_rule(rule)?;
    }
    Ok(pass.out)
}

struct Pass<'a> {
    conn: &'a Connection,
    db: &'a Path,
    posts: &'a [Post],
    source: &'a str,
    dry_run: bool,
    out: Vec<Firing>,
}

impl Pass<'_> {
    fn one_rule(&mut self, rule: &Rule) -> Result<()> {
        if !matches!(rule.trigger, Trigger::PostSynced) { return Ok(()); }
        for post in self.posts {
            if self.already_fired(rule.id, &post.post_id)? {
                self.record(rule, &post.post_id, "rule", "skipped", "already fired")?;
                continue;
            }
            match self.matches(rule, post)? {
                Match::No => {}
                Match::Bad(detail) => self.record(rule, &post.post_id, "rule", "error", &detail)?,
                Match::Yes => self.apply_all(rule, post)?,
            }
        }
        Ok(())
    }

    fn already_fired(&self, rule_id: i64, post_id: &str) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM rule_firings WHERE rule_id = ?1 AND post_id = ?2 AND outcome = 'fired'",
            params![rule_id, post_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    fn matches(&self, rule: &Rule, post: &Post) -> Result<Match> {
        for cond in &rule.conditions {
            match self.one_condition(cond, post)? {
                Match::Yes => {}
                other => return Ok(other),
            }
        }
        Ok(Match::Yes)
    }

    fn one_condition(&self, cond: &Condition, post: &Post) -> Result<Match> {
        let hay = format!("{}\n{}", post.title, post.content);
        let yes = Match::Yes;
        let no = Match::No;
        Ok(match cond {
            Condition::Author { nick } => if same_nick(&post.nick, nick) { yes } else { no },
            Condition::Keyword { text } => if contains_ci(&hay, text) { yes } else { no },
            Condition::Regex { pattern } => match regex::Regex::new(pattern) {
                Ok(re) => if re.is_match(&hay) { yes } else { no },
                Err(e) => Match::Bad(format!("invalid regex: {e}")),
            },
            Condition::Source { source } => if same_source(source, self.source) { yes } else { no },
            Condition::Tag { tag } => if self.has_tag(post, tag)? { yes } else { no },
            Condition::SavedSearch { keyword, nicks } => {
                let text_ok = contains_ci(&hay, keyword);
                let nick_ok = nicks.is_empty() || nicks.iter().any(|n| same_nick(&post.nick, n));
                if text_ok && nick_ok { yes } else { no }
            }
            Condition::RepliesTo { nick } => if self.replies_to(post, nick)? { yes } else { no },
        })
    }

    fn has_tag(&self, post: &Post, tag: &str) -> Result<bool> {
        let want = tag.trim().trim_start_matches('#').to_ascii_lowercase();
        if want.is_empty() { return Ok(false); }
        let from_body = extract_tags(&post.body_json, &post.content);
        if from_body.iter().any(|t| t.eq_ignore_ascii_case(&want)) { return Ok(true); }
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM post_tags WHERE post_id = ?1 AND lower(tag) = ?2",
            params![post.post_id, want],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    fn replies_to(&self, post: &Post, nick: &str) -> Result<bool> {
        let parent = if !post.in_reply_to_id.is_empty() { post.in_reply_to_id.as_str() } else { post.parent_post_id.as_str() };
        if parent.is_empty() { return Ok(false); }
        let found = if let Some(p) = self.posts.iter().find(|p| p.post_id == parent) {
            Some(p.nick.clone())
        } else {
            self.conn.query_row("SELECT nick FROM posts WHERE post_id = ?1", params![parent], |r| r.get(0)).optional()?
        };
        Ok(found.is_some_and(|n| same_nick(&n, nick)))
    }

    fn apply_all(&mut self, rule: &Rule, post: &Post) -> Result<()> {
        if rule.actions.is_empty() {
            return self.record(rule, &post.post_id, "rule", "error", "a rule needs an action");
        }
        for action in &rule.actions {
            self.apply_one(rule, post, action)?;
        }
        Ok(())
    }

    fn apply_one(&mut self, rule: &Rule, post: &Post, action: &RuleAction) -> Result<()> {
        let kind = action_kind(action);
        if self.dry_run {
            return self.record(rule, &post.post_id, &kind, "skipped", "dry-run");
        }
        let result = self.effect(post, action);
        match result {
            Ok(detail) => self.record(rule, &post.post_id, &kind, "fired", &detail),
            Err(e) => self.record(rule, &post.post_id, &kind, "error", &format!("{e:#}")),
        }
    }

    fn effect(&self, post: &Post, action: &RuleAction) -> Result<String> {
        match action {
            RuleAction::Notify => {
                let url = thread_url(&post.post_id);
                let detail = format!("@{} {} {url}", post.nick, post.post_id);
                info!("rule notify: {detail}");
                Ok(detail)
            }
            RuleAction::Bookmark => {
                self.conn.execute_batch(BOOKMARKS_SCHEMA)?;
                let n = add_bookmarks(self.conn, &[post.post_id.clone()], &now_stamp())?;
                Ok(format!("bookmarked {n}"))
            }
            RuleAction::Tag { tag } => {
                let tag = tag.trim().trim_start_matches('#').trim();
                if tag.is_empty() { bail!("tag action needs a tag"); }
                self.conn.execute("INSERT OR IGNORE INTO post_tags (post_id, tag) VALUES (?1, ?2)", params![post.post_id, tag])?;
                Ok(format!("tagged {tag}"))
            }
            RuleAction::Research { name } => {
                let name = name.trim();
                if name.is_empty() { bail!("research action needs a project name"); }
                let rc = research_conn(self.db)?;
                let (id, _) = get_or_create(&rc, name, None)?;
                let (items, missing) = post_items(self.db, &[post.post_id.clone()], None, json!({}), "")?;
                if !missing.is_empty() { bail!("post {} is not in the database", post.post_id); }
                let mut added = 0usize;
                for it in &items {
                    if add_item(&rc, id, it)? { added += 1; }
                }
                Ok(format!("research {name} ({added} new)"))
            }
            RuleAction::Export => {
                let dir = self.db.parent().unwrap_or_else(|| Path::new(".")).join("exports").join("rules");
                std::fs::create_dir_all(&dir)?;
                let path = dir.join(format!("{}.json", post.post_id));
                let body = serde_json::to_string_pretty(&json!({
                    "post_id": post.post_id,
                    "nick": post.nick,
                    "title": post.title,
                    "content": post.content,
                    "posted_at": post.posted_at,
                    "url": thread_url(&post.post_id),
                }))?;
                std::fs::write(&path, body)?;
                Ok(path.display().to_string())
            }
        }
    }

    fn record(&mut self, rule: &Rule, post_id: &str, action: &str, outcome: &str, detail: &str) -> Result<()> {
        let stamp = now_stamp();
        self.conn.execute(
            "INSERT INTO rule_firings (rule_id, post_id, source, action, outcome, detail, fired_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![rule.id, post_id, self.source, action, outcome, detail, stamp],
        )?;
        self.out.push(Firing {
            rule_id: rule.id,
            rule_name: rule.name.clone(),
            post_id: post_id.to_string(),
            source: self.source.to_string(),
            action: action.to_string(),
            outcome: outcome.to_string(),
            detail: detail.to_string(),
        });
        Ok(())
    }
}

enum Match { Yes, No, Bad(String) }

fn action_kind(action: &RuleAction) -> String {
    match action {
        RuleAction::Notify => "notify".into(),
        RuleAction::Bookmark => "bookmark".into(),
        RuleAction::Tag { tag } => format!("tag:{tag}"),
        RuleAction::Research { name } => format!("research:{name}"),
        RuleAction::Export => "export".into(),
    }
}

fn same_nick(post_nick: &str, want: &str) -> bool {
    let want = want.trim().trim_start_matches('@');
    !want.is_empty() && post_nick.trim().eq_ignore_ascii_case(want)
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    let needle = needle.trim();
    needle.is_empty() || hay.to_ascii_lowercase().contains(&needle.to_ascii_lowercase())
}

fn same_source(cond: &str, event: &str) -> bool {
    fn norm(s: &str) -> String {
        match s.trim().to_ascii_lowercase().as_str() {
            "site" | "redstar" => "site".into(),
            other => other.to_string(),
        }
    }
    norm(cond) == norm(event)
}

/// True when `args` asked for a rules command.
pub fn cli_requested(args: &Args) -> bool {
    args.rules_list
        || args.rules_show.is_some()
        || args.rules_enable.is_some()
        || args.rules_disable.is_some()
        || args.rules_clone.is_some()
        || args.rules_delete.is_some()
        || args.rules_priority.is_some()
        || args.rules_import.is_some()
        || args.rules_export.is_some()
        || args.rules_preview
}
