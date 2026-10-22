//! Red Star capabilities used by feeds and the outbox.
//!
//! The service split has not landed. These ids are the ones that split keeps.

use crate::prelude::*;

/// One thing Red Star can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityId {
    /// Named local views over the database.
    Feeds,
    /// Local drafts and the send queue.
    Outbox,
    /// A post stored on this machine before it is sent.
    LocalPost,
}

impl CapabilityId {
    /// Stable id.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feeds => "feeds",
            Self::Outbox => "outbox",
            Self::LocalPost => "local_post",
        }
    }
}

/// Whether a capability can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The local feed view or the local queue is shipped.
    Ready,
    /// Prepared, and it does not send.
    Gated,
}

/// The Red Star service, as far as this binary offers it.
pub struct RedStar;

impl RedStar {
    /// Service id.
    pub const ID: &'static str = "redstar";

    /// Capabilities this binary implements.
    pub fn implemented() -> &'static [CapabilityId] {
        &[CapabilityId::Feeds, CapabilityId::Outbox, CapabilityId::LocalPost]
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
