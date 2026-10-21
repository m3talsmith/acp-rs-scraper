//! Whole-site read of Red Star (`acprs --scrape-all`).
//!
//! The frontier and the checkpoint live in the encrypted database. A run that
//! stops for `--limit` or `--scrape-budget` continues on the next run. After a
//! finished run, the next one is incremental: newest-first feeds stop when a
//! page is already known, and recent threads are read again for new replies.
//! Nothing here publishes. Text posts are a separate path.

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::prelude::*;

/// Give up on a URL after this many failures. Earlier failures wait and retry.
const MAX_ATTEMPTS: i64 = 3;
/// Longest retry wait. The floor is the polite pause, including a robots crawl-delay.
const BACKOFF_CAP_MS: u64 = 30_000;

/// What one `acprs --scrape-all` run is allowed to do.
#[derive(Clone, Debug)]
pub struct CrawlOpts {
    /// Site origin, no trailing slash. Tests pass this so they do not read the process base URL.
    pub origin: String,
    /// Stop chronological feeds at a page of known ids, and re-read recent threads.
    /// `--full` turns this off.
    pub incremental: bool,
    /// Most new posts to store this run. The frontier stays when this is hit.
    pub limit: Option<usize>,
    /// Stop after this many milliseconds. The frontier stays when this is hit.
    pub budget_ms: Option<u64>,
    /// Store attachment URLs and types. The file bytes are not downloaded.
    pub attachments: bool,
}

/// What the run stored, and whether the frontier is empty.
#[derive(Clone, Debug, Default)]
pub struct CrawlReport {
    /// Posts inserted this run. A sync passes these to the rule engine.
    pub posts: Vec<Post>,
    /// Pages fetched, including robots.txt.
    pub pages: u32,
    /// [`CrawlReport::posts`] length.
    pub new_posts: usize,
    /// Frontier rows still pending.
    pub pending: usize,
    /// The frontier drained. A limit or a time budget leaves this false.
    pub finished: bool,
    /// `post limit` or `time budget` when the run stopped early.
    pub stopped: Option<String>,
}

/// One fetched page. Fixtures and the live reader both build this.
#[derive(Clone, Debug, Default)]
struct CrawlPage {
    posts: Vec<Post>,
    raw: Vec<Value>,
    next: Option<String>,
    links: Vec<String>,
    robots: Option<String>,
}

/// Hint for the live reader. Fixtures ignore it.
#[derive(Clone, Debug, Default)]
struct FetchHint {
    kind: String,
    chronological: bool,
    first_only: bool,
    limit: Option<usize>,
    known: HashSet<String>,
}

struct Task {
    url: String,
    kind: String,
    chronological: bool,
    attempts: i64,
}

struct Policy {
    delay_ms: u64,
    disallow: Vec<String>,
}

struct Link {
    url: String,
    kind: &'static str,
    chronological: bool,
    priority: i64,
}

enum Plan {
    Fetch(Task),
    Wait(u64),
    Stop(String),
    Finished,
}

enum Absorbed {
    Continue,
    Limit,
}

/// Clock, pause, and page source for one run. The live reader sleeps. Tests advance a clock.
trait CrawlEnv: Send {
    fn fetch<'a>(&'a mut self, url: &'a str, hint: &'a FetchHint) -> Pin<Box<dyn Future<Output = Result<CrawlPage>> + Send + 'a>>;
    fn pause<'a>(&'a mut self, ms: u64) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn elapsed_ms(&mut self) -> u64;
    fn now_ms(&mut self) -> u64;
}

struct HttpEnv {
    http: reqwest::Client,
    jar: Arc<Jar>,
    start: Instant,
}

impl CrawlEnv for HttpEnv {
    fn fetch<'a>(&'a mut self, url: &'a str, hint: &'a FetchHint) -> Pin<Box<dyn Future<Output = Result<CrawlPage>> + Send + 'a>> {
        let http = self.http.clone();
        let jar = Arc::clone(&self.jar);
        let url = url.to_string();
        let hint = hint.clone();
        Box::pin(async move { fetch_http(&http, &jar, &url, &hint).await })
    }
    fn pause<'a>(&'a mut self, ms: u64) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move { tokio::time::sleep(Duration::from_millis(ms)).await; })
    }
    fn elapsed_ms(&mut self) -> u64 { self.start.elapsed().as_millis() as u64 }
    fn now_ms(&mut self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
    }
}

/// Log in is the caller's job. This only GETs pages and reads threads.
pub async fn scrape_site(http: &reqwest::Client, jar: &Arc<Jar>, db: &Path, opts: CrawlOpts) -> Result<CrawlReport> {
    let mut env = HttpEnv { http: http.clone(), jar: Arc::clone(jar), start: Instant::now() };
    run_crawl(db, &mut env, opts).await
}

