//! Keys for the account: the master secret, the recovery phrase and the `acp-id`.
//!
//! The derivation, canonical CBOR, and the secret-store trait live in
//! [`acp_rs_node_core`]. This module keeps encryption at rest, backup, and the
//! terminal progress screen, which depend on the database and the terminal.
//! Certificates, enrollment, revocation, relays and peers stay out until the
//! node-identity stage. The master signing key and the master wrapping key are
//! derived in the core crate.
//!
//! The plaintext phrase and the master secret are not written to disk. A pinned
//! [`pin::Pin`] stores the public `acp-id`, the Argon2 parameters and the two public
//! keys. [`recovery`] may store the 256-bit phrase entropy sealed under the password,
//! so the words can be shown again on this computer. The password alone cannot
//! recreate them.

pub mod atrest;
pub mod backup;
pub mod cmd;
pub mod master;
pub mod password;
pub mod node_store;
pub mod pin;
pub mod progress;
pub mod recovery;
pub mod slip39;

pub use master::{AcpId, Master, derive_master, phrase_from_entropy, recover_seed};
pub use password::{Argon2Params, check_password, default_params};
pub use pin::Pin;

#[cfg(test)]
mod tests;
