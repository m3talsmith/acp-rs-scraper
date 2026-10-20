//! Shift-S search modal: keyword, nicks, post id, date range, posts/replies, order, and limit.

use super::*;
use crate::prelude::ScrapeOrder;
use crate::tui::data::{KindFilter, SearchFilter};
use crate::tui::widgets::text::TextField;
use crossterm::event::KeyCode;

/// Field labels, in order. Kind stays at index 5. Order and Limit follow Where.
pub const SEARCH_FIELDS: &[&str] = &[
    "Keyword", "Nicks", "Post id", "From (YYYY-MM-DD)", "To (YYYY-MM-DD)", "Kind", "Where", "Order", "Limit",
];
/// Index of the Kind choice. Tabs in tests land here.
pub const SEARCH_KIND: usize = 5;
/// Index of the Where choice.
pub const SEARCH_WHERE: usize = 6;
/// Index of the Order choice.
pub const SEARCH_ORDER: usize = 7;
/// Index of the Limit field.
pub const SEARCH_LIMIT: usize = 8;
/// Buttons.
pub const SEARCH_BUTTONS: &[&str] = &["Search", "Clear", "Cancel"];
/// Tooltip for the Order row, including its Help topic.
pub const SEARCH_ORDER_TIP: &str = "Newest or oldest for these results. Same window as --order.  ? search-order";
/// Tooltip for the Limit row, including its Help topic.
pub const SEARCH_LIMIT_TIP: &str = "Keep this many matches at the chosen end. Blank keeps all.  ? search-limit";

/// Search form.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchModal {
    /// Text fields (Keyword, Nicks, Post id, From, To, Limit).
    pub fields: Vec<TextField>,
    /// Kind choice.
    pub kind: KindFilter,
    /// Where: the site (`true`) or the local database.
    pub site: bool,
    /// Which end of the matches to keep.
    pub order: ScrapeOrder,
    /// Focus: fields, kind, where, order, limit, then buttons.
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
        let limit = f.limit.map(|n| n.to_string()).unwrap_or_default();
        SearchModal {
            fields: [f.keyword.as_str(), &n, &f.post_id, &f.from, &f.to, limit.as_str()].iter().map(|s| TextField::new(s)).collect(),
            kind: f.kind, site: false, order: f.order, focus: 0, cleared: false, error: None,
        }
    }
    /// Text-field index for a focus row, when that row is a text field.
    pub fn text_index(focus: usize) -> Option<usize> {
        match focus { 0..=4 => Some(focus), SEARCH_LIMIT => Some(5), _ => None }
    }
    /// The filter as typed.
    pub fn filter(&self) -> std::result::Result<SearchFilter, String> {
        let nicks = self.fields[1].value.trim();
        let raw = self.fields[5].value.trim();
        let limit = if raw.is_empty() { None } else { Some(crate::cli::parse_post_limit(raw)?) };
        Ok(SearchFilter {
            keyword: self.fields[0].value.trim().into(),
            nicks: if nicks.is_empty() { Vec::new() } else { crate::cli::parse_nicks(nicks)?.0 },
            post_id: self.fields[2].value.trim().into(),
            from: self.fields[3].value.trim().into(),
            to: self.fields[4].value.trim().into(),
            kind: self.kind,
            order: self.order,
            limit,
        })
    }
    fn n(&self) -> usize { SEARCH_FIELDS.len() + SEARCH_BUTTONS.len() }
    fn kind_i(&self) -> usize { SEARCH_KIND }
    fn where_i(&self) -> usize { SEARCH_WHERE }
    fn cycle_kind(&mut self, d: i32) {
        let all = [KindFilter::Any, KindFilter::Posts, KindFilter::Replies];
        let i = all.iter().position(|k| *k == self.kind).unwrap_or(0);
        self.kind = all[cycle(i, 3, d)];
    }
    fn toggle_order(&mut self) {
        self.order = match self.order { ScrapeOrder::Newest => ScrapeOrder::Oldest, ScrapeOrder::Oldest => ScrapeOrder::Newest };
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
            KeyCode::Char('?') | KeyCode::F(1) if self.focus == SEARCH_ORDER => return Some(ModalResult::Help("search-order")),
            KeyCode::Char('?') | KeyCode::F(1) if self.focus == SEARCH_LIMIT => return Some(ModalResult::Help("search-limit")),
            KeyCode::Tab | KeyCode::Down => self.focus = cycle(self.focus, self.n(), 1),
            KeyCode::BackTab | KeyCode::Up => self.focus = cycle(self.focus, self.n(), -1),
            KeyCode::Enter => return if self.focus >= nf { self.press(self.focus - nf) } else { self.press(0) },
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if self.focus == self.kind_i() =>
                self.cycle_kind(if k.code == KeyCode::Left { -1 } else { 1 }),
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if self.focus == self.where_i() =>
                self.site = !self.site,
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if self.focus == SEARCH_ORDER => self.toggle_order(),
            _ => {
                if let Some(i) = Self::text_index(self.focus) { self.fields[i].on_key(k); self.error = None; }
            }
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Field(i) => {
                self.focus = i;
                if i == self.kind_i() { self.cycle_kind(1); }
                if i == self.where_i() { self.site = !self.site; }
                if i == SEARCH_ORDER { self.toggle_order(); }
                None
            }
            ModalHit::Button(b) => self.press(b),
            ModalHit::Item(_) => None,
        }
    }
}
