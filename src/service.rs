//! Red Star capabilities.
//!
//! The service split has not landed. These ids are the ones that split keeps.
//! A site feature is a capability this service implements. The menu and the
//! command line offer an action when it is implemented and ready. A gated
//! write stays visible and is not sent.

use crate::prelude::*;

/// One thing Red Star can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityId {
    /// Local drafts and the send queue.
    Outbox,
    /// A post stored on this machine before it is sent.
    LocalPost,
    /// Live updates on the open feed or thread.
    Live,
    /// The site Notifications pane (`/rs/notifications`), read-only.
    SiteNotifications,
}

impl CapabilityId {
    /// Stable id. The keymap names this, not a key.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Outbox => "outbox",
            Self::LocalPost => "local_post",
            Self::Live => "live",
            Self::SiteNotifications => "site_notifications",
        }
    }
}

/// Whether a capability can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The user can run it.
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
        &[
            CapabilityId::Outbox,
            CapabilityId::LocalPost,
            CapabilityId::Live,
            CapabilityId::SiteNotifications,
        ]
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
