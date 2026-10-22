//! Service traits and the plugin ABI.
//!
//! No I/O. The host opens the database, draws the terminal, and loads plugins.
//! A plugin built with another compiler, or against another version of this
//! crate, is refused before `register`.

use crate::records::{
    AccountRec, Availability, Budget, CapFut, Cursor, Draft, Expand, FeedRule, Field, FilledFields,
    ItemRec, MediaBytes, MediaRequest, NoticeRec, Page, ResearchRef, Resolved, RouteKind,
    RuntimeCtx, SealedMaterial, ServiceError, SourceId, Spend, Target, VoteChoice, WriteOutcome,
    not_implemented,
};

/// `rustc --version` this crate was built with. Plugin authors read it from `--doctor`.
pub const RUSTC_VERSION: &str = env!("ACP_RUSTC_VERSION");

/// Version of this crate. A plugin's `core_version` must equal it.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// One capability a service may implement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityId {
    /// Session or API login.
    Authenticator,
    /// Incremental read of accounts, searches, and threads.
    Reader,
    /// Updates for the open view.
    Live,
    /// Notices, rules, alerts, and local toasts.
    Notification,
    /// A new post.
    Post,
    /// A reply.
    Reply,
    /// An attachment upload.
    Upload,
    /// A vote or a like.
    Vote,
    /// A subscribe or a follow.
    Subscribe,
    /// An edit.
    Edit,
    /// A delete.
    Delete,
    /// A repost or a quote.
    Repost,
    /// Add the current item to a research project.
    Research,
    /// Refresh a feed rule.
    Feeds,
    /// Video, audio, transcripts, thumbnails, and attachments.
    MediaFetcher,
    /// Spend estimate and caps. Required on a paid API route.
    CostMeter,
    /// Red Star Spaces. This is not an X Spaces read.
    Spaces,
    /// Local drafts and the send queue.
    Outbox,
    /// A post stored on this machine before it is sent.
    LocalPost,
    /// Replace the signed-in account's profile.
    UpdateProfile,
    /// Report one post.
    ReportPost,
    /// Report one account.
    ReportUser,
    /// Bookmark or unbookmark one post.
    Bookmark,
}

impl CapabilityId {
    /// Stable id used in help and logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authenticator => "authenticator",
            Self::Reader => "reader",
            Self::Live => "live",
            Self::Notification => "notification",
            Self::Post => "post",
            Self::Reply => "reply",
            Self::Upload => "upload",
            Self::Vote => "vote",
            Self::Subscribe => "subscribe",
            Self::Edit => "edit",
            Self::Delete => "delete",
            Self::Repost => "repost",
            Self::Research => "research",
            Self::Feeds => "feeds",
            Self::MediaFetcher => "media",
            Self::CostMeter => "cost",
            Self::Spaces => "spaces",
            Self::Outbox => "outbox",
            Self::LocalPost => "local_post",
            Self::UpdateProfile => "update_profile",
            Self::ReportPost => "report_post",
            Self::ReportUser => "report_user",
            Self::Bookmark => "bookmark",
        }
    }

    /// Every id, in a fixed order.
    pub fn all() -> &'static [CapabilityId] {
        &ALL
    }
}

const ALL: [CapabilityId; 23] = [
    CapabilityId::Authenticator,
    CapabilityId::Reader,
    CapabilityId::Live,
    CapabilityId::Notification,
    CapabilityId::Post,
    CapabilityId::Reply,
    CapabilityId::Upload,
    CapabilityId::Vote,
    CapabilityId::Subscribe,
    CapabilityId::Edit,
    CapabilityId::Delete,
    CapabilityId::Repost,
    CapabilityId::Research,
    CapabilityId::Feeds,
    CapabilityId::MediaFetcher,
    CapabilityId::CostMeter,
    CapabilityId::Spaces,
    CapabilityId::Outbox,
    CapabilityId::LocalPost,
    CapabilityId::UpdateProfile,
    CapabilityId::ReportPost,
    CapabilityId::ReportUser,
    CapabilityId::Bookmark,
];

