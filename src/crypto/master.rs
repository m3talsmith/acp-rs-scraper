//! Master secret, the 24-word phrase, and the `acp-id`.
//!
//! The derivation lives in `acp_rs_node_core`. This module re-exports it so
//! existing `crate::crypto::master` paths keep working.

pub use acp_rs_node_core::master::{
    AcpId, Master, derive_master, new_entropy, normalize_phrase, phrase_from_entropy, recover_seed,
    seed_from_material,
};