async fn run_crawl(db: &Path, env: &mut dyn CrawlEnv, opts: CrawlOpts) -> Result<CrawlReport> {
    let mut c = open_db(db)?;
    let mut policy = prepare(&mut c, &opts, env.now_ms())?;
    let mut known = load_ids(&c)?;
    let mut report = CrawlReport::default();
    let mut fetched = 0u32;
    loop {
        match plan(&c, &opts, &report, env.elapsed_ms(), env.now_ms())? {
            Plan::Stop(why) => {
                report.stopped = Some(why);
                break;
            }
            Plan::Finished => {
                report.finished = true;
                break;
            }
            Plan::Wait(ms) => env.pause(ms).await,
            Plan::Fetch(task) => {
                if fetched > 0 { env.pause(polite_ms(&policy)).await; }
                if blocked(&policy, &task.url) {
                    mark(&c, &task.url, "done", 0, 0, Some("robots"), env.now_ms())?;
                    continue;
                }
                let hint = FetchHint {
                    kind: task.kind.clone(),
                    chronological: task.chronological,
                    first_only: opts.incremental && !task.chronological && task.kind != "thread" && task.kind != "robots",
                    limit: opts.limit.map(|n| n.saturating_sub(report.new_posts)),
                    known: known.clone(),
                };
                let now = env.now_ms();
                match env.fetch(&task.url, &hint).await {
                    Ok(page) => {
                        let stop = absorb(&mut c, &task, &page, &opts, &mut policy, &mut known, &mut report, now)?;
                        fetched += 1;
                        report.pages = fetched;
                        info!("{}", progress_line(report.pages, report.new_posts, pending_count(&c)?));
                        if matches!(stop, Absorbed::Limit) {
                            report.stopped = Some("post limit".into());
                            break;
                        }
                    }
                    Err(e) => fail(&c, &task, &policy, &e, now)?,
                }
            }
        }
    }
    report.pending = pending_count(&c)?;
    finish(&c, &report, env.now_ms())?;
    Ok(report)
}

/// Status line while a crawl is running.
fn progress_line(pages: u32, posts: usize, pending: usize) -> String {
    format!("crawl: {pages} pages, {posts} posts, {pending} waiting")
}

fn prepare(c: &mut Connection, opts: &CrawlOpts, now: u64) -> Result<Policy> {
    ensure_posts_schema(c)?;
    c.execute(
        "INSERT INTO crawl_checkpoint (id, running, posts_saved, pages, crawl_delay_ms, disallow) VALUES (1, 0, 0, 0, 0, '') ON CONFLICT(id) DO NOTHING",
        [],
    )?;
    let running: i64 = c.query_row("SELECT running FROM crawl_checkpoint WHERE id = 1", [], |r| r.get(0))?;
    let n: i64 = c.query_row("SELECT count(*) FROM crawl_frontier", [], |r| r.get(0))?;
    let stamp = now.to_string();
    if n == 0 {
        seed(c, opts, &stamp)?;
    } else if !opts.incremental {
        c.execute("UPDATE crawl_frontier SET status = 'pending', attempts = 0, wait_until = 0, last_error = NULL, updated_at = ?1", params![stamp])?;
        seed(c, opts, &stamp)?;
    } else if running == 0 {
        c.execute(
            "UPDATE crawl_frontier SET status = 'pending', attempts = 0, wait_until = 0, last_error = NULL, updated_at = ?1
             WHERE priority <= 1 OR url IN (SELECT url FROM crawl_recent)",
            params![stamp],
        )?;
        seed(c, opts, &stamp)?;
    } else {
        seed(c, opts, &stamp)?;
    }
    c.execute("UPDATE crawl_checkpoint SET running = 1, finished_at = NULL WHERE id = 1", [])?;
    load_policy(c)
}

fn seed(c: &Connection, opts: &CrawlOpts, stamp: &str) -> Result<()> {
    for link in seed_links(&opts.origin) {
        enqueue(c, &link, stamp)?;
    }
    Ok(())
}

fn seed_links(origin: &str) -> Vec<Link> {
    let mut v = vec![Link { url: format!("{origin}/robots.txt"), kind: "robots", chronological: false, priority: 0 }];
    let feeds = [
        ("/rs/latest", true),
        ("/rs", false),
        ("/rs/eb", true),
        ("/rs/eb?replies=1", true),
        ("/rs/explore", false),
        ("/rs/explore/latest", true),
    ];
    for (path, chronological) in feeds {
        v.push(Link { url: format!("{origin}{path}"), kind: "feed", chronological, priority: 1 });
    }
    for period in ["week", "day", "month", "year", "all"] {
        for replies in [false, true] {
            let path = if replies { format!("/rs/top?t={period}&replies=1") } else { format!("/rs/top?t={period}") };
            v.push(Link { url: format!("{origin}{path}"), kind: "feed", chronological: false, priority: 1 });
        }
    }
    // Last priority-1 feed. A short time budget still stops on robots and the first two feeds.
    v.push(Link { url: format!("{origin}/rs/spaces"), kind: "feed", chronological: true, priority: 1 });
    v
}

fn plan(c: &Connection, opts: &CrawlOpts, report: &CrawlReport, elapsed: u64, now: u64) -> Result<Plan> {
    if report.pages > 0 && let Some(budget) = opts.budget_ms && elapsed >= budget {
        return Ok(Plan::Stop("time budget".into()));
    }
    if let Some(limit) = opts.limit && report.new_posts >= limit {
        return Ok(Plan::Stop("post limit".into()));
    }
    let row = c.query_row(
        "SELECT url, kind, chronological, attempts FROM crawl_frontier WHERE status = 'pending' AND wait_until <= ?1 ORDER BY priority, rowid LIMIT 1",
        params![now as i64],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)),
    ).optional()?;
    if let Some((url, kind, chronological, attempts)) = row {
        return Ok(Plan::Fetch(Task { url, kind, chronological: chronological != 0, attempts }));
    }
    let soon: Option<i64> = c.query_row(
        "SELECT min(wait_until) FROM crawl_frontier WHERE status = 'pending' AND wait_until > ?1",
        params![now as i64],
        |r| r.get(0),
    )?;
    if let Some(at) = soon {
        return Ok(Plan::Wait((at as u64).saturating_sub(now).max(1)));
    }
    Ok(Plan::Finished)
}