/// Object-safe service. Async calls return a boxed future so the value can cross a plugin.
///
/// Override a method to provide that capability. The default returns
/// [`ServiceError::NotImplemented`]. The host does not call a capability the
/// service left off [`DynService::implemented`]; a call that arrives anyway
/// gets that error.
///
/// An out-of-tree crate can implement this trait directly. It can also
/// implement [`crate::Service`] and the capability traits. That path gets this
/// trait from a blanket impl. Do not implement both.
pub trait DynService: Send + Sync {
    /// Stable id: `redstar`, `x`, `youtube`, or a plugin id.
    fn id(&self) -> &'static str;
    /// Name shown in the Services screen.
    fn display_name(&self) -> &'static str;
    /// Traits this crate implements. Fixed for the life of the binary.
    fn implemented(&self) -> &'static [CapabilityId];
    /// Settings fields. `Config` goes to toml. `Secret` is sealed.
    fn schema(&self) -> &'static [Field];
    /// Logged-in, gated, over cap, and so on.
    fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability;

    /// This crate implements `cap`.
    fn has(&self, cap: CapabilityId) -> bool {
        self.implemented().contains(&cap)
    }

    /// Probe `cap` against a real target, draft, or page.
    ///
    /// A capability left off `implemented()` returns not-implemented. A
    /// capability that is implemented still refuses an empty subject or the
    /// wrong kind. The check is [`probe_subject`].
    fn probe<'a>(&'a self, cap: CapabilityId, subject: Probe<'a>) -> CapFut<'a, ()> {
        let result = if self.has(cap) {
            probe_subject(cap, subject)
        } else {
            Err(ServiceError::not_implemented(self.id(), cap.as_str()))
        };
        Box::pin(async move { result })
    }

    /// The host calls this after the package gate, before the service is stored.
    ///
    /// The default does nothing. A service that calls [`crate::Host`] stores
    /// the value here.
    fn attach(&mut self, _host: std::sync::Arc<dyn crate::host::Host>) {}

