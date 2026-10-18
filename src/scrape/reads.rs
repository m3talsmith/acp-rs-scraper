//! Read-only site views (phase P1): fetch a forum feed, store its posts, and
//! remember the order so the TUI can open the same view offline.
//!
//! Nothing here publishes. Filters (`configure-feed`) only change which page
//! is read. The direct URL is tried first; if it has no `forum/Feed`, the same
//! fields are submitted on the page's LiveView, the way the profile replies
//! filter already falls back.

use crate::prelude::*;

/// One of the five periods on `/rs/top` (`t` in `configure-feed`).
/// The site's profile Top link uses `t=all`; the other four are the usual
/// day / week / month / year windows. Names were not captured live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopPeriod {
    /// The last day.
    Day,
    /// The last week (the TUI's default).
    Week,
    /// The last month.
    Month,
    /// The last year.
    Year,
    /// All time.
    All,
}

impl TopPeriod {
    /// Every period, in the View menu's order (week first: the default).
    pub const ALL: [TopPeriod; 5] = [
        TopPeriod::Week,
        TopPeriod::Day,
        TopPeriod::Month,
        TopPeriod::Year,
        TopPeriod::All,
    ];
    /// The `t` query value.
    pub fn as_str(self) -> &'static str {
        match self {
            TopPeriod::Day => "day",
            TopPeriod::Week => "week",
            TopPeriod::Month => "month",
            TopPeriod::Year => "year",
            TopPeriod::All => "all",
        }
    }
    /// Parse `t` (`day`, `week`, `month`, `year`, `all`), ignoring case.
    pub fn parse(s: &str) -> std::result::Result<TopPeriod, String> {
        Self::ALL
            .iter()
            .copied()
            .find(|p| p.as_str().eq_ignore_ascii_case(s.trim()))
            .ok_or_else(|| format!("period must be day, week, month, year or all, not {s:?}"))
    }
}

/// Clap value parser for [`TopPeriod`].
pub fn parse_top_period(s: &str) -> std::result::Result<TopPeriod, String> {
    TopPeriod::parse(s)
}

/// Site-wide Top (`/rs/top?t=…`, plus `replies=1` when the replies filter is on).
pub fn feed_top_url(period: TopPeriod, replies: bool) -> String {
    let mut u = format!("{}/rs/top?t={}", base_url(), period.as_str());
    if replies {
        u.push_str("&replies=1");
    }
    u
}

/// The Top page a `configure-feed` fallback opens.
pub fn feed_top_page_url() -> String {
    format!("{}/rs/top", base_url())
}

/// `configure-feed` fields for Top: `t`, and `replies=1` when asked.
pub fn top_fields(period: TopPeriod, replies: bool) -> Vec<(&'static str, &'static str)> {
    let mut v = vec![("t", period.as_str())];
    if replies {
        v.push(("replies", "1"));
    }
    v
}

/// Database key for a Top view (`top:week`, `top:all:replies`).
pub fn top_key(period: TopPeriod, replies: bool) -> String {
    if replies {
        format!("top:{}:replies", period.as_str())
    } else {
        format!("top:{}", period.as_str())
    }
}

/// Body title for a Top view.
pub fn top_title(period: TopPeriod, replies: bool) -> String {
    if replies {
        format!("Top · {} · posts and replies", period.as_str())
    } else {
        format!("Top · {}", period.as_str())
    }
}

/// Feed items as stored posts, first occurrence of each id, in site order.
pub fn feed_posts(items: &[Value]) -> Vec<Post> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for v in items {
        let p = feed_item_post(v);
        if p.post_id.is_empty() || !seen.insert(p.post_id.clone()) {
            continue;
        }
        out.push(p);
    }
    out
}

/// Store `items` (posts + this view's order). Returns how many distinct posts the view holds.
pub fn remember_feed(items: &[Value], key: &str, title: &str) -> Result<usize> {
    let posts = feed_posts(items);
    let n = save(&posts)?;
    let db = home()?.join(DB_REL_PATH);
    let c = open_db(&db)?;
    let ids: Vec<String> = posts.iter().map(|p| p.post_id.clone()).collect();
    save_site_feed(
        &c,
        &SiteFeed {
            key: key.into(),
            title: title.into(),
            fetched_at: chrono::Local::now().to_rfc3339(),
            post_ids: ids.clone(),
        },
    )?;
    info!("{title}: {} posts ({n} rows written)", ids.len());
    Ok(ids.len())
}

