//! Named local views over the database.
//!
//! A feed is rules, not a copy of anyone's posts. Sharing writes the rules
//! only. Opening a feed reads the local database. Refresh asks first, then
//! reads the nicks, tags, and searches the rules name.

use crate::prelude::*;
use crate::service::{CapabilityId, RedStar};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::Read as _;

/// Largest accepted definition, file or share string payload.
pub const MAX_BYTES: usize = 64 * 1024;
/// Rules in one feed.
pub const MAX_RULES: usize = 200;
/// A search query.
pub const MAX_QUERY: usize = 500;

const KINDS: &[&str] = &["nick", "tag", "search"];
const MODES: &[&str] = &["include", "exclude"];
const PERSONAL: &[&str] = &["email", "cookie", "cookies", "path", "local_path", "post_id", "bookmark", "note", "last_refreshed_at", "feed_seen", "session"];

/// One rule in a feed definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// `nick`, `tag`, or `search`.
    pub kind: String,
    /// `include` or `exclude`.
    #[serde(default = "include_mode")]
    pub mode: String,
    /// Nicks or tags. A search leaves this empty.
    #[serde(default)]
    pub values: Vec<String>,
    /// The search text. Empty for a nick or a tag.
    #[serde(default)]
    pub query: String,
    /// Rules that share a group are OR. Groups are AND. `0` means no grouping.
    #[serde(default)]
    pub group: i64,
}

fn include_mode() -> String {
    "include".into()
}

/// The portable definition. The same JSON is stored and shared.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    /// Always `acp-feed`.
    pub format: String,
    /// Always `1`.
    pub version: u32,
    /// Feed name.
    pub name: String,
    /// Optional sentence.
    #[serde(default)]
    pub description: String,
    /// `any` or `all` for includes. Excludes always subtract.
    #[serde(rename = "match", default = "match_any")]
    pub match_mode: String,
    /// `newest`, `oldest`, or `score`.
    #[serde(default = "sort_newest")]
    pub sort: String,
    /// Start. An absolute ISO date, or `30d`.
    #[serde(default)]
    pub since: String,
    /// End. An absolute ISO date, or `30d`.
    #[serde(default)]
    pub until: String,
    /// `posts`, `replies`, or both.
    #[serde(default)]
    pub kinds: Vec<String>,
    /// Drop posts below this score. A missing score is dropped too.
    #[serde(default)]
    pub min_score: Option<i64>,
    /// Keep posts that have a stored attachment.
    #[serde(default)]
    pub has_media: bool,
    /// The rules, in file order.
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
}

fn match_any() -> String {
    "any".into()
}
fn sort_newest() -> String {
    "newest".into()
}

impl Definition {
    /// A new empty feed.
    pub fn new(name: &str, description: &str) -> Self {
        Definition {
            format: "acp-feed".into(),
            version: 1,
            name: name.trim().into(),
            description: description.trim().into(),
            match_mode: "any".into(),
            sort: "newest".into(),
            since: String::new(),
            until: String::new(),
            kinds: Vec::new(),
            min_score: None,
            has_media: false,
            rules: Vec::new(),
        }
    }

