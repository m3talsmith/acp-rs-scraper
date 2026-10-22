//! Red Star capabilities used by feeds.
//!
//! The service split has not landed. This id is the one that split keeps.

use crate::prelude::*;

/// One thing Red Star can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityId {
    /// Named local views over the database.
    Feeds,
}

impl CapabilityId {
    /// Stable id.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feeds => "feeds",
        }
    }
}

/// Whether a capability can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The local feed view is shipped.
    Ready,
    /// Prepared, and it does not run.
    Gated,
}

/// The Red Star service, as far as this binary offers it.
pub struct RedStar;

impl RedStar {
    /// Service id.
    pub const ID: &'static str = "redstar";

    /// Capabilities this binary implements.
    pub fn implemented() -> &'static [CapabilityId] {
        &[CapabilityId::Feeds]
    }

    /// Runtime state. `None` means this service does not implement `id`.
    pub fn availability(id: CapabilityId) -> Option<Availability> {
        if !Self::implemented().contains(&id) {
            return None;
        }
        Some(Availability::Ready)
    }

    /// The capability is ready.
    pub fn require(id: CapabilityId) -> Result<()> {
        match Self::availability(id) {
            Some(Availability::Ready) => Ok(()),
            Some(Availability::Gated) => bail!("{} is gated until a capture is in the tree", id.as_str()),
            None => bail!("Red Star does not implement {}", id.as_str()),
        }
    }
}
