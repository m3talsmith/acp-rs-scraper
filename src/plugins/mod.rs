//! Plugin packages: the manifest gate, quarantine, and reload.
//!
//! An external plugin is `<name>-<os>-<arch>-<version>.tar.gz` in `~/.acp/plugins/`.
//! The app does not download plugins. A failed package is a potential security
//! threat and moves to `~/.acp/quarantined/plugins/`. A rustc or core-version
//! mismatch is refused and is not quarantined. The pin is printed only by `--doctor`.

mod gate;
mod host;
mod quarantine;

pub use gate::{host_arch, host_os, verify_archive, GateFailure};
pub use host::{reload, startup, summaries, Loaded, ReloadReport, Summary};
pub use quarantine::{delete, list, restore, show, Item};