    /// Red Star Spaces for `target`.
    fn spaces<'a>(
        &'a self,
        _target: &'a Target,
        _cursor: &'a Cursor,
        _budget: &'a mut Budget,
    ) -> CapFut<'a, Page> {
        not_implemented(self.id(), CapabilityId::Spaces.as_str())
    }

    /// [`schema`](DynService::schema) plus the service id.
    fn config_schema(&self) -> crate::records::ConfigSchema {
        crate::records::ConfigSchema {
            service: crate::records::ServiceId::new(self.id()),
            fields: self.schema(),
        }
    }

    /// Exchange filled schema fields for secret material the core seals.
    fn authenticate<'a>(
        &'a self,
        _route: RouteKind,
        _fields: &'a FilledFields,
    ) -> CapFut<'a, SealedMaterial> {
        not_implemented(self.id(), CapabilityId::Authenticator.as_str())
    }

    /// Turn `target` into a stable id.
    fn resolve<'a>(&'a self, _target: &'a Target) -> CapFut<'a, Resolved> {
        not_implemented(self.id(), CapabilityId::Reader.as_str())
    }

    /// One page, newest first. `stop_at` ends an incremental sync.
    fn page<'a>(
        &'a self,
        _resolved: &'a Resolved,
        _cursor: &'a Cursor,
        _stop_at: Option<&'a str>,
        _budget: &'a mut Budget,
    ) -> CapFut<'a, Page> {
        not_implemented(self.id(), CapabilityId::Reader.as_str())
    }

    /// More of one item.
    fn expand<'a>(
        &'a self,
        _item: &'a SourceId,
        _what: Expand,
        _budget: &'a mut Budget,
    ) -> CapFut<'a, Page> {
        not_implemented(self.id(), CapabilityId::Reader.as_str())
    }

    /// Incremental items for the open view.
    fn live<'a>(&'a self, _view: &'a SourceId) -> CapFut<'a, Vec<ItemRec>> {
        not_implemented(self.id(), CapabilityId::Live.as_str())
    }

    /// A notice the core stores.
    fn notify<'a>(&'a self, _event: &'a NoticeRec) -> CapFut<'a, NoticeRec> {
        not_implemented(self.id(), CapabilityId::Notification.as_str())
    }

    /// Publish a new post.
    fn publish<'a>(&'a self, _draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.id(), CapabilityId::Post.as_str())
    }

    /// Publish a reply.
    fn reply<'a>(&'a self, _draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.id(), CapabilityId::Reply.as_str())
    }

    /// Upload attachments the core has already loaded.
    fn upload<'a>(&'a self, _draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.id(), CapabilityId::Upload.as_str())
    }

    /// Vote on `item`.
    fn vote<'a>(&'a self, _item: &'a SourceId, _choice: VoteChoice) -> CapFut<'a, ()> {
        not_implemented(self.id(), CapabilityId::Vote.as_str())
    }

    /// Follow `account` when `enabled` is set.
    fn subscribe<'a>(&'a self, _account: &'a SourceId, _enabled: bool) -> CapFut<'a, ()> {
        not_implemented(self.id(), CapabilityId::Subscribe.as_str())
    }

    /// Replace the body of `item`.
    fn edit<'a>(&'a self, _item: &'a SourceId, _body: &'a str) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.id(), CapabilityId::Edit.as_str())
    }

    /// Delete `item`.
    fn delete<'a>(&'a self, _item: &'a SourceId) -> CapFut<'a, ()> {
        not_implemented(self.id(), CapabilityId::Delete.as_str())
    }

    /// Repost `item`. The service may refuse an empty `quote`.
    fn repost<'a>(
        &'a self,
        _item: &'a SourceId,
        _quote: Option<&'a str>,
    ) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.id(), CapabilityId::Repost.as_str())
    }

    /// Record `item` against a research project. The core owns that table.
    fn add_research<'a>(&'a self, _item: &'a ResearchRef) -> CapFut<'a, ()> {
        not_implemented(self.id(), CapabilityId::Research.as_str())
    }

    /// Rows for a feed rule. The core inserts them.
    fn refresh<'a>(&'a self, _rule: &'a FeedRule, _budget: &'a mut Budget) -> CapFut<'a, Page> {
        not_implemented(self.id(), CapabilityId::Feeds.as_str())
    }

    /// Bytes for a URL the service already resolved.
    fn fetch<'a>(
        &'a self,
        _req: &'a MediaRequest,
        _budget: &'a mut Budget,
    ) -> CapFut<'a, MediaBytes> {
        not_implemented(self.id(), CapabilityId::MediaFetcher.as_str())
    }

    /// The estimate for what `budget` still allows.
    fn estimate<'a>(&'a self, _budget: &'a Budget) -> CapFut<'a, Spend> {
        not_implemented(self.id(), CapabilityId::CostMeter.as_str())
    }

    /// Save `draft` under `id`. `keep` true leaves it off the send queue.
    fn save_local<'a>(&'a self, _id: &'a str, _draft: &'a Draft, _keep: bool) -> CapFut<'a, String> {
        not_implemented(self.id(), CapabilityId::LocalPost.as_str())
    }

    /// Drafts and keep-local posts, newest update first.
    fn list_local<'a>(&'a self) -> CapFut<'a, Vec<String>> {
        not_implemented(self.id(), CapabilityId::LocalPost.as_str())
    }

    /// Queue a saved local post.
    fn enqueue<'a>(&'a self, _local_id: &'a str) -> CapFut<'a, String> {
        not_implemented(self.id(), CapabilityId::Outbox.as_str())
    }

    /// Queue rows, newest first.
    fn queued<'a>(&'a self) -> CapFut<'a, Vec<String>> {
        not_implemented(self.id(), CapabilityId::Outbox.as_str())
    }

    /// Cancel a queued item.
    fn cancel_queued<'a>(&'a self, _id: &'a str) -> CapFut<'a, String> {
        not_implemented(self.id(), CapabilityId::Outbox.as_str())
    }

    /// Replace the profile in `account`. The host stores the row the service returns.
    fn update_profile<'a>(&'a self, _account: &'a AccountRec) -> CapFut<'a, AccountRec> {
        not_implemented(self.id(), CapabilityId::UpdateProfile.as_str())
    }

    /// Report `item`. `reason` is the text the user confirmed.
    fn report_post<'a>(&'a self, _item: &'a SourceId, _reason: &'a str) -> CapFut<'a, ()> {
        not_implemented(self.id(), CapabilityId::ReportPost.as_str())
    }

    /// Report `account`. `reason` is the text the user confirmed.
    fn report_user<'a>(&'a self, _account: &'a SourceId, _reason: &'a str) -> CapFut<'a, ()> {
        not_implemented(self.id(), CapabilityId::ReportUser.as_str())
    }

    /// Bookmark `item` when `on` is set. The host records the mark through
    /// [`crate::Host::bookmark_post`]. This call is the site half.
    fn bookmark<'a>(&'a self, _item: &'a SourceId, _on: bool) -> CapFut<'a, ()> {
        not_implemented(self.id(), CapabilityId::Bookmark.as_str())
    }
}

