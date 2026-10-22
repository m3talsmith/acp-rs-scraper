//! X as one registered service.
//!
//! This crate names the session operations and the API endpoints the plan
//! describes, and it refuses every read and write. It does not open a socket,
//! import cookies, or mint a guest token. Enabling the service does not fetch.

#![warn(missing_docs)]

use acp_red_star_node_core::service::{
    Availability, CapabilityId, DynService, Field, FieldKind, RuntimeCtx, Store,
};

/// The X service.
#[derive(Debug, Default)]
pub struct X;

/// Capabilities this crate advertises. Reads and writes stay gated.
pub const IMPLEMENTED: &[CapabilityId] = &[
    CapabilityId::Authenticator,
    CapabilityId::Reader,
    CapabilityId::Notification,
    CapabilityId::Post,
    CapabilityId::Reply,
    CapabilityId::Repost,
    CapabilityId::Delete,
    CapabilityId::CostMeter,
];

/// `route` values. Session is the default. API is pay-per-use.
pub const ROUTES: &[&str] = &["session", "api"];

/// Non-secret settings, plus the two secret fields the host seals.
///
/// Budgets are text. The schema has no float kind. `0` is not a budget here:
/// the plan's defaults are `10.0` and `2.0` US dollars, stored as text.
pub const SCHEMA: &[Field] = &[
    Field {
        key: "enabled",
        label: "Enabled",
        help: "Off when this file is missing. Turning it on does not fetch.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "route",
        label: "Route",
        help: "session (default) or api.",
        kind: FieldKind::Enum(ROUTES),
        store: Store::Config,
    },
    Field {
        key: "users",
        label: "Users",
        help: "Handles, without @.",
        kind: FieldKind::StringList,
        store: Store::Config,
    },
    Field {
        key: "searches",
        label: "Searches",
        help: "Saved searches. A search does not run until a read is ungated.",
        kind: FieldKind::StringList,
        store: Store::Config,
    },
    Field {
        key: "include_replies",
        label: "Replies",
        help: "The account's own replies. Default on.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "include_conversations",
        label: "Conversations",
        help: "Other people's replies under those posts. Default off.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "include_reposts",
        label: "Reposts",
        help: "Reposts of other posts. Default off.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "max_posts_per_run",
        label: "Posts per run",
        help: "Cap for one run. Default 500.",
        kind: FieldKind::Int,
        store: Store::Config,
    },
    Field {
        key: "monthly_budget_usd",
        label: "Monthly budget",
        help: "API route only, US dollars as text. Default 10.0. Ignored on session.",
        kind: FieldKind::Text,
        store: Store::Config,
    },
    Field {
        key: "per_run_budget_usd",
        label: "Per-run budget",
        help: "API route only, US dollars as text. Default 2.0.",
        kind: FieldKind::Text,
        store: Store::Config,
    },
    Field {
        key: "web_delay_ms",
        label: "Web delay",
        help: "Milliseconds between session requests. Default 4000.",
        kind: FieldKind::Int,
        store: Store::Config,
    },
    Field {
        key: "web_daily_post_cap",
        label: "Daily post cap",
        help: "Session route. Default 2000. A challenge stops the job.",
        kind: FieldKind::Int,
        store: Store::Config,
    },
    Field {
        key: "session_ack",
        label: "Session acknowledgement",
        help: "Set after the session-route warning. The API route does not use it.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "bearer",
        label: "Bearer",
        help: "API route. Sealed as x.bearer. Never written to the toml file.",
        kind: FieldKind::Secret,
        store: Store::Secret,
    },
    Field {
        key: "session",
        label: "Session",
        help: "Session cookies. Sealed as x.session. Never written to the toml file.",
        kind: FieldKind::Secret,
        store: Store::Secret,
    },
];

impl DynService for X {
    fn id(&self) -> &'static str { "x" }

    fn display_name(&self) -> &'static str { "X" }

    fn implemented(&self) -> &'static [CapabilityId] { IMPLEMENTED }

    fn schema(&self) -> &'static [Field] { SCHEMA }

    fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
        if !ctx.enabled || !self.has(cap) {
            return Availability::Off;
        }
        if ctx.limited.is_some() {
            return Availability::Limited;
        }
        match cap {
            CapabilityId::Notification | CapabilityId::CostMeter => Availability::Ready,
            CapabilityId::Reader
            | CapabilityId::Authenticator
            | CapabilityId::Post
            | CapabilityId::Reply
            | CapabilityId::Repost
            | CapabilityId::Delete => Availability::Gated,
            _ => Availability::Off,
        }
    }
}

