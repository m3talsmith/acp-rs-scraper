//! Red Star capabilities.
//!
//! The service split in `architecture/PLAN-SCRAPERS.md` has not landed. These
//! ids are the ones that split keeps. A missing capability is not offered. A
//! gated one is implemented and does not send.

use crate::prelude::*;

/// One thing Red Star can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityId {
    /// Read a profile card.
    ProfileRead,
    /// Read `/rs/settings`.
    SettingsRead,
    /// Edit the signed-in profile. Gated.
    UpdateProfile,
    /// Save forum settings. Gated.
    UpdateSettings,
    /// Report a post. Gated.
    ReportPost,
    /// Report a user. Gated.
    ReportUser,
}

impl CapabilityId {
    /// Stable id. The keymap and the service split use this string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProfileRead => "profile_read",
            Self::SettingsRead => "settings_read",
            Self::UpdateProfile => "update_profile",
            Self::UpdateSettings => "update_settings",
            Self::ReportPost => "report_post",
            Self::ReportUser => "report_user",
        }
    }
}

/// Whether a capability can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The read is shipped.
    Ready,
    /// The write is prepared and does not send.
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
            CapabilityId::ProfileRead,
            CapabilityId::SettingsRead,
            CapabilityId::UpdateProfile,
            CapabilityId::UpdateSettings,
            CapabilityId::ReportPost,
            CapabilityId::ReportUser,
        ]
    }

    /// Runtime state. `None` means this service does not implement `id`.
    pub fn availability(id: CapabilityId) -> Option<Availability> {
        if !Self::implemented().contains(&id) {
            return None;
        }
        match id {
            CapabilityId::ProfileRead | CapabilityId::SettingsRead => Some(Availability::Ready),
            CapabilityId::UpdateProfile | CapabilityId::UpdateSettings | CapabilityId::ReportPost | CapabilityId::ReportUser => {
                Some(Availability::Gated)
            }
        }
    }

    /// The capability exists. Reads and gated writes both pass.
    pub fn known(id: CapabilityId) -> Result<Availability> {
        Self::availability(id).with_context(|| format!("Red Star does not implement {}", id.as_str()))
    }

    /// The capability is ready. A gated write bails here.
    pub fn require(id: CapabilityId) -> Result<()> {
        match Self::known(id)? {
            Availability::Ready => Ok(()),
            Availability::Gated => bail!("{} is gated until a capture is in the tree", id.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_and_settings_are_ready_and_the_writes_are_gated() {
        assert_eq!(RedStar::ID, "redstar");
        assert!(RedStar::require(CapabilityId::ProfileRead).is_ok());
        assert!(RedStar::require(CapabilityId::SettingsRead).is_ok());
        for id in [CapabilityId::UpdateProfile, CapabilityId::UpdateSettings, CapabilityId::ReportPost, CapabilityId::ReportUser] {
            assert_eq!(RedStar::availability(id), Some(Availability::Gated));
            let err = RedStar::require(id).unwrap_err().to_string();
            assert!(err.contains("gated until a capture"), "{err}");
            assert_eq!(RedStar::known(id).unwrap(), Availability::Gated);
        }
    }
}
