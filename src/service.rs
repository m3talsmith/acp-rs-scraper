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
#[derive(Default)]
pub struct Registry {
    services: Vec<Box<dyn DynService>>,
}

impl acp_rs_plugin::PluginRegistrar for Registry {
    fn register_service(&mut self, service: Box<dyn DynService>) {
        if self.services.iter().any(|s| s.id() == service.id()) {
            return;
        }
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

    #[test]
    fn the_in_tree_adapter_keeps_the_outbox_ready() {
        assert!(RedStar::require(CapabilityId::Outbox).is_ok());
        assert!(RedStar::require(CapabilityId::LocalPost).is_ok());
        assert!(RedStar::require(CapabilityId::Vote).is_err());
        let reg = Registry::builtins();
        assert_eq!(reg.services().len(), 1);
        assert_eq!(reg.services()[0].id(), "redstar");
        Registry::builtins();
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