acp_red_star_node_core::declare_builtin!(X);

/// Defaults from the plan. The file is not created until the user enables X.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Defaults {
    /// `session` or `api`.
    pub route: &'static str,
    /// The account's own replies.
    pub include_replies: bool,
    /// Other people's replies under those posts.
    pub include_conversations: bool,
    /// Reposts.
    pub include_reposts: bool,
    /// Posts stored in one run.
    pub max_posts_per_run: i64,
    /// API monthly cap, US dollars as text.
    pub monthly_budget_usd: &'static str,
    /// API cap for one run, US dollars as text.
    pub per_run_budget_usd: &'static str,
    /// Delay between session requests.
    pub web_delay_ms: i64,
    /// Session posts in one day.
    pub web_daily_post_cap: i64,
}

impl Defaults {
    /// The plan's defaults. Session route, replies on, conversations and reposts off.
    pub fn plan() -> Self {
        Self {
            route: "session",
            include_replies: true,
            include_conversations: false,
            include_reposts: false,
            max_posts_per_run: 500,
            monthly_budget_usd: "10.0",
            per_run_budget_usd: "2.0",
            web_delay_ms: 4000,
            web_daily_post_cap: 2000,
        }
    }
}

/// A session-route GraphQL operation. Query ids are unpinned in this build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionOp {
    /// Resolve a handle to a numeric id.
    UserByScreenName,
    /// The account's posts.
    UserTweets,
    /// The account's posts and their own replies.
    UserTweetsAndReplies,
    /// One thread.
    TweetDetail,
    /// Search.
    SearchTimeline,
}

impl SessionOp {
    /// Operation name on the GraphQL path.
    pub fn name(self) -> &'static str {
        match self {
            Self::UserByScreenName => "UserByScreenName",
            Self::UserTweets => "UserTweets",
            Self::UserTweetsAndReplies => "UserTweetsAndReplies",
            Self::TweetDetail => "TweetDetail",
            Self::SearchTimeline => "SearchTimeline",
        }
    }

    /// This build does not ship a captured query id.
    pub fn query_id(self) -> &'static str {
        let _ = self;
        "unpinned"
    }

    /// Path a later build would request. `{query_id}` is not filled in here.
    pub fn path(self) -> String {
        format!("/i/api/graphql/{}/{}", self.query_id(), self.name())
    }
}

/// Operations a session read of one handle would use, in order.
///
/// `include_replies` adds `UserTweetsAndReplies` and `TweetDetail`.
/// Search is [`SessionOp::SearchTimeline`] and is not part of a user read.
pub fn session_ops(include_replies: bool) -> Vec<SessionOp> {
    let mut ops = vec![SessionOp::UserByScreenName, SessionOp::UserTweets];
    if include_replies {
        ops.push(SessionOp::UserTweetsAndReplies);
        ops.push(SessionOp::TweetDetail);
    }
    ops
}

/// An API-route call. Used only when `route = api`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiCall {
    /// `GET /2/users/by/username/:username`.
    UserByUsername,
    /// `GET /2/users/:id/tweets`.
    UserTweets,
    /// `GET /2/tweets/search/recent`.
    SearchRecent,
    /// `GET /2/tweets/search/all`. Pay-per-use.
    SearchAll,
    /// `GET /2/tweets?ids=`.
    Lookup,
    /// `GET /2/users/:id/mentions`.
    Mentions,
}

impl ApiCall {
    /// Path relative to the API host. No host is contacted.
    pub fn path(self) -> &'static str {
        match self {
            Self::UserByUsername => "/2/users/by/username/:username",
            Self::UserTweets => "/2/users/:id/tweets",
            Self::SearchRecent => "/2/tweets/search/recent",
            Self::SearchAll => "/2/tweets/search/all",
            Self::Lookup => "/2/tweets",
            Self::Mentions => "/2/users/:id/mentions",
        }
    }
}

/// Calls a user read would make on the API route.
pub fn api_ops_for_user() -> &'static [ApiCall] {
    &[ApiCall::UserByUsername, ApiCall::UserTweets]
}

/// Tweet fields the API route would request. Not sent.
pub const TWEET_FIELDS: &str =
    "created_at,conversation_id,referenced_tweets,entities,public_metrics,note_tweet,edit_history_tweet_ids";

/// Date the price table was checked.
pub const PRICES_AS_OF: &str = "2026-10-21";