/// What a plugin exports. The host casts the `plugin_declaration` symbol to this.
#[allow(improper_ctypes_definitions)]
#[derive(Clone, Copy)]
pub struct PluginDeclaration {
    /// `rustc --version` the plugin was built with.
    pub rustc_version: &'static str,
    /// [`CORE_VERSION`] the plugin was built against.
    pub core_version: &'static str,
    /// Register every service in the plugin. The host calls this after the version check.
    pub register: unsafe extern "C" fn(&mut dyn PluginRegistrar),
}

/// The host's side of registration. One list for built-ins and plugins.
pub trait PluginRegistrar {
    /// Add `service`. A duplicate id is refused and the first registration stays.
    fn register_service(&mut self, service: Box<dyn DynService>);
}

/// `plugin_declaration` for a crate linked into the binary.
///
/// Several built-ins share one process, so this static is not `no_mangle`.
/// An external plugin uses [`declare_plugin!`], which is `no_mangle`, because
/// that crate is its own cdylib.
#[macro_export]
macro_rules! declare_builtin {
    ($ty:ty) => {
        #[allow(improper_ctypes_definitions)]
        unsafe extern "C" fn _acp_plugin_register(reg: &mut dyn $crate::PluginRegistrar) {
            reg.register_service(Box::new(<$ty>::default()));
        }

        #[doc = "Plugin entry. Built-ins call `register` directly. There is no `dlopen`."]
        pub static plugin_declaration: $crate::PluginDeclaration = $crate::PluginDeclaration {
            rustc_version: $crate::RUSTC_VERSION,
            core_version: $crate::CORE_VERSION,
            register: _acp_plugin_register,
        };
    };
}

/// `plugin_declaration` for an external cdylib. The symbol is `no_mangle`.
#[macro_export]
macro_rules! declare_plugin {
    ($ty:ty) => {
        #[allow(improper_ctypes_definitions)]
        unsafe extern "C" fn _acp_plugin_register(reg: &mut dyn $crate::PluginRegistrar) {
            reg.register_service(Box::new(<$ty>::default()));
        }

        #[doc = "Plugin entry the host reads after the package gate and the version check."]
        #[used]
        #[unsafe(no_mangle)]
        pub static plugin_declaration: $crate::PluginDeclaration = $crate::PluginDeclaration {
            rustc_version: $crate::RUSTC_VERSION,
            core_version: $crate::CORE_VERSION,
            register: _acp_plugin_register,
        };
    };
}

