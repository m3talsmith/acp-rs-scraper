//! Red Star as one registered service.
//!
//! The scrape, login, and write code stays in the root crate, which already
//! opens the database and talks to the site. This crate is what the registrar
//! stores: the capability set, the settings schema, and `plugin_declaration`.
//! The host shows a control when this set includes that trait. Moving the
//! parsers would change the prelude those modules use, so this version does
//! not move them. Behaviour of the existing commands stays where it is.
//!
//! Red Star is on when `~/.acp/services/redstar.toml` is missing.

#![warn(missing_docs)]

use acp_red_star_node_core::service::{
    Availability, CapabilityId, DynService, Field, FieldKind, RuntimeCtx, Store,
};

/// The Red Star service.
#[derive(Debug, Default)]
pub struct RedStar;

/// Capabilities this binary already has.
///
/// Live updates and feeds are not in this set: those crates have not landed on
/// this branch. Vote, subscribe, delete, and repost are implemented and gated.
/// Reply edit is ready. A top-level post edit stays gated in the site module.
pub const IMPLEMENTED: &[CapabilityId] = &[
    CapabilityId::Authenticator,
    CapabilityId::Reader,
    CapabilityId::Notification,
    CapabilityId::Post,
    CapabilityId::Reply,
    CapabilityId::Upload,
    CapabilityId::Edit,
    CapabilityId::Vote,
    CapabilityId::Subscribe,
    CapabilityId::Delete,
    CapabilityId::Repost,
    CapabilityId::Research,
    CapabilityId::MediaFetcher,
    CapabilityId::Outbox,
    CapabilityId::LocalPost,
];

/// Non-secret settings. The session cookie stays in `~/.acp/session.json`.
pub const SCHEMA: &[Field] = &[
    Field {
        key: "enabled",
        label: "Enabled",
        help: "On when this file is missing.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "nicks",
        label: "Nicks",
        help: "Nicks this service reads.",
        kind: FieldKind::StringList,
        store: Store::Config,
    },
    Field {
        key: "mentions",
        label: "Mentions",
        help: "Read mentions.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
    Field {
        key: "bookmarks",
        label: "Bookmarks",
        help: "Read bookmarks.",
        kind: FieldKind::Bool,
        store: Store::Config,
    },
];

impl DynService for RedStar {
    fn id(&self) -> &'static str { "redstar" }

    fn display_name(&self) -> &'static str { "Red Star" }

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
            CapabilityId::Vote | CapabilityId::Subscribe | CapabilityId::Delete | CapabilityId::Repost => {
                Availability::Gated
            }
            _ => Availability::Ready,
        }
    }
}

acp_red_star_node_core::declare_builtin!(RedStar);

/// Every service implements notification. Red Star does.
pub fn implements_notification(service: &dyn DynService) -> bool {
    service.has(CapabilityId::Notification)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_red_star_node_core::service::{ServiceError, versions_match, CORE_VERSION, RUSTC_VERSION};

    #[test]
    fn red_star_matches_today_and_hides_what_it_does_not_implement() {
        let svc = RedStar;
        assert!(implements_notification(&svc));
        for cap in IMPLEMENTED {
            assert!(svc.has(*cap), "{}", cap.as_str());
        }
        assert!(!svc.has(CapabilityId::Live));
        assert!(!svc.has(CapabilityId::Feeds));
        assert!(!svc.has(CapabilityId::CostMeter));
        let ctx = RuntimeCtx::ready();
        assert_eq!(svc.availability(CapabilityId::Post, &ctx), Availability::Ready);
        assert_eq!(svc.availability(CapabilityId::Edit, &ctx), Availability::Ready);
        assert_eq!(svc.availability(CapabilityId::Vote, &ctx), Availability::Gated);
        assert_eq!(svc.availability(CapabilityId::Live, &ctx), Availability::Off);
        let off = RuntimeCtx { enabled: false, ..RuntimeCtx::ready() };
        assert_eq!(svc.availability(CapabilityId::Reader, &off), Availability::Off);
    }

    #[test]
    fn the_declaration_matches_this_binary() {
        assert!(versions_match(plugin_declaration.rustc_version, plugin_declaration.core_version).is_ok());
        assert_eq!(plugin_declaration.rustc_version, RUSTC_VERSION);
        assert_eq!(plugin_declaration.core_version, CORE_VERSION);
        let mut reg = Collect::default();
        unsafe { (plugin_declaration.register)(&mut reg) };
        assert_eq!(reg.ids, vec!["redstar"]);
    }

    #[test]
    fn a_probe_of_a_missing_trait_is_not_implemented() {
        let svc = RedStar;
        let err = block(svc.probe(CapabilityId::Live)).unwrap_err();
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
