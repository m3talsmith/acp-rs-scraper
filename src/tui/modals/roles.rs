//! Organizations › Roles. Built-ins, custom roles, and the two defaults.

use super::*;
use crate::config::{DB_REL_PATH, home};
use crate::orgs::access::MemberStatus;
use crate::orgs::{get_org, org_conn};
use crate::prelude::*;
use crossterm::event::KeyCode;

/// Roles for one organization. The last Owner is marked and is not cleared here.
#[derive(Debug)]
pub struct RolesModal {
    /// Organization slug.
    pub slug: String,
    /// Role lines, plus the two defaults.
    pub rows: Vec<String>,
}

impl RolesModal {
    /// Load roles for `slug`.
    pub fn open(slug: &str) -> Result<Self> {
        let db = home()?.join(DB_REL_PATH);
        let c = org_conn(&db)?;
        let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug}"))?;
        let owners = crate::orgs::access::members(&c, org.id)?
            .into_iter()
            .filter(|m| m.role.eq_ignore_ascii_case("Owner") && m.status == MemberStatus::Active)
            .count();
        let mut rows = vec![
            format!("register default: {}", org.default_register_role),
            format!("invite default: {}", org.default_invite_role),
        ];
        for role in crate::orgs::access::roles(&c, org.id)? {
            let last = if role.name.eq_ignore_ascii_case("Owner") && owners <= 1 { "  last owner" } else { "" };
            rows.push(format!("{}  rank {}  {}{last}", role.name, role.rank, role.permissions));
        }
        rows.push("The last Owner cannot be removed from this screen.".into());
        Ok(Self { slug: slug.into(), rows })
    }
}

impl ModalState for RolesModal {
    fn title(&self) -> String {
        format!("Roles {}", self.slug)
    }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => Some(ModalResult::ClosedCancelled),
            KeyCode::Char('?') => Some(ModalResult::Help("org-roles")),
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        matches!(h, ModalHit::Button(_)).then_some(ModalResult::ClosedCancelled)
    }
}