/// Built-in ids a third-party package is not allowed to replace.
pub const RESERVED_IDS: &[&str] = &["redstar", "x", "youtube"];

/// Reserved ids an official package may claim. `x` and `youtube` stay reserved.
///
/// This is an allow-list, not a signature. The repository does not ship a
/// private key. A package that rewrites its own manifest can write the field.
/// That is the same limit as a rewritten sha256.
pub const OFFICIAL_IDS: &[&str] = &["redstar"];

/// `meta_name` may be the package name when `[meta] official` is `official`.
///
/// A name that is not reserved needs no `official` field. A reserved name is
/// accepted only when `official` equals that name and the name is in
/// [`OFFICIAL_IDS`].
pub fn claim_allowed(meta_name: &str, official: Option<&str>) -> Result<(), String> {
    if !RESERVED_IDS.contains(&meta_name) {
        if let Some(claimed) = official
            && claimed != meta_name
        {
            return Err(format!(
                "official {claimed} does not match package {meta_name}"
            ));
        }
        return Ok(());
    }
    match official {
        Some(claimed) if claimed == meta_name && OFFICIAL_IDS.contains(&meta_name) => Ok(()),
        Some(_) | None => Err(format!("{meta_name} is reserved")),
    }
}

/// `id` may be registered.
///
/// `from_official_package` is true for a built-in and for a package whose
/// manifest passed [`claim_allowed`]. A package registrar must pass the
/// manifest's flag. Passing true for every plugin would let a third party
/// take `redstar`.
pub fn service_id_allowed(id: &str, from_official_package: bool) -> Result<(), String> {
    if !RESERVED_IDS.contains(&id) {
        return Ok(());
    }
    if from_official_package && OFFICIAL_IDS.contains(&id) {
        return Ok(());
    }
    Err(format!("{id} is reserved"))
}

