//! Paged feeds (profile, site-wide, tabs) via the `page-next` hook event.

use crate::prelude::*;

/// One feed (profile, /rs, tag...) fully paged via `page-next {cursor}`.
pub struct FeedPage {
    /// Feed posts in order, deduplicated by id.
    pub items: Vec<Value>,
    /// HTML of the first page (used for profile parsing).
    pub html: String,
    /// Paging stopped early at an all-known page.
    pub stopped_incremental: bool,
    /// Pages fetched.
    pub pages: u32,
}

/// True when every id in `page` is already known (and the page is non-empty).
pub fn page_all_known(page: &[Value], known: &HashSet<String>) -> bool {
    !page.is_empty() && page.iter().all(|x| {
        let id = s(x, "id");
        !id.is_empty() && known.contains(&id)
    })
}

/// Nick from a `/rs/@{nick}` URL path, if it is a profile URL.
pub fn profile_nick_of(url: &str) -> Option<String> {
    let u = Url::parse(url).ok()?;
    let rest = u.path().strip_prefix("/rs/@")?;
    let nick = rest.split('/').next().unwrap_or_default();
    (!nick.is_empty()).then(|| nick.to_string())
}

/// Fail when a profile URL (`/rs/@nick`) did not land on that profile.
/// ACP answers an unknown nick with a 302 to `/rs` (Trending) and a flash
/// "No profile exists with the username @nick"; reqwest follows redirects,
/// so without this check the Trending feed would be scraped as the profile.
pub fn check_profile_landing(requested: &str, final_url: &str, html: &str) -> Result<()> {
    let Some(want) = profile_nick_of(requested) else { return Ok(()) };
    let got = profile_nick_of(final_url);
    if !got.as_deref().is_some_and(|g| g.eq_ignore_ascii_case(&want)) {
        let to = Url::parse(final_url).map(|u| u.path().to_string()).unwrap_or_else(|_| final_url.to_string());
        bail!("no profile @{want}: {requested} redirected to {to} (no such user? check the nick)");
    }
    if html.contains("No profile exists with the username") {
        bail!("no profile @{want}: site says \"No profile exists with the username @{want}\"");
    }
    Ok(())
}

/// Fetch a feed page (profile, site or filter URL), then page it to the end with `page-next`, honouring `limit` and incremental early-stop against `known`.
pub async fn fetch_feed(
    http: &reqwest::Client, jar: &Jar, url: &str, limit: Option<usize>,
    known: Option<&HashSet<String>>,
    allow_incremental: bool, // false for Trending: no early stop on all-known pages
) -> Result<FeedPage> {
    let resp = http.get(url).send().await?.error_for_status()?;
    let final_url = resp.url().to_string();
    let html = resp.text().await?;
    check_profile_landing(url, &final_url, &html)?;
    let feed = props(&html, FEED_PROPS_SEL).with_context(|| format!("no forum/Feed data-props on {url}"))?;
    let mut items: Vec<Value> = feed["posts"].as_array().cloned().unwrap_or_default();
    let mut at_end = feed["at_end"].as_bool().unwrap_or(true);
    let cursor = feed["next_cursor"].as_str().map(str::to_string);
    let hit_limit = |n: usize| limit.is_some_and(|l| n >= l);
    let mut stopped_incremental = false;
    let mut pages: u32 = 1;
    info!("feed {url}: {} posts from initial page", items.len());
    if allow_incremental
        && let Some(k) = known
        && page_all_known(&items, k) {
            info!("feed: initial page all known — stopping (use --full to re-crawl)");
            stopped_incremental = true;
            at_end = true;
        }

    if !at_end && cursor.is_some() && !hit_limit(items.len()) && !stopped_incremental {
        let page = parse_lv(&final_url, &html)?;
        let mut lv = Lv::connect_bounded(&page, jar, explore_reply_timeout(&final_url)).await?;
        let (stop, extra) = page_rest(&mut lv, &mut items, cursor, limit, known, allow_incremental).await?;
        stopped_incremental = stop;
        pages += extra;
    }
    if let Some(l) = limit { items.truncate(l); }
    Ok(FeedPage { items, html, stopped_incremental, pages })
}

