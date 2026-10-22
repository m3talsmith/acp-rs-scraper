//! ACP Red Star (<https://acp.us/rs>) scraper library.
//!
//! Logs in to the Red Star forum (a Phoenix LiveView app), scrapes members'
//! posts, replies, mentions and profiles into SQLite, downloads attachments,
//! and exports/imports JSON or HTML. The binary (`src/main.rs`) is a thin
//! wrapper around [`app::run`].
//!
//! Modules by domain:
//! - [`cli`]: command-line arguments, nick parsing, prompts, man/completions.
//! - [`config`]: every constant (paths, site URLs, LiveView events, selectors)
//!   plus the `ACP_BASE_URL` override used by the mocked end-to-end tests.
//! - [`logging`]: `-q`/`-v`/`--debug` levels and the `info!`/`verbose!`/`err!` macros.
//! - [`http`]: the Firefox-like HTTP client.
//! - [`auth`]: email + one-time-code login, saved preferences.
//! - [`liveview`]: LiveView websocket client and diff renderer.
//! - [`scrape`]: profile/site feeds, threads, "Posts and replies" filter, mentions.
//! - [`site`]: New post and Reply (3.4.0), with local attachments (3.5.0). Text publish sends validate and save (3.31.0). The site copy of images, videos, and PDFs uploads first (3.32.0). A reply edit sends validate and save (3.33.0). Votes, subscribes, deletes, reposts, and top-level post edits stay gated. The site does not support changing attachments on an edit.
//! - [`outbox`]: Drafts and the send queue (3.34.0). A draft is stored before it is sent. A queued text post or reply uses the same confirm rules as a direct publish.
//! - [`media`]: attachment discovery and download.
//! - [`storage`]: SQLite schema and queries.
//! - [`export`]: JSON / HTML rendering and `.tar.gz` archives; [`import`] reads them back.
//! - [`branding`]: the slogan shared by the welcome screen, login, About, and the status bar.
//! - [`brands`]: logo, title, and motto. The built-in default is ACP Red Star. User files live in `~/.acp/brands`.
//! - [`bookmarks`]: local bookmarks and `--sync-bookmarks` with the site.
//! - [`maintenance`]: `--remove-nick` and `--clear-all`.
//! - [`themes`]: theme files in `~/.acp/themes` (format, loading, first-run download, migration).
//! - [`tui`]: the terminal UI (the default with no flags): browse nicks and posts,
//!   drive scrape/search/export/import/maintenance from a menu bar.
//! - [`crypto`]: master secret, recovery phrase and shares, `acp-id`, encryption
//!   at rest, and encrypted backups.
//! - [`eggs`]: a few surprises for the curious (`docs/.secrets/EASTER_EGGS.md`).
//! - [`app`]: wires the above into the command-line program.

#![warn(missing_docs)]

#[macro_use]
pub mod logging;
pub mod notify;
pub mod app;
pub mod auth;
pub mod branding;
pub mod brands;
pub mod bookmarks;
pub mod cli;
pub mod config;
pub mod crypto;
pub mod eggs;
pub mod export;
pub mod http;
pub mod import;
pub mod livefeed;
pub mod liveview;
pub mod maintenance;
pub mod media;
pub mod outbox;
pub mod research;
pub mod rules;
pub mod scrape;
pub mod service;
pub mod site;
pub mod storage;
pub mod themes;
pub mod tui;

/// Everything most modules need, re-exported in one place: the crate's own
/// items (so domain modules can call each other without long paths) and the
/// external types used throughout.
pub mod prelude {
    pub use crate::app::*;
    pub use crate::auth::{prefs::*, session::*, *};
    pub use crate::bookmarks::*;
    pub use crate::cli::*;
    pub use crate::config::*;
    pub use crate::export::{archive::*, html::*, json::*, *};
    pub use crate::http::*;
    pub use crate::import::*;
    pub use crate::liveview::{render::*, *};
    pub use crate::logging::*;
    pub use crate::maintenance::*;
    pub use crate::media::*;
    pub use crate::research::*;
    pub use crate::scrape::{edits::*, feed::*, mentions::*, profile::*, reads::*, refetch::*, thread::*, *};
    pub use crate::storage::{db::*, schema::*};

    pub use anyhow::{Context, Result, anyhow, bail};
    pub use futures_util::{SinkExt, StreamExt};
    pub use reqwest::cookie::{CookieStore, Jar};
    pub use rusqlite::{Connection, OptionalExtension, params};
    pub use scraper::{Html, Selector};
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::{Value, json};
    pub use std::collections::{BTreeSet, HashMap, HashSet, hash_map::DefaultHasher};
    pub use std::hash::{Hash, Hasher};
    pub use std::io::{self, Read, Write};
    pub use std::path::{Path, PathBuf};
    pub use std::sync::Arc;
    pub use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    pub use url::Url;
}
