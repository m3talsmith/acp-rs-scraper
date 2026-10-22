//! Records that cross the service boundary.
//!
//! No I/O. The host stores these. A service fills them and returns them.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// Where a settings field is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    /// `~/.acp/services/<id>.toml`.
    Config,
    /// Sealed in the encrypted database. Never written to a file and never printed.
    Secret,
}

/// The shape of one settings field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// A boolean.
    Bool,
    /// A single line.
    Text,
    /// An integer.
    Int,
    /// One of these labels.
    Enum(&'static [&'static str]),
    /// A list of strings.
    StringList,
    /// A secret. The value is never shown.
    Secret,
}

/// One field in a service schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    /// Toml key.
    pub key: &'static str,
    /// Label in the Services screen.
    pub label: &'static str,
    /// Help line.
    pub help: &'static str,
    /// Value shape.
    pub kind: FieldKind,
    /// Where it is stored.
    pub store: Store,
}

/// What the host knows when it asks whether a capability can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeCtx {
    /// The user has enabled this service.
    pub enabled: bool,
    /// A session or token is present.
    pub logged_in: bool,
    /// A cap that is blocking the service, when one is.
    pub limited: Option<&'static str>,
}

impl RuntimeCtx {
    /// Enabled, signed in, under the caps. Used when the host has not loaded a file.
    pub fn ready() -> Self {
        Self {
            enabled: true,
            logged_in: true,
            limited: None,
        }
    }
}

/// Whether a capability can run right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// Usable now.
    Ready,
    /// Implemented, and the user is not signed in.
    NeedsAuth,
    /// Implemented, and not sent until a capture is in the tree.
    Gated,
    /// Over a cap or paused.
    Limited,
    /// The user has not enabled the service, or the trait is absent.
    Off,
}

impl Availability {
    /// Short name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NeedsAuth => "needs auth",
            Self::Gated => "gated",
            Self::Limited => "limited",
            Self::Off => "off",
        }
    }
}

/// Failure from a capability call.
///
/// A missing trait is [`ServiceError::NotImplemented`]. A charge that does not
/// fit the run cap is [`ServiceError::BudgetExhausted`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceError {
    /// The service left this capability off `implemented()`.
    NotImplemented {
        /// Service id.
        service: String,
        /// Capability id.
        capability: String,
    },
    /// The charge does not fit the run cap. The budget is unchanged.
    BudgetExhausted,
    /// Implemented, and the user is not signed in. Nothing was sent.
    NeedsAuth,
    /// Implemented, and not sent until a capture is in the tree.
    Gated {
        /// Capability id.
        capability: String,
    },
    /// Any other refusal.
    Message(String),
}

impl ServiceError {
    /// The host called a capability the service does not implement.
    pub fn not_implemented(service: &str, capability: &str) -> Self {
        Self::NotImplemented {
            service: service.into(),
            capability: capability.into(),
        }
    }

    /// The write is implemented and waiting on a capture.
    pub fn gated(capability: &str) -> Self {
        Self::Gated {
            capability: capability.into(),
        }
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented {
                service,
                capability,
            } => {
                write!(f, "{service} does not implement {capability}")
            }
            Self::BudgetExhausted => write!(f, "budget exhausted"),
            Self::NeedsAuth => write!(f, "not logged in"),
            Self::Gated { capability } => write!(f, "{capability} is gated until captured"),
            Self::Message(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// A boxed capability future. Object-safe, so it can cross a plugin.
pub type CapFut<'a, T> = Pin<Box<dyn Future<Output = Result<T, ServiceError>> + Send + 'a>>;

/// A ready future that returns [`ServiceError::NotImplemented`].
pub fn not_implemented<'a, T: Send + 'a>(service: &str, capability: &str) -> CapFut<'a, T> {
    let err = ServiceError::not_implemented(service, capability);
    Box::pin(async move { Err(err) })
}

/// Stable service id. Built-ins are `redstar`, `x`, and `youtube`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ServiceId(pub &'static str);

impl ServiceId {
    /// `id` is the registrar key.
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// The text stored in `source` columns and plugin ids.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// A comment, transcript turn, or chat line inside a native id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubId {
    /// Kind such as `c`, `t`, or `chat`.
    pub kind: String,
    /// The service's own id for that part.
    pub native: String,
}

impl SubId {
    /// A sub-address the host stores and does not parse.
    pub fn new(kind: impl Into<String>, native: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            native: native.into(),
        }
    }
}

/// One item, account, or view on one service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceId {
    /// Which service minted `native`.
    pub service: ServiceId,
    /// The service's own id, without a prefix the host adds later.
    pub native: String,
    /// A part of `native`, when the row is a comment, a cue, or a chat line.
    pub sub: Option<SubId>,
}

