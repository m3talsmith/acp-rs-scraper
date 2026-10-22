//! Service traits and the plugin package gate.
//!
//! This crate is the ABI. It is not the P9 node crate. A plugin built with
//! another compiler, or against another version of this crate, is refused
//! before `register`. A bad archive is the host's to quarantine. This crate
//! only reports that the check failed.

#![warn(missing_docs)]

mod abi;
mod gate;

pub use abi::*;
pub use gate::*;