/// One row in the dated price table. Amounts are millionths of a US dollar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApiPrice {
    /// Resource name.
    pub resource: &'static str,
    /// Price in millionths of a US dollar. 5000 is $0.005.
    pub usd_micros: u64,
}

/// Pay-per-use prices checked 2026-10-21. Post read $0.005, user read $0.010, owned read $0.001.
pub const API_PRICES: &[ApiPrice] = &[
    ApiPrice { resource: "post_read", usd_micros: 5_000 },
    ApiPrice { resource: "user_read", usd_micros: 10_000 },
    ApiPrice { resource: "owned_read", usd_micros: 1_000 },
];

/// Platform cap on post reads in one month.
pub const MONTHLY_POST_READ_CAP: u64 = 3_000_000;

/// Estimate a run in millionths of a US dollar. This does not connect.
pub fn estimate_usd_micros(post_reads: u64, user_reads: u64, owned_reads: u64) -> u64 {
    post_reads * 5_000 + user_reads * 10_000 + owned_reads * 1_000
}

/// Guest tokens are not part of this service.
pub fn uses_guest_token() -> bool { false }

/// Refuse a read. The handle is trimmed. Nothing is fetched.
pub fn prepare_read(handle: &str) -> Result<(), String> {
    let handle = handle.trim().trim_start_matches('@');
    if handle.is_empty() {
        return Err("x read needs a handle. Nothing was fetched.".into());
    }
    if handle.contains('/') || handle.contains(' ') {
        return Err("x read needs one handle. Nothing was fetched.".into());
    }
    Err(format!("x read of {handle} is prepared, gated. Nothing was fetched."))
}

