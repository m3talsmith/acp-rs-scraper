//! Service traits and the plugin ABI.
//!
//! No I/O. The host opens the database, draws the terminal, and loads plugins.
//! A plugin built with another compiler, or against another version of this
//! crate, is refused before `register`.

use crate::records::{
    Availability, Budget, CapFut, Cursor, Draft, Expand, FeedRule, Field, FilledFields, ItemRec,
    MediaBytes, MediaRequest, NoticeRec, Page, ResearchRef, Resolved, RouteKind, RuntimeCtx,
    SealedMaterial, ServiceError, SourceId, Spend, Target, VoteChoice, WriteOutcome,
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
    /// Local drafts and the send queue.
    Outbox,
    /// A post stored on this machine before it is sent.
    LocalPost,
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
            Self::Outbox => "outbox",
            Self::LocalPost => "local_post",
        }
    }

    /// Every id, in a fixed order.
    pub fn all() -> &'static [CapabilityId] {
        &ALL
    }
}

const ALL: [CapabilityId; 18] = [
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
    CapabilityId::Outbox,
    CapabilityId::LocalPost,
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

    /// Probe `cap`. A capability left off `implemented()` returns not-implemented.
    fn probe<'a>(&'a self, cap: CapabilityId) -> CapFut<'a, ()> {
        let ok = self.has(cap);
        let service = self.id().to_string();
        let capability = cap.as_str().to_string();
        Box::pin(async move {
            if ok {
                Ok(())
            } else {
                Err(ServiceError::not_implemented(&service, &capability))
            }
        })
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
        #[unsafe(no_mangle)]
        pub static plugin_declaration: $crate::PluginDeclaration = $crate::PluginDeclaration {
            rustc_version: $crate::RUSTC_VERSION,
            core_version: $crate::CORE_VERSION,
            register: _acp_plugin_register,
        };
    };
}

/// Built-in ids a plugin is not allowed to replace.
pub const RESERVED_IDS: &[&str] = &["redstar", "x", "youtube"];

/// The rustc and core versions match this binary.
pub fn versions_match(rustc_version: &str, core_version: &str) -> Result<(), String> {
    if rustc_version != RUSTC_VERSION {
        return Err(format!(
            "rustc mismatch: plugin has {rustc_version}, this binary has {RUSTC_VERSION}"
        ));
    }
    if core_version != CORE_VERSION {
        return Err(format!(
            "core mismatch: plugin has {core_version}, this binary has acp_rs_plugin {CORE_VERSION}"
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
        assert!(err.contains(RUSTC_VERSION), "{err}");
        let err = versions_match(RUSTC_VERSION, "0.0.0").unwrap_err();
        assert!(err.contains("core mismatch"), "{err}");
        assert!(err.contains("0.0.0"), "{err}");
        assert!(versions_match(RUSTC_VERSION, CORE_VERSION).is_ok());
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
        let err = futures_block(bare.probe(CapabilityId::Vote)).unwrap_err();
        assert!(err.to_string().contains("does not implement vote"), "{err}");
        assert!(futures_block(bare.probe(CapabilityId::Notification)).is_ok());
        let sid = crate::records::SourceId::new(crate::records::ServiceId::new("bare"), "1");
        let err = futures_block(bare.vote(&sid, crate::records::VoteChoice::Up)).unwrap_err();
        assert!(err.to_string().contains("does not implement vote"), "{err}");
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
