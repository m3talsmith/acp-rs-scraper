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

/// Pinned account id and the Argon2 parameters used to derive the master secret.
/// Public: the phrase, the password and the master secret are never written here.
pub const ACP_ID_REL_PATH: &str = ".acp/acp-id";

/// Encrypted copy of the 24-word phrase entropy (mode `0600`). The words are not
/// stored in the clear. Absent when `--identity-init` ran before this file existed.
pub const RECOVERY_SEAL_REL_PATH: &str = ".acp/recovery.seal";

/// Present after a database is created or migrated, until the recovery steps have
/// been shown. Not a secret, and not part of a backup.
pub const RECOVERY_NEXT_REL_PATH: &str = ".acp/recovery-next";

/// Present until `--shares-print` or a successful `--shares-verify`. Not a secret.
pub const SHARES_PENDING_REL_PATH: &str = ".acp/shares-pending";

/// Unix time of the last successful `--shares-verify` or `--shares-print`. Not a secret.
pub const SHARES_VERIFIED_REL_PATH: &str = ".acp/shares-verified";

/// Plain node secret from an earlier draft. This build does not create it.
/// Backups skip it so a restore cannot clone a node identity.
pub const NODE_SECRET_REL_PATH: &str = ".acp/node/identity.key";

/// Sealed node keystore (`0600`). The database key opens it. Backups skip it.
pub const NODE_KEYSTORE_REL_PATH: &str = ".acp/node/keystore";

/// Public node record: label, peer id, fingerprint, and certificates. No secrets.
pub const NODE_PUBLIC_REL_PATH: &str = ".acp/node/node.public";

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

/// CSS selector for the edit-history component's JSON props (`forum/Edits`).
pub const EDITS_PROPS_SEL: &str = "div#forum\\/Edits[data-props]";

/// Found in Feed-*.js: `live.pushEvent("page-next", {cursor}, reply => ...)`,
/// reply = {posts, next_cursor, at_end}. No phx-target (sent to the root LiveView).
pub const FEED_NEXT_EVENT: &str = "page-next";

/// GUESSED: thread replies use the generic InfiniteScroll hook
/// (`pushEventTo(this.el, "next-page", {})`), target cid read from the page.
pub const THREAD_NEXT_EVENT: &str = "next-page";
/// Thread sort hook (`sort {sort:"oldest"|"top"}`). Newest is the page default and sends nothing.
pub const THREAD_SORT_EVENT: &str = "sort";

/// Safety cap on pages fetched from any one feed or thread.
pub const MAX_PAGES: u32 = 10_000;
/// Milliseconds to wait after each thread page fetched only to complete a
/// truncated or missing body (scrape fill-in and `--refetch-truncated`).
pub const THREAD_FETCH_PAUSE_MS: u64 = 250;

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
/// Explore search form (`<form phx-submit="search">`, field `q`).
pub const SEARCH_EVENT: &str = "search";
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

// ---------------------------------------------------------------- terminal UI
/// Arguments that do not count as "a flag was given" when deciding whether a bare
/// invocation opens the terminal UI (logging only; they carry over to TUI scans).
pub const TUI_NEUTRAL_ARGS: &[&str] = &["tui", "quiet", "verbose", "debug", "enable_logging", "log", "error_log", "theme"];

/// Theme files (`<name>.toml`), under `$HOME`. Every theme, the default included, lives here.
pub const THEMES_REL_DIR: &str = ".acp/themes";
/// Where themes are downloaded from: the repo's `themes/` at this version's tag
/// (`<base><name>.toml`). Themes are never installed by packages or built into the binary.
pub const THEME_DOWNLOAD_BASE: &str = concat!("https://raw.githubusercontent.com/m3talsmith/acp-rs-scraper/v", env!("CARGO_PKG_VERSION"), "/themes/");
/// The default theme's download URL (fetched on the first run into an empty themes folder).
pub const DEFAULT_THEME_URL: &str = concat!("https://raw.githubusercontent.com/m3talsmith/acp-rs-scraper/v", env!("CARGO_PKG_VERSION"), "/themes/default.toml");
/// Overrides [`THEME_DOWNLOAD_BASE`] (a URL ending in `/`), e.g. the mocked tests' server.
pub const THEME_URL_ENV: &str = "ACP_THEME_URL";
/// `off` / `0` / `false` / `no` turns the automatic first-run theme download off.
pub const THEME_DOWNLOAD_ENV: &str = "ACP_THEME_DOWNLOAD";
/// How long a theme download may take.
pub const THEME_DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
/// Themes in the repo's `themes/` that `--theme-init` and the palette offer by name.
pub const SHIPPED_THEMES: &[&str] = &["default", "soviet", "constructivist", "cuba", "red-star", "chinese-poster", "plattenbau"];