/// Fetch a feed at `direct`. When that page has no `forum/Feed`, open `page`
/// and submit `event` with `fields`, then page it the same way.
pub async fn fetch_configured(
    http: &reqwest::Client,
    jar: &Jar,
    direct: &str,
    page: &str,
    event: &str,
    fields: &[(&str, &str)],
) -> Result<FeedPage> {
    match fetch_feed(http, jar, direct, None, None, false).await {
        Ok(p) => Ok(p),
        Err(e) => {
            verbose!("feed {direct} failed ({e:#}); {event} on {page}");
            configured_via_form(http, jar, page, event, fields)
                .await
                .with_context(|| format!("{direct} had no feed and {page} failed"))
        }
    }
}

async fn configured_via_form(
    http: &reqwest::Client,
    jar: &Jar,
    page: &str,
    event: &str,
    fields: &[(&str, &str)],
) -> Result<FeedPage> {
    let resp = http.get(page).send().await?.error_for_status()?;
    let final_url = resp.url().to_string();
    let html = resp.text().await?;
    let lvpage = parse_lv(&final_url, &html)?;
    let mut lv = Lv::connect(&lvpage, jar).await?;
    let r = lv.form_fields(event, fields, None).await?;
    if r["status"] != "ok" {
        bail!("{event} failed: {r}");
    }
    let body = reply_body(&r);
    let feed = if body["posts"].is_array() {
        body.clone()
    } else {
        props(&lv.html(), FEED_PROPS_SEL).with_context(|| format!("no forum/Feed after {event}"))?
    };
    let mut items: Vec<Value> = feed["posts"].as_array().cloned().unwrap_or_default();
    let at_end = feed["at_end"].as_bool().unwrap_or(true);
    let cursor = feed["next_cursor"].as_str().map(str::to_string);
    let mut pages = 1;
    if !at_end && cursor.is_some() {
        let (_, extra) = page_rest(&mut lv, &mut items, cursor, None, None, false).await?;
        pages += extra;
    }
    Ok(FeedPage {
        items,
        html,
        stopped_incremental: false,
        pages,
    })
}

/// `--top`: read the site-wide Top feed and store it.
pub async fn read_top(
    http: &reqwest::Client,
    jar: &Jar,
    period: TopPeriod,
    replies: bool,
) -> Result<usize> {
    let fields = top_fields(period, replies);
    read_view(
        http,
        jar,
        &feed_top_url(period, replies),
        &feed_top_page_url(),
        PROFILE_FILTER_EVENT,
        &fields,
        &top_key(period, replies),
        &top_title(period, replies),
    )
    .await
    .with_context(|| format!("reading Top ({})", period.as_str()))
}

/// Executive Board feed (`/rs/eb`, plus `replies=1` when the replies filter is on).
pub fn feed_eb_url(replies: bool) -> String {
    let mut u = format!("{}/rs/eb", base_url());
    if replies {
        u.push_str("?replies=1");
    }
    u
}

/// The EB page a `configure-feed` fallback opens.
pub fn feed_eb_page_url() -> String {
    format!("{}/rs/eb", base_url())
}

/// `configure-feed` fields for EB: `replies=1` when asked, otherwise none.
pub fn eb_fields(replies: bool) -> Vec<(&'static str, &'static str)> {
    if replies { vec![("replies", "1")] } else { Vec::new() }
}

/// Database key for the EB view (`eb`, `eb:replies`).
pub fn eb_key(replies: bool) -> String {
    if replies { "eb:replies".into() } else { "eb".into() }
}

/// Body title for the EB view.
pub fn eb_title(replies: bool) -> String {
    if replies { "EB · posts and replies".into() } else { "EB".into() }
}

/// `--eb`: read the Executive Board feed and store it.
pub async fn read_eb(http: &reqwest::Client, jar: &Jar, replies: bool) -> Result<usize> {
    let fields = eb_fields(replies);
    read_view(
        http, jar, &feed_eb_url(replies), &feed_eb_page_url(), PROFILE_FILTER_EVENT, &fields,
        &eb_key(replies), &eb_title(replies),
    )
        .await
        .context("reading the Executive Board feed")
}

/// Fetch one configured feed and store it under `key`.
async fn read_view(
    http: &reqwest::Client,
    jar: &Jar,
    direct: &str,
    page: &str,
    event: &str,
    fields: &[(&str, &str)],
    key: &str,
    title: &str,
) -> Result<usize> {
    let fetched = fetch_configured(http, jar, direct, page, event, fields).await?;
    remember_feed(&fetched.items, key, title)
}

/// Explore (`/rs/explore`) or its Latest tab (`/rs/explore/latest`).
pub fn feed_explore_url(latest: bool) -> String {
    if latest { format!("{}/rs/explore/latest", base_url()) } else { format!("{}/rs/explore", base_url()) }
}

