//! Calls a service makes into the core.
//!
//! This crate does not open a database, and it does not depend on the node
//! crate. The core implements [`Host`]. The host passes that value to
//! [`crate::DynService::attach`] before the service is stored. A service keeps
//! the [`std::sync::Arc`] and calls it. HTTP, configuration, storage, sealed
//! secrets, bookmarks, the outbox, export, import, rules, research,
//! maintenance, notices, preferences, and the binary name stay in the core.

use std::sync::{Arc, Mutex};

use crate::records::{NoticeRec, Page, SourceId};

/// A host call failed. The message never includes secret bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostError {
    /// What failed.
    pub message: String,
}

impl HostError {
    /// An error whose text is `message`.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HostError {}

/// Bytes from one host HTTP call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body.
    pub body: Vec<u8>,
    /// `Content-Type`, or empty when the response had none.
    pub content_type: String,
}

/// One extra request header. Cookies go through [`Host::set_cookie`], not this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpHeader {
    /// Header name.
    pub name: String,
    /// Header value.
    pub value: String,
}

impl HttpHeader {
    /// A header named `name`.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// One cookie in a service's jar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CookieRec {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// URL the cookie was stored against.
    pub url: String,
}

/// Post ids the core already has, plus stored reply counts per thread root.
///
/// A trait read uses `reply_counts` the way the headless path does: a thread
/// whose stored reply count still matches the feed can be skipped.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct KnownIds {
    /// Post ids with a body that is not `truncated`.
    pub ids: Vec<String>,
    /// Stored replies for each thread root.
    pub reply_counts: Vec<ThreadReplies>,
}

/// How many replies the core has stored under one thread root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadReplies {
    /// Thread root id.
    pub thread_id: String,
    /// Rows whose `parent_post_id` is `thread_id`.
    pub replies: u64,
}

/// JSON or HTML. The host renders both. A service does not ship its own exporter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    /// Pretty JSON.
    Json,
    /// A self-contained HTML page.
    Html,
}

impl ExportFormat {
    /// `json` or `html`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Html => "html",
        }
    }
}

/// The core, as a service is allowed to see it.
///
/// The service does not receive a SQLCipher connection, the database key, or
/// the root crate. Paths are text. Secrets are opaque bytes the core stores.
pub trait Host: Send + Sync {
    /// Directory that holds `~/.acp` for this process, or the test root.
    fn home(&self) -> Result<String, HostError>;
    /// Database file the core opens. The service does not open it.
    fn database_path(&self) -> Result<String, HostError>;
    /// Site origin, without a trailing slash.
    fn base_url(&self) -> Result<String, HostError>;
    /// Saved login email. Empty when the preferences file has none.
    fn email(&self) -> Result<String, HostError>;

    /// Store `page` in the core database.
    ///
    /// Each item's [`crate::BodyState`] is written to `posts.body_state`.
    /// Attachment rows are written to `attachments`. Returns how many item and
    /// segment rows were written.
    fn save_page(&self, page: &Page) -> Result<usize, HostError>;
    /// One stored item, when `id`'s native value is a post id in the core database.
    fn load_item(&self, id: &SourceId) -> Result<Option<crate::records::ItemRec>, HostError>;
    /// Known post ids and the stored reply count of each thread root.
    fn known_ids(&self) -> Result<KnownIds, HostError>;

    /// Seal `material` under `field` in the encrypted database. The service does not see the key.
    fn store_secret(&self, field: &str, material: &[u8]) -> Result<(), HostError>;
    /// The bytes stored for `field`, when there are any.
    fn load_secret(&self, field: &str) -> Result<Option<Vec<u8>>, HostError>;

    /// Bookmark ids that are not pending removal.
    fn list_bookmarks(&self) -> Result<Vec<String>, HostError>;
    /// Bookmark `id` locally. Site sync stays the service's job.
    fn add_bookmark(&self, id: &str) -> Result<(), HostError>;
    /// Remove `id` locally.
    fn remove_bookmark(&self, id: &str) -> Result<(), HostError>;

    /// Outbox lines, newest first. An empty queue is one line, `outbox none`.
    fn list_outbox(&self) -> Result<Vec<String>, HostError>;

    /// Write stored posts to `path` as JSON or HTML.
    fn export(&self, path: &str, format: ExportFormat) -> Result<(), HostError>;
    /// Read a JSON export at `path` and store its posts. Returns how many posts were written.
    fn import_json(&self, path: &str) -> Result<usize, HostError>;

    /// Preview enabled rules against stored posts. One line per firing.
    fn preview_rules(&self) -> Result<String, HostError>;

    /// Add `item_id` to research project `project`, creating the project when it is new.
    fn add_research(&self, project: &str, item_id: &str, note: &str) -> Result<(), HostError>;

    /// Remove `nick` from the local database. `commit` false is a preview.
    fn remove_nick(&self, nick: &str, keep_replies: bool, commit: bool) -> Result<String, HostError>;

    /// Store `notice` and return its row id.
    fn push_notice(&self, notice: &NoticeRec) -> Result<i64, HostError>;

    /// One preference. `email`, `theme`, and `brand` are the file. `base_url` is the process origin.
    fn pref(&self, key: &str) -> Result<Option<String>, HostError>;
    /// Set `email`, `theme`, or `brand`. `base_url` is the `ACP_BASE_URL` environment variable.
    fn set_pref(&self, key: &str, value: &str) -> Result<(), HostError>;

    /// The core binary's name. This does not spawn a process.
    fn argv0(&self) -> String;

    /// GET `url` with the core's HTTP client.
    ///
    /// `service` selects that service's cookie jar. `headers` are extra request
    /// headers. A `Cookie` header is refused: cookies go through [`Host::set_cookie`].
    fn http_get(
        &self,
        service: &str,
        url: &str,
        headers: &[HttpHeader],
    ) -> Result<HttpResponse, HostError>;
    /// POST `body` to `url` with the core's HTTP client. Same jar and header rules as [`Host::http_get`].
    fn http_post(
        &self,
        service: &str,
        url: &str,
        headers: &[HttpHeader],
        body: &[u8],
        content_type: &str,
    ) -> Result<HttpResponse, HostError>;
    /// Store `name`=`value` in `service`'s jar for `url`. A later call with the same name replaces the value.
    fn set_cookie(&self, service: &str, url: &str, name: &str, value: &str) -> Result<(), HostError>;
    /// Cookies stored for `service`, across every URL.
    fn cookies(&self, service: &str) -> Result<Vec<CookieRec>, HostError>;
    /// Drop every cookie stored for `service`.
    fn clear_cookies(&self, service: &str) -> Result<(), HostError>;
}

/// Where a service keeps the host from [`crate::Service::attach`].
#[derive(Default)]
pub struct HostSlot {
    inner: Mutex<Option<Arc<dyn Host>>>,
}

impl HostSlot {
    /// No host yet.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// Remember `host`. A later call replaces it.
    pub fn store(&self, host: Arc<dyn Host>) {
        *self.inner.lock().unwrap_or_else(|e| e.into_inner()) = Some(host);
    }

    /// The host from [`Self::store`].
    pub fn get(&self) -> Result<Arc<dyn Host>, HostError> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| HostError::new("the host has not attached"))
    }
}
