//! Organizations › Access. The gate, Open versus Closed, and pending registrations.

use super::*;
use crate::config::{DB_REL_PATH, home};
use crate::orgs::{get_org, org_conn};
use crate::prelude::*;
use crossterm::event::KeyCode;

/// Gated flag, registration mode, and the pending list for one organization.
#[derive(Debug)]
pub struct AccessModal {
    /// Organization slug.
    pub slug: String,
    /// Lines the screen shows.
    pub rows: Vec<String>,
}

impl AccessModal {
    /// Load the access screen for `slug`.
    pub fn open(slug: &str) -> Result<Self> {
        let db = home()?.join(DB_REL_PATH);
        let c = org_conn(&db)?;
        let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug}"))?;
        let mut rows = vec![
            format!("gated: {}", if org.gated { "yes" } else { "no" }),
            format!("registration: {}", org.registration.as_str()),
        ];
        let pending = crate::orgs::gate::pending(&c, slug)?;
        if pending.is_empty() {
            rows.push("No pending registrations".into());
        }
        for (identity, fingerprint) in pending {
            rows.push(format!("{identity}  fingerprint: {fingerprint}"));
        }
        rows.push("Approving a pending member shows the fingerprint.".into());
        rows.push("Opening a code shows the org name and origin before the yes.".into());
        Ok(Self { slug: slug.into(), rows })
    }
}

impl ModalState for AccessModal {
    fn title(&self) -> String {
        format!("Access {}", self.slug)
    }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => Some(ModalResult::ClosedCancelled),
            KeyCode::Char('?') => Some(ModalResult::Help("org-access")),
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        matches!(h, ModalHit::Button(_)).then_some(ModalResult::ClosedCancelled)
    }
}
