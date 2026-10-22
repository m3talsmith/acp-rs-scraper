//! Service traits and the plugin ABI.
//!
//! No I/O. The host opens the database, draws the terminal, and loads plugins.
//! A plugin built with another compiler, or against another version of this
//! crate, is refused before `register`.

use std::future::Future;
use std::pin::Pin;

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
        Self { enabled: true, logged_in: true, limited: None }
    }
}

/// Failure from a capability call. A missing trait is [`ServiceError::NotImplemented`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceError {
    /// The service left this capability off `implemented()`.
    NotImplemented {
        /// Service id.
        service: String,
        /// Capability id.
        capability: String,
    },
    /// Any other refusal.
    Message(String),
}

impl ServiceError {
    /// The host called a capability the service does not implement.
    pub fn not_implemented(service: &str, capability: &str) -> Self {
        Self::NotImplemented { service: service.into(), capability: capability.into() }
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotImplemented { service, capability } => {
                write!(f, "{service} does not implement {capability}")
            }
            Self::Message(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// Object-safe service. Async calls return a boxed future so the value can cross a plugin.
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
    fn probe<'a>(
        &'a self,
        cap: CapabilityId,
    ) -> Pin<Box<dyn Future<Output = Result<(), ServiceError>> + Send + 'a>> {
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
        unsafe extern "C" fn _acp_plugin_register(
            reg: &mut dyn $crate::PluginRegistrar,
        ) {
            reg.register_service(Box::new(<$ty>::default()));
        }

        #[doc = "Plugin entry. Built-ins call `register` directly. There is no `dlopen`."]
        pub static plugin_declaration: $crate::PluginDeclaration =
            $crate::PluginDeclaration {
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
        unsafe extern "C" fn _acp_plugin_register(
            reg: &mut dyn $crate::PluginRegistrar,
        ) {
            reg.register_service(Box::new(<$ty>::default()));
        }

        #[doc = "Plugin entry the host reads after the package gate and the version check."]
        #[unsafe(no_mangle)]
        pub static plugin_declaration: $crate::PluginDeclaration =
            $crate::PluginDeclaration {
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
        return Err(format!("rustc mismatch: plugin has {rustc_version}, this binary has {RUSTC_VERSION}"));
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
        assert_eq!(combine_sort_key("2020-01-01", Some("2024-05-01")), "2024-05-01");
        assert_eq!(combine_sort_key("2020-01-01", None), "2020-01-01");
    }

    struct Bare;

    impl DynService for Bare {
        fn id(&self) -> &'static str { "bare" }
        fn display_name(&self) -> &'static str { "Bare" }
        fn implemented(&self) -> &'static [CapabilityId] { &[CapabilityId::Notification] }
        fn schema(&self) -> &'static [Field] { &[] }
        fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
            if !ctx.enabled || !self.has(cap) { Availability::Off } else { Availability::Ready }
        }
    }

    #[test]
    fn a_missing_trait_is_not_offered_and_a_probe_says_so() {
        let bare = Bare;
        assert!(bare.has(CapabilityId::Notification));
        assert!(!bare.has(CapabilityId::Vote));
        assert!(!any_implements([&bare as &dyn DynService], CapabilityId::Vote));
        assert!(any_implements([&bare as &dyn DynService], CapabilityId::Notification));
        let err = futures_block(bare.probe(CapabilityId::Vote)).unwrap_err();
        assert!(err.to_string().contains("does not implement vote"), "{err}");
        assert!(futures_block(bare.probe(CapabilityId::Notification)).is_ok());
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
