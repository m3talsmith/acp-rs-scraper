//! Calls a service makes into the core.
//!
//! This crate does not open a database, and it does not depend on the node
//! crate. The core implements [`Host`]. The host passes that value to
//! [`crate::DynService::attach`] before the service is stored. A service keeps
//! the [`std::sync::Arc`] and calls it. HTTP, configuration, storage, sealed
//! secrets, bookmarks, the outbox, export, import, rules, research,
//! maintenance, notices, preferences, and the binary name stay in the core.
//!
//! [`Host`] methods are synchronous. They block the caller until the core
//! finishes. HTTP runs on a worker thread with its own runtime, so a plugin
//! may call them from a thread or runtime it owns.
//!
//! A websocket upgrade is out of scope. The service keeps its own LiveView
//! socket. [`Host::http_get`] and [`Host::http_post`] return an HTTP response.

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

/// Bytes and headers from one host HTTP call.
///
/// `url` is the final URL after redirects. `headers` are that response's
/// headers. Each `Location` the client followed is included as the resolved
/// URL. `set_cookie_names` lists cookie names from `Set-Cookie` on every hop.
/// Cookie values are not logged. [`Debug`] redacts `Set-Cookie` and `Cookie`.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code of the final response.
    pub status: u16,
    /// Response body.
    pub body: Vec<u8>,
    /// `Content-Type`, or empty when the response had none.
    pub content_type: String,
    /// Final URL after redirects.
    pub url: String,
    /// Final response headers, plus each followed `Location`.
    pub headers: Vec<HttpHeader>,
    /// Cookie names set on any hop in this call. Values stay in the jar.
    pub set_cookie_names: Vec<String>,
}

impl std::fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let headers: Vec<HttpHeader> = self
            .headers
            .iter()
            .map(|header| {
                if header.name.eq_ignore_ascii_case("set-cookie")
                    || header.name.eq_ignore_ascii_case("cookie")
                {
                    HttpHeader::new(header.name.clone(), "[redacted]")
                } else {
                    header.clone()
                }
            })
            .collect();
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("body", &self.body)
            .field("content_type", &self.content_type)
            .field("url", &self.url)
            .field("headers", &headers)
            .field("set_cookie_names", &self.set_cookie_names)
            .finish()
    }
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
///
/// [`Debug`] prints the name and URL. The value is `[redacted]`.
#[derive(Clone, PartialEq, Eq)]
pub struct CookieRec {
    /// Cookie name.
    pub name: String,
    /// Cookie value. Callers read this field. Logs and [`Debug`] do not.
    pub value: String,
    /// URL the cookie was stored against.
    pub url: String,
}

impl std::fmt::Debug for CookieRec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CookieRec")
            .field("name", &self.name)
            .field("value", &"[redacted]")
            .field("url", &self.url)
            .finish()
    }
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

/// What [`Host::export`] writes.
///
/// `post_ids` empty means every stored post. `compress` writes a gzip-compressed
/// tar that holds the rendered file, attachment files, and profile images when
/// `profile_images` is set. A service does not ship its own exporter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportRequest {
    /// JSON or HTML.
    pub format: ExportFormat,
    /// Write a `.tar.gz` archive instead of one rendered file.
    pub compress: bool,
    /// Include stored profile images in the document and, when compressed, in the archive.
    pub profile_images: bool,
    /// Post ids to export. Empty exports every stored post.
    pub post_ids: Vec<String>,
}

impl ExportRequest {
    /// One rendered file, every stored post, no profile images.
    pub fn new(format: ExportFormat) -> Self {
        Self {
            format,
            compress: false,
            profile_images: false,
            post_ids: Vec::new(),
        }
    }
}

/// A bookmark the service saw on the site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteBookmark {
    /// The site's id for this bookmark. It may differ from the post id.
    pub site_id: String,
    /// Post id in the host store. Empty when the service has not resolved one.
    pub post_id: String,
}

impl SiteBookmark {
    /// A site bookmark for `post_id` whose site id is `site_id`.
    pub fn new(site_id: impl Into<String>, post_id: impl Into<String>) -> Self {
        Self {
            site_id: site_id.into(),
            post_id: post_id.into(),
        }
    }
}

/// One bookmark the service still has to push.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkChange {
    /// Site bookmark id. Empty when the host has not seen one.
    pub site_id: String,
    /// Post id in the host store.
    pub post_id: String,
}

/// The host's half of a bookmark sync.
///
/// The service reads the site, then calls [`Host::sync_bookmarks`]. `push_add`
/// and `push_remove` are what the service still has to send. After each push
/// it calls [`Host::bookmark_pushed`].
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct BookmarkSync {
    /// Local bookmarks the site does not have.
    pub push_add: Vec<BookmarkChange>,
    /// Local removals the site still has.
    pub push_remove: Vec<BookmarkChange>,
    /// Rows the host inserted from the site list.
    pub pulled: u64,
    /// Rows the host dropped because the site list no longer has them.
    pub dropped: u64,
}

/// The core, as a service is allowed to see it.
///
/// The service does not receive a SQLCipher connection, the database key, or
/// the root crate. Paths are text. Secrets are opaque bytes the core stores.
///
/// Every method is synchronous and blocks until the core finishes. A plugin
/// may call them from a thread or runtime it owns. HTTP does not use that
/// runtime. A websocket upgrade is not part of this trait. The service keeps
/// its own LiveView socket.
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
    /// Save a local draft. `keep` stores it as a local post that is not queued.
    fn save_outbox(
        &self,
        id: &str,
        title: &str,
        body: &str,
        thread: &str,
        keep: bool,
    ) -> Result<String, HostError>;
    /// Queue the local post `local_id`.
    fn enqueue_outbox(&self, local_id: &str) -> Result<String, HostError>;
    /// Cancel a queued or draft outbox row. A sent row stays sent.
    fn cancel_outbox(&self, id: &str) -> Result<String, HostError>;

    /// Write stored posts to `path`.
    ///
    /// `request.compress` writes a gzip-compressed tar. `request.profile_images`
    /// includes profile images. `request.post_ids` limits the posts. Empty means
    /// every stored post.
    fn export(&self, path: &str, request: &ExportRequest) -> Result<(), HostError>;
    /// Reconcile `site` with the host bookmark store for `service`.
    ///
    /// The host keeps the site-id to post-id mapping. `complete` means `site` is
    /// the whole list, so a remote bookmark missing from it was removed on the site.
    fn sync_bookmarks(
        &self,
        service: &str,
        site: &[SiteBookmark],
        complete: bool,
    ) -> Result<BookmarkSync, HostError>;
    /// The service finished one push from [`Host::sync_bookmarks`].
    ///
    /// `added` true marks the bookmark remote and stores `site_id`. `added` false
    /// drops the local row.
    fn bookmark_pushed(
        &self,
        service: &str,
        post_id: &str,
        site_id: &str,
        added: bool,
    ) -> Result<(), HostError>;
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
    /// [`HttpResponse::url`] is the final URL after redirects. The call blocks.
    /// It is safe to make from a plugin-owned thread or runtime.
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
    /// Store `name`=`value` in `service`'s jar for `url`.
    ///
    /// That jar is the one source of truth. A cookie a response already stored
    /// stays. The same name and URL replace the value. Every other cookie stays.
    fn set_cookie(&self, service: &str, url: &str, name: &str, value: &str) -> Result<(), HostError>;
    /// Cookies in `service`'s jar, including ones a response `Set-Cookie` stored.
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
