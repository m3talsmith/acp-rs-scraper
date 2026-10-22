//! In-tree Red Star adapter.
//!
//! The site modules still live in this crate. This adapter is how the host
//! sees them: the outbox and a local post are ready, and a capability this
//! binary does not implement is not offered. An external package is a later
//! step. Until then, a missing `~/.acp/services/redstar.toml` does not turn
//! this adapter off.

pub use acp_rs_plugin::{Availability, CapabilityId};
use acp_rs_plugin::{DynService, Field, RuntimeCtx};

use crate::prelude::*;

/// The Red Star service linked into this binary.
#[derive(Clone, Copy, Debug, Default)]
pub struct RedStar;

impl DynService for RedStar {
    fn id(&self) -> &'static str {
        "redstar"
    }

    fn display_name(&self) -> &'static str {
        "Red Star"
    }

    fn implemented(&self) -> &'static [CapabilityId] {
        &[CapabilityId::Outbox, CapabilityId::LocalPost]
    }

    fn schema(&self) -> &'static [Field] {
        acp_rs_plugin::SESSION_SCHEMA
    }

    fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
        if !ctx.enabled || !self.has(cap) {
            Availability::Off
        } else {
            Availability::Ready
        }
    }

    fn save_local<'a>(
        &'a self,
        id: &'a str,
        draft: &'a acp_rs_plugin::Draft,
        keep: bool,
    ) -> acp_rs_plugin::CapFut<'a, String> {
        let thread = draft
            .in_reply_to
            .as_ref()
            .map(|source| source.native.clone())
            .unwrap_or_default();
        let title = draft.title.clone();
        let body = draft.body.clone();
        let id = id.to_string();
        let result = crate::outbox::save_fields(&id, &title, &body, &thread, keep)
            .map_err(|err| acp_rs_plugin::ServiceError::Message(err.to_string()));
        Box::pin(async move { result })
    }

    fn list_local<'a>(&'a self) -> acp_rs_plugin::CapFut<'a, Vec<String>> {
        let result = crate::outbox::show_drafts()
            .map(lines)
            .map_err(|err| acp_rs_plugin::ServiceError::Message(err.to_string()));
        Box::pin(async move { result })
    }

    fn enqueue<'a>(&'a self, local_id: &'a str) -> acp_rs_plugin::CapFut<'a, String> {
        let id = local_id.to_string();
        let result = crate::outbox::queue_local(&id)
            .map_err(|err| acp_rs_plugin::ServiceError::Message(err.to_string()));
        Box::pin(async move { result })
    }

    fn queued<'a>(&'a self) -> acp_rs_plugin::CapFut<'a, Vec<String>> {
        let result = crate::outbox::show_outbox()
            .map(lines)
            .map_err(|err| acp_rs_plugin::ServiceError::Message(err.to_string()));
        Box::pin(async move { result })
    }

    fn cancel_queued<'a>(&'a self, id: &'a str) -> acp_rs_plugin::CapFut<'a, String> {
        let id = id.to_string();
        let result = crate::outbox::cancel_item(&id)
            .map_err(|err| acp_rs_plugin::ServiceError::Message(err.to_string()));
        Box::pin(async move { result })
    }
}

fn lines(text: String) -> Vec<String> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.lines().map(str::to_string).collect()
    }
}

impl RedStar {
    /// Service id.
    pub const ID: &'static str = "redstar";

    /// Capabilities this binary implements.
    pub fn implemented() -> &'static [CapabilityId] {
        DynService::implemented(&Self)
    }

    /// Runtime state. `None` means this service does not implement `id`.
    pub fn availability(id: CapabilityId) -> Option<Availability> {
        let svc = Self;
        if !svc.has(id) {
            return None;
        }
        Some(svc.availability(id, &RuntimeCtx::ready()))
    }

    /// The capability is ready.
    pub fn require(id: CapabilityId) -> Result<()> {
        match Self::availability(id) {
            Some(Availability::Ready) => Ok(()),
            Some(Availability::Gated) => {
                bail!("{} is gated until a capture is in the tree", id.as_str())
            }
            Some(_) | None => bail!("Red Star does not implement {}", id.as_str()),
        }
    }
}

