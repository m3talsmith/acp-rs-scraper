//! Shared core for an ACP node: identity, canonical CBOR, and sealed secrets.
//!
//! The terminal client, a future relay-only package, tray apps, and the Dart
//! FFI SDK depend on this crate. It does not open the SQLite database, draw
//! the terminal, or talk to the network.
//!
//! The master secret, the 24-word phrase, and the SLIP-39 shares live here.
//! Node keys are not derived from the master. A later module certifies an
//! independent node key with the master.

#![warn(missing_docs)]

pub mod cbor;
pub mod hpke;
pub mod master;
pub mod node;
pub mod password;
pub mod slip39;
pub mod stage;
pub mod store;

pub use cbor::{Cbor, blake3_hash, from_slice, to_vec};
pub use master::{AcpId, Master, derive_master, phrase_from_entropy, recover_seed};
pub use password::{Argon2Params, Stretch, check_password, default_params};