#[allow(clippy::too_many_arguments)]
fn absorb(
    c: &mut Connection, task: &Task, page: &CrawlPage, opts: &CrawlOpts, policy: &mut Policy,
    known: &mut HashSet<String>, report: &mut CrawlReport, now: u64,
) -> Result<Absorbed> {
    let stamp = now.to_string();
    if let Some(text) = &page.robots {
        let (delay, disallow) = parse_robots(text);
        policy.delay_ms = delay;
        policy.disallow = disallow;
        let text = policy.disallow.join("\n");
        c.execute(
            "UPDATE crawl_checkpoint SET crawl_delay_ms = ?1, disallow = ?2 WHERE id = 1",
            params![delay as i64, text],
        )?;
        mark(c, &task.url, "done", 0, 0, None, now)?;
        return Ok(Absorbed::Continue);
    }
    let all_known = !page.posts.is_empty() && page.posts.iter().all(|p| p.post_id.is_empty() || known.contains(&p.post_id));
    let mut hit_limit = false;
    for (post, raw) in page.posts.iter().zip(page.raw.iter()) {
        if post.post_id.is_empty() { continue; }
        let is_new = !known.contains(&post.post_id);
        if is_new && opts.limit.is_some_and(|n| report.new_posts >= n) {
            hit_limit = true;
            break;
        }
        if is_new {
            save_posts(c, std::slice::from_ref(post))?;
            if opts.attachments { save_media(c, &post.post_id, raw)?; }
            known.insert(post.post_id.clone());
            report.posts.push(post.clone());
            report.new_posts += 1;
        }
        // A known post can still change live → ended. Secrets are dropped before the write.
        store_from_posts(c, std::slice::from_ref(raw))?;
        remember_links(c, opts, policy, task, post, raw, all_known, &stamp)?;
    }
    if !hit_limit {
        for raw in &page.links {
            if let Some(link) = classify(&opts.origin, raw) { enqueue_if(c, policy, &link, &stamp)?; }
        }
        if follow_next(opts, task, all_known) && let Some(next) = &page.next && next != &task.url {
            let mut link = classify(&opts.origin, next).unwrap_or(Link {
                url: abs(&opts.origin, next), kind: "feed", chronological: task.chronological, priority: 2,
            });
            link.chronological = task.chronological;
            enqueue_if(c, policy, &link, &stamp)?;
        }
        mark(c, &task.url, "done", 0, 0, None, now)?;
        return Ok(Absorbed::Continue);
    }
    Ok(Absorbed::Limit)
}

fn follow_next(opts: &CrawlOpts, task: &Task, all_known: bool) -> bool {
    if !opts.incremental { return true; }
    if !task.chronological { return false; }
    !all_known
}

#[allow(clippy::too_many_arguments)]
fn remember_links(
    c: &Connection, opts: &CrawlOpts, policy: &Policy, task: &Task, post: &Post, raw: &Value,
    all_known: bool, stamp: &str,
) -> Result<()> {
    let thread = if post.parent_post_id.is_empty() { post.post_id.as_str() } else { post.parent_post_id.as_str() };
    if !thread.is_empty() {
        let link = thread_link(&opts.origin, thread);
        enqueue_if(c, policy, &link, stamp)?;
        if task.chronological && task.kind != "thread" {
            c.execute("INSERT OR IGNORE INTO crawl_recent (url) VALUES (?1)", params![link.url])?;
        }
    }
    if opts.incremental && all_known { return Ok(()); }
    if !post.post_id.is_empty() {
        enqueue_if(c, policy, &reposts_link(&opts.origin, &post.post_id), stamp)?;
    }
    if !post.nick.is_empty() {
        for link in profile_links(&opts.origin, &post.nick) { enqueue_if(c, policy, &link, stamp)?; }
    }
    for tag in extract_tags(&post.body_json, &post.content) {
        enqueue_if(c, policy, &tag_link(&opts.origin, &tag), stamp)?;
    }
    let mut extra = Vec::new();
    discover_value(&opts.origin, raw, &mut extra);
    for link in extra { enqueue_if(c, policy, &link, stamp)?; }
    Ok(())
}

fn fail(c: &Connection, task: &Task, policy: &Policy, err: &anyhow::Error, now: u64) -> Result<()> {
    let attempts = task.attempts + 1;
    let msg = format!("{err:#}");
    if attempts >= MAX_ATTEMPTS {
        mark(c, &task.url, "error", attempts, 0, Some(&msg), now)?;
        err!("warning: crawl {}: {msg}", task.url);
        return Ok(());
    }
    let wait = backoff_ms(polite_ms(policy), attempts);
    mark(c, &task.url, "pending", attempts, now.saturating_add(wait) as i64, Some(&msg), now)?;
    Ok(())
}

fn finish(c: &Connection, report: &CrawlReport, now: u64) -> Result<()> {
    if report.finished {
        c.execute(
            "UPDATE crawl_checkpoint SET running = 0, finished_at = ?1, posts_saved = posts_saved + ?2, pages = pages + ?3 WHERE id = 1",
            params![now.to_string(), report.new_posts as i64, report.pages as i64],
        )?;
    } else {
        c.execute(
            "UPDATE crawl_checkpoint SET posts_saved = posts_saved + ?1, pages = pages + ?2 WHERE id = 1",
            params![report.new_posts as i64, report.pages as i64],
        )?;
    }
    Ok(())
}

