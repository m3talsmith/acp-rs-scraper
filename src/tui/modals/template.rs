//! Organizations › Template. Source on the left, the rendered page on the right.
//!
//! The preview uses the theme on screen as CSS variables and the brand's title
//! and motto. Editing the theme stays in the theme editor. This preview follows it.

use super::*;
use crate::brands::Brand;
use crate::config::{home, DB_REL_PATH};
use crate::orgs::template::{self, Page};
use crate::orgs::{Audience, org_conn};
use crate::prelude::*;
use crate::tui::theme::Theme;
use crossterm::event::KeyCode;

/// What Save or Reset asked to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateIntent {
    /// Write the source.
    Save,
    /// Restore the default.
    Reset,
}

/// Side-by-side template editor.
#[derive(Debug)]
pub struct TemplateModal {
    /// Organization slug.
    pub slug: String,
    /// Template source. Typing updates [`Self::preview`].
    pub source: String,
    /// Cursor, in characters.
    pub cursor: usize,
    /// Which control has the keyboard. 0 is the source.
    pub focus: usize,
    /// Set when Save or Reset is chosen.
    pub intent: TemplateIntent,
    /// Brand title for the preview.
    pub brand_title: String,
    /// Brand motto for the preview. Empty hides the slogan.
    pub brand_motto: String,
    /// CSS variables from the theme on screen.
    pub theme_css: String,
    /// Organization display name.
    pub org_name: String,
    /// Organization description.
    pub org_description: String,
    /// Posts the owner may see, including private rows.
    pub posts: Vec<template::PostSlot>,
    /// Assets the owner may see.
    pub assets: Vec<template::AssetSlot>,
}

impl TemplateModal {
    /// Load `slug` and the theme that is on screen.
    pub fn open(slug: &str, brand: &Brand, theme: &Theme) -> Result<Self> {
        let db = home()?.join(DB_REL_PATH);
        let c = org_conn(&db)?;
        let org = crate::orgs::get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug}"))?;
        let paint = template::paint_of(theme);
        let page = template::page_for(&c, slug, &brand.title, &brand.motto, &paint.css(), Audience::Owner)?;
        let source = template::source(&c, org.id)?;
        Ok(Self::from_page(slug, source, page))
    }

    /// An editor with the page already loaded. Tests use this.
    pub fn from_page(slug: &str, source: String, page: Page) -> Self {
        let cursor = source.chars().count();
        Self {
            slug: slug.into(),
            source,
            cursor,
            focus: 0,
            intent: TemplateIntent::Save,
            brand_title: page.brand_title,
            brand_motto: page.brand_motto,
            theme_css: page.theme_css,
            org_name: page.org_name,
            org_description: page.org_description,
            posts: page.posts,
            assets: page.assets,
        }
    }

    /// The rendered page, or the template error while the source is unfinished.
    pub fn preview(&self) -> String {
        let page = Page {
            org_name: self.org_name.clone(),
            org_description: self.org_description.clone(),
            brand_title: self.brand_title.clone(),
            brand_motto: self.brand_motto.clone(),
            theme_css: self.theme_css.clone(),
            posts: self.posts.clone(),
            assets: self.assets.clone(),
        };
        template::render(&self.source, &page).unwrap_or_else(|e| format!("template error: {e:#}"))
    }

    fn insert(&mut self, ch: char) {
        let at = self.source.char_indices().nth(self.cursor).map(|(i, _)| i).unwrap_or(self.source.len());
        self.source.insert(at, ch);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 { return; }
        let at = self.source.char_indices().nth(self.cursor - 1).map(|(i, _)| i).unwrap_or(0);
        self.source.remove(at);
        self.cursor -= 1;
    }
}

impl ModalState for TemplateModal {
    fn title(&self) -> String { format!("Template {} · source and preview", self.slug) }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc => Some(ModalResult::ClosedCancelled),
            KeyCode::Tab => { self.focus = (self.focus + 1) % 4; None }
            KeyCode::BackTab => { self.focus = (self.focus + 3) % 4; None }
            KeyCode::Enter if self.focus == 1 => { self.intent = TemplateIntent::Save; Some(ModalResult::ClosedSubmitted) }
            KeyCode::Enter if self.focus == 2 => { self.intent = TemplateIntent::Reset; Some(ModalResult::ClosedSubmitted) }
            KeyCode::Enter if self.focus == 3 => Some(ModalResult::ClosedCancelled),
            KeyCode::Char('?') if self.focus != 0 => Some(ModalResult::Help("org-templates")),
            KeyCode::Char(ch) if self.focus == 0 && !k.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.insert(ch);
                None
            }
            KeyCode::Backspace if self.focus == 0 => { self.backspace(); None }
            KeyCode::Left if self.focus == 0 => { self.cursor = self.cursor.saturating_sub(1); None }
            KeyCode::Right if self.focus == 0 => { self.cursor = (self.cursor + 1).min(self.source.chars().count()); None }
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Button(0) => { self.intent = TemplateIntent::Save; Some(ModalResult::ClosedSubmitted) }
            ModalHit::Button(1) => { self.intent = TemplateIntent::Reset; Some(ModalResult::ClosedSubmitted) }
            ModalHit::Button(2) => Some(ModalResult::ClosedCancelled),
            ModalHit::Field(0) => { self.focus = 0; None }
            _ => None,
        }
    }
}