/// Refuse a write. Nothing is sent.
pub fn prepare_write(action: &str) -> Result<(), String> {
    let action = action.trim();
    if action.is_empty() {
        return Err("x write is prepared, gated. Nothing was sent.".into());
    }
    Err(format!("x {action} is prepared, gated. Nothing was sent."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_red_star_node_core::service::{ServiceError, versions_match, CORE_VERSION, RUSTC_VERSION};

    #[test]
    fn x_is_off_until_enabled_and_reads_stay_gated() {
        let svc = X;
        assert_eq!(svc.id(), "x");
        assert!(svc.has(CapabilityId::Notification));
        assert!(svc.has(CapabilityId::CostMeter));
        assert!(svc.has(CapabilityId::Reader));
        assert!(!svc.has(CapabilityId::Live));
        assert!(!svc.has(CapabilityId::Feeds));
        assert!(!svc.has(CapabilityId::Upload));
        let on = RuntimeCtx::ready();
        assert_eq!(svc.availability(CapabilityId::Notification, &on), Availability::Ready);
        assert_eq!(svc.availability(CapabilityId::CostMeter, &on), Availability::Ready);
        assert_eq!(svc.availability(CapabilityId::Reader, &on), Availability::Gated);
        assert_eq!(svc.availability(CapabilityId::Post, &on), Availability::Gated);
        assert_eq!(svc.availability(CapabilityId::Reply, &on), Availability::Gated);
        assert_eq!(svc.availability(CapabilityId::Repost, &on), Availability::Gated);
        assert_eq!(svc.availability(CapabilityId::Delete, &on), Availability::Gated);
        assert_eq!(svc.availability(CapabilityId::Authenticator, &on), Availability::Gated);
        let off = RuntimeCtx { enabled: false, ..RuntimeCtx::ready() };
        assert_eq!(svc.availability(CapabilityId::Reader, &off), Availability::Off);
        assert_eq!(svc.availability(CapabilityId::Notification, &off), Availability::Off);
        let limited = RuntimeCtx { limited: Some("budget"), ..RuntimeCtx::ready() };
        assert_eq!(svc.availability(CapabilityId::Reader, &limited), Availability::Limited);
    }

    #[test]
    fn the_schema_keeps_secrets_out_of_the_file_and_budgets_as_text() {
        let bearer = SCHEMA.iter().find(|f| f.key == "bearer").unwrap();
        let session = SCHEMA.iter().find(|f| f.key == "session").unwrap();
        assert_eq!(bearer.store, Store::Secret);
        assert_eq!(session.store, Store::Secret);
        let monthly = SCHEMA.iter().find(|f| f.key == "monthly_budget_usd").unwrap();
        assert_eq!(monthly.kind, FieldKind::Text);
        let route = SCHEMA.iter().find(|f| f.key == "route").unwrap();
        assert_eq!(route.kind, FieldKind::Enum(ROUTES));
        assert!(SCHEMA.iter().any(|f| f.key == "session_ack"));
    }

    #[test]
    fn defaults_match_the_plan() {
        let d = Defaults::plan();
        assert_eq!(d.route, "session");
        assert!(d.include_replies);
        assert!(!d.include_conversations);
        assert!(!d.include_reposts);
        assert_eq!(d.max_posts_per_run, 500);
        assert_eq!(d.monthly_budget_usd, "10.0");
        assert_eq!(d.per_run_budget_usd, "2.0");
        assert_eq!(d.web_delay_ms, 4000);
        assert_eq!(d.web_daily_post_cap, 2000);
    }

    #[test]
    fn session_and_api_plans_name_operations_and_do_not_pin_query_ids() {
        let ops = session_ops(true);
        assert_eq!(ops[0], SessionOp::UserByScreenName);
        assert!(ops.contains(&SessionOp::UserTweetsAndReplies));
        assert!(!session_ops(false).contains(&SessionOp::UserTweetsAndReplies));
        for op in ops {
            assert_eq!(op.query_id(), "unpinned");
            assert!(op.path().contains(op.name()));
            assert!(op.path().starts_with("/i/api/graphql/"));
        }
        assert!(SessionOp::SearchTimeline.path().contains("SearchTimeline"));
        assert_eq!(api_ops_for_user(), &[ApiCall::UserByUsername, ApiCall::UserTweets]);
        assert_eq!(ApiCall::SearchAll.path(), "/2/tweets/search/all");
        assert_eq!(ApiCall::Mentions.path(), "/2/users/:id/mentions");
        assert!(TWEET_FIELDS.contains("edit_history_tweet_ids"));
        assert!(!uses_guest_token());
    }

    #[test]
    fn prices_are_dated_and_the_estimate_is_local() {
        assert_eq!(PRICES_AS_OF, "2026-10-21");
        assert_eq!(API_PRICES[0].usd_micros, 5_000);
        assert_eq!(API_PRICES[1].usd_micros, 10_000);
        assert_eq!(API_PRICES[2].usd_micros, 1_000);
        assert_eq!(estimate_usd_micros(2, 1, 4), 2 * 5_000 + 10_000 + 4 * 1_000);
        assert_eq!(MONTHLY_POST_READ_CAP, 3_000_000);
    }

    #[test]
    fn a_read_and_a_write_fetch_and_send_nothing() {
        let err = prepare_read("alice").unwrap_err();
        assert!(err.contains("prepared, gated"), "{err}");
        assert!(err.contains("Nothing was fetched"), "{err}");
        assert!(err.contains("alice"), "{err}");
        let err = prepare_read(" @bob ").unwrap_err();
        assert!(err.contains("bob"), "{err}");
        assert!(!err.contains('@'), "{err}");
        let err = prepare_read("  ").unwrap_err();
        assert!(err.contains("Nothing was fetched"), "{err}");
        let err = prepare_write("reply").unwrap_err();
        assert!(err.contains("prepared, gated"), "{err}");
        assert!(err.contains("Nothing was sent"), "{err}");
    }

    #[test]
    fn the_declaration_registers_x() {
        assert!(versions_match(plugin_declaration.rustc_version, plugin_declaration.core_version).is_ok());
        assert_eq!(plugin_declaration.rustc_version, RUSTC_VERSION);
        assert_eq!(plugin_declaration.core_version, CORE_VERSION);
        let mut reg = Collect::default();
        unsafe { (plugin_declaration.register)(&mut reg) };
        assert_eq!(reg.ids, vec!["x"]);
    }

    #[test]
    fn a_probe_of_a_missing_trait_is_not_implemented() {
        let err = block(X.probe(CapabilityId::Live)).unwrap_err();
        assert!(matches!(err, ServiceError::NotImplemented { .. }), "{err}");
    }

    fn block<F: std::future::Future>(f: F) -> F::Output {
        let mut f = Box::pin(f);
        let waker = std::task::Waker::from(std::sync::Arc::new(Nop));
        let mut cx = std::task::Context::from_waker(&waker);
        match f.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => panic!("ready"),
        }
    }

    struct Nop;
    impl std::task::Wake for Nop {
        fn wake(self: std::sync::Arc<Self>) {}
    }

    #[derive(Default)]
    struct Collect {
        ids: Vec<&'static str>,
    }

    impl acp_red_star_node_core::service::PluginRegistrar for Collect {
        fn register_service(&mut self, service: Box<dyn DynService>) {
            self.ids.push(service.id());
        }
    }
}