impl SourceId {
    /// An id with no sub-address.
    pub fn new(service: ServiceId, native: impl Into<String>) -> Self {
        Self {
            service,
            native: native.into(),
            sub: None,
        }
    }

    /// An id that points at a comment, cue, or chat line.
    pub fn with_sub(service: ServiceId, native: impl Into<String>, sub: SubId) -> Self {
        Self {
            service,
            native: native.into(),
            sub: Some(sub),
        }
    }
}

/// Which login a service is using.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    /// The service's own session (cookies or a logged-in page).
    Session,
    /// The service's API key or bearer route.
    Api,
}

impl RouteKind {
    /// `session` or `api`, matching the `route` key in the service file.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Api => "api",
        }
    }
}

/// What the host asks a [`crate::Reader`](crate::Reader) to open.
///
/// A service that cannot express one of these does not advertise it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A nick, handle, or channel.
    Account {
        /// The handle as the user typed it, without a service prefix.
        handle: String,
    },
    /// A remote search on that service. Local search is not this target.
    Search {
        /// The query string.
        query: String,
    },
    /// A playlist or a list the service names as one.
    Playlist {
        /// The service's playlist id.
        id: String,
    },
    /// One post, video, or other item.
    Item {
        /// The item.
        id: SourceId,
    },
    /// A thread or a conversation.
    Conversation {
        /// The thread root.
        id: SourceId,
    },
}

/// Opaque resume token. The host stores the text and does not parse it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Cursor(pub String);

impl Cursor {
    /// A cursor whose text is `text`.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The token as stored.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// No resume token.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One charge against a run cap. Money is integer micros, not a float.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Spend {
    /// Items this charge accounts for.
    pub items: u64,
    /// Millionths of the service's currency unit.
    pub micros: i64,
}

impl Spend {
    /// A charge of `items` and `micros`.
    pub fn new(items: u64, micros: i64) -> Self {
        Self { items, micros }
    }
}

/// Caps for one run. `None` means that cap is not set.
///
/// The service calls [`Budget::try_spend`] before a request and returns
/// [`crate::ServiceError::BudgetExhausted`] when the charge does not fit.
/// A refused charge leaves the budget unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Items still allowed in this run.
    pub remaining_items: Option<u64>,
    /// Spend still allowed in this run, in micros.
    pub remaining_micros: Option<i64>,
}

impl Budget {
    /// No count cap and no spend cap.
    pub fn unlimited() -> Self {
        Self {
            remaining_items: None,
            remaining_micros: None,
        }
    }

    /// Both caps set.
    pub fn capped(items: u64, micros: i64) -> Self {
        Self {
            remaining_items: Some(items),
            remaining_micros: Some(micros),
        }
    }

    /// Subtract `spend` when both caps allow it.
    pub fn try_spend(&mut self, spend: Spend) -> Result<(), ServiceError> {
        if let Some(left) = self.remaining_items {
            if spend.items > left {
                return Err(ServiceError::BudgetExhausted);
            }
        }
        if let Some(left) = self.remaining_micros {
            if spend.micros > left {
                return Err(ServiceError::BudgetExhausted);
            }
        }
        if let Some(left) = self.remaining_items.as_mut() {
            *left -= spend.items;
        }
        if let Some(left) = self.remaining_micros.as_mut() {
            *left -= spend.micros;
        }
        Ok(())
    }
}

/// A resolved media URL on an item. Bytes are a separate fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaRef {
    /// URL the service already resolved.
    pub url: String,
    /// `image`, `video`, `gif`, `thumbnail`, `subtitle`, or `audio`.
    pub kind: String,
    /// MIME type when the service knows it.
    pub mime: String,
}

impl MediaRef {
    /// A reference with no MIME type yet.
    pub fn new(url: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            kind: kind.into(),
            mime: String::new(),
        }
    }
}