/// Database key for Explore (`explore`, `explore:latest`).
pub fn explore_key(latest: bool) -> String {
    if latest { "explore:latest".into() } else { "explore".into() }
}

/// Body title for Explore.
pub fn explore_title(latest: bool) -> String {
    if latest { "Explore · latest".into() } else { "Explore".into() }
}

/// `--explore` / `--explore-latest`.
pub async fn read_explore(http: &reqwest::Client, jar: &Jar, latest: bool) -> Result<usize> {
    let url = feed_explore_url(latest);
    read_view(http, jar, &url, &url, PROFILE_FILTER_EVENT, &[], &explore_key(latest), &explore_title(latest))
        .await
        .context("reading Explore")
}

/// `/rs/explore?q=` — the site's search form (`phx-submit=search`, field `q`).
pub fn site_search_url(query: &str) -> String {
    format!("{}/rs/explore?q={}", base_url(), urlenc(query.trim()))
}

/// Database key for a site search (`search:{query}`).
pub fn search_key(query: &str) -> String {
    format!("search:{}", query.trim())
}

/// Body title for a site search.
pub fn search_title(query: &str) -> String {
    format!("Site search · {}", query.trim())
}

/// Reposts of a post (`/rs/post/{id}/reposts`).
pub fn feed_reposts_url(id: &str) -> String {
    format!("{}/rs/post/{}/reposts", base_url(), id.trim())
}

/// Database key for a reposts list.
pub fn reposts_key(id: &str) -> String { format!("reposts:{}", id.trim()) }

/// Body title for a reposts list.
pub fn reposts_title(id: &str) -> String { format!("Reposts · {}", id.trim()) }

/// `--reposts`: read who reposted `id` and store that feed.
pub async fn read_reposts(http: &reqwest::Client, jar: &Jar, id: &str) -> Result<usize> {
    let id = id.trim();
    if id.is_empty() { bail!("reposts need a post id"); }
    let url = feed_reposts_url(id);
    read_view(http, jar, &url, &url, PROFILE_FILTER_EVENT, &[], &reposts_key(id), &reposts_title(id))
        .await
        .with_context(|| format!("reading reposts of {id}"))
}

/// Tag name without a leading `#` and surrounding space.
pub fn tag_name(tag: &str) -> String {
    tag.trim().trim_start_matches('#').trim().to_string()
}

/// Tag feed (`/rs/tag/{tag}`).
pub fn feed_tag_url(tag: &str) -> String {
    format!("{}/rs/tag/{}", base_url(), urlenc(&tag_name(tag)))
}

/// Database key for a tag feed. Case is folded so `#Strike` and `#strike` share a view.
pub fn tag_key(tag: &str) -> String {
    format!("tag:{}", tag_name(tag).to_lowercase())
}

/// Body title for a tag feed (the spelling that was asked for, with `#`).
pub fn tag_title(tag: &str) -> String {
    format!("#{}", tag_name(tag))
}

/// `--tag`: read `/rs/tag/{tag}` and store it.
pub async fn read_tag(http: &reqwest::Client, jar: &Jar, tag: &str) -> Result<usize> {
    let name = tag_name(tag);
    if name.is_empty() { bail!("a tag needs a name"); }
    let url = feed_tag_url(&name);
    read_view(http, jar, &url, &url, PROFILE_FILTER_EVENT, &[], &tag_key(&name), &tag_title(&name))
        .await
        .with_context(|| format!("reading #{name}"))
}

/// Counts from [`backfill_tags`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BackfillStats {
    /// Rows looked at.
    pub scanned: usize,
    /// Rows whose `post_tags` were rewritten from what is already stored.
    pub tagged: usize,
    /// Thread pages fetched for a row that still had no JSON body.
    pub threads: usize,
    /// Rows that gained a JSON body from their thread page.
    pub json_filled: usize,
    /// Rows still without a JSON body after that page (or the page failed).
    pub missing: usize,
}

struct JsonGap {
    post_id: String,
    thread_id: String,
    content: String,
}

fn in_nicks(nick: &str, want: &HashSet<String>) -> bool {
    want.is_empty() || want.contains(&nick.trim_start_matches('@').to_lowercase())
}

/// Replace `post_tags` for `post_id` when this body has JSON or at least one tag.
/// An empty feed item leaves a previous backfill alone. Returns whether tags were rewritten.
fn replace_post_tags(c: &Connection, post_id: &str, body_json: &str, content: &str) -> Result<bool> {
    let tags = extract_tags(body_json, content);
    if body_json.is_empty() && tags.is_empty() { return Ok(false); }
    c.execute("DELETE FROM post_tags WHERE post_id = ?1", params![post_id])?;
    for t in &tags {
        c.execute("INSERT OR IGNORE INTO post_tags (post_id, tag) VALUES (?1, ?2)", params![post_id, t])?;
    }
    Ok(true)
}

