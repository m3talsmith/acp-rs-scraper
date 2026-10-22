//! A declaration the host must refuse. `register` is a no-op and must not run.

#![warn(missing_docs)]

use acp_rs_plugin::{PluginDeclaration, PluginRegistrar};

#[allow(improper_ctypes_definitions)]
unsafe extern "C" fn register_nothing(_reg: &mut dyn PluginRegistrar) {}

/// Declaration with this binary's rustc pin and a plugin-crate version it will refuse.
#[used]
#[unsafe(no_mangle)]
pub static plugin_declaration: PluginDeclaration = PluginDeclaration {
    rustc_version: acp_rs_plugin::RUSTC_VERSION,
    core_version: "9.9.9",
    register: register_nothing,
};
