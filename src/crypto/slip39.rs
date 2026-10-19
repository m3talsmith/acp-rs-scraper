//! SLIP-39 shares, one group, 3 of 5.
//!
//! The construction lives in `acp_rs_node_core` and is checked there against
//! the official test vectors.

pub use acp_rs_node_core::slip39::{
    combine, extract_mnemonics, member_position, set_id, sheet, split_seed,
};
