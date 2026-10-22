//! Service traits and the plugin package gate.
//!
//! This crate is the ABI. It is not the P9 node crate. A plugin built with
//! another compiler, or against another version of this crate, is refused
//! before `register`. A bad archive is the host's to quarantine. This crate
//! only reports that the check failed.
//!
//! # Depending on this crate from another repository
//!
//! `publish = false` blocks crates.io. It does not block a git dependency.
//! Cargo selects this workspace member by its package name. Pin `rev` to a
//! commit that contains `crates/acp_rs_plugin`. `<core-repo>` is the
//! placeholder until the repository rename. The pin is the commit, not a
//! branch name.
//!
//! ```toml
//! [dependencies]
//! acp_rs_plugin = { git = "https://github.com/m3talsmith/<core-repo>", rev = "<commit>" }
//! ```
//!
//! Implement [`Service`] and the capability traits (`Reader`, `Post`, and the
//! rest). Default method bodies return [`ServiceError::NotImplemented`].
//! Override the ones this service provides. [`impl_default_caps!`] fills in
//! the traits left at the default. That set is a [`DynService`], and
//! [`declare_plugin!`] exports it. Do not also implement [`DynService`] by
//! hand, and do not import [`DynService`] in the service crate: the method
//! names match [`Service`].
//!
//! A crate that implements [`DynService`] directly, and does not implement
//! [`Service`], is the other path. The in-tree adapter uses that path.
//! [`CORE_VERSION`] in the declaration is this crate's version, not the root
//! crate's version.
//!
//! The core opens the database. A service calls [`Host`] for storage, HTTP,
//! configuration, sealed secrets, bookmarks, the outbox, export, import,
//! rules, research, maintenance, notices, and preferences. HTTP takes request
//! headers and a cookie jar scoped to the service. `save_page` stores
//! `body_state` and attachment rows. `known_ids` includes per-thread reply
//! counts. `export` takes [`ExportRequest`]: JSON or HTML, a compressed
//! archive, profile images, and a chosen set of post ids. [`Host::save_outbox`],
//! [`Host::enqueue_outbox`], and [`Host::cancel_outbox`] write the outbox.
//! [`Host::sync_bookmarks`] keeps a site-bookmark id mapping. [`Outbox`] and
//! [`LocalPost`] are traits. `impl_default_caps!(Ty, except Reader, Post)` takes
//! a comma-separated list. The `test-host` feature exposes `MemHost`, an
//! in-memory host for a service's tests. Store the host in [`Service::attach`].
//! This crate does not link SQLCipher or the node crate. This crate is `0.5.0`.
//!
//! [`CapabilityId::UpdateProfile`], [`CapabilityId::ReportPost`],
//! [`CapabilityId::ReportUser`], and [`CapabilityId::Bookmark`] are the
//! account and moderation operations. [`Host::bookmark_post`] is the local
//! half of bookmarking a post. The `test-host` feature's in-memory host
//! implements that same path.
//!
//! [`Host`] calls are synchronous. A plugin may make them from a thread or
//! runtime it owns. A websocket upgrade is out of scope. The service keeps its
//! own LiveView socket. [`HttpResponse`] carries the final URL, the response
//! headers (including `Location`), and the names of cookies a `Set-Cookie`
//! stored. Cookie values are not logged. One jar is the source of truth:
//! [`Host::cookies`] reads it, and [`Host::set_cookie`] merges into it.
//!
//! [`SESSION_SCHEMA`] is the email and base URL fields. [`CapabilityId::Spaces`]
//! is the Red Star Spaces id. An official package claims the reserved
//! `redstar` id with `[meta] official = "redstar"`. Libraries are
//! `[install].libraries` entries with `path` and `sha256`.

#![warn(missing_docs)]

mod abi;
mod caps;
mod gate;
mod host;
mod records;
#[cfg(feature = "test-host")]
mod test_host;

pub use abi::*;
pub use caps::*;
pub use gate::*;
pub use host::*;
pub use records::*;
#[cfg(feature = "test-host")]
pub use test_host::MemHost;
