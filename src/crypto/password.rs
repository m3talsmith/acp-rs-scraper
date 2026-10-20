//! The mandatory password: length, a strength check, and Argon2id.
//!
//! The implementation lives in `acp_red_star_node_core`.

pub use acp_red_star_node_core::password::{
    Argon2Params, MIN_PASSWORD_CHARS, Stretch, argon2id, check_password, default_params,
};

#[cfg(test)]
pub(crate) use acp_red_star_node_core::password::{argon_count, start_argon_count};
