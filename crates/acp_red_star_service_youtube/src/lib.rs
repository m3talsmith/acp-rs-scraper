//! YouTube as one registered service.
//!
//! The session route plans a yt-dlp argument list. The API route names the
//! Data API lists. Neither path runs a process or opens a socket. Enabling
//! the service does not fetch. Captions use the listed locales. A locale the
//! video does not have is skipped. Comments are full depth unless a cap is set.
//! Live chat replay is its own argument list, separate from captions.

#![warn(missing_docs)]

use acp_red_star_node_core::service::{
    Availability, CapabilityId, DynService, Field, FieldKind, RuntimeCtx, Store,
};

/// The YouTube service.
#[derive(Debug, Default)]
pub struct YouTube;

/// Capabilities this crate advertises. Reads and media stay gated.
pub const IMPLEMENTED: &[CapabilityId] = &[
    CapabilityId::Authenticator,
    CapabilityId::Reader,
    CapabilityId::Notification,
    CapabilityId::MediaFetcher,
    CapabilityId::CostMeter,
];

/// `route` values. Session (yt-dlp) is the default.
pub const ROUTES: &[&str] = &["session", "api"];

/// Comment order.
pub const COMMENT_SORTS: &[&str] = &["top", "new"];

/// Non-secret settings, plus the secret fields the host seals.
///
/// `sleep_requests` is text because the schema has no float kind. `0` on
/// `max_comments` and `comment_depth` means no cap.
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
        help: "session (yt-dlp, default) or api.",
        kind: FieldKind::Enum(ROUTES),
        store: Store::Config,
    },
    Field {
        key: "ytdlp_path",
        label: "yt-dlp",
        help: "Program name on the planned argument list. Default yt-dlp. Nothing is spawned.",
        kind: FieldKind::Text,
        store: Store::Config,
    },
    Field {
        key: "channels",
        label: "Channels",
        help: "Handles, without @.",
        kind: FieldKind::StringList,
        store: Store::Config,
    },
    Field {
        key: "playlists",
        label: "Playlists",
        help: "Playlist ids or URLs. A read does not run until it is ungated.",
        kind: FieldKind::StringList,
        store: Store::Config,
    },
    Field {
        key: "transcripts",
        label: "Transcripts",
        help: "Plan caption downloads. Default on.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "audio",
        label: "Audio",
        help: "Audio is available, bounded by max_storage_gb. Default on.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "video",
        label: "Video",
        help: "Video is available, bounded by max_storage_gb. Default on.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "caption_locales",
        label: "Caption locales",
        help: "Locales to keep. Default en. A missing locale is skipped.",
        kind: FieldKind::StringList,
        store: Store::Config,
    },
    Field {
        key: "comments",
        label: "Comments",
        help: "Plan a full-depth comment read. Default on.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "max_comments",
        label: "Max comments",
        help: "0 means no count cap. A positive value caps the count.",
        kind: FieldKind::Int,
        store: Store::Config,
    },
    Field {
        key: "comment_depth",
        label: "Comment depth",
        help: "0 means full depth. A positive value caps the depth.",
        kind: FieldKind::Int,
        store: Store::Config,
    },
    Field {
        key: "comment_sort",
        label: "Comment sort",
        help: "top or new.",
        kind: FieldKind::Enum(COMMENT_SORTS),
        store: Store::Config,
    },
    Field {
        key: "live_chat",
        label: "Live chat replay",
        help: "On by default. Planned as its own argument list, kind=chat.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "shorts",
        label: "Shorts",
        help: "List the shorts tab. Default on.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "streams",
        label: "Streams",
        help: "List the streams tab. Default off.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "max_storage_gb",
        label: "Storage cap",
        help: "Gigabytes for video, audio, and transcripts. Default 20.",
        kind: FieldKind::Int,
        store: Store::Config,
    },
    Field {
        key: "sleep_requests",
        label: "Sleep",
        help: "Seconds between requests, as text. Default 1.0.",
        kind: FieldKind::Text,
        store: Store::Config,
    },
    Field {
        key: "api_daily_quota",
        label: "Daily quota",
        help: "API route units per day. Default 10000.",
        kind: FieldKind::Int,
        store: Store::Config,
    },
    Field {
        key: "api_per_run_quota",
        label: "Per-run quota",
        help: "API route units per run. Default 2000.",
        kind: FieldKind::Int,
        store: Store::Config,
    },
    Field {
        key: "api_key",
        label: "API key",
        help: "Data API key. Sealed as youtube.api_key. Never written to the toml file.",
        kind: FieldKind::Secret,
        store: Store::Secret,
    },
    Field {
        key: "cookies",
        label: "Cookies",
        help: "Only when a video needs sign-in. Sealed as youtube.cookies. Never written to the toml file.",
        kind: FieldKind::Secret,
        store: Store::Secret,
    },
];

