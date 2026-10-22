//! HTTP client posing as desktop Firefox, with a shared cookie jar.

use crate::prelude::*;

/// HTTP client with Firefox headers, sharing `jar` for cookies (the login session).
pub fn client<C>(jar: Arc<C>) -> Result<reqwest::Client>
where
    C: CookieStore + 'static,
{
    client_with(jar, reqwest::redirect::Policy::default())
}

/// Same client as [`client`], with `redirect` instead of the default policy.
///
/// The host uses this to record each `Location` it follows. Other callers keep
/// [`client`].
pub fn client_with<C>(jar: Arc<C>, redirect: reqwest::redirect::Policy) -> Result<reqwest::Client>
where
    C: CookieStore + 'static,
{
    use reqwest::header::*;
    let mut h = HeaderMap::new();
    h.insert(ACCEPT, HeaderValue::from_static(
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"));
    h.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
    h.insert("DNT", HeaderValue::from_static("1"));
    h.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
    h.insert("Sec-Fetch-Dest", HeaderValue::from_static("document"));
    h.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
    h.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    Ok(reqwest::Client::builder()
        .user_agent(FIREFOX_UA)
        .default_headers(h)
        .redirect(redirect)
        .cookie_provider(jar)
        .build()?)
}