fn enqueue(c: &Connection, link: &Link, stamp: &str) -> Result<()> {
    c.execute(
        "INSERT INTO crawl_frontier (url, kind, chronological, priority, status, attempts, wait_until, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'pending', 0, 0, ?5) ON CONFLICT(url) DO NOTHING",
        params![link.url, link.kind, i64::from(link.chronological), link.priority, stamp],
    )?;
    Ok(())
}

fn enqueue_if(c: &Connection, policy: &Policy, link: &Link, stamp: &str) -> Result<()> {
    if blocked(policy, &link.url) {
        c.execute(
            "INSERT INTO crawl_frontier (url, kind, chronological, priority, status, attempts, wait_until, last_error, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'done', 0, 0, 'robots', ?5) ON CONFLICT(url) DO NOTHING",
            params![link.url, link.kind, i64::from(link.chronological), link.priority, stamp],
        )?;
        return Ok(());
    }
    enqueue(c, link, stamp)
}

fn mark(c: &Connection, url: &str, status: &str, attempts: i64, wait_until: i64, err: Option<&str>, now: u64) -> Result<()> {
    c.execute(
        "UPDATE crawl_frontier SET status = ?1, attempts = ?2, wait_until = ?3, last_error = ?4, updated_at = ?5 WHERE url = ?6",
        params![status, attempts, wait_until, err, now.to_string(), url],
    )?;
    Ok(())
}

fn load_policy(c: &Connection) -> Result<Policy> {
    let (delay, text): (i64, String) = c.query_row(
        "SELECT crawl_delay_ms, disallow FROM crawl_checkpoint WHERE id = 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(Policy {
        delay_ms: delay as u64,
        disallow: text.lines().filter(|l| !l.is_empty()).map(str::to_string).collect(),
    })
}

fn load_ids(c: &Connection) -> Result<HashSet<String>> {
    let mut st = c.prepare("SELECT post_id FROM posts")?;
    let rows = st.query_map([], |r| r.get(0))?;
    let mut known = HashSet::new();
    for id in rows { known.insert(id?); }
    Ok(known)
}

fn pending_count(c: &Connection) -> Result<usize> {
    let n: i64 = c.query_row("SELECT count(*) FROM crawl_frontier WHERE status = 'pending'", [], |r| r.get(0))?;
    Ok(n as usize)
}

fn polite_ms(policy: &Policy) -> u64 { THREAD_FETCH_PAUSE_MS.max(policy.delay_ms) }

fn backoff_ms(polite: u64, attempts: i64) -> u64 {
    let shift = attempts.clamp(1, 8) as u32;
    polite.saturating_mul(1u64 << shift).min(BACKOFF_CAP_MS)
}

fn blocked(policy: &Policy, url: &str) -> bool {
    if path_of(url) == "/robots.txt" { return false; }
    let path = path_of(url);
    policy.disallow.iter().any(|rule| path_blocked(&path, rule))
}

fn path_blocked(path: &str, rule: &str) -> bool {
    if rule.is_empty() { return false; }
    path == rule || path.starts_with(&format!("{}/", rule.trim_end_matches('/')))
}

fn parse_robots(text: &str) -> (u64, Vec<String>) {
    let mut delay = 0u64;
    let mut disallow = Vec::new();
    let mut applies = false;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let Some((key, value)) = line.split_once(':') else { continue };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if key == "user-agent" {
            if applies { break; }
            applies = value == "*";
            continue;
        }
        if !applies { continue; }
        if key == "disallow" && !value.is_empty() { disallow.push(value.to_string()); }
        if key == "crawl-delay" && let Ok(secs) = value.parse::<f64>() {
            delay = (secs * 1000.0) as u64;
        }
    }
    (delay, disallow)
}

fn abs(origin: &str, raw: &str) -> String {
    if raw.starts_with("http://") || raw.starts_with("https://") { raw.to_string() }
    else if raw.starts_with('/') { format!("{origin}{raw}") }
    else { format!("{origin}/{raw}") }
}

fn path_of(url: &str) -> String {
    Url::parse(url).map(|u| u.path().to_string()).unwrap_or_default()
}

fn thread_link(origin: &str, id: &str) -> Link {
    Link { url: format!("{origin}/rs/post/{id}"), kind: "thread", chronological: true, priority: 2 }
}
fn reposts_link(origin: &str, id: &str) -> Link {
    Link { url: format!("{origin}/rs/post/{id}/reposts"), kind: "reposts", chronological: false, priority: 2 }
}
fn tag_link(origin: &str, tag: &str) -> Link {
    Link { url: format!("{origin}/rs/tag/{}", urlenc(&tag_name(tag))), kind: "tag", chronological: true, priority: 2 }
}
fn space_link(origin: &str, slug: &str) -> Link {
    Link { url: format!("{origin}/rs/space/{slug}"), kind: "space", chronological: true, priority: 2 }
}
fn community_link(origin: &str, slug: &str) -> Link {
    Link { url: format!("{origin}/rs/community/{slug}"), kind: "community", chronological: true, priority: 2 }
}
fn profile_links(origin: &str, nick: &str) -> Vec<Link> {
    let nick = nick.trim().trim_start_matches('@');
    if nick.is_empty() || nick.contains('/') { return Vec::new(); }
    vec![
        Link { url: format!("{origin}/rs/@{nick}"), kind: "profile", chronological: true, priority: 2 },
        Link { url: format!("{origin}/rs/@{nick}?replies=1"), kind: "profile", chronological: true, priority: 2 },
        Link { url: format!("{origin}/rs/@{nick}/top?t=all"), kind: "profile", chronological: false, priority: 2 },
    ]
}