/// Register the in-tree adapter. A second call keeps the first registration.
pub fn install(reg: &mut dyn acp_rs_plugin::PluginRegistrar) {
    reg.register_service(Box::new(RedStar));
}

/// The host's list of built-in services.
pub struct Registry {
    services: Vec<Box<dyn DynService>>,
    /// Built-in registration may claim `redstar`. A package registrar copies this from [`acp_rs_plugin::Verified::official`].
    official: bool,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            services: Vec::new(),
            official: true,
        }
    }
}

impl acp_rs_plugin::PluginRegistrar for Registry {
    fn register_service(&mut self, mut service: Box<dyn DynService>) {
        if self.services.iter().any(|s| s.id() == service.id()) {
            return;
        }
        if acp_rs_plugin::service_id_allowed(service.id(), self.official).is_err() {
            return;
        }
        service.attach(crate::host_api::CoreHost::shared());
        self.services.push(service);
    }
}

impl Registry {
    /// Built-ins for this binary. External packages are not in this list.
    pub fn builtins() -> Self {
        let mut reg = Self::default();
        install(&mut reg);
        reg
    }

    /// A registrar for one package. `official` comes from the manifest, not from the plugin.
    pub fn external(official: bool) -> Self {
        Self {
            services: Vec::new(),
            official,
        }
    }

    /// Services in registration order.
    pub fn services(&self) -> &[Box<dyn DynService>] {
        &self.services
    }
}

/// Refuse a plugin whose rustc or `acp_rs_plugin` version does not match.
///
/// This does not quarantine. A bad archive is a different failure.
pub fn admit(rustc_version: &str, core_version: &str) -> Result<(), String> {
    acp_rs_plugin::versions_match(rustc_version, core_version)
}

/// The process-wide built-in registry. A package is not opened here.
pub fn runtime() -> &'static Registry {
    static REG: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    REG.get_or_init(Registry::builtins)
}

/// Text `--doctor` prints. Authors read the pin only here.
pub fn doctor_lines() -> [String; 2] {
    [
        acp_rs_plugin::RUSTC_VERSION.to_string(),
        format!("acp_rs_plugin {}", acp_rs_plugin::CORE_VERSION),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_rs_plugin::PluginRegistrar;

    #[test]
    fn the_in_tree_adapter_keeps_the_outbox_ready() {
        assert!(RedStar::require(CapabilityId::Outbox).is_ok());
        assert!(RedStar::require(CapabilityId::LocalPost).is_ok());
        assert!(RedStar::require(CapabilityId::Vote).is_err());
        let reg = Registry::builtins();
        assert_eq!(reg.services().len(), 1);
        assert_eq!(reg.services()[0].id(), "redstar");
        let fields = reg.services()[0].schema();
        assert_eq!(fields[0].key, "email");
        assert_eq!(fields[1].key, "base_url");
        Registry::builtins();
        let mut external = Registry::external(false);
        external.register_service(Box::new(RedStar));
        assert!(external.services().is_empty());
        let mut official = Registry::external(true);
        official.register_service(Box::new(RedStar));
        assert_eq!(official.services().len(), 1);
    }

    #[test]
    fn a_version_mismatch_is_refused_and_is_not_a_package_error() {
        let err = admit("rustc 0.0.0", acp_rs_plugin::CORE_VERSION).unwrap_err();
        assert!(err.contains("rustc mismatch"), "{err}");
        assert!(admit(acp_rs_plugin::RUSTC_VERSION, acp_rs_plugin::CORE_VERSION).is_ok());
        let lines = doctor_lines();
        assert!(lines[0].starts_with("rustc "));
        assert!(lines[1].contains(acp_rs_plugin::CORE_VERSION));
    }
}