/// Download URL of theme `name`: [`THEME_URL_ENV`] (or [`THEME_DOWNLOAD_BASE`]) + `name.toml`.
pub fn theme_download_url(name: &str) -> String {
    let base = std::env::var(THEME_URL_ENV).ok().filter(|b| !b.trim().is_empty()).unwrap_or_else(|| THEME_DOWNLOAD_BASE.into());
    format!("{}{}{name}.toml", base.trim_end(), if base.trim_end().ends_with('/') { "" } else { "/" })
}
/// The theme used when `[ui] theme` is not set.
pub const DEFAULT_THEME: &str = "default";
/// Theme file format written by this build (files with a newer `version` are refused).
pub const THEME_FORMAT_VERSION: i64 = 1;
/// Largest theme file read (bigger ones are refused).
pub const THEME_MAX_BYTES: u64 = 64 * 1024;
/// Name a migrated `[theme]` table from preferences.toml is saved as.
pub const MIGRATED_THEME: &str = "custom";
/// Lowest text contrast the theme editor accepts without a warning.
pub const THEME_MIN_CONTRAST: f64 = 3.0;

/// Title shown in the TUI top bar.
pub const TUI_TITLE: &str = "ACP Red Star Scraper";
/// The header text next to the big "ACP".
pub const TUI_SUBTITLE: &str = "Red Star Scraper";
/// Deepest stack of nested post details (linked posts opened from a detail).
pub const MAX_DETAIL_DEPTH: usize = 8;
/// Width of the nick bar (cells).
pub const NICK_BAR_WIDTH: u16 = 28;
/// Rows the post list keeps visible above and below an open detail (the "residual strip").
pub const DETAIL_RESIDUAL_ROWS: u16 = 4;
/// Rows each collapsed (parent) detail keeps as a header strip once a linked post opens over it.
pub const DETAIL_HEADER_ROWS: u16 = 2;
/// Slide/push animation length.
pub const TUI_ANIM_MS: u64 = 180;
/// Redraw interval while an animation runs.
pub const TUI_FRAME_MS: u64 = 16;
/// Idle redraw / background-job poll interval.
pub const TUI_TICK_MS: u64 = 250;
/// Status lines kept from scans and log output.
pub const TUI_STATUS_KEEP: usize = 500;
/// Most posts open at once in the flat view modes (Stack, Pane, Carousel, Grid).
pub const MAX_FLAT_DEPTH: usize = 256;
/// Default minutes between automatic TUI syncs (Sync › Interval).
pub const TUI_SYNC_DEFAULT_MINUTES: u64 = 15;
/// Smallest terminal the layout supports.
pub const TUI_MIN_SIZE: (u16, u16) = (60, 19);
/// Rows of the TUI header (logo, big "ACP" with a row of room above it, title, session).
pub const TUI_HEADER_ROWS: u16 = 4;
/// Research project files (copies, web snapshots), under `$HOME`.
pub const RESEARCH_REL_DIR: &str = ".acp/research";
/// Converted image cache (AVIF → PNG), under `$HOME`.
pub const THUMBS_REL_DIR: &str = ".acp/cache/thumbs";
/// Your own empty-Body animation (`tools/acp-anim` writes it), relative to `$HOME`; `.acpanim.gz` or `.acpanim`.
pub const ANIMATION_REL_PATH: &str = ".acp/animations/empty-body.acpanim";
/// Smallest Body area (columns, rows) the empty-Body animation plays in; smaller pauses it.
pub const EMPTY_ANIM_MIN: (u16, u16) = (24, 8);
/// Log files of scans/jobs started from the TUI (`tui-<kind>-<stamp>.log`), inside [`LOGS_REL_DIR`].
pub const TUI_JOB_LOG_PREFIX: &str = "tui-";
/// Key bindings shown in the Help modal (and the README).
pub const TUI_KEYBINDINGS: &[(&str, &str)] = &[
    ("Tab / Shift-Tab", "Focus Nicks → Body → Menu → Status"),
    ("↑ ↓  j k", "Move in the focused list; scroll a detail"),
    ("Enter", "Open: nick (only it), post detail, linked post, menu item; on Status, job details"),
    ("Status · Enter / click", "Job details (status bar or the header ⟳). Esc closes. c cancels a running job. j/k scroll the log"),
    ("Space", "Nicks: add/remove from the selection; menu: toggle checkbox"),
    ("Shift-↑ / Shift-↓", "Nicks: extend the selection"),
    ("Ctrl-A", "Nicks: select every (shown) nick"),
    ("/", "Nicks: filter the nick list (type to filter, Enter/↓ to the list, Esc clears)"),
    ("← →  [ ]", "Detail: previous / next link"),
    ("Esc / Backspace", "Close the top detail; close menu / modal"),
    ("<", "Back from search results / bookmarks"),
    ("b", "Bookmark / unbookmark the open or highlighted post (or click its ☆/★)"),
    ("p", "Post menu: view reposts or the edit history of the open or highlighted post"),
    ("s", "In a thread: cycle reply order newest → oldest → top"),
    ("n", "New post. Cancel is the default; Publish does not send until a guided capture"),
    ("r", "On an open post: reply (a comment pre-fills the reference). Otherwise add to a research project"),
    ("Q", "On an open post: reply with that post quoted"),
    ("x", "Add the open or highlighted post to the export basket, or take it out (or click its + / ✓); Export › Export basket writes it"),
    (", .  /  Alt-← Alt-→", "Previous / next post of the detail stack (◂ prev / next ▸ on the nav bar)"),
    ("v / V", "Next / previous view mode: Nested, Stack, Stack + pane, Carousel ∞, Carousel, Grid"),
    ("← →  (Carousel, Grid)", "Previous / next post; ↑ ↓ move a row in the Grid"),
    ("B", "Show bookmarks (the ★ Bookmarks row on the nick bar)"),
    ("PgUp PgDn Home End", "Page / jump"),
    ("m / F10 / Alt+letter", "Menu bar (underlined letter opens a command)"),
    ("F6", "Start Scanning (Cancel while a job runs)"),
    ("F5", "Theme picker (also the ‹theme› ▾ switch in the header)"),
    ("Shift-F5", "Theme editor"),
    ("Shift-S", "Search modal: keyword, nicks, post id, dates, posts/replies"),
    ("Ctrl-S (Ctrl-Shift-S)", "Clear the search"),
    ("Ctrl-Space / Ctrl-P", "Command palette"),
    ("l", "Show / hide the log panel"),
    ("Ctrl-R", "Re-read nicks from the database"),
    ("? / F1", "Help"),
    ("q / Ctrl-C", "Quit (a running scan keeps going in the background)"),
    ("Mouse", "Click anything; Ctrl/Shift-click nicks to multi-select; wheel scrolls"),
];

// ---------------------------------------------------------------- bookmarks
/// CONFIRMED (live sync, 2.5.0): the logged-in user's bookmarks feed, a
/// `forum/Feed` page like a profile. Override the path with `ACP_BOOKMARKS_PATH`.
pub fn bookmarks_url() -> String {
    let p = std::env::var("ACP_BOOKMARKS_PATH").ok().filter(|v| v.starts_with('/')).unwrap_or_else(|| "/rs/bookmarks".into());
    format!("{}{p}", base_url())
}
/// LiveView hook that sets a post's bookmark (`{"id", "state": "1"|"0"}`).
/// The site's current page sends this. 2.5.0 used the click `toggle-bookmark`;
/// that name still worked then and may disappear (2.24.1).
pub const BOOKMARK_EVENT: &str = "bookmark";