    /// Canonical JSON. Field order is the struct order.
    pub fn canonical(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

/// Refuse a definition that is too big, the wrong version, or carrying a personal field.
pub fn validate(def: &Definition) -> Result<()> {
    if def.format != "acp-feed" {
        bail!("a feed file needs format = \"acp-feed\"");
    }
    if def.version != 1 {
        bail!("feed version {} is not supported", def.version);
    }
    check_name(&def.name)?;
    if def.description.chars().count() > 500 {
        bail!("the description is longer than 500 characters");
    }
    if !matches!(def.match_mode.as_str(), "any" | "all") {
        bail!("match must be any or all");
    }
    if !matches!(def.sort.as_str(), "newest" | "oldest" | "score") {
        bail!("sort must be newest, oldest, or score");
    }
    for kind in &def.kinds {
        if !matches!(kind.as_str(), "posts" | "replies") {
            bail!("kinds must be posts or replies");
        }
    }
    if def.rules.len() > MAX_RULES {
        bail!("a feed can have at most {MAX_RULES} rules");
    }
    for rule in &def.rules {
        if !KINDS.contains(&rule.kind.as_str()) {
            bail!("a rule kind must be nick, tag, or search");
        }
        if !MODES.contains(&rule.mode.as_str()) {
            bail!("a rule mode must be include or exclude");
        }
        match rule.kind.as_str() {
            "nick" => {
                if rule.values.is_empty() {
                    bail!("a nick rule needs a nick");
                }
                for nick in &rule.values {
                    if !nick_ok(nick) {
                        bail!("nick {nick} is not a nick");
                    }
                }
            }
            "tag" => {
                if rule.values.is_empty() {
                    bail!("a tag rule needs a tag");
                }
                for tag in &rule.values {
                    if !tag_ok(tag) {
                        bail!("tag {tag} is not a tag");
                    }
                }
            }
            "search" => {
                let n = rule.query.chars().count();
                if n == 0 || n > MAX_QUERY {
                    bail!("a search rule needs a query of 1 to {MAX_QUERY} characters");
                }
            }
            _ => {}
        }
    }
    let raw = def.canonical()?;
    if raw.len() > MAX_BYTES {
        bail!("the feed definition is larger than 64 KiB");
    }
    for key in PERSONAL {
        if raw.contains(&format!("\"{key}\"")) {
            bail!("a shared feed must not contain {key}");
        }
    }
    Ok(())
}

fn check_name(name: &str) -> Result<()> {
    let n = name.chars().count();
    if n == 0 || n > 80 {
        bail!("a feed name is 1 to 80 characters");
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_') {
        bail!("a feed name uses letters, digits, spaces, hyphens, and underscores");
    }
    Ok(())
}

fn nick_ok(nick: &str) -> bool {
    let nick = nick.trim().trim_start_matches('@');
    !nick.is_empty() && nick.len() <= 40 && nick.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn tag_ok(tag: &str) -> bool {
    let tag = tag.trim().trim_start_matches('#');
    !tag.is_empty() && tag.len() <= 40 && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Parse TOML or JSON. Unknown keys are dropped and named. A personal key is refused.
pub fn parse_definition(text: &str) -> Result<Definition> {
    if text.len() > MAX_BYTES {
        bail!("the feed definition is larger than 64 KiB");
    }
    let value = if text.trim_start().starts_with('{') {
        serde_json::from_str::<Value>(text).context("reading the feed JSON")?
    } else {
        toml::from_str::<Value>(text).context("reading the feed TOML")?
    };
    let value = warn_unknown(value)?;
    let def: Definition = serde_json::from_value(value).context("the feed definition")?;
    validate(&def)?;
    Ok(def)
}

fn warn_unknown(value: Value) -> Result<Value> {
    let Value::Object(map) = value else { return Ok(value) };
    let known = ["format", "version", "name", "description", "match", "sort", "since", "until", "kinds", "min_score", "has_media", "rule"];
    let mut kept = serde_json::Map::new();
    for (k, v) in map {
        if PERSONAL.contains(&k.as_str()) {
            bail!("a shared feed must not contain {k}");
        }
        if known.contains(&k.as_str()) {
            kept.insert(k, v);
        } else {
            eprintln!("warning: ignored unknown feed key {k}");
        }
    }
    Ok(Value::Object(kept))
}

/// Optional filters applied when a feed is created or edited.
#[derive(Clone, Debug, Default)]
pub struct Filters {
    /// `any` or `all`.
    pub match_mode: Option<String>,
    /// `newest`, `oldest`, or `score`.
    pub sort: Option<String>,
    /// Start.
    pub since: Option<String>,
    /// End.
    pub until: Option<String>,
    /// `posts` and `replies`.
    pub kinds: Option<Vec<String>>,
    /// Drop posts below this score.
    pub min_score: Option<i64>,
    /// Keep posts that have a stored attachment.
    pub has_media: Option<bool>,
}

impl Filters {
    /// Filters named on the command line. Empty means leave the stored values.
    pub fn from_args(args: &Args) -> Self {
        let kinds = args.feed_kinds.as_deref().map(|s| {
            s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
        });
        Filters {
            match_mode: args.feed_match.clone(),
            sort: args.feed_sort.clone(),
            since: args.feed_since.clone(),
            until: args.feed_until.clone(),
            kinds,
            min_score: args.feed_min_score,
            has_media: args.feed_has_media.then_some(true),
        }
    }

    /// Write the set fields onto `def`.
    pub fn apply(&self, def: &mut Definition) {
        if let Some(v) = &self.match_mode { def.match_mode.clone_from(v); }
        if let Some(v) = &self.sort { def.sort.clone_from(v); }
        if let Some(v) = &self.since { def.since.clone_from(v); }
        if let Some(v) = &self.until { def.until.clone_from(v); }
        if let Some(v) = &self.kinds { def.kinds.clone_from(v); }
        if self.min_score.is_some() { def.min_score = self.min_score; }
        if let Some(v) = self.has_media { def.has_media = v; }
    }
}

/// `acpfeed1:` and the deflated canonical JSON.
pub fn share_string(def: &Definition) -> Result<String> {
    validate(def)?;
    let raw = def.canonical()?;
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
    std::io::Write::write_all(&mut enc, raw.as_bytes())?;
    let bytes = enc.finish()?;
    Ok(format!("acpfeed1:{}", data_encoding::BASE64URL_NOPAD.encode(&bytes)))
}

/// Read a share string back into a definition.
pub fn parse_share(text: &str) -> Result<Definition> {
    let payload = text.trim().strip_prefix("acpfeed1:").context("a share string starts with acpfeed1:")?;
    let bytes = data_encoding::BASE64URL_NOPAD.decode(payload.as_bytes()).context("the share string is not base64url")?;
    if bytes.len() > MAX_BYTES {
        bail!("the feed definition is larger than 64 KiB");
    }
    let mut dec = DeflateDecoder::new(&bytes[..]);
    let mut raw = String::new();
    dec.read_to_string(&mut raw).context("the share string did not inflate")?;
    if raw.len() > MAX_BYTES {
        bail!("the feed definition is larger than 64 KiB");
    }
    parse_definition(&raw)
}

fn stamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn open() -> Result<Connection> {
    crate::crypto::cmd::prepare_db_quiet()?;
    let c = open_db(&home()?.join(DB_REL_PATH))?;
    ensure_posts_schema(&c)?;
    Ok(c)
}

fn open_existing() -> Result<Option<Connection>> {
    let path = home()?.join(DB_REL_PATH);
    if !path.is_file() {
        return Ok(None);
    }
    let c = open_db(&path)?;
    ensure_posts_schema(&c)?;
    Ok(Some(c))
}

fn hash_def(def: &Definition) -> Result<String> {
    Ok(blake3::hash(def.canonical()?.as_bytes()).to_hex().to_string())
}

/// Create a feed. The same name updates the description and the filters and keeps the rules.
pub fn create(c: &Connection, name: &str, description: &str, filters: &Filters) -> Result<String> {
    RedStar::require(CapabilityId::Feeds)?;
    ensure_posts_schema(c)?;
    if let Some((id, mut def)) = find_feed(c, name)? {
        if !description.trim().is_empty() { def.description = description.trim().into(); }
        filters.apply(&mut def);
        store_def(c, id, &def, "local")?;
        return Ok(format!("feed {} updated", def.name));
    }
    let mut def = Definition::new(name, description);
    filters.apply(&mut def);
    validate(&def)?;
    let now = stamp();
    let json = def.canonical()?;
    c.execute(
        "INSERT INTO feeds (name, description, definition, source, origin_hash, created_at, updated_at, last_refreshed_at)
         VALUES (?1, ?2, ?3, 'local', ?4, ?5, ?5, NULL)",
        params![def.name, def.description, json, hash_def(&def)?, now],
    )
    .context("creating the feed")?;
    Ok(format!("feed {} created", def.name))
}

fn find_feed(c: &Connection, name: &str) -> Result<Option<(i64, Definition)>> {
    let Some((id, raw)) = c
        .query_row("SELECT id, definition FROM feeds WHERE name = ?1 COLLATE NOCASE", params![name.trim()], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .optional()?
    else {
        return Ok(None);
    };
    let def: Definition = serde_json::from_str(&raw).context("stored feed definition")?;
    Ok(Some((id, def)))
}

fn feed_id(c: &Connection, name: &str) -> Result<(i64, Definition)> {
    find_feed(c, name)?.with_context(|| format!("no feed {name}"))
}

fn store_def(c: &Connection, id: i64, def: &Definition, source: &str) -> Result<()> {
    validate(def)?;
    let json = def.canonical()?;
    c.execute("DELETE FROM feed_rules WHERE feed_id = ?1", params![id])?;
    c.execute(
        "UPDATE feeds SET name = ?2, description = ?3, definition = ?4, source = ?5, origin_hash = ?6, updated_at = ?7 WHERE id = ?1",
        params![id, def.name, def.description, json, source, hash_def(def)?, stamp()],
    )?;
    let mut ins = c.prepare("INSERT INTO feed_rules (feed_id, kind, value, mode, grp) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    for rule in &def.rules {
        let values = if rule.kind == "search" { vec![rule.query.clone()] } else { rule.values.clone() };
        for value in values {
            ins.execute(params![id, rule.kind, value, rule.mode, rule.group])?;
        }
    }
    Ok(())
}

/// Add one rule. A missing feed is created. `values` are nicks or tags. `query` is the search text.
pub fn add_rule(c: &Connection, name: &str, kind: &str, values: &[String], query: &str, exclude: bool, filters: &Filters) -> Result<String> {
    RedStar::require(CapabilityId::Feeds)?;
    ensure_posts_schema(c)?;
    if find_feed(c, name)?.is_none() {
        create(c, name, "", filters)?;
    }
    let (id, mut def) = feed_id(c, name)?;
    filters.apply(&mut def);
    def.rules.push(Rule {
        kind: kind.into(),
        mode: if exclude { "exclude".into() } else { "include".into() },
        values: values.iter().map(|v| v.trim().trim_start_matches(['@', '#']).to_string()).filter(|v| !v.is_empty()).collect(),
        query: query.trim().into(),
        group: 0,
    });
    store_def(c, id, &def, "local")?;
    Ok(format!("feed {} rules={}", def.name, def.rules.len()))
}

/// Remove rules of `kind` whose value or query is in `values`.
pub fn remove_rule(c: &Connection, name: &str, kind: &str, values: &[String]) -> Result<String> {
    RedStar::require(CapabilityId::Feeds)?;
    let (id, mut def) = feed_id(c, name)?;
    let want: Vec<String> = values.iter().map(|v| v.trim().trim_start_matches(['@', '#']).to_lowercase()).collect();
    let before = def.rules.len();
    def.rules.retain(|rule| {
        if rule.kind != kind {
            return true;
        }
        let hit = if kind == "search" {
            want.iter().any(|w| w == &rule.query.to_lowercase())
        } else {
            rule.values.iter().any(|v| want.iter().any(|w| w == &v.to_lowercase()))
        };
        !hit
    });
    store_def(c, id, &def, "local")?;
    Ok(format!("feed {} removed={} rules={}", def.name, before - def.rules.len(), def.rules.len()))
}

/// One feed, for the nick bar and the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedSummary {
    /// Feed name.
    pub name: String,
    /// Description.
    pub description: String,
    /// How many rules.
    pub rules: usize,
    /// Matching posts.
    pub matches: usize,
    /// Matching posts not yet marked seen.
    pub unread: usize,
    /// A short rule line.
    pub summary: String,
}

fn rule_summary(def: &Definition) -> String {
    if def.rules.is_empty() {
        return "no rules".into();
    }
    def.rules.iter().map(|r| {
        let what = if r.kind == "search" { r.query.clone() } else { r.values.join(",") };
        format!("{} {} {what}", r.mode, r.kind)
    }).collect::<Vec<_>>().join("; ")
}

/// Feeds in name order, with a match count.
pub fn summaries(c: &Connection, now: chrono::DateTime<chrono::Utc>) -> Result<Vec<FeedSummary>> {
    RedStar::require(CapabilityId::Feeds)?;
    ensure_posts_schema(c)?;
    let mut st = c.prepare("SELECT name FROM feeds ORDER BY name COLLATE NOCASE")?;
    let names: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for name in names {
        let (id, def) = feed_id(c, &name)?;
        let ids = matching_ids(c, &def, now)?;
        let seen: i64 = c.query_row("SELECT count(*) FROM feed_seen WHERE feed_id = ?1", params![id], |r| r.get(0))?;
        let unread = ids.len().saturating_sub(seen as usize);
        out.push(FeedSummary {
            name: def.name.clone(),
            description: def.description.clone(),
            rules: def.rules.len(),
            matches: ids.len(),
            unread,
            summary: rule_summary(&def),
        });
    }
    Ok(out)
}

/// Feed names and how many matching posts are still unseen.
pub fn list(c: &Connection, now: chrono::DateTime<chrono::Utc>) -> Result<Vec<String>> {
    let rows = summaries(c, now)?;
    if rows.is_empty() {
        return Ok(vec!["feed none".into()]);
    }
    Ok(rows.into_iter().map(|r| format!("feed {} rules={} unread={}", r.name, r.rules, r.unread)).collect())
}

/// The definition and the matching post ids. Does not mark them seen.
pub fn load(c: &Connection, name: &str, now: chrono::DateTime<chrono::Utc>) -> Result<(Definition, Vec<String>)> {
    RedStar::require(CapabilityId::Feeds)?;
    let (_, def) = feed_id(c, name)?;
    let ids = matching_ids(c, &def, now)?;
    Ok((def, ids))
}

/// Set the description and the filters. The rules stay.
pub fn edit_meta(c: &Connection, name: &str, description: Option<&str>, filters: &Filters) -> Result<String> {
    RedStar::require(CapabilityId::Feeds)?;
    let (id, mut def) = feed_id(c, name)?;
    if let Some(d) = description {
        def.description = d.trim().into();
    }
    filters.apply(&mut def);
    store_def(c, id, &def, "local")?;
    Ok(format!("feed {} updated", def.name))
}

/// Post ids the definition matches. `now` is the clock for `30d`.
pub fn matching_ids(c: &Connection, def: &Definition, now: chrono::DateTime<chrono::Utc>) -> Result<Vec<String>> {
    let (sql, params) = compile(def, now)?;
    let mut st = c.prepare(&sql)?;
    let rows = st.query_map(rusqlite::params_from_iter(params.iter()), |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// The parameterised query. Values are bound, not pasted into the SQL.
pub fn compile(def: &Definition, now: chrono::DateTime<chrono::Utc>) -> Result<(String, Vec<String>)> {
    validate(def)?;
    let mut params = Vec::new();
    let mut where_sql = Vec::new();
    let includes: Vec<&Rule> = def.rules.iter().filter(|r| r.mode == "include").collect();
    let excludes: Vec<&Rule> = def.rules.iter().filter(|r| r.mode == "exclude").collect();
    if !includes.is_empty() {
        let grouped = includes.iter().any(|r| r.group != 0);
        if grouped {
            let mut groups: Vec<i64> = includes.iter().map(|r| r.group).collect();
            groups.sort_unstable();
            groups.dedup();
            let mut parts = Vec::new();
            for g in groups {
                let clauses: Vec<String> = includes.iter().filter(|r| r.group == g).map(|r| rule_clause(r, &mut params)).collect::<Result<_>>()?;
                parts.push(format!("({})", clauses.join(" OR ")));
            }
            where_sql.push(format!("({})", parts.join(" AND ")));
        } else if def.match_mode == "all" {
            let clauses: Vec<String> = includes.iter().map(|r| rule_clause(r, &mut params)).collect::<Result<_>>()?;
            where_sql.push(format!("({})", clauses.join(" AND ")));
        } else {
            let clauses: Vec<String> = includes.iter().map(|r| rule_clause(r, &mut params)).collect::<Result<_>>()?;
            where_sql.push(format!("({})", clauses.join(" OR ")));
        }
    }
    for rule in excludes {
        where_sql.push(format!("NOT ({})", rule_clause(rule, &mut params)?));
    }
    if let Some(cut) = bound(&def.since, now)? {
        params.push(cut);
        where_sql.push(format!("posted_at >= ?{}", params.len()));
    }
    if let Some(cut) = bound(&def.until, now)? {
        params.push(cut);
        where_sql.push(format!("posted_at <= ?{}", params.len()));
    }
    let posts = def.kinds.iter().any(|k| k == "posts");
    let replies = def.kinds.iter().any(|k| k == "replies");
    if posts && !replies {
        where_sql.push("parent_post_id = ''".into());
    } else if replies && !posts {
        where_sql.push("parent_post_id <> ''".into());
    }
    if let Some(score) = def.min_score {
        params.push(score.to_string());
        where_sql.push(format!("score >= CAST(?{} AS INTEGER)", params.len()));
    }
    if def.has_media {
        where_sql.push("EXISTS (SELECT 1 FROM attachments WHERE attachments.post_id = posts.post_id)".into());
    }
    let where_sql = if where_sql.is_empty() { "1".into() } else { where_sql.join(" AND ") };
    let order = match def.sort.as_str() {
        "oldest" => "posted_at ASC, post_id ASC",
        "score" => "score DESC, posted_at DESC, post_id ASC",
        _ => "posted_at DESC, post_id ASC",
    };
    Ok((format!("SELECT post_id FROM posts WHERE {where_sql} ORDER BY {order}"), params))
}

fn rule_clause(rule: &Rule, params: &mut Vec<String>) -> Result<String> {
    match rule.kind.as_str() {
        "nick" => {
            let mut slots = Vec::new();
            for nick in &rule.values {
                params.push(nick.trim().trim_start_matches('@').to_lowercase());
                slots.push(format!("?{}", params.len()));
            }
            Ok(format!("lower(nick) IN ({})", slots.join(", ")))
        }
        "tag" => {
            let mut slots = Vec::new();
            for tag in &rule.values {
                params.push(tag.trim().trim_start_matches('#').to_lowercase());
                slots.push(format!("?{}", params.len()));
            }
            Ok(format!("post_id IN (SELECT post_id FROM post_tags WHERE lower(tag) IN ({}))", slots.join(", ")))
        }
        "search" => {
            let escaped = rule.query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            params.push(format!("%{escaped}%"));
            let i = params.len();
            Ok(format!("(title LIKE ?{i} ESCAPE '\\' OR content LIKE ?{i} ESCAPE '\\')"))
        }
        other => bail!("unknown rule kind {other}"),
    }
}

fn bound(text: &str, now: chrono::DateTime<chrono::Utc>) -> Result<Option<String>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if let Some(days) = text.strip_suffix('d')
        && let Ok(n) = days.parse::<i64>()
        && n >= 0
    {
        let at = now - chrono::Duration::days(n);
        return Ok(Some(at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)));
    }
    if chrono::DateTime::parse_from_rfc3339(text).is_ok() || text.len() >= 10 && text.as_bytes().get(4) == Some(&b'-') {
        return Ok(Some(text.to_string()));
    }
    bail!("a date is an ISO time or a number of days like 30d");
}

/// Matching posts, then mark them seen.
pub fn show(c: &Connection, name: &str, now: chrono::DateTime<chrono::Utc>) -> Result<Vec<String>> {
    RedStar::require(CapabilityId::Feeds)?;
    let (id, def) = feed_id(c, name)?;
    let ids = matching_ids(c, &def, now)?;
    let seen = stamp();
    let mut mark = c.prepare("INSERT OR IGNORE INTO feed_seen (feed_id, post_id, seen_at) VALUES (?1, ?2, ?3)")?;
    let mut line = c.prepare("SELECT nick, posted_at, title FROM posts WHERE post_id = ?1")?;
    let mut out = vec![format!("feed {} matches={}", def.name, ids.len())];
    for post_id in &ids {
        mark.execute(params![id, post_id, seen])?;
        if let Some((nick, at, title)) = line.query_row(params![post_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))).optional()? {
            out.push(format!("post {post_id} @{nick} {at} {title}"));
        }
    }
    Ok(out)
}

/// Mark the feed's current matches seen. A missing database, or one that is locked, is an error.
pub fn mark_seen_path(path: &Path, name: &str) -> Result<()> {
    let c = open_db(path)?;
    ensure_posts_schema(&c)?;
    let (id, def) = feed_id(&c, name)?;
    let ids = matching_ids(&c, &def, chrono::Utc::now())?;
    let seen = stamp();
    let mut mark = c.prepare("INSERT OR IGNORE INTO feed_seen (feed_id, post_id, seen_at) VALUES (?1, ?2, ?3)")?;
    for post_id in ids {
        mark.execute(params![id, post_id, seen])?;
    }
    Ok(())
}

/// The fetches a refresh would make. Excludes are not fetched.
pub fn refresh_plan(def: &Definition) -> Vec<String> {
    let mut out = Vec::new();
    for rule in def.rules.iter().filter(|r| r.mode == "include") {
        match rule.kind.as_str() {
            "nick" => {
                for nick in &rule.values {
                    out.push(format!("refresh nick {}", nick.trim().trim_start_matches('@')));
                }
            }
            "tag" => {
                for tag in &rule.values {
                    out.push(format!("refresh tag {}", tag.trim().trim_start_matches('#')));
                }
            }
            "search" => out.push(format!("refresh search {}", rule.query)),
            _ => {}
        }
    }
    if out.is_empty() {
        out.push("refresh none".into());
    }
    out
}

/// Delete a feed and its rules and seen rows.
pub fn delete_feed(c: &Connection, name: &str) -> Result<String> {
    RedStar::require(CapabilityId::Feeds)?;
    let (id, def) = feed_id(c, name)?;
    c.execute("DELETE FROM feed_seen WHERE feed_id = ?1", params![id])?;
    c.execute("DELETE FROM feed_rules WHERE feed_id = ?1", params![id])?;
    c.execute("DELETE FROM feeds WHERE id = ?1", params![id])?;
    Ok(format!("feed {} deleted", def.name))
}

/// Write an imported definition. `replace` overwrites the same name.
pub fn import_def(c: &Connection, def: &Definition, rename: Option<&str>, replace: bool) -> Result<String> {
    RedStar::require(CapabilityId::Feeds)?;
    ensure_posts_schema(c)?;
    let mut def = def.clone();
    if let Some(name) = rename {
        def.name = name.trim().into();
    }
    validate(&def)?;
    let existing: Option<i64> = c.query_row("SELECT id FROM feeds WHERE name = ?1 COLLATE NOCASE", params![def.name], |r| r.get(0)).optional()?;
    if let Some(id) = existing {
        if !replace {
            bail!("feed {} already exists. Pass --replace, or --feed-name for a new name.", def.name);
        }
        store_def(c, id, &def, "imported")?;
        return Ok(format!("feed {} replaced", def.name));
    }
    let now = stamp();
    c.execute(
        "INSERT INTO feeds (name, description, definition, source, origin_hash, created_at, updated_at, last_refreshed_at)
         VALUES (?1, ?2, ?3, 'imported', ?4, ?5, ?5, NULL)",
        params![def.name, def.description, def.canonical()?, hash_def(&def)?, now],
    )?;
    let id: i64 = c.query_row("SELECT id FROM feeds WHERE name = ?1 COLLATE NOCASE", params![def.name], |r| r.get(0))?;
    store_def(c, id, &def, "imported")?;
    Ok(format!("feed {} imported", def.name))
}

/// TOML definition. It has no seen rows and no refresh time.
pub fn to_toml(def: &Definition) -> Result<String> {
    validate(def)?;
    let mut value = serde_json::to_value(def)?;
    if let Some(obj) = value.as_object_mut() {
        obj.retain(|_, v| !v.is_null());
    }
    toml::to_string_pretty(&value).context("writing the feed TOML")
}

/// Whether the command line asked for a feed.
pub fn cli_requested(args: &Args) -> bool {
    args.feed_list
        || args.feed_create.is_some()
        || args.feed_edit.is_some()
        || args.feed_add.is_some()
        || args.feed_remove_rule.is_some()
        || args.feed_show.is_some()
        || args.feed_refresh.is_some()
        || args.feed_export.is_some()
        || args.feed_share.is_some()
        || args.feed_import.is_some()
        || args.feed_delete.is_some()
}

fn confirm(args: &Args, prompt: &str) -> Result<bool> {
    if args.dry_run {
        return Ok(false);
    }
    if args.yes {
        return Ok(true);
    }
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        eprint!("{prompt}");
        io::stderr().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        return Ok(matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"));
    }
    Ok(false)
}

/// Run a feed command. Refresh connects only after confirm.
pub async fn run_cli(args: &Args) -> Result<()> {
    if args.feed_list {
        println!("{}", show_list()?);
        return Ok(());
    }
    crate::crypto::cmd::prepare_db()?;
    let now = chrono::Utc::now();
    let filters = Filters::from_args(args);
    if let Some(name) = args.feed_create.as_deref() {
        let c = open()?;
        println!("{}", create(&c, name, args.feed_description.as_deref().unwrap_or(""), &filters)?);
        return Ok(());
    }
    if let Some(name) = args.feed_edit.as_deref() {
        let c = open()?;
        println!("{}", edit_meta(&c, name, args.feed_description.as_deref(), &filters)?);
        return Ok(());
    }
    if let Some(name) = args.feed_add.as_deref() {
        let c = open()?;
        let mut lines = Vec::new();
        if !args.feed_nick.is_empty() {
            lines.push(add_rule(&c, name, "nick", &args.feed_nick, "", args.feed_exclude, &filters)?);
        }
        if !args.feed_tag.is_empty() {
            lines.push(add_rule(&c, name, "tag", &args.feed_tag, "", args.feed_exclude, &filters)?);
        }
        if let Some(q) = args.feed_search.as_deref() {
            lines.push(add_rule(&c, name, "search", &[], q, args.feed_exclude, &filters)?);
        }
        if lines.is_empty() {
            bail!("--feed-add needs --feed-nick, --feed-tag, or --feed-search");
        }
        for line in lines {
            println!("{line}");
        }
        return Ok(());
    }
    if let Some(name) = args.feed_remove_rule.as_deref() {
        let c = open()?;
        let mut values = args.feed_nick.clone();
        values.extend(args.feed_tag.iter().cloned());
        if let Some(q) = &args.feed_search {
            values.push(q.clone());
        }
        let kind = if !args.feed_nick.is_empty() {
            "nick"
        } else if !args.feed_tag.is_empty() {
            "tag"
        } else if args.feed_search.is_some() {
            "search"
        } else {
            bail!("--feed-remove-rule needs --feed-nick, --feed-tag, or --feed-search");
        };
        println!("{}", remove_rule(&c, name, kind, &values)?);
        return Ok(());
    }
    if let Some(name) = args.feed_show.as_deref() {
        let c = open()?;
        for line in show(&c, name, now)? {
            println!("{line}");
        }
        return Ok(());
    }
    if let Some(name) = args.feed_share.as_deref() {
        let c = open()?;
        let (_, def) = feed_id(&c, name)?;
        if args.feed_share_string {
            println!("{}", share_string(&def)?);
        }
        if let Some(path) = &args.output {
            std::fs::write(path, to_toml(&def)?).with_context(|| format!("writing {}", path.display()))?;
            println!("wrote {}", path.display());
        } else if !args.feed_share_string {
            println!("{}", to_toml(&def)?);
        }
        return Ok(());
    }
    if let Some(src) = args.feed_import.as_deref() {
        let text = if let Some(rest) = src.trim().strip_prefix("acpfeed1:") {
            parse_share(&format!("acpfeed1:{rest}"))?
        } else if src == "-" {
            let mut raw = String::new();
            io::stdin().read_to_string(&mut raw)?;
            parse_definition(&raw)?
        } else if std::path::Path::new(src).is_file() {
            let raw = std::fs::read_to_string(src).with_context(|| format!("reading {src}"))?;
            if raw.trim_start().starts_with("acpfeed1:") { parse_share(&raw)? } else { parse_definition(&raw)? }
        } else {
            bail!("--feed-import needs a file, -, or an acpfeed1: string");
        };
        let c = open()?;
        println!("{}", import_def(&c, &text, args.feed_name.as_deref(), args.feed_replace)?);
        return Ok(());
    }
    if let Some(name) = args.feed_export.as_deref() {
        let c = open()?;
        let (_, def) = feed_id(&c, name)?;
        let ids = matching_ids(&c, &def, now)?;
        let db = home()?.join(DB_REL_PATH);
        let (posts, atts, _) = posts_by_ids(&db, &ids)?;
        export(args, &def.name.replace(' ', "-"), &def.name, &posts, &atts, &[], &[], &home()?.join(EXPORTS_REL_PATH))?;
        return Ok(());
    }
    if let Some(name) = args.feed_delete.as_deref() {
        if !confirm(args, "Delete this feed? [y/N] ")? {
            println!("nothing deleted");
            return Ok(());
        }
        let c = open()?;
        println!("{}", delete_feed(&c, name)?);
        return Ok(());
    }
    if let Some(name) = args.feed_refresh.as_deref() {
        let def = {
            let c = open()?;
            feed_id(&c, name)?.1
        };
        let plan = refresh_plan(&def);
        if args.dry_run || !confirm(args, "Refresh this feed from the site? [y/N] ")? {
            println!("dry-run: no connection opened");
            for line in plan {
                println!("{line}");
            }
            if !args.dry_run {
                println!("nothing fetched");
            }
            return Ok(());
        }
        let jar = Arc::new(Jar::default());
        if let Err(e) = load_session(&jar) {
            eprintln!("ignoring saved session: {e:#}");
        }
        let http = client(jar.clone())?;
        login(&http, &jar).await?;
        let mut n = 0usize;
        for rule in def.rules.iter().filter(|r| r.mode == "include") {
            match rule.kind.as_str() {
                "nick" => {
                    for nick in &rule.values {
                        let known = HashSet::new();
                        let opts = ScrapeOptions {
                            known: &known,
                            reply_counts: &HashMap::new(),
                            full: false,
                            said_by: true,
                            limit: Some(50),
                            profile_feed: ProfileFeed::Newest,
                            order: ScrapeOrder::Newest,
                        };
                        let (posts, _, _) = scrape(&http, &jar, nick, &opts).await?;
                        let mut c = open()?;
                        n += save_posts(&mut c, &posts)?;
                        println!("refresh nick {} posts={}", nick.trim_start_matches('@'), posts.len());
                    }
                }
                "tag" => {
                    for tag in &rule.values {
                        let got = read_tag(&http, &jar, tag).await?;
                        n += got;
                        println!("refresh tag {} posts={got}", tag.trim_start_matches('#'));
                    }
                }
                "search" => {
                    let got = read_site_search(&http, &jar, &rule.query).await?;
                    n += got;
                    println!("refresh search {} posts={got}", rule.query);
                }
                _ => {}
            }
        }
        let c = open()?;
        let (id, _) = feed_id(&c, name)?;
        c.execute("UPDATE feeds SET last_refreshed_at = ?2 WHERE id = ?1", params![id, stamp()])?;
        println!("feed {name} refreshed rows={n}");
        return Ok(());
    }
    Ok(())
}

/// List feeds without creating a database when the home has none.
pub fn show_list() -> Result<String> {
    match open_existing()? {
        None => Ok("feed none".into()),
        Some(c) => Ok(list(&c, chrono::Utc::now())?.join("\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        ensure_posts_schema(&c).unwrap();
        c.execute_batch(ATTACHMENTS_SCHEMA).unwrap();
        c.execute(
            "INSERT INTO posts (post_id, parent_post_id, in_reply_to_id, nick, title, content, posted_at, body_state, body_json, score)
             VALUES ('P1', '', '', 'ada', 'Books', 'theory of the shop', '2026-10-01T00:00:00Z', 'full', '', 3),
                    ('P2', 'P1', 'P1', 'ada', 'Reply', 'a meme reply', '2026-10-02T00:00:00Z', 'full', '', 1),
                    ('P3', '', '', 'bea', 'Other', 'no tag here', '2026-09-01T00:00:00Z', 'full', '', 9)",
            [],
        )
        .unwrap();
        c.execute("INSERT INTO post_tags (post_id, tag) VALUES ('P1', 'books'), ('P2', 'memes')", []).unwrap();
        c.execute(
            "INSERT INTO attachments (post_id, url, local_path, mime_type, on_site) VALUES ('P1', 'https://example.test/a.jpg', 'a.jpg', 'image/jpeg', 1)",
            [],
        )
        .unwrap();
        c
    }

    fn sample() -> Definition {
        let mut def = Definition::new("Theory reading", "Books and theory");
        def.since = "2026-09-15T00:00:00Z".into();
        def.kinds = vec!["posts".into(), "replies".into()];
        def.rules = vec![
            Rule { kind: "tag".into(), mode: "include".into(), values: vec!["books".into()], query: String::new(), group: 0 },
            Rule { kind: "nick".into(), mode: "include".into(), values: vec!["ada".into()], query: String::new(), group: 0 },
            Rule { kind: "search".into(), mode: "include".into(), values: vec![], query: "theory".into(), group: 0 },
            Rule { kind: "tag".into(), mode: "exclude".into(), values: vec!["memes".into()], query: String::new(), group: 0 },
        ];
        def
    }

    #[test]
    fn any_includes_and_an_exclude_compile_to_bound_sql() {
        let c = mem();
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-21T00:00:00Z").unwrap().with_timezone(&chrono::Utc);
        let def = sample();
        let (sql, params) = compile(&def, now).unwrap();
        assert!(sql.contains("lower(tag) IN"), "{sql}");
        assert!(sql.contains("NOT"), "{sql}");
        assert!(!sql.contains("ada"), "{sql}");
        assert!(params.iter().any(|p| p == "ada"), "{params:?}");
        let ids = matching_ids(&c, &def, now).unwrap();
        assert_eq!(ids, ["P1"], "{sql} {params:?}");
        let mut all = def.clone();
        all.match_mode = "all".into();
        all.rules.retain(|r| r.mode == "include" && r.kind != "search");
        let ids = matching_ids(&c, &all, now).unwrap();
        assert_eq!(ids, ["P1"]);
        let mut old = Definition::new("Old", "");
        old.since = "10d".into();
        let ids = matching_ids(&c, &old, now).unwrap();
        assert!(ids.is_empty(), "{ids:?}");
    }

    #[test]
    fn a_share_string_round_trips_and_drops_personal_fields() {
        let def = sample();
        let share = share_string(&def).unwrap();
        assert!(share.starts_with("acpfeed1:"));
        let back = parse_share(&share).unwrap();
        assert_eq!(back.name, def.name);
        assert_eq!(back.rules.len(), def.rules.len());
        let toml = to_toml(&def).unwrap();
        assert!(toml.contains("format = \"acp-feed\"") || toml.contains("format = 'acp-feed'"), "{toml}");
        assert!(!toml.contains("last_refreshed"));
        assert!(!toml.contains("feed_seen"));
        let mut bad = def.clone();
        bad.version = 9;
        assert!(validate(&bad).is_err());
        let raw = "{\"format\":\"acp-feed\",\"version\":1,\"name\":\"X\",\"email\":\"a@b.c\",\"rule\":[]}";
        let err = parse_definition(raw).unwrap_err().to_string();
        assert!(err.contains("email"), "{err}");
    }

    #[test]
    fn create_show_and_import_do_not_copy_posts() {
        let c = mem();
        let now = chrono::Utc::now();
        assert_eq!(create(&c, "Shop", "notes", &Filters::default()).unwrap(), "feed Shop created");
        add_rule(&c, "Shop", "nick", &["ada".into()], "", false, &Filters::default()).unwrap();
        let shown = show(&c, "Shop", now).unwrap();
        assert!(shown.iter().any(|l| l.contains("P1")), "{shown:?}");
        assert!(shown.iter().any(|l| l.contains("P2")), "{shown:?}");
        let listed = list(&c, now).unwrap().join("\n");
        assert!(listed.contains("unread=0"), "{listed}");
        let (_, def) = feed_id(&c, "Shop").unwrap();
        let share = share_string(&def).unwrap();
        let imported = parse_share(&share).unwrap();
        assert!(import_def(&c, &imported, Some("Shop 2"), false).unwrap().contains("imported"));
        let raw: String = c.query_row("SELECT definition FROM feeds WHERE name = 'Shop 2'", [], |r| r.get(0)).unwrap();
        assert!(!raw.contains("P1"), "{raw}");
        assert!(!raw.contains("session"));
    }
}