fn classify(origin: &str, raw: &str) -> Option<Link> {
    let url = abs(origin, raw);
    let path = path_of(&url);
    if path == "/robots.txt" {
        return Some(Link { url, kind: "robots", chronological: false, priority: 0 });
    }
    if let Some(rest) = path.strip_prefix("/rs/@") {
        let nick = rest.split('/').next().unwrap_or("");
        if nick.is_empty() { return None; }
        let chronological = !path.contains("/top");
        let kind = "profile";
        return Some(Link { url, kind, chronological, priority: 2 });
    }
    if let Some(rest) = path.strip_prefix("/rs/post/") {
        let id = rest.split('/').next().unwrap_or("");
        if id.is_empty() { return None; }
        if rest.contains("/reposts") { return Some(reposts_link(origin, id)); }
        return Some(thread_link(origin, id));
    }
    if let Some(tag) = path.strip_prefix("/rs/tag/") {
        if tag.is_empty() { return None; }
        return Some(tag_link(origin, tag));
    }
    if let Some(slug) = path.strip_prefix("/rs/space/").or_else(|| path.strip_prefix("/rs/spaces/")) {
        let slug = slug.split('/').next().unwrap_or("");
        if slug.is_empty() { return None; }
        return Some(space_link(origin, slug));
    }
    if let Some(slug) = path.strip_prefix("/rs/community/").or_else(|| path.strip_prefix("/rs/c/")) {
        let slug = slug.split('/').next().unwrap_or("");
        if slug.is_empty() { return None; }
        return Some(community_link(origin, slug));
    }
    path.starts_with("/rs").then_some(Link { url, kind: "feed", chronological: true, priority: 2 })
}

fn discover_value(origin: &str, v: &Value, links: &mut Vec<Link>) {
    for key in ["repost_of", "repost_id", "reposted_post_id"] {
        if let Some(id) = v.get(key).and_then(Value::as_str) && !id.is_empty() {
            links.push(thread_link(origin, id));
            links.push(reposts_link(origin, id));
        }
    }
    if let Some(slug) = object_slug(v.get("space")) { links.push(space_link(origin, &slug)); }
    if let Some(slug) = object_slug(v.get("community")) { links.push(community_link(origin, &slug)); }
    if let Some(arr) = v.get("tags").and_then(Value::as_array) {
        for tag in arr { if let Some(s) = tag.as_str() { links.push(tag_link(origin, s)); } }
    }
    if let Some(arr) = v.get("spaces").and_then(Value::as_array) {
        for item in arr { if let Some(slug) = object_slug(Some(item)) { links.push(space_link(origin, &slug)); } }
    }
    if let Some(arr) = v.get("communities").and_then(Value::as_array) {
        for item in arr { if let Some(slug) = object_slug(Some(item)) { links.push(community_link(origin, &slug)); } }
    }
}

fn object_slug(v: Option<&Value>) -> Option<String> {
    let v = v?;
    if let Some(s) = v.as_str() && !s.is_empty() { return Some(s.to_string()); }
    for key in ["slug", "id", "name"] {
        if let Some(s) = v.get(key).and_then(Value::as_str) && !s.is_empty() { return Some(s.to_string()); }
    }
    None
}

fn save_media(c: &Connection, post_id: &str, raw: &Value) -> Result<()> {
    let mut media = Vec::new();
    collect_media(raw, &mut media);
    if media.is_empty() { return Ok(()); }
    ensure_attachments_schema(c)?;
    let mut st = c.prepare(
        "INSERT INTO attachments (post_id, url, local_path, mime_type, on_site) VALUES (?1, ?2, '', ?3, 1)
         ON CONFLICT(post_id, url) DO NOTHING",
    )?;
    for item in media {
        st.execute(params![post_id, item.url, meta_mime(&item.url, &item.kind)])?;
    }
    Ok(())
}

fn meta_mime(url: &str, kind: &str) -> String {
    let ext = url.split('?').next().unwrap_or(url).rsplit('.').next().unwrap_or("");
    let mime = mime_for_attachment_ext(ext);
    if mime != "application/octet-stream" { return mime.to_string(); }
    match kind {
        "video" => "video/mp4".into(),
        "image" | "thumbnail" => "image/jpeg".into(),
        _ => mime.to_string(),
    }
}

fn page_from_items(items: &[Value]) -> CrawlPage {
    let mut posts = Vec::new();
    let mut raw = Vec::new();
    for item in items {
        let post = feed_item_post(item);
        if post.post_id.is_empty() { continue; }
        posts.push(post);
        raw.push(item.clone());
    }
    CrawlPage { posts, raw, next: None, links: Vec::new(), robots: None }
}