/// An account page's identity. The host maps this into its profile row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRec {
    /// Stable account id on this service.
    pub source_id: SourceId,
    /// Handle, without a service prefix.
    pub nick: String,
    /// Display name. Empty when the service has none.
    pub display_name: String,
    /// Bio text. Empty when the service has none.
    pub bio: String,
    /// Profile URL. Empty when the service has none.
    pub url: String,
    /// Avatar URL. Empty when the service has none.
    pub avatar_url: String,
    /// Location text. Empty when the service has none.
    pub location: String,
    /// Join time, as the service reported it.
    pub joined_at: Option<String>,
    /// Follower or subscriber count.
    pub followers: Option<i64>,
    /// Following count.
    pub following: Option<i64>,
    /// Posts or videos the profile shows.
    pub posts_count: Option<i64>,
}

impl AccountRec {
    /// An account with empty optional text.
    pub fn new(source_id: SourceId, nick: impl Into<String>) -> Self {
        Self {
            source_id,
            nick: nick.into(),
            display_name: String::new(),
            bio: String::new(),
            url: String::new(),
            avatar_url: String::new(),
            location: String::new(),
            joined_at: None,
            followers: None,
            following: None,
            posts_count: None,
        }
    }
}

/// One post, reply, video, comment, or other item.
///
/// `body_json` is the service's own markup. The host stores it and does not interpret it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemRec {
    /// Stable item id on this service.
    pub source_id: SourceId,
    /// `post`, `reply`, `video`, `comment`, `segment`, or another kind the service names.
    pub kind: String,
    /// Author handle, without a service prefix.
    pub nick: String,
    /// Title. Empty for a reply or a service that has no title.
    pub title: String,
    /// Plain text.
    pub body: String,
    /// Service markup. Empty when the body is plain text only.
    pub body_json: String,
    /// When the item was published, as the service reported it.
    pub posted_at: String,
    /// When the item was edited. A `~` prefix is a placeholder, not a time.
    pub edited_at: Option<String>,
    /// Permalink. Empty when the service has none.
    pub url: String,
    /// Thread root, when this item is not the root.
    pub parent: Option<SourceId>,
    /// The item this one answers, when that is not the thread root.
    pub in_reply_to: Option<SourceId>,
    /// Language tag. Empty when the service has none.
    pub lang: String,
    /// Speaker, for a transcript. Empty otherwise.
    pub speaker: String,
    /// Start offset in milliseconds.
    pub start_ms: Option<i64>,
    /// End offset in milliseconds.
    pub end_ms: Option<i64>,
    /// Service metrics as JSON text. Empty when there are none.
    pub metrics_json: String,
    /// How this row was fetched (`liveview`, `x-web`, `yt-dlp`, and so on).
    pub fetched_via: String,
    /// The body is a shortened preview.
    pub truncated: bool,
    /// A score the service sent.
    pub score: Option<i64>,
    /// Media the item points at. The bytes are not in this record.
    pub media: Vec<MediaRef>,
}

impl ItemRec {
    /// An item with empty optional fields.
    pub fn new(source_id: SourceId, body: impl Into<String>) -> Self {
        Self {
            source_id,
            kind: String::new(),
            nick: String::new(),
            title: String::new(),
            body: body.into(),
            body_json: String::new(),
            posted_at: String::new(),
            edited_at: None,
            url: String::new(),
            parent: None,
            in_reply_to: None,
            lang: String::new(),
            speaker: String::new(),
            start_ms: None,
            end_ms: None,
            metrics_json: String::new(),
            fetched_via: String::new(),
            truncated: false,
            score: None,
            media: Vec::new(),
        }
    }
}

/// One transcript cue. The host may also store it as an [`ItemRec`] of kind `segment`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentRec {
    /// This cue's id.
    pub source_id: SourceId,
    /// The video or item the cue belongs to.
    pub parent: SourceId,
    /// Cue text.
    pub body: String,
    /// Speaker. Empty until the service knows one.
    pub speaker: String,
    /// Start offset in milliseconds.
    pub start_ms: i64,
    /// End offset in milliseconds.
    pub end_ms: i64,
}