/// The subject of [`DynService::probe`].
#[derive(Clone, Copy, Debug)]
pub enum Probe<'a> {
    /// A read or a write aimed at one account, item, or thread.
    Target(&'a Target),
    /// A local draft.
    Draft(&'a Draft),
    /// A page the service already returned.
    Page(&'a Page),
}

/// Whether `subject` is a real input for `cap`.
///
/// An empty subject, or the wrong kind, is [`ServiceError::Message`]. This
/// does not check [`DynService::implemented`].
pub fn probe_subject(cap: CapabilityId, subject: Probe<'_>) -> Result<(), ServiceError> {
    let fits = match cap {
        CapabilityId::Post
        | CapabilityId::Reply
        | CapabilityId::Edit
        | CapabilityId::Outbox
        | CapabilityId::LocalPost => match subject {
            Probe::Draft(draft) => !draft.body.trim().is_empty(),
            Probe::Target(_) | Probe::Page(_) => false,
        },
        CapabilityId::Upload => match subject {
            Probe::Draft(draft) => !draft.attachments.is_empty(),
            Probe::Target(_) | Probe::Page(_) => false,
        },
        CapabilityId::Repost => match subject {
            Probe::Draft(draft) => !draft.body.trim().is_empty() || draft.in_reply_to.is_some(),
            Probe::Target(Target::Item { id }) => !id.native.is_empty(),
            Probe::Page(page) => !page.items.is_empty(),
            Probe::Target(_) => false,
        },
        CapabilityId::Reader
        | CapabilityId::Live
        | CapabilityId::Feeds
        | CapabilityId::MediaFetcher
        | CapabilityId::Spaces => match subject {
            Probe::Target(target) => target_filled(target),
            Probe::Page(page) => page_filled(page),
            Probe::Draft(_) => false,
        },
        CapabilityId::Authenticator => match subject {
            Probe::Target(Target::Account { handle }) => !handle.trim().is_empty(),
            Probe::Target(_) | Probe::Draft(_) | Probe::Page(_) => false,
        },
        CapabilityId::Vote | CapabilityId::Subscribe | CapabilityId::Delete | CapabilityId::Bookmark | CapabilityId::ReportPost => {
            match subject {
                Probe::Target(Target::Item { id } | Target::Conversation { id }) => {
                    !id.native.is_empty()
                }
                Probe::Page(page) => !page.items.is_empty(),
                Probe::Target(_) | Probe::Draft(_) => false,
            }
        }
        CapabilityId::UpdateProfile | CapabilityId::ReportUser => match subject {
            Probe::Target(Target::Account { handle }) => !handle.trim().is_empty(),
            Probe::Target(_) | Probe::Draft(_) | Probe::Page(_) => false,
        },
        CapabilityId::Notification | CapabilityId::CostMeter => match subject {
            Probe::Target(target) => target_filled(target),
            Probe::Draft(draft) => draft_filled(draft),
            Probe::Page(page) => page_filled(page),
        },
        CapabilityId::Research => match subject {
            Probe::Target(Target::Item { id }) => !id.native.is_empty(),
            Probe::Page(page) => !page.items.is_empty(),
            Probe::Target(_) | Probe::Draft(_) => false,
        },
    };
    if fits {
        Ok(())
    } else {
        Err(ServiceError::Message(format!(
            "{} probe needs {}",
            cap.as_str(),
            probe_expect(cap)
        )))
    }
}

fn probe_expect(cap: CapabilityId) -> &'static str {
    match cap {
        CapabilityId::Post
        | CapabilityId::Reply
        | CapabilityId::Edit
        | CapabilityId::Outbox
        | CapabilityId::LocalPost => "a draft with text",
        CapabilityId::Upload => "a draft with an attachment",
        CapabilityId::Repost => "a draft, an item, or a page of items",
        CapabilityId::Reader
        | CapabilityId::Live
        | CapabilityId::Feeds
        | CapabilityId::MediaFetcher
        | CapabilityId::Spaces => "a target or a page",
        CapabilityId::Authenticator => "an account target",
        CapabilityId::Vote
        | CapabilityId::Subscribe
        | CapabilityId::Delete
        | CapabilityId::Bookmark
        | CapabilityId::ReportPost => "an item, a conversation, or a page of items",
        CapabilityId::UpdateProfile | CapabilityId::ReportUser => "an account target",
        CapabilityId::Notification | CapabilityId::CostMeter => "a target, a draft, or a page",
        CapabilityId::Research => "an item or a page of items",
    }
}

fn target_filled(target: &Target) -> bool {
    match target {
        Target::Account { handle } => !handle.trim().is_empty(),
        Target::Search { query } => !query.trim().is_empty(),
        Target::Playlist { id } => !id.trim().is_empty(),
        Target::Item { id } | Target::Conversation { id } => !id.native.is_empty(),
        Target::Bookmarks | Target::Site => true,
    }
}

fn page_filled(page: &Page) -> bool {
    !page.accounts.is_empty() || !page.items.is_empty() || !page.segments.is_empty()
}

fn draft_filled(draft: &Draft) -> bool {
    !draft.body.trim().is_empty() || !draft.attachments.is_empty() || draft.in_reply_to.is_some()
}

/// The rustc ABI and the plugin-crate version match this binary.
pub fn versions_match(rustc_version: &str, core_version: &str) -> Result<(), String> {
    if rustc_version != RUSTC_VERSION {
        return Err(format!(
            "rustc mismatch (ABI): plugin has {rustc_version}, this binary has {RUSTC_VERSION}"
        ));
    }
    if core_version != CORE_VERSION {
        return Err(format!(
            "core mismatch (plugin crate): plugin has {core_version}, this binary has acp_rs_plugin {CORE_VERSION}"
        ));
    }
    Ok(())
}

/// A control is shown when any service in the view implements the trait.
pub fn any_implements<'a>(
    services: impl IntoIterator<Item = &'a dyn DynService>,
    cap: CapabilityId,
) -> bool {
    services.into_iter().any(|s| s.has(cap))
}

