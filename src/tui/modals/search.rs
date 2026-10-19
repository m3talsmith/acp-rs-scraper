//! Shift-S search modal: keyword, nicks, post id, date range, posts/replies.

use super::*;
use crate::tui::data::{KindFilter, SearchFilter};
use crate::tui::widgets::text::TextField;
use crossterm::event::KeyCode;

/// Field labels, in order. The last two rows are choices: Kind, then Where (local database or the site).
pub const SEARCH_FIELDS: &[&str] = &["Keyword", "Nicks", "Post id", "From (YYYY-MM-DD)", "To (YYYY-MM-DD)", "Kind", "Where"];
/// Buttons.
pub const SEARCH_BUTTONS: &[&str] = &["Search", "Clear", "Cancel"];

/// Search form.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchModal {
    /// Text fields (Keyword, Nicks, Post id, From, To).
    pub fields: Vec<TextField>,
    /// Kind choice.
    pub kind: KindFilter,
    /// Where: the site (`true`) or the local database.
    pub site: bool,
    /// Focus: fields, kind, where, then buttons.
    pub focus: usize,
    /// Submitted via Clear.
    pub cleared: bool,
    /// Validation error.
    pub error: Option<String>,
}

impl SearchModal {
    /// Form pre-filled from `f`.
    pub fn new(f: &SearchFilter) -> Self {
        let n = f.nicks.join(",");
        SearchModal { fields: [f.keyword.as_str(), &n, &f.post_id, &f.from, &f.to].iter().map(|s| TextField::new(s)).collect(),
            kind: f.kind, site: false, focus: 0, cleared: false, error: None }
    }
    /// The filter as typed.
    pub fn filter(&self) -> std::result::Result<SearchFilter, String> {
        let nicks = self.fields[1].value.trim();
        Ok(SearchFilter {
            keyword: self.fields[0].value.trim().into(),
            nicks: if nicks.is_empty() { Vec::new() } else { crate::cli::parse_nicks(nicks)?.0 },
            post_id: self.fields[2].value.trim().into(),
            from: self.fields[3].value.trim().into(),
            to: self.fields[4].value.trim().into(),
            kind: self.kind,
        })
    }
    fn n(&self) -> usize { SEARCH_FIELDS.len() + SEARCH_BUTTONS.len() }
    fn kind_i(&self) -> usize { SEARCH_FIELDS.len() - 2 }
    fn where_i(&self) -> usize { SEARCH_FIELDS.len() - 1 }
    fn cycle_kind(&mut self, d: i32) {
        let all = [KindFilter::Any, KindFilter::Posts, KindFilter::Replies];
        let i = all.iter().position(|k| *k == self.kind).unwrap_or(0);
        self.kind = all[cycle(i, 3, d)];
    }
    fn press(&mut self, b: usize) -> Option<ModalResult> {
        match b {
            0 => {
                if self.site && self.fields[0].value.trim().is_empty() {
                    self.error = Some("site search needs a keyword".into());
                    return None;
                }
                match self.filter() {
                    Ok(_) => Some(ModalResult::ClosedSubmitted),
                    Err(e) => { self.error = Some(e); None }
                }
            }
            1 => { self.cleared = true; Some(ModalResult::ClosedSubmitted) }
            _ => Some(ModalResult::ClosedCancelled),
        }
    }
}

impl ModalState for SearchModal {
    fn title(&self) -> String { "Search".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        let nf = SEARCH_FIELDS.len();
        match k.code {
            KeyCode::Esc => return Some(ModalResult::ClosedCancelled),
            KeyCode::Tab | KeyCode::Down => self.focus = cycle(self.focus, self.n(), 1),
            KeyCode::BackTab | KeyCode::Up => self.focus = cycle(self.focus, self.n(), -1),
            KeyCode::Enter => return if self.focus >= nf { self.press(self.focus - nf) } else { self.press(0) },
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if self.focus == self.kind_i() =>
                self.cycle_kind(if k.code == KeyCode::Left { -1 } else { 1 }),
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if self.focus == self.where_i() =>
                self.site = !self.site,
            _ if self.focus < self.kind_i() => { self.fields[self.focus].on_key(k); self.error = None; }
            _ => {}
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Field(i) => {
                self.focus = i;
                if i == self.kind_i() { self.cycle_kind(1); }
                if i == self.where_i() { self.site = !self.site; }
                None
            }
            ModalHit::Button(b) => self.press(b),
            ModalHit::Item(_) => None,
        }
    }
}