/// One page from a read, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Page {
    /// Accounts discovered on this page.
    pub accounts: Vec<AccountRec>,
    /// Items on this page.
    pub items: Vec<ItemRec>,
    /// Transcript cues on this page.
    pub segments: Vec<SegmentRec>,
    /// Cursor for the next page. `None` means this page is the last.
    pub next: Option<Cursor>,
    /// What this page cost.
    pub cost: Spend,
}

impl Page {
    /// A last page with nothing on it.
    pub fn empty() -> Self {
        Self::default()
    }
}

/// A [`Target`] the service has turned into a stable id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The request the host made.
    pub target: Target,
    /// The id to pass to later reads.
    pub source_id: SourceId,
}

/// What [`crate::Reader::expand`](crate::Reader::expand) should add to an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expand {
    /// Replies to the item.
    Replies,
    /// The whole thread.
    Thread,
    /// Media attached to the item.
    Media,
    /// Transcript cues.
    Transcript,
    /// Comments under the item.
    Comments,
}

/// The settings a service declares. `fields` is the same slice as [`crate::Service::schema`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigSchema {
    /// Service these fields belong to.
    pub service: ServiceId,
    /// Fields, in the order the Services screen shows them.
    pub fields: &'static [Field],
}

/// Values the user typed, keyed by schema field. The host owns the strings.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FilledFields {
    /// `(field key, value)` pairs.
    pub values: Vec<(String, String)>,
}

impl FilledFields {
    /// No values.
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }

    /// Set `key` to `value`.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        if let Some(slot) = self.values.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value.into();
        } else {
            self.values.push((key, value.into()));
        }
    }

    /// The value for `key`, when the user set one.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Secret bytes the core seals. The service does not write the cookie file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedMaterial {
    /// Schema field this secret belongs to.
    pub field: String,
    /// Raw bytes. The core seals them under `source_secrets`.
    pub material: Vec<u8>,
}

impl SealedMaterial {
    /// A secret for `field`.
    pub fn new(field: impl Into<String>, material: impl Into<Vec<u8>>) -> Self {
        Self {
            field: field.into(),
            material: material.into(),
        }
    }
}

/// A local draft the core already has. The service does not read the user's files for this.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Draft {
    /// Body text.
    pub body: String,
    /// Attachments the core has already loaded.
    pub attachments: Vec<DraftAttachment>,
    /// The item this draft answers, when it is a reply.
    pub in_reply_to: Option<SourceId>,
}

impl Draft {
    /// A text draft with no attachments.
    pub fn text(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            attachments: Vec::new(),
            in_reply_to: None,
        }
    }
}

/// One attachment on a [`Draft`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftAttachment {
    /// File name the user sees.
    pub name: String,
    /// MIME type.
    pub mime: String,
    /// Bytes the core already read.
    pub bytes: Vec<u8>,
}

/// The service's id for a write, or a reason the write did not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    /// The service accepted the write.
    NativeId(SourceId),
    /// The write did not run. Nothing was sent.
    NotReady(Availability),
}

/// A notice the core keeps. The service does not own the notice table.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct NoticeRec {
    /// Notice kind.
    pub kind: String,
    /// Short title.
    pub title: String,
    /// Body text.
    pub body: String,
    /// The item this notice is about, when there is one.
    pub source_id: Option<SourceId>,
}

/// A feed rule the host asks a service to refresh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedRule {
    /// Host id for the rule.
    pub id: String,
    /// The rule's query.
    pub query: String,
}

impl FeedRule {
    /// A rule with `id` and `query`.
    pub fn new(id: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            query: query.into(),
        }
    }
}

/// An item the user wants in a research project.
///
/// The core's research store is the local implementation. A service does not own that table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchRef {
    /// The item.
    pub source_id: SourceId,
    /// Project name.
    pub project: String,
}

/// A URL the service already resolved. [`crate::MediaFetcher`] returns the bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaRequest {
    /// The URL.
    pub url: String,
    /// The item the URL belongs to.
    pub source_id: SourceId,
}

/// Bytes the core stores under its attachment path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaBytes {
    /// MIME type.
    pub mime: String,
    /// File bytes.
    pub bytes: Vec<u8>,
    /// What the fetch cost.
    pub cost: Spend,
}

/// Up, down, or clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoteChoice {
    /// A positive vote.
    Up,
    /// A negative vote.
    Down,
    /// Remove the vote.
    Clear,
}