impl DynService for YouTube {
    fn id(&self) -> &'static str { "youtube" }

    fn display_name(&self) -> &'static str { "YouTube" }

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
            CapabilityId::Reader | CapabilityId::MediaFetcher | CapabilityId::Authenticator => Availability::Gated,
            _ => Availability::Off,
        }
    }
}

acp_red_star_node_core::declare_builtin!(YouTube);

/// Defaults from the plan. The file is not created until the user enables YouTube.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Defaults {
    /// `session` or `api`.
    pub route: &'static str,
    /// Program name on the planned argv. Not spawned.
    pub ytdlp_path: &'static str,
    /// Caption locales, English first.
    pub caption_locales: &'static [&'static str],
    /// `0` means no count cap.
    pub max_comments: i64,
    /// `0` means full depth.
    pub comment_depth: i64,
    /// `top` or `new`.
    pub comment_sort: &'static str,
    /// Live chat replay.
    pub live_chat: bool,
    /// Shorts tab.
    pub shorts: bool,
    /// Streams tab.
    pub streams: bool,
    /// Uploads tab.
    pub uploads: bool,
    /// Storage cap in gigabytes.
    pub max_storage_gb: i64,
    /// Seconds between requests, as text.
    pub sleep_requests: &'static str,
    /// API units per day.
    pub api_daily_quota: i64,
    /// API units per run.
    pub api_per_run_quota: i64,
}

impl Defaults {
    /// The plan's defaults.
    pub fn plan() -> Self {
        Self {
            route: "session",
            ytdlp_path: "yt-dlp",
            caption_locales: &["en"],
            max_comments: 0,
            comment_depth: 0,
            comment_sort: "top",
            live_chat: true,
            shorts: true,
            streams: false,
            uploads: true,
            max_storage_gb: 20,
            sleep_requests: "1.0",
            api_daily_quota: 10_000,
            api_per_run_quota: 2_000,
        }
    }
}

/// A channel tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// Uploads.
    Videos,
    /// Shorts.
    Shorts,
    /// Livestream VODs.
    Streams,
}

impl Tab {
    /// Path segment.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Videos => "videos",
            Self::Shorts => "shorts",
            Self::Streams => "streams",
        }
    }
}

/// Tabs a channel listing would walk. Streams stay off unless asked.
pub fn listing_tabs(uploads: bool, shorts: bool, streams: bool) -> Vec<Tab> {
    let mut tabs = Vec::new();
    if uploads { tabs.push(Tab::Videos); }
    if shorts { tabs.push(Tab::Shorts); }
    if streams { tabs.push(Tab::Streams); }
    tabs
}

/// Planned channel URL. This does not connect.
pub fn channel_url(handle: &str, tab: Tab) -> String {
    let handle = handle.trim().trim_start_matches('@');
    format!("https://www.youtube.com/@{handle}/{}", tab.as_str())
}