/// Sort key for a combined view: the updated time when it is a real timestamp, otherwise `posted_at`.
///
/// A `~NNNN` placeholder is not a time.
pub fn combine_sort_key<'a>(posted_at: &'a str, edited_at: Option<&'a str>) -> &'a str {
    match edited_at {
        Some(t) if !t.is_empty() && !t.starts_with('~') => t,
        _ => posted_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::{Availability, Field, RuntimeCtx};
    use std::future::Future;

    #[test]
    fn the_pin_names_rustc_and_this_crate() {
        assert!(RUSTC_VERSION.starts_with("rustc "), "{RUSTC_VERSION}");
        assert_eq!(CORE_VERSION, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn a_version_mismatch_names_both_sides() {
        let err = versions_match("rustc 0.0.0", CORE_VERSION).unwrap_err();
        assert!(err.contains("rustc mismatch"), "{err}");
        assert!(err.contains("(ABI)"), "{err}");
        assert!(err.contains(RUSTC_VERSION), "{err}");
        let err = versions_match(RUSTC_VERSION, "0.0.0").unwrap_err();
        assert!(err.contains("core mismatch"), "{err}");
        assert!(err.contains("plugin crate"), "{err}");
        assert!(err.contains("0.0.0"), "{err}");
        assert!(versions_match(RUSTC_VERSION, CORE_VERSION).is_ok());
    }

    #[test]
    fn the_official_redstar_id_passes_and_x_stays_reserved() {
        assert!(claim_allowed("redstar", Some("redstar")).is_ok());
        assert!(claim_allowed("redstar", None).unwrap_err().contains("reserved"));
        assert!(claim_allowed("x", Some("x")).unwrap_err().contains("reserved"));
        assert!(claim_allowed("youtube", Some("youtube"))
            .unwrap_err()
            .contains("reserved"));
        assert!(claim_allowed("example", None).is_ok());
        assert!(claim_allowed("example", Some("redstar")).is_err());
        assert!(service_id_allowed("redstar", true).is_ok());
        assert!(service_id_allowed("redstar", false).is_err());
        assert!(service_id_allowed("x", true).is_err());
        assert!(service_id_allowed("demo", false).is_ok());
        assert_eq!(CapabilityId::Spaces.as_str(), "spaces");
        assert_eq!(CapabilityId::UpdateProfile.as_str(), "update_profile");
        assert_eq!(CapabilityId::ReportPost.as_str(), "report_post");
        assert_eq!(CapabilityId::ReportUser.as_str(), "report_user");
        assert_eq!(CapabilityId::Bookmark.as_str(), "bookmark");
        assert_eq!(CapabilityId::all().len(), 23);
    }

    #[test]
    fn a_placeholder_edit_is_not_a_time() {
        assert_eq!(combine_sort_key("2020-01-01", Some("~0001")), "2020-01-01");
        assert_eq!(
            combine_sort_key("2020-01-01", Some("2024-05-01")),
            "2024-05-01"
        );
        assert_eq!(combine_sort_key("2020-01-01", None), "2020-01-01");
    }

    struct Bare;

    impl DynService for Bare {
        fn id(&self) -> &'static str {
            "bare"
        }
        fn display_name(&self) -> &'static str {
            "Bare"
        }
        fn implemented(&self) -> &'static [CapabilityId] {
            &[CapabilityId::Notification]
        }
        fn schema(&self) -> &'static [Field] {
            &[]
        }
        fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
            if !ctx.enabled || !self.has(cap) {
                Availability::Off
            } else {
                Availability::Ready
            }
        }
    }

    #[test]
    fn a_missing_trait_is_not_offered_and_a_probe_says_so() {
        let bare = Bare;
        assert!(bare.has(CapabilityId::Notification));
        assert!(!bare.has(CapabilityId::Vote));
        assert!(!any_implements(
            [&bare as &dyn DynService],
            CapabilityId::Vote
        ));
        assert!(any_implements(
            [&bare as &dyn DynService],
            CapabilityId::Notification
        ));
        let item = Target::Item {
            id: crate::records::SourceId::new(crate::records::ServiceId::new("bare"), "1"),
        };
        let err = futures_block(bare.probe(CapabilityId::Vote, Probe::Target(&item))).unwrap_err();
        assert!(err.to_string().contains("does not implement vote"), "{err}");
        let empty = Page::empty();
        let err =
            futures_block(bare.probe(CapabilityId::Notification, Probe::Page(&empty))).unwrap_err();
        assert!(err.to_string().contains("notification probe needs"), "{err}");
        let page = Page {
            items: vec![crate::records::ItemRec::new(
                crate::records::SourceId::new(crate::records::ServiceId::new("bare"), "1"),
                "hi",
            )],
            ..Page::empty()
        };
        assert!(futures_block(bare.probe(CapabilityId::Notification, Probe::Page(&page))).is_ok());
        let draft = Draft::text("hello");
        let err = probe_subject(CapabilityId::Post, Probe::Draft(&Draft::default())).unwrap_err();
        assert!(err.to_string().contains("post probe needs a draft with text"), "{err}");
        assert!(probe_subject(CapabilityId::Post, Probe::Draft(&draft)).is_ok());
        assert!(probe_subject(CapabilityId::Spaces, Probe::Target(&item)).is_ok());
        assert!(probe_subject(CapabilityId::Spaces, Probe::Draft(&draft)).is_err());
        let sid = crate::records::SourceId::new(crate::records::ServiceId::new("bare"), "1");
        let err = futures_block(bare.vote(&sid, crate::records::VoteChoice::Up)).unwrap_err();
        assert!(err.to_string().contains("does not implement vote"), "{err}");
        let account = Target::Account { handle: "ada".into() };
        assert!(probe_subject(CapabilityId::UpdateProfile, Probe::Target(&account)).is_ok());
        assert!(probe_subject(CapabilityId::ReportUser, Probe::Target(&account)).is_ok());
        assert!(probe_subject(CapabilityId::ReportPost, Probe::Target(&item)).is_ok());
        assert!(probe_subject(CapabilityId::Bookmark, Probe::Target(&item)).is_ok());
        assert!(probe_subject(CapabilityId::Bookmark, Probe::Target(&account)).is_err());
        let profile = crate::records::AccountRec::new(sid.clone(), "ada");
        let err = futures_block(bare.update_profile(&profile)).unwrap_err();
        assert!(err.to_string().contains("does not implement update_profile"), "{err}");
        let err = futures_block(bare.report_post(&sid, "spam")).unwrap_err();
        assert!(err.to_string().contains("does not implement report_post"), "{err}");
        let err = futures_block(bare.report_user(&sid, "spam")).unwrap_err();
        assert!(err.to_string().contains("does not implement report_user"), "{err}");
        let err = futures_block(bare.bookmark(&sid, true)).unwrap_err();
        assert!(err.to_string().contains("does not implement bookmark"), "{err}");
    }

    fn futures_block<F: Future>(f: F) -> F::Output {
        let mut f = Box::pin(f);
        let waker = std::task::Waker::from(std::sync::Arc::new(Nop));
        let mut cx = std::task::Context::from_waker(&waker);
        match f.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => panic!("probe should be ready"),
        }
    }

    struct Nop;
    impl std::task::Wake for Nop {
        fn wake(self: std::sync::Arc<Self>) {}
    }
}
