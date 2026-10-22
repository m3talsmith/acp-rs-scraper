//! Host side of the plugin registrar.
//!
//! This binary does not register a site service. `--doctor` still prints the
//! rustc pin and the `acp_rs_plugin` version a package must be built against.

pub use acp_rs_plugin::{Availability, CapabilityId};
use acp_rs_plugin::DynService;

/// What this binary says when a command needs the separate service package.
pub const SERVICE_ABSENT: &str = "fetching, login, and publishing are not in this binary";

/// Register nothing. External packages are not loaded at startup.
pub fn install(reg: &mut dyn acp_rs_plugin::PluginRegistrar) {
    let _ = reg;
}

/// The host's list of built-in services. This binary has none.
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
    /// Built-ins for this binary. There are none.
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
    fn this_binary_registers_no_built_in_service() {
        assert!(Registry::builtins().services().is_empty());
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
