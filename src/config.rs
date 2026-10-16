//! Shared constants, grouped by domain, and the site base URL.
//!
//! The site URL defaults to <https://acp.us> and can be overridden with the
//! `ACP_BASE_URL` environment variable (e.g. `http://127.0.0.1:4000`), which is
//! how the mocked end-to-end tests point the scraper at a local fake server.
//! Websocket URLs follow the scheme (`http` → `ws`, `https` → `wss`).

use crate::prelude::*;

// ---------------------------------------------------------------- site URLs
/// Default site origin.
pub const DEFAULT_BASE_URL: &str = "https://acp.us";
/// Environment variable overriding [`DEFAULT_BASE_URL`].
pub const BASE_URL_ENV: &str = "ACP_BASE_URL";

/// Site origin without a trailing slash (`ACP_BASE_URL` or [`DEFAULT_BASE_URL`]).
pub fn base_url() -> &'static str {
    static B: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    B.get_or_init(|| {
        std::env::var(BASE_URL_ENV).ok().filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.into()).trim_end_matches('/').to_string()
    })
}
/// Forum start page; redirects to `/auth` when logged out.
pub fn start_url() -> String { format!("{}/rs", base_url()) }
/// Profile page (Latest feed) for a nick.
pub fn profile_url(nick: &str) -> String { format!("{}/rs/@{nick}", base_url()) }
/// Profile feed including the nick's replies (the Filter dialog's
/// "Posts and replies" option, `replies=1`).
pub fn profile_replies_url(nick: &str) -> String { format!("{}/rs/@{nick}?replies=1", base_url()) }
/// Profile Top feed over all time.
pub fn profile_top_url(nick: &str) -> String { format!("{}/rs/@{nick}/top?t=all", base_url()) }
/// Thread page for a root post id.
pub fn thread_url(id: &str) -> String { format!("{}/rs/post/{id}", base_url()) }
/// Site-wide Trending feed (`/rs`, the site's own default tab; `/rs/trending` is 404).
pub fn feed_trending_url() -> String { format!("{}/rs", base_url()) }
/// Site-wide Latest feed.
pub fn feed_latest_url() -> String { format!("{}/rs/latest", base_url()) }
/// LiveView websocket endpoint for an (already URL-encoded) CSRF token.
pub fn ws_url(csrf_enc: &str) -> String {
    let b = base_url();
    let ws = if let Some(r) = b.strip_prefix("https://") { format!("wss://{r}") }
        else if let Some(r) = b.strip_prefix("http://") { format!("ws://{r}") } else { b.to_string() };
    format!("{ws}/live/websocket?_csrf_token={csrf_enc}&vsn=2.0.0")
}

// ---------------------------------------------------------------- local paths

/// Single place to change the DB location (spec also mentioned ~/.acp/ai/rs.database).
pub const DB_REL_PATH: &str = ".acp/ai/data/rs.database";

/// Downloaded attachments, relative to `$HOME`.
pub const ATTACH_REL_PATH: &str = ".acp/ai/data/attachments";

/// Default export / archive directory, relative to `$HOME`.
pub const EXPORTS_REL_PATH: &str = ".acp/ai/data/exports";

/// Preferences file (the login email), relative to `$HOME`.
pub const PREFS_REL_PATH: &str = ".acp/preferences.toml";

/// Saved login session cookies (JSON, mode `0600`), relative to `$HOME`. Removed by `--logout`.
pub const SESSION_REL_PATH: &str = ".acp/session.json";

/// Log directory (for `--enable-logging` / `--debug`), relative to `$HOME`.
pub const LOGS_REL_DIR: &str = ".acp/logs";

/// Default success/access log file name inside [`LOGS_REL_DIR`] (override with `--log`).
pub const SUCCESS_LOG_NAME: &str = "success.log";

/// Default error log file name inside [`LOGS_REL_DIR`] (override with `--error-log`).
pub const ERROR_LOG_NAME: &str = "error.log";

// ---------------------------------------------------------------- browser identity
/// User-Agent sent on every request (desktop Firefox).
pub const FIREFOX_UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0";

/// Email form submit event on `/auth`.
pub const EMAIL_EVENT: &str = "send-login-email";

/// Email input name on `/auth`.
pub const EMAIL_FIELD: &str = "user[email]";

/// Component id of the email form (`phx-target="1"`).
pub const EMAIL_FORM_CID: u64 = 1; // phx-target="1" on #auth-page-email-form

/// Code form event used when none can be read from the page (guessed).
pub const CODE_EVENT_FALLBACK: &str = "verify-code";

/// Verified from a real run: action of the phx-trigger-action form.
pub const OTP_SIGN_IN_FALLBACK: &str = "/auth/user/otp/sign_in";

/// Code input name used when none can be read from the page (guessed).
pub const CODE_FIELD_FALLBACK: &str = "user[code]";

/// CSS selector of the Svelte Feed mount (`data-props` = `{posts, next_cursor, at_end}`).
pub const FEED_PROPS_SEL: &str = "div#forum\\/Feed[data-props]";

/// CSS selector of the Svelte Thread mount (`data-props` = `{root, replies}`).
pub const THREAD_PROPS_SEL: &str = "div#forum\\/Thread[data-props]";

/// Found in Feed-*.js: `live.pushEvent("page-next", {cursor}, reply => ...)`,
/// reply = {posts, next_cursor, at_end}. No phx-target (sent to the root LiveView).
pub const FEED_NEXT_EVENT: &str = "page-next";

/// GUESSED: thread replies use the generic InfiniteScroll hook
/// (`pushEventTo(this.el, "next-page", {})`), target cid read from the page.
pub const THREAD_NEXT_EVENT: &str = "next-page";

/// Safety cap on pages fetched from any one feed or thread.
pub const MAX_PAGES: u32 = 10_000;

/// GUESSED (ForumProfileInfo hook): hover-card event, unused for scraping.
pub const PROFILE_SHOW_INFO_EVENT: &str = "show-info";

/// Default `--crawl-limit`: site-feed posts scanned for mentions.
pub const DEFAULT_CRAWL_LIMIT: usize = 500;

/// The user's home directory (`$HOME`); every data path is relative to it.
pub fn home() -> Result<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| anyhow!("HOME not set"))
}

/// Profile Filter form submit event (`<form phx-submit="configure-feed">`).
pub const PROFILE_FILTER_EVENT: &str = "configure-feed";
/// Filter form radio: `0` = Posts (default), `1` = Posts and replies.
pub const PROFILE_REPLIES_FIELD: &str = "replies";

// ---------------------------------------------------------------- media
/// Verified from the site CSP img-src/media-src.
pub const MEDIA_HOSTS: &[&str] = &["media.acp.us", "posts-media.acp.us", "backblazeb2.com"];

// ---------------------------------------------------------------- export format
/// `format` field of every export document.
pub const EXPORT_FORMAT: &str = "acp-rs-export";

/// Export document version written by this build (imports accept 1 and 2).
pub const EXPORT_FORMAT_VERSION: u32 = 2;

/// Folder (under attachments) for downloaded profile images: `profiles/{nick}.{ext}`.
pub const PROFILE_IMAGES_DIR: &str = "profiles";

/// Id of the machine-readable JSON copy embedded in HTML exports.
pub const HTML_DATA_ID: &str = "acp-data";