/// Page a feed that is open on `lv` to its end with `page-next {cursor}`,
/// appending new (id-deduped) posts to `items`. Returns
/// `(stopped_incremental, extra_pages)`. Shared by profile, site and
/// "Posts and replies" feeds so incremental/limit rules are identical.
pub async fn page_rest(
    lv: &mut Lv, items: &mut Vec<Value>, mut cursor: Option<String>, limit: Option<usize>,
    known: Option<&HashSet<String>>, allow_incremental: bool,
) -> Result<(bool, u32)> {
    let hit_limit = |n: usize| limit.is_some_and(|l| n >= l);
    let mut ids: HashSet<String> = items.iter().map(|x| s(x, "id")).collect();
    let mut stale = 0;
    let mut pages = 0;
    let mut at_end = false;
    let mut stopped_incremental = false;
    for _ in 0..MAX_PAGES {
        let Some(c) = cursor.clone() else { at_end = true; break };
        let r = lv.push("event", json!({"type": "hook", "event": FEED_NEXT_EVENT, "value": {"cursor": c}})).await?;
        if r["status"] != "ok" { err!("warning: page-next failed: {r}"); break; }
        let body = reply_body(&r);
        let body = if body["posts"].is_array() { body.clone() }
            else { props(&lv.html(), FEED_PROPS_SEL).unwrap_or(Value::Null) };
        let ps = body["posts"].as_array().cloned().unwrap_or_default();
        pages += 1;
        if allow_incremental
            && let Some(k) = known
            && page_all_known(&ps, k) {
                info!("feed: page {} all known — stopping (use --full to re-crawl)", pages + 1);
                stopped_incremental = true;
                break;
            }
        let new: Vec<Value> = ps.into_iter().filter(|x| ids.insert(s(x, "id"))).collect();
        verbose!("feed: +{} new posts", new.len());
        items.extend(new.iter().cloned());
        at_end = body["at_end"].as_bool().unwrap_or(new.is_empty());
        cursor = body["next_cursor"].as_str().map(str::to_string).or(cursor);
        if new.is_empty() { stale += 1; } else { stale = 0; }
        if hit_limit(items.len()) { info!("feed: reached limit {}", limit.unwrap_or(0)); break; }
        if at_end || stale >= 2 || (new.is_empty() && cursor.as_deref() == Some(c.as_str())) { break; }
    }
    if !at_end && !hit_limit(items.len()) && !stopped_incremental {
        err!("warning: feed paging stopped before at_end (run with --debug)");
    }
    Ok((stopped_incremental, pages))
}

/// Which profile feed a nick scrape reads.
///
/// `newest` is `/rs/@nick` (and Posts and replies). `top` is the captured
/// profile Top URL `/rs/@nick/top?t=all`. No other profile ranking was captured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProfileFeed {
    /// Latest profile (`/rs/@nick`), newest first.
    #[default]
    Newest,
    /// Profile Top (`/rs/@nick/top?t=all`). Not chronological.
    Top,
}

impl ProfileFeed {
    /// `newest` or `top`, ignoring case.
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "newest" => Ok(Self::Newest),
            "top" => Ok(Self::Top),
            other => Err(format!("--profile-feed must be newest or top, not {other:?}")),
        }
    }
}

impl std::fmt::Display for ProfileFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Self::Newest => "newest", Self::Top => "top" })
    }
}

/// Which end of a profile feed `--limit` keeps.
///
/// The site lists both the Latest profile and the profile Top feed from the
/// start of that ranking. There is no oldest-first profile URL. `oldest` reads
/// the list to the end and keeps that end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScrapeOrder {
    /// Keep the start of the site's list (newest posts, or the top of Top).
    #[default]
    Newest,
    /// Keep the end of the site's list (oldest posts, or the end of Top).
    Oldest,
}

impl ScrapeOrder {
    /// `newest` or `oldest`, ignoring case.
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "newest" => Ok(Self::Newest),
            "oldest" => Ok(Self::Oldest),
            other => Err(format!("--order must be newest or oldest, not {other:?}")),
        }
    }
}

impl std::fmt::Display for ScrapeOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Self::Newest => "newest", Self::Oldest => "oldest" })
    }
}

