//! Keys for the account: the master secret, the recovery phrase and the `acp-id`.
//!
//! This is the part of §7.13 that does not need a node. Certificates, enrollment,
//! revocation, relays and peers stay out of this crate until that stage; the types
//! they will need (a master signing key and a master wrapping key) are derived here.
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
pub mod pin;
pub mod progress;
pub mod recovery;
pub mod slip39;

pub use master::{AcpId, Master, derive_master, phrase_from_entropy, recover_seed};
pub use password::{Argon2Params, check_password, default_params};
pub use pin::Pin;

#[cfg(test)]
mod tests;
