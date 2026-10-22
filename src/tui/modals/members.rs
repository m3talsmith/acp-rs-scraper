//! Organizations › Members. The roster and this install's fingerprint.

use super::*;
use crate::config::{DB_REL_PATH, home};
use crate::orgs::{get_org, org_conn};
use crate::prelude::*;
use crossterm::event::KeyCode;

/// Roster, invite, and the fingerprint this install shows.
#[derive(Debug)]
pub struct MembersModal {
    /// Organization slug.
    pub slug: String,
    /// `identity  role  status` lines.
    pub rows: Vec<String>,
    /// Short fingerprint of this install's membership key. Not a seed.
    pub fingerprint: String,
}

impl MembersModal {
    /// Load the roster for `slug`.
    pub fn open(slug: &str) -> Result<Self> {
        let db = home()?.join(DB_REL_PATH);
        let c = org_conn(&db)?;
        let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug}"))?;
        let rows = crate::orgs::access::members(&c, org.id)?.into_iter().map(|m| format!("{}  {}  {}", m.identity_pub, m.role, m.status.as_str())).collect();
        let fingerprint = crate::orgs::access::local_identity(&db)?.fingerprint();
        Ok(Self { slug: slug.into(), rows, fingerprint })
    }
}

impl ModalState for MembersModal {
    fn title(&self) -> String {
        format!("Members {}", self.slug)
    }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => Some(ModalResult::ClosedCancelled),
            KeyCode::Char('?') => Some(ModalResult::Help("org-members")),
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        matches!(h, ModalHit::Button(_)).then_some(ModalResult::ClosedCancelled)
    }
}