#[cfg(test)]
fn page_from_fixture(origin: &str, v: &Value) -> CrawlPage {
    if let Some(text) = v.get("robots").and_then(Value::as_str) {
        return CrawlPage { robots: Some(text.to_string()), ..CrawlPage::default() };
    }
    let items = v.get("posts").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut page = page_from_items(&items);
    if let Some(next) = v.get("next").and_then(Value::as_str) { page.next = Some(abs(origin, next)); }
    if let Some(links) = v.get("links").and_then(Value::as_array) {
        page.links = links.iter().filter_map(Value::as_str).map(str::to_string).collect();
    }
    page
}

async fn fetch_http(http: &reqwest::Client, jar: &Jar, url: &str, hint: &FetchHint) -> Result<CrawlPage> {
    if hint.kind == "robots" || path_of(url) == "/robots.txt" {
        let text = http.get(url).send().await?.error_for_status()?.text().await?;
        return Ok(CrawlPage { robots: Some(text), ..CrawlPage::default() });
    }
    if hint.kind == "thread" {
        let id = path_of(url).trim_start_matches("/rs/post/").split('/').next().unwrap_or("").to_string();
        if id.is_empty() { bail!("thread url {url} has no id"); }
        let (th, replies) = fetch_thread(http, jar, &id).await?;
        let root = th.get("root").cloned().unwrap_or(th);
        let mut page = page_from_items(std::slice::from_ref(&root));
        for reply in replies {
            let post = to_post(&reply, &id, "");
            if post.post_id.is_empty() { continue; }
            page.posts.push(post);
            page.raw.push(reply);
        }
        return Ok(page);
    }
    if hint.first_only {
        let resp = http.get(url).send().await?.error_for_status()?;
        let final_url = resp.url().to_string();
        let html = resp.text().await?;
        check_profile_landing(url, &final_url, &html)?;
        let feed = props(&html, FEED_PROPS_SEL).with_context(|| format!("no forum/Feed data-props on {url}"))?;
        let items = feed.get("posts").and_then(Value::as_array).cloned().unwrap_or_default();
        return Ok(page_from_items(&items));
    }
    let known = hint.chronological.then_some(&hint.known);
    let feed = fetch_feed(http, jar, url, hint.limit, known, hint.chronological).await?;
    Ok(page_from_items(&feed.items))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Condition, NewRule, RuleAction, Trigger, insert_user, on_sync};
    use std::collections::HashMap;

    const ORIGIN: &str = "https://acp.us";

    struct MapEnv {
        pages: HashMap<String, CrawlPage>,
        fails: HashMap<String, usize>,
        fetched: Vec<String>,
        pauses: Vec<u64>,
        now: u64,
        fetches: u64,
        elapsed_step: u64,
    }

    impl CrawlEnv for MapEnv {
        fn fetch<'a>(&'a mut self, url: &'a str, _hint: &'a FetchHint) -> Pin<Box<dyn Future<Output = Result<CrawlPage>> + Send + 'a>> {
            self.fetches += 1;
            self.fetched.push(url.to_string());
            if let Some(left) = self.fails.get_mut(url) && *left > 0 {
                *left -= 1;
                let url = url.to_string();
                return Box::pin(async move { bail!("fixture refused {url}") });
            }
            let page = self.pages.get(url).cloned().unwrap_or_default();
            Box::pin(async move { Ok(page) })
        }
        fn pause<'a>(&'a mut self, ms: u64) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.now += ms;
            self.pauses.push(ms);
            Box::pin(async {})
        }
        fn elapsed_ms(&mut self) -> u64 { self.fetches.saturating_mul(self.elapsed_step) }
        fn now_ms(&mut self) -> u64 { self.now }
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("acp-crawl-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("rs.database")
    }

    fn opts(incremental: bool, limit: Option<usize>, budget_ms: Option<u64>) -> CrawlOpts {
        CrawlOpts { origin: ORIGIN.into(), incremental, limit, budget_ms, attachments: true }
    }

    fn fixture(path: &str, body: &str) -> (String, CrawlPage) {
        let v: Value = serde_json::from_str(body).unwrap();
        (format!("{ORIGIN}{path}"), page_from_fixture(ORIGIN, &v))
    }

    fn pages() -> HashMap<String, CrawlPage> {
        let mut m = HashMap::new();
        let add = |m: &mut HashMap<_, _>, path: &str, body: &str| {
            let (k, v) = fixture(path, body);
            m.insert(k, v);
        };
        add(&mut m, "/rs/latest", include_str!("../../tests/fixtures/site/crawl/latest.json"));
        add(&mut m, "/rs/latest?cursor=2", include_str!("../../tests/fixtures/site/crawl/latest-next.json"));
        add(&mut m, "/rs/post/POST0001", include_str!("../../tests/fixtures/site/crawl/thread.json"));
        add(&mut m, "/rs/@ada", include_str!("../../tests/fixtures/site/crawl/profile.json"));
        add(&mut m, "/rs/tag/strike", include_str!("../../tests/fixtures/site/crawl/tag.json"));
        add(&mut m, "/rs/space/forum", include_str!("../../tests/fixtures/site/crawl/space.json"));
        add(&mut m, "/rs/spaces", include_str!("../../tests/fixtures/site/crawl/spaces.json"));
        add(&mut m, "/rs/community/local", include_str!("../../tests/fixtures/site/crawl/community.json"));
        add(&mut m, "/rs/post/POST0001/reposts", include_str!("../../tests/fixtures/site/crawl/reposts.json"));
        m.insert(
            format!("{ORIGIN}/robots.txt"),
            CrawlPage { robots: Some(include_str!("../../tests/fixtures/site/crawl/robots.txt").into()), ..CrawlPage::default() },
        );
        m
    }

    fn env() -> MapEnv {
        MapEnv { pages: pages(), fails: HashMap::new(), fetched: Vec::new(), pauses: Vec::new(), now: 1_700_000_000_000, fetches: 0, elapsed_step: 0 }
    }

    fn ids(db: &Path) -> Vec<String> {
        let c = open_db(db).unwrap();
        let mut st = c.prepare("SELECT post_id FROM posts ORDER BY post_id").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }

    #[tokio::test]
    async fn full_crawl_discovers_the_site_and_dedups() {
        let db = temp("full");
        let mut env = env();
        let report = run_crawl(&db, &mut env, opts(false, None, None)).await.unwrap();
        assert!(report.finished, "{report:?}");
        assert!(report.stopped.is_none());
        let got = ids(&db);
        for id in ["POST0001", "POST0002", "POST0002B", "POST0003", "POST0004", "POST0005", "POST0006", "POST0007", "POST0008"] {
            assert!(got.contains(&id.to_string()), "{id} missing from {got:?}");
        }
        assert_eq!(got.iter().filter(|id| id.as_str() == "POST0001").count(), 1, "one row per post id");
        assert!(env.fetched.iter().any(|u| u.ends_with("/rs/post/POST0009")), "a repost target is queued");
        assert!(!env.fetched.iter().any(|u| u.contains("/rs/private")), "robots disallow is not fetched");
        assert!(env.pauses.iter().any(|ms| *ms >= 1000), "crawl-delay raises the polite pause: {:?}", env.pauses);
        let c = open_db(&db).unwrap();
        let mime: String = c.query_row(
            "SELECT mime_type FROM attachments WHERE post_id = 'POST0001' AND url = 'https://acp.us/media/a.jpg'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(mime, "image/jpeg");
        assert!(seed_links(ORIGIN).iter().all(|l| l.url.starts_with(ORIGIN) && (l.url.contains("/rs") || l.url.ends_with("/robots.txt"))));
        assert!(crate::site::CAPTURE_BLOCK.contains("text posts and replies are published"));
        assert!(crate::site::CAPTURE_BLOCK.contains("are not sent"));
    }

    #[tokio::test]
    async fn time_budget_leaves_a_frontier_the_next_run_finishes() {
        let db = temp("budget");
        let mut first = env();
        first.elapsed_step = 1000;
        let stopped = run_crawl(&db, &mut first, opts(true, None, Some(2500))).await.unwrap();
        assert_eq!(stopped.stopped.as_deref(), Some("time budget"));
        assert!(!stopped.finished);
        assert!(stopped.pending > 0, "{stopped:?}");
        assert_eq!(stopped.pages, 3, "robots and two feeds, then the budget");
        let mut second = env();
        let done = run_crawl(&db, &mut second, opts(true, None, None)).await.unwrap();
        assert!(done.finished, "{done:?}");
        assert!(ids(&db).iter().any(|id| id == "POST0004"), "the resume stored the profile post");
    }

    #[tokio::test]
    async fn post_limit_stops_and_a_later_run_keeps_going() {
        let db = temp("limit");
        let mut first = env();
        let stopped = run_crawl(&db, &mut first, opts(true, Some(1), None)).await.unwrap();
        assert_eq!(stopped.stopped.as_deref(), Some("post limit"));
        assert_eq!(stopped.new_posts, 1);
        let mut second = env();
        let done = run_crawl(&db, &mut second, opts(true, None, None)).await.unwrap();
        assert!(done.finished, "{done:?}");
        assert!(ids(&db).contains(&"POST0002".into()));
    }

    #[tokio::test]
    async fn incremental_stops_at_known_ids_and_rereads_the_thread() {
        let db = temp("incr");
        let mut first = env();
        let full = run_crawl(&db, &mut first, opts(false, None, None)).await.unwrap();
        assert!(full.finished);
        let before = ids(&db);
        assert!(before.contains(&"POST0003".into()));
        let mut second = env();
        let (key, mut page) = fixture("/rs/post/POST0001", include_str!("../../tests/fixtures/site/crawl/thread-new-reply.json"));
        page.posts.retain(|p| !p.post_id.is_empty());
        second.pages.insert(key, page);
        let again = run_crawl(&db, &mut second, opts(true, None, None)).await.unwrap();
        assert!(again.finished, "{again:?}");
        assert!(!second.fetched.iter().any(|u| u.contains("cursor=2")), "a known next page stays put: {:?}", second.fetched);
        assert!(second.fetched.iter().any(|u| u.ends_with("/rs/post/POST0001")));
        assert!(ids(&db).contains(&"POST0010".into()), "the new reply is stored");
        assert_eq!(again.new_posts, 1, "{again:?}");
    }

    #[tokio::test]
    async fn a_refused_page_waits_then_succeeds() {
        let db = temp("backoff");
        let mut env = env();
        env.pages.insert(format!("{ORIGIN}/robots.txt"), CrawlPage {
            robots: Some("User-agent: *\nDisallow:\n".into()),
            ..CrawlPage::default()
        });
        let latest = format!("{ORIGIN}/rs/latest");
        env.fails.insert(latest.clone(), 1);
        let report = run_crawl(&db, &mut env, opts(false, None, None)).await.unwrap();
        assert!(report.finished, "{report:?}");
        assert_eq!(env.fetched.iter().filter(|u| *u == &latest).count(), 2, "one refusal, then the page");
        assert_eq!(backoff_ms(THREAD_FETCH_PAUSE_MS, 1), 500);
        assert!(ids(&db).contains(&"POST0001".into()));
    }

    #[tokio::test]
    async fn three_refusals_record_an_error_and_the_crawl_continues() {
        let db = temp("giveup");
        let mut env = env();
        let latest = format!("{ORIGIN}/rs/latest");
        env.fails.insert(latest.clone(), 5);
        let report = run_crawl(&db, &mut env, opts(false, None, None)).await.unwrap();
        assert!(report.finished, "{report:?}");
        let c = open_db(&db).unwrap();
        let status: String = c.query_row("SELECT status FROM crawl_frontier WHERE url = ?1", params![latest], |r| r.get(0)).unwrap();
        assert_eq!(status, "error");
        assert_eq!(env.fetched.iter().filter(|u| *u == &latest).count(), 3, "three tries, then the row is an error");
        assert!(env.fetched.iter().any(|u| u.ends_with("/rs")), "the other feeds still run");
    }

    #[tokio::test]
    async fn sync_rules_see_the_posts_this_run_stored() {
        let db = temp("rules");
        let mut env = env();
        let report = run_crawl(&db, &mut env, opts(false, None, None)).await.unwrap();
        let c = crate::rules::open_rules(&db).unwrap();
        insert_user(&c, &NewRule {
            name: "Watch ada".into(),
            enabled: true,
            priority: 10,
            cloned_from: None,
            template_key: None,
            trigger: Trigger::PostSynced,
            conditions: vec![Condition::Author { nick: "ada".into() }],
            actions: vec![RuleAction::Notify],
        }).unwrap();
        drop(c);
        let fired = on_sync(&db, &report.posts).unwrap();
        let notes: Vec<_> = fired.iter().filter(|f| f.action == "notify" && f.outcome == "fired").collect();
        assert!(!notes.is_empty(), "{fired:?}");
        assert!(notes.iter().all(|f| f.detail != "rate limit"));
        assert!(report.posts.iter().any(|p| p.nick == "ada" && notes.iter().any(|f| f.post_id == p.post_id)));
    }

    #[tokio::test]
    async fn spaces_are_stored_and_the_livekit_token_is_dropped() {
        let db = temp("spaces");
        let mut first = env();
        let report = run_crawl(&db, &mut first, opts(false, None, None)).await.unwrap();
        assert!(report.finished, "{report:?}");
        assert!(first.fetched.iter().any(|u| u.ends_with("/rs/spaces")));
        let c = open_db(&db).unwrap();
        let live: i64 = c.query_row("SELECT live FROM spaces WHERE slug = 'strike-meeting'", [], |r| r.get(0)).unwrap();
        assert_eq!(live, 1);
        let rec: String = c.query_row("SELECT recording_url FROM spaces WHERE slug = 'strike-meeting'", [], |r| r.get(0)).unwrap();
        assert_eq!(rec, "https://example.test/recordings/strike");
        let sched: String = c.query_row("SELECT state FROM spaces WHERE slug = 'later-forum'", [], |r| r.get(0)).unwrap();
        assert_eq!(sched, "scheduled");
        let ended: String = c.query_row("SELECT state FROM spaces WHERE slug = 'old-forum'", [], |r| r.get(0)).unwrap();
        assert_eq!(ended, "ended");
        let bytes = std::fs::read(&db).unwrap();
        assert!(!bytes.windows(b"lk-secret-token-do-not-keep".len()).any(|w| w == b"lk-secret-token-do-not-keep"), "livekit token was written");
        assert!(!bytes.windows(b"wss://livekit".len()).any(|w| w == b"wss://livekit"), "livekit url was written");
        drop(c);

        let mut second = env();
        let mut v: Value = serde_json::from_str(include_str!("../../tests/fixtures/site/crawl/spaces.json")).unwrap();
        v["posts"][0]["space"]["state"] = json!("ended");
        v["posts"][0]["space"]["live"] = json!(false);
        v["posts"][0]["space"]["ended_at"] = json!("2026-10-21T19:00:00Z");
        second.pages.insert(format!("{ORIGIN}/rs/spaces"), page_from_fixture(ORIGIN, &v));
        let again = run_crawl(&db, &mut second, opts(true, None, None)).await.unwrap();
        assert!(again.finished, "{again:?}");
        let c = open_db(&db).unwrap();
        let live: i64 = c.query_row("SELECT live FROM spaces WHERE slug = 'strike-meeting'", [], |r| r.get(0)).unwrap();
        assert_eq!(live, 0, "a known space post can leave live");
        let state: String = c.query_row("SELECT state FROM spaces WHERE slug = 'strike-meeting'", [], |r| r.get(0)).unwrap();
        assert_eq!(state, "ended");
    }

    #[test]
    fn progress_line_names_pages_posts_and_the_queue() {
        assert_eq!(progress_line(2, 3, 4), "crawl: 2 pages, 3 posts, 4 waiting");
    }

    #[test]
    fn robots_crawl_delay_and_disallow_parse() {
        let (delay, rules) = parse_robots("User-agent: *\nDisallow: /rs/private\nCrawl-delay: 1\n");
        assert_eq!(delay, 1000);
        assert!(path_blocked("/rs/private", &rules[0]));
        assert!(path_blocked("/rs/private/secret", &rules[0]));
        assert!(!path_blocked("/rs/privateer", &rules[0]));
    }
}
