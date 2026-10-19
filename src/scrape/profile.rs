//! Profile "Posts and replies" feed filter.
//!
//! Verified live on the logged-in site: the profile page has a Filter dialog
//! (`<dialog id="js-configureFeed">`) with `<form phx-submit="configure-feed">`
//! and radios `name="replies"` (`0` = Posts, the default; `1` = Posts and
//! replies). Submitting patches the URL to `/rs/@{nick}?replies=1`, and the feed
//! then also lists the nick's replies ("Replying to …"). The scraper loads that
//! URL directly and pages it with `page-next`; if the URL has no feed, it falls
//! back to pushing the `configure-feed` form on the profile LiveView.

use crate::prelude::*;

/// Fetch the nick's "Posts and replies" feed, paged to the end (or until a page
/// is all known when `known` is given). Items are not yet deduplicated against
/// the Posts feed; see [`merge_feed_items`].
pub async fn fetch_replies_feed(
    http: &reqwest::Client, jar: &Jar, nick: &str, profile_html: &str,
    known: Option<&HashSet<String>>, limit: Option<usize>,
) -> Result<Vec<Value>> {
    let url = profile_replies_url(nick);
    match fetch_feed(http, jar, &url, limit, known, true).await {
        Ok(feed) => {
            verbose!("profile: posts and replies via {url}: {} items", feed.items.len());
            Ok(feed.items)
        }
        Err(e) => {
            verbose!("profile: {url} failed ({e:#}); falling back to {PROFILE_FILTER_EVENT}");
            replies_via_form(jar, nick, profile_html, known, limit).await
        }
    }
}

/// Fallback: submit the Filter form (`configure-feed`, `replies=1`) on the
/// profile LiveView, read the re-rendered Feed props, then page as usual.
async fn replies_via_form(
    jar: &Jar, nick: &str, profile_html: &str, known: Option<&HashSet<String>>,
    limit: Option<usize>,
) -> Result<Vec<Value>> {
    let page = parse_lv(&profile_url(nick), profile_html)?;
    let mut lv = Lv::connect(&page, jar).await?;
    let r = lv.form(PROFILE_FILTER_EVENT, PROFILE_REPLIES_FIELD, "1", None).await?;
    if r["status"] != "ok" { bail!("{PROFILE_FILTER_EVENT} failed: {r}"); }
    let body = reply_body(&r);
    let feed = if body["posts"].is_array() { body.clone() }
        else { props(&lv.html(), FEED_PROPS_SEL).context("no forum/Feed props after configure-feed")? };
    let mut items: Vec<Value> = feed["posts"].as_array().cloned().unwrap_or_default();
    let at_end = feed["at_end"].as_bool().unwrap_or(true);
    let cursor = feed["next_cursor"].as_str().map(str::to_string);
    if known.is_some_and(|k| page_all_known(&items, k)) {
        info!("feed: initial page all known — stopping (use --full to re-crawl)");
    } else if !at_end && cursor.is_some() && limit.is_none_or(|l| items.len() < l) {
        page_rest(&mut lv, &mut items, cursor, limit, known, true).await?;
    }
    if let Some(l) = limit { items.truncate(l); }
    Ok(items)
}

#[cfg(test)]
mod tests;