/// Nick profile items to keep.
///
/// With no limit and [`ScrapeOrder::Newest`], the Posts feed stays first and
/// the replies feed is merged in. The second value is how many kept items came
/// only from the replies feed.
///
/// With a limit and `newest`, the replies feed is newest-first and already
/// lists posts, so it is the window. Posts-feed items fill that window only
/// when the replies feed is shorter. Without a replies feed, the Posts feed
/// is truncated to `limit`.
///
/// [`ScrapeOrder::Oldest`] keeps the other end of those same newest-first
/// lists, then reverses the window so the oldest item is first. With a limit
/// that is the oldest `limit` items. With no limit the set matches `newest`
/// and only the order changes.
pub fn window_feed_items(
    posts: Vec<Value>, replies: Option<Vec<Value>>, limit: Option<usize>, order: ScrapeOrder,
) -> (Vec<Value>, usize) {
    let id_of = |v: &Value| s(v, "id");
    let post_ids: HashSet<String> = posts.iter().map(&id_of).filter(|id| !id.is_empty()).collect();
    let items = match replies {
        None => take_window(posts, limit, order),
        Some(extra) => match (limit, order) {
            (None, ScrapeOrder::Newest) => {
                let mut items = posts;
                merge_feed_items(&mut items, extra);
                items
            }
            (Some(n), ScrapeOrder::Newest) => newest_window(posts, extra, n),
            (_, ScrapeOrder::Oldest) => oldest_window(posts, extra, limit),
        },
    };
    let added = items.iter().filter(|it| {
        let id = id_of(it);
        !id.is_empty() && !post_ids.contains(&id)
    }).count();
    (items, added)
}

/// Keep the head (`newest`) or the tail (`oldest`) of a newest-first list.
///
/// `items` is already in the list's natural order: newest posts first, or the
/// site's own order for a ranking. `newest` keeps the start. `oldest` keeps
/// the end and reverses it, so the oldest item (or the end of a ranking) is
/// first. `limit` is how many to keep; `None` keeps the whole list.
pub fn apply_order_limit<T>(mut items: Vec<T>, limit: Option<usize>, order: ScrapeOrder) -> Vec<T> {
    if let Some(n) = limit {
        match order {
            ScrapeOrder::Newest => items.truncate(n),
            ScrapeOrder::Oldest => {
                if items.len() > n { items = items.split_off(items.len() - n); }
                items.reverse();
            }
        }
    } else if order == ScrapeOrder::Oldest {
        items.reverse();
    }
    items
}

/// Keep the head (`newest`) or the tail (`oldest`) of a newest-first feed page.
fn take_window(items: Vec<Value>, limit: Option<usize>, order: ScrapeOrder) -> Vec<Value> {
    apply_order_limit(items, limit, order)
}

/// Newest `n` authored items: the head of the replies feed, filled from the Posts head.
fn newest_window(posts: Vec<Value>, mut items: Vec<Value>, n: usize) -> Vec<Value> {
    items.truncate(n);
    if items.len() < n {
        let mut have: HashSet<String> = items.iter().map(|v| s(v, "id")).collect();
        for p in posts {
            if items.len() >= n { break; }
            let id = s(&p, "id");
            if id.is_empty() || !have.insert(id) { continue; }
            items.push(p);
        }
    }
    items
}

/// Oldest end of the replies feed, filled from the Posts tail when the replies feed is shorter.
/// The result is oldest-first. With no limit the merged set is reversed.
fn oldest_window(posts: Vec<Value>, extra: Vec<Value>, limit: Option<usize>) -> Vec<Value> {
    let Some(n) = limit else {
        let mut items = posts;
        merge_feed_items(&mut items, extra);
        items.reverse();
        return items;
    };
    let mut items = extra;
    if items.len() > n { items = items.split_off(items.len() - n); }
    if items.len() < n {
        let mut have: HashSet<String> = items.iter().map(|v| s(v, "id")).collect();
        let mut fill = Vec::new();
        for p in posts.into_iter().rev() {
            if items.len() + fill.len() >= n { break; }
            let id = s(&p, "id");
            if id.is_empty() || !have.insert(id) { continue; }
            fill.push(p);
        }
        fill.reverse();
        items.extend(fill);
    }
    items.reverse();
    items
}

/// Append `extra` feed items whose `id` isn't already in `items`.
/// Returns how many were added.
pub fn merge_feed_items(items: &mut Vec<Value>, extra: Vec<Value>) -> usize {
    let mut ids: HashSet<String> = items.iter().map(|x| s(x, "id")).collect();
    let before = items.len();
    items.extend(extra.into_iter().filter(|x| { let id = s(x, "id"); !id.is_empty() && ids.insert(id) }));
    items.len() - before
}

#[cfg(test)]
mod tests;