/// `--sub-langs` value for captions.
///
/// A bare locale becomes `en.*` so every track in that language matches.
/// `live_chat` is left out of this list. It has its own command.
pub fn caption_sub_langs(locales: &[&str]) -> String {
    locales
        .iter()
        .copied()
        .filter(|l| *l != "live_chat" && !l.is_empty())
        .map(|l| {
            if l.contains('.') || l.contains('*') { l.to_string() } else { format!("{l}.*") }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Locales from `requested` that `available` actually has.
///
/// A missing locale is absent from the result. This does not log or fail.
/// Comparison is on the language prefix: `en` matches `en` and `en-US`.
pub fn available_caption_locales<'a>(requested: &[&str], available: &'a [&'a str]) -> Vec<&'a str> {
    available
        .iter()
        .copied()
        .filter(|have| {
            requested.iter().any(|want| {
                let want = want.trim();
                if want.is_empty() || want == "live_chat" { return false; }
                let want = want.trim_end_matches(".*").trim_end_matches('.');
                *have == want || have.starts_with(&format!("{want}-"))
            })
        })
        .collect()
}

/// Listing argv. Nothing is spawned.
pub fn list_args(bin: &str, handle: &str, tab: Tab, sleep_requests: &str) -> Vec<String> {
    vec![
        bin.to_string(),
        "--flat-playlist".into(),
        "--dump-json".into(),
        "--lazy-playlist".into(),
        "--sleep-requests".into(),
        sleep_requests.to_string(),
        channel_url(handle, tab),
    ]
}

/// Metadata argv for one video URL. Nothing is spawned.
pub fn metadata_args(bin: &str, video_url: &str, sleep_requests: &str) -> Vec<String> {
    vec![
        bin.to_string(),
        "-J".into(),
        "--skip-download".into(),
        "--no-write-playlist-metafiles".into(),
        "--sleep-requests".into(),
        sleep_requests.to_string(),
        video_url.to_string(),
    ]
}

/// Caption argv. `live_chat` is not in `--sub-langs`.
pub fn caption_args(bin: &str, video_url: &str, locales: &[&str], sleep_requests: &str) -> Vec<String> {
    vec![
        bin.to_string(),
        "--write-subs".into(),
        "--write-auto-subs".into(),
        "--sub-langs".into(),
        caption_sub_langs(locales),
        "--sub-format".into(),
        "json3/vtt/best".into(),
        "--skip-download".into(),
        "--sleep-requests".into(),
        sleep_requests.to_string(),
        "--sleep-subtitles".into(),
        "5".into(),
        video_url.to_string(),
    ]
}

/// Live chat replay argv, separate from captions. Rows would be `kind=chat`.
pub fn live_chat_args(bin: &str, video_url: &str, sleep_requests: &str) -> Vec<String> {
    vec![
        bin.to_string(),
        "--sub-langs".into(),
        "live_chat".into(),
        "--skip-download".into(),
        "--sleep-requests".into(),
        sleep_requests.to_string(),
        "--sleep-subtitles".into(),
        "5".into(),
        video_url.to_string(),
    ]
}

/// Comment argv. Caps are omitted when they are `0`.
pub fn comment_args(
    bin: &str,
    video_url: &str,
    max_comments: i64,
    comment_depth: i64,
    comment_sort: &str,
    sleep_requests: &str,
) -> Vec<String> {
    let mut args = vec![
        bin.to_string(),
        "--write-comments".into(),
        "--sleep-requests".into(),
        sleep_requests.to_string(),
    ];
    let mut limits = Vec::new();
    if max_comments > 0 { limits.push(format!("max_comments={max_comments}")); }
    if comment_depth > 0 { limits.push(format!("comment_depth={comment_depth}")); }
    if !comment_sort.is_empty() { limits.push(format!("comment_sort={comment_sort}")); }
    if !limits.is_empty() {
        args.push("--extractor-args".into());
        args.push(format!("youtube:{}", limits.join(",")));
    }
    args.push(video_url.to_string());
    args
}

/// Data API lists the API route would call. Each costs one unit. Nothing is called.
pub const API_LISTS: &[&str] = &[
    "channels.list",
    "playlistItems.list",
    "videos.list",
    "commentThreads.list",
    "comments.list",
];

/// The API cannot download other people's captions. Transcripts stay on the session plan.
pub fn api_captions_use_session_plan() -> bool { true }

/// True when `used` has reached `limit`.
pub fn quota_reached(used: i64, limit: i64) -> bool { limit > 0 && used >= limit }

/// Refuse a read. The planned argv is built and dropped. Nothing is spawned or fetched.
pub fn prepare_read(target: &str) -> Result<(), String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("youtube read needs a target. Nothing was fetched.".into());
    }
    let defaults = Defaults::plan();
    let tabs = listing_tabs(defaults.uploads, defaults.shorts, defaults.streams);
    if tabs.is_empty() {
        return Err("youtube read has no plan. Nothing was fetched.".into());
    }
    let planned = list_args(defaults.ytdlp_path, target, tabs[0], defaults.sleep_requests);
    if planned.iter().any(|a| a == "--cookies-from-browser" || a == "--download-archive") {
        return Err("youtube read refused a forbidden flag. Nothing was fetched.".into());
    }
    let _ = planned;
    Err(format!("youtube read of {target} is prepared, gated. Nothing was fetched."))
}

/// Refuse a write. Nothing is sent.
pub fn prepare_write(action: &str) -> Result<(), String> {
    let action = action.trim();
    if action.is_empty() {
        return Err("youtube write is prepared, gated. Nothing was sent.".into());
    }
    Err(format!("youtube {action} is prepared, gated. Nothing was sent."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_red_star_node_core::service::{ServiceError, versions_match, CORE_VERSION, RUSTC_VERSION};

    #[test]
    fn youtube_is_off_until_enabled_and_reads_stay_gated() {
        let svc = YouTube;
        assert_eq!(svc.id(), "youtube");
        assert!(svc.has(CapabilityId::MediaFetcher));
        assert!(svc.has(CapabilityId::Reader));
        assert!(svc.has(CapabilityId::Notification));
        assert!(!svc.has(CapabilityId::Live));
        assert!(!svc.has(CapabilityId::Feeds));
        assert!(!svc.has(CapabilityId::Post));
        let on = RuntimeCtx::ready();
        assert_eq!(svc.availability(CapabilityId::Notification, &on), Availability::Ready);
        assert_eq!(svc.availability(CapabilityId::CostMeter, &on), Availability::Ready);
        assert_eq!(svc.availability(CapabilityId::Reader, &on), Availability::Gated);
        assert_eq!(svc.availability(CapabilityId::MediaFetcher, &on), Availability::Gated);
        let off = RuntimeCtx { enabled: false, ..RuntimeCtx::ready() };
        assert_eq!(svc.availability(CapabilityId::Reader, &off), Availability::Off);
    }

    #[test]
    fn the_schema_uses_text_for_sleep_and_seals_the_key() {
        let sleep = SCHEMA.iter().find(|f| f.key == "sleep_requests").unwrap();
        assert_eq!(sleep.kind, FieldKind::Text);
        let key = SCHEMA.iter().find(|f| f.key == "api_key").unwrap();
        assert_eq!(key.store, Store::Secret);
        let cookies = SCHEMA.iter().find(|f| f.key == "cookies").unwrap();
        assert_eq!(cookies.store, Store::Secret);
        assert!(SCHEMA.iter().any(|f| f.key == "caption_locales"));
    }

    #[test]
    fn defaults_match_the_plan() {
        let d = Defaults::plan();
        assert_eq!(d.route, "session");
        assert_eq!(d.caption_locales, &["en"]);
        assert_eq!(d.max_comments, 0);
        assert_eq!(d.comment_depth, 0);
        assert!(d.live_chat);
        assert!(d.shorts);
        assert!(!d.streams);
        assert!(d.uploads);
        assert_eq!(d.max_storage_gb, 20);
        assert_eq!(d.sleep_requests, "1.0");
        assert_eq!(d.api_daily_quota, 10_000);
        assert_eq!(d.api_per_run_quota, 2_000);
        assert_eq!(listing_tabs(true, true, false), vec![Tab::Videos, Tab::Shorts]);
    }

    #[test]
    fn the_argv_plans_captions_comments_and_chat_without_spawning() {
        let list = list_args("yt-dlp", "@alice", Tab::Videos, "1.0");
        assert_eq!(list[0], "yt-dlp");
        assert!(list.contains(&"--flat-playlist".into()));
        assert!(list.contains(&"--lazy-playlist".into()));
        assert!(list.iter().any(|a| a == "https://www.youtube.com/@alice/videos"));
        assert!(!list.iter().any(|a| a == "--cookies-from-browser" || a == "--download-archive"));

        let caps = caption_args("yt-dlp", "https://www.youtube.com/watch?v=abc", &["en", "live_chat"], "1.0");
        let langs = caps.iter().position(|a| a == "--sub-langs").unwrap();
        assert_eq!(caps[langs + 1], "en.*");
        assert!(!caps[langs + 1].contains("live_chat"));
        assert!(caps.contains(&"--write-subs".into()));
        assert!(caps.contains(&"--write-auto-subs".into()));
        assert!(caps.contains(&"json3/vtt/best".into()));

        let chat = live_chat_args("yt-dlp", "https://www.youtube.com/watch?v=abc", "1.0");
        assert!(chat.contains(&"live_chat".into()));
        assert!(!chat.contains(&"--write-subs".into()));

        let open = comment_args("yt-dlp", "https://www.youtube.com/watch?v=abc", 0, 0, "top", "1.0");
        assert!(open.contains(&"--write-comments".into()));
        assert!(!open.iter().any(|a| a.contains("max_comments") || a.contains("comment_depth")));
        let capped = comment_args("yt-dlp", "https://www.youtube.com/watch?v=abc", 20, 3, "new", "1.0");
        assert!(capped.iter().any(|a| a.contains("max_comments=20") && a.contains("comment_depth=3")));

        let meta = metadata_args("yt-dlp", "https://www.youtube.com/watch?v=abc", "1.0");
        assert!(meta.contains(&"--skip-download".into()));
        assert!(meta.contains(&"--no-write-playlist-metafiles".into()));
    }

    #[test]
    fn a_missing_caption_locale_is_skipped() {
        let got = available_caption_locales(&["en", "fr"], &["en", "en-US", "es"]);
        assert_eq!(got, vec!["en", "en-US"]);
        assert!(!got.contains(&"es"));
        assert!(!got.iter().any(|l| l.contains("fr")));
    }

    #[test]
    fn the_api_route_names_lists_and_stops_on_quota() {
        assert_eq!(API_LISTS[0], "channels.list");
        assert!(API_LISTS.contains(&"commentThreads.list"));
        assert!(api_captions_use_session_plan());
        assert!(!quota_reached(10, 100));
        assert!(quota_reached(100, 100));
        assert!(!quota_reached(1, 0));
    }

    #[test]
    fn a_read_and_a_write_fetch_and_send_nothing() {
        let err = prepare_read("alice").unwrap_err();
        assert!(err.contains("prepared, gated"), "{err}");
        assert!(err.contains("Nothing was fetched"), "{err}");
        assert!(!err.contains("youtube.com"), "{err}");
        let err = prepare_read("  ").unwrap_err();
        assert!(err.contains("Nothing was fetched"), "{err}");
        let err = prepare_write("comment").unwrap_err();
        assert!(err.contains("Nothing was sent"), "{err}");
    }

    #[test]
    fn the_declaration_registers_youtube() {
        assert!(versions_match(plugin_declaration.rustc_version, plugin_declaration.core_version).is_ok());
        assert_eq!(plugin_declaration.rustc_version, RUSTC_VERSION);
        assert_eq!(plugin_declaration.core_version, CORE_VERSION);
        let mut reg = Collect::default();
        unsafe { (plugin_declaration.register)(&mut reg) };
        assert_eq!(reg.ids, vec!["youtube"]);
    }

    #[test]
    fn a_probe_of_a_missing_trait_is_not_implemented() {
        let err = block(YouTube.probe(CapabilityId::Post)).unwrap_err();
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
