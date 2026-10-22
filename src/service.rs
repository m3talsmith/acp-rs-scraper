//! Host side of the plugin registrar.
//!
//! This binary does not register a site service. An installed package is
//! loaded by [`crate::plugins`] after the gate. `--doctor` still prints the
//! rustc pin and the `acp_rs_plugin` version a package must be built against.

pub use acp_rs_plugin::{Availability, CapabilityId};
use acp_rs_plugin::DynService;

/// What this binary says when a command needs the separate service package.
pub const SERVICE_ABSENT: &str = "fetching, login, and publishing are not in this binary";

/// Register the linked services. External packages are not loaded at startup.
pub fn install(reg: &mut dyn acp_rs_plugin::PluginRegistrar) {
    // SAFETY: these call the built-in entries linked into this binary. They register X and YouTube and return.
    unsafe { (acp_red_star_service_x::plugin_declaration.register)(reg) }
    unsafe { (acp_red_star_service_youtube::plugin_declaration.register)(reg) }
}

/// The host's list of built-in services. X is linked. Red Star is not.
pub struct Registry {
    services: Vec<Box<dyn DynService>>,
    /// Built-in registration may claim `redstar`. A package registrar copies this from [`acp_rs_plugin::Verified::official`].
    official: bool,
}

/// Accept `x` and `youtube` from this binary only. A package still goes through [`Registry`]'s registrar, which refuses those ids.
struct Linked<'a>(&'a mut Registry);

impl acp_rs_plugin::PluginRegistrar for Linked<'_> {
    fn register_service(&mut self, mut service: Box<dyn DynService>) {
        let id = service.id();
        if self.0.services.iter().any(|s| s.id() == id) {
            return;
        }
        if id != "x" && id != "youtube" && acp_rs_plugin::service_id_allowed(id, self.0.official).is_err() {
            return;
        }
        service.attach(crate::host_api::CoreHost::shared());
        self.0.services.push(service);
    }
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
    /// Built-ins for this binary. X is linked.
    pub fn builtins() -> Self {
        let mut reg = Self::default();
        install(&mut Linked(&mut reg));
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

    /// Services this registrar accepted. The caller stores them with the library.
    pub fn into_services(self) -> Vec<Box<dyn DynService>> {
        self.services
    }
}

/// Refuse a plugin whose rustc or `acp_rs_plugin` version does not match.
///
/// This does not quarantine. A bad archive is a different failure.
pub fn admit(rustc_version: &str, core_version: &str) -> Result<(), String> {
    acp_rs_plugin::versions_match(rustc_version, core_version)
}

/// The process-wide built-in registry. Installed packages live in [`crate::plugins`].
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
    use acp_rs_plugin::{Field, PluginRegistrar, RuntimeCtx};

    struct Claim;

    impl DynService for Claim {
        fn id(&self) -> &'static str {
            "redstar"
        }
        fn display_name(&self) -> &'static str {
            "Red Star"
        }
        fn implemented(&self) -> &'static [CapabilityId] {
            &[]
        }
        fn schema(&self) -> &'static [Field] {
            &[]
        }
        fn availability(&self, _cap: CapabilityId, _ctx: &RuntimeCtx) -> Availability {
            Availability::Off
        }
    }

    #[test]
    fn this_binary_links_x_and_youtube_and_a_package_cannot_claim_redstar() {
        let builtins = Registry::builtins();
        assert_eq!(builtins.services().len(), 2);
        assert_eq!(builtins.services()[0].id(), "x");
        assert_eq!(builtins.services()[1].id(), "youtube");
        let mut external = Registry::external(false);
        external.register_service(Box::new(Claim));
        assert!(external.services().is_empty());
        let mut official = Registry::external(true);
        official.register_service(Box::new(Claim));
        assert_eq!(official.services().len(), 1);
        assert_eq!(official.services()[0].id(), "redstar");
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
