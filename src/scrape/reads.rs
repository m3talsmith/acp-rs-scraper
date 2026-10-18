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
/// and submit `configure-feed` with `fields`, then page it the same way.
pub async fn fetch_configured(
    http: &reqwest::Client,
    jar: &Jar,
    direct: &str,
    page: &str,
    fields: &[(&str, &str)],
) -> Result<FeedPage> {
    match fetch_feed(http, jar, direct, None, None, false).await {
        Ok(p) => Ok(p),
        Err(e) => {
            verbose!("feed {direct} failed ({e:#}); {PROFILE_FILTER_EVENT} on {page}");
            configured_via_form(http, jar, page, fields)
                .await
                .with_context(|| format!("{direct} had no feed and {page} failed"))
        }
    }
}

async fn configured_via_form(
    http: &reqwest::Client,
    jar: &Jar,
    page: &str,
    fields: &[(&str, &str)],
) -> Result<FeedPage> {
    let resp = http.get(page).send().await?.error_for_status()?;
    let final_url = resp.url().to_string();
    let html = resp.text().await?;
    let lvpage = parse_lv(&final_url, &html)?;
    let mut lv = Lv::connect(&lvpage, jar).await?;
    let r = lv.form_fields(PROFILE_FILTER_EVENT, fields, None).await?;
    if r["status"] != "ok" {
        bail!("{PROFILE_FILTER_EVENT} failed: {r}");
    }
    let body = reply_body(&r);
    let feed = if body["posts"].is_array() {
        body.clone()
    } else {
        props(&lv.html(), FEED_PROPS_SEL).context("no forum/Feed after configure-feed")?
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
    read_view(http, jar, &feed_eb_url(replies), &feed_eb_page_url(), &fields, &eb_key(replies), &eb_title(replies))
        .await
        .context("reading the Executive Board feed")
}

/// Fetch one configured feed and store it under `key`.
async fn read_view(
    http: &reqwest::Client,
    jar: &Jar,
    direct: &str,
    page: &str,
    fields: &[(&str, &str)],
    key: &str,
    title: &str,
) -> Result<usize> {
    let fetched = fetch_configured(http, jar, direct, page, fields).await?;
    remember_feed(&fetched.items, key, title)
}

#[cfg(test)]
mod tests;
