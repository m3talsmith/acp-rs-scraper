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
        let mut lv = Lv::connect(&page, jar).await?;
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
