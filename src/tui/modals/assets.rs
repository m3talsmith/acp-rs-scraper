//! Organizations › Assets. The list matches `--org-asset-list`.

use super::*;
use crate::config::{home, DB_REL_PATH};
use crate::orgs::{get_org, org_conn};
use crate::prelude::*;
use crossterm::event::KeyCode;

/// One organization's files.
#[derive(Debug)]
pub struct AssetsModal {
    /// Organization slug.
    pub slug: String,
    /// `name  visibility` lines, archived ones labeled.
    pub rows: Vec<String>,
}

impl AssetsModal {
    /// Load the files for `slug`.
    pub fn open(slug: &str) -> Result<Self> {
        let db = home()?.join(DB_REL_PATH);
        let c = org_conn(&db)?;
        let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug}"))?;
        let rows = crate::orgs::access::list_assets(&c, org.id)?
            .into_iter()
            .map(|a| {
                let archived = if a.archived { "  archived" } else { "" };
                format!("{}  {}  visibility: {}{archived}", a.id, a.name, a.visibility.as_str())
            })
            .collect();
        Ok(Self { slug: slug.into(), rows })
    }
}

impl ModalState for AssetsModal {
    fn title(&self) -> String { format!("Assets {}", self.slug) }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => Some(ModalResult::ClosedCancelled),
            KeyCode::Char('?') => Some(ModalResult::Help("org-assets")),
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        matches!(h, ModalHit::Button(_)).then_some(ModalResult::ClosedCancelled)
    }
}