/// `--backfill-tags`: tag what is already stored, then re-read threads whose
/// `body_json` is still empty. Post text already stored is not replaced.
/// `nicks` (optional, case-insensitive) limits the rows.
pub async fn backfill_tags(http: &reqwest::Client, jar: &Jar, db: &Path, nicks: &[String]) -> Result<BackfillStats> {
    if !db.exists() {
        info!("backfill-tags: no database yet");
        return Ok(BackfillStats::default());
    }
    let c = Connection::open(db)?;
    ensure_posts_schema(&c)?;
    let want: HashSet<String> = nicks.iter().map(|n| n.trim_start_matches('@').to_lowercase()).collect();
    let mut stats = BackfillStats::default();
    {
        let mut st = c.prepare("SELECT post_id, body_json, content, nick FROM posts")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?;
        for row in rows {
            let (id, json, content, nick) = row?;
            if !in_nicks(&nick, &want) { continue; }
            stats.scanned += 1;
            if replace_post_tags(&c, &id, &json, &content)? { stats.tagged += 1; }
        }
    }
    let mut gaps: Vec<JsonGap> = Vec::new();
    {
        let mut st = c.prepare(
            "SELECT post_id, CASE WHEN parent_post_id = '' THEN post_id ELSE parent_post_id END, content, nick
             FROM posts WHERE body_json = '' ORDER BY 2, posted_at")?;
        let rows = st.query_map([], |r| Ok((JsonGap { post_id: r.get(0)?, thread_id: r.get(1)?, content: r.get(2)? }, r.get::<_, String>(3)?)))?;
        for row in rows {
            let (g, nick) = row?;
            if in_nicks(&nick, &want) { gaps.push(g); }
        }
    }
    let mut by_thread: Vec<(String, Vec<JsonGap>)> = Vec::new();
    for g in gaps {
        match by_thread.last_mut() {
            Some((t, v)) if *t == g.thread_id => v.push(g),
            _ => by_thread.push((g.thread_id.clone(), vec![g])),
        }
    }
    info!("backfill-tags: {} rows, {} threads still without a JSON body", stats.scanned, by_thread.len());
    for (i, (tid, rows)) in by_thread.iter().enumerate() {
        if i > 0 { polite_pause().await; }
        stats.threads += 1;
        let items = match fetch_thread(http, jar, tid).await {
            Ok((th, mut r)) => { if let Some(root) = th.get("root") { r.push(root.clone()); } r }
            Err(e) => { err!("warning: backfill-tags thread {tid}: {e:#}"); stats.missing += rows.len(); continue; }
        };
        for row in rows {
            let Some(item) = items.iter().find(|x| s(x, "id") == row.post_id) else {
                stats.missing += 1;
                continue;
            };
            let parent = if row.thread_id == row.post_id { "" } else { row.thread_id.as_str() };
            let fp = to_post(item, parent, "");
            if fp.body_json.is_empty() { stats.missing += 1; continue; }
            c.execute("UPDATE posts SET body_json = ?1 WHERE post_id = ?2 AND body_json = ''", params![fp.body_json, row.post_id])?;
            let text = if fp.content.len() > row.content.len() { fp.content.as_str() } else { row.content.as_str() };
            replace_post_tags(&c, &row.post_id, &fp.body_json, text)?;
            stats.json_filled += 1;
        }
        if (i + 1) % 25 == 0 { info!("backfill-tags: {}/{} threads", i + 1, by_thread.len()); }
    }
    info!("backfill-tags: {} scanned, {} tagged, {} json filled, {} still missing ({} threads)",
        stats.scanned, stats.tagged, stats.json_filled, stats.missing, stats.threads);
    Ok(stats)
}

/// `--site-search`: read the Explore search results and store them.
pub async fn read_site_search(http: &reqwest::Client, jar: &Jar, query: &str) -> Result<usize> {
    let q = query.trim();
    if q.is_empty() { bail!("site search needs a keyword"); }
    let url = site_search_url(q);
    let page = feed_explore_url(false);
    let fields = [("q", q)];
    read_view(http, jar, &url, &page, SEARCH_EVENT, &fields, &search_key(q), &search_title(q))
        .await
        .with_context(|| format!("searching the site for {q:?}"))
}

#[cfg(test)]
mod tests;
