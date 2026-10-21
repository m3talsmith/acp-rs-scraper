//! Red Star capabilities.
//!
//! A site feature is a capability this service implements. The coming split in
//! `architecture/PLAN-SCRAPERS.md` moves these ids onto the service traits.
//! The menu and the command line offer an action when it is implemented and
//! ready. A gated write stays visible and is not sent.

/// One capability id. The string is stable for the keymap and the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityId {
    /// Live updates on the open feed or thread.
    Live,
    /// The site Notifications pane (`/rs/notifications`), read-only.
    SiteNotifications,
}

impl CapabilityId {
    /// Stable id. The keymap names this, not a key.
    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityId::Live => "live",
            CapabilityId::SiteNotifications => "site_notifications",
        }
    }
}

/// Whether a capability can run now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The user can run it.
    Ready,
    /// Implemented, and not sent until a capture is in the tree.
    Gated,
}

/// The built-in Red Star service.
pub struct RedStar;

impl RedStar {
    /// Service id. Red Star is on unless its services file says otherwise.
    pub const ID: &'static str = "redstar";

    /// Capabilities this binary implements. Fixed for the life of the process.
    pub fn implemented() -> &'static [CapabilityId] {
        &[CapabilityId::Live, CapabilityId::SiteNotifications]
    }

    /// Runtime state. A missing capability is not offered.
    pub fn availability(cap: CapabilityId) -> Option<Availability> {
        Self::implemented().contains(&cap).then_some(Availability::Ready)
    }

    /// Refuse a capability this build does not offer as ready.
    pub fn require(cap: CapabilityId) -> crate::prelude::Result<()> {
        match Self::availability(cap) {
            Some(Availability::Ready) => Ok(()),
            Some(Availability::Gated) => {
                crate::prelude::bail!("{} is gated until a capture is in the tree", cap.as_str())
            }
            None => crate::prelude::bail!("Red Star does not implement {}", cap.as_str()),
        }
    }
}
