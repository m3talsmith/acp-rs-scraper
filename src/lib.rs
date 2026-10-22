//! ACP Red Star, a local client for posts already on this machine.
//!
//! The terminal UI browses, searches, exports, and imports stored rows.
//! Identity, the encrypted database, brands, and themes stay here. A service
//! that fetches from a site is a separate package. The binary (`src/main.rs`)
//! is a thin wrapper around [`app::run`].
//!
//! Modules by domain:
//! - [`cli`]: command-line arguments, nick parsing, prompts, man/completions.
//! - [`config`]: paths and limits the core still uses.
//! - [`logging`]: `-q`/`-v`/`--debug` levels and the `info!`/`verbose!`/`err!` macros.
//! - [`get`]: a plain GET client for theme files.
//! - [`prefs`]: saved preferences.
//! - [`stored`]: rows the database and the export already know.
//! - [`outbox`]: Drafts and the local send queue (3.34.0). Sending a row is the service package.
//! - [`plugins`]: the package gate. A failed archive is quarantined. A version mismatch is refused and is not moved. A discovered package stays off.
//! - [`host_api`]: the [`acp_rs_plugin::Host`] a service calls instead of opening the database.
//! - [`drive`]: scrape, sync, login, and compose flags call a loaded service. Until one implements the capability, those commands are not in this binary.
//! - [`storage`]: SQLite schema and queries.
//! - [`export`]: JSON / HTML rendering and `.tar.gz` archives; [`import`] reads them back.
//! - [`branding`]: the slogan shared by the welcome screen, About, and the status bar.
//! - [`brands`]: logo, title, and motto. The built-in default is ACP Red Star. User files live in `~/.acp/brands`.
//! - [`bookmarks`]: local bookmarks.
//! - [`maintenance`]: `--remove-nick` and `--clear-all`.
//! - [`themes`]: theme files in `~/.acp/themes` (format, loading, first-run download, migration).
//! - [`tui`]: the terminal UI (the default with no flags): browse nicks and posts,
//!   drive search, export, import, and maintenance from a menu bar.
//! - [`crypto`]: master secret, recovery phrase and shares, `acp-id`, encryption
//!   at rest, and encrypted backups.
//! - [`eggs`]: a few surprises for the curious (`docs/.secrets/EASTER_EGGS.md`).
//! - [`app`]: wires the above into the command-line program.

#![warn(missing_docs)]

#[macro_use]
pub mod logging;
pub mod notify;
pub mod app;
pub mod get;
pub mod prefs;
pub mod stored;
pub mod lists;
pub mod files;
pub mod branding;
pub mod brands;
pub mod bookmarks;
pub mod cli;
pub mod config;
pub mod crypto;
pub mod drive;
pub mod eggs;
pub mod export;
pub mod host_api;
pub mod import;
pub mod maintenance;
pub mod outbox;
pub mod plugins;
pub mod research;
pub mod rules;
pub mod service;
pub mod storage;
pub mod themes;
pub mod tui;

/// Everything most modules need, re-exported in one place: the crate's own
/// items (so domain modules can call each other without long paths) and the
/// external types used throughout.
pub mod prelude {
    pub use crate::app::*;
    pub use crate::prefs::*;
    pub use crate::stored::*;
    pub use crate::lists::*;
    pub use crate::files::{
        AttachmentKind, Completion, NickHint, PathCompletion, PlannedAttachment, complete_path,
        completions, expand_with_home, plan_attachment,
    };
    pub use crate::bookmarks::*;
    pub use crate::cli::*;
    pub use crate::config::*;
    pub use crate::export::{archive::*, html::*, json::*, *};
    pub use crate::import::*;
    pub use crate::logging::*;
    pub use crate::maintenance::*;
    pub use crate::research::*;
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
