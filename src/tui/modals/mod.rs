//! Modals: floating over the layout (small margin, dimmed background), each
//! closing with [`ModalResult::ClosedCancelled`] or [`ModalResult::ClosedSubmitted`].

pub mod about;
pub mod branches;
pub mod compose;
pub mod confirm;
pub mod help;
pub mod history;
pub mod job;
pub mod message;
pub mod palette;
pub mod post;
pub mod research;
pub mod rules;
pub mod search;
pub mod brand_picker;
pub mod theme_picker;
pub mod template;
pub mod assets;
pub mod members;
pub mod roles;
pub mod access;

use crossterm::event::KeyEvent;

pub use about::*;
pub use branches::*;
pub use compose::*;
pub use confirm::*;
pub use help::*;
pub use history::*;
pub use job::JobModal;
pub use message::*;
pub use palette::*;
pub use post::*;
pub use research::*;
pub use rules::*;
pub use search::*;
pub use brand_picker::*;
pub use theme_picker::*;
pub use template::*;
pub use assets::*;
pub use members::*;
pub use roles::*;
pub use access::*;

/// How a modal closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalResult {
    /// `CLOSED_CANCELLED`: discard the draft state.
    ClosedCancelled,
    /// `CLOSED_SUBMITTED`: apply it.
    ClosedSubmitted,
    /// Close this modal and open Help scrolled to a topic.
    Help(&'static str),
}

/// A clickable element inside a modal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalHit {
    /// The n-th button.
    Button(usize),
    /// The n-th field.
    Field(usize),
    /// The n-th list row.
    Item(usize),
}

/// Behaviour every modal shares.
pub trait ModalState {
    /// Title in the modal's border.
    fn title(&self) -> String;
    /// A key; `Some` closes the modal with that result.
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult>;
    /// A click on one of its elements; `Some` closes it.
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult>;
    /// Mouse wheel (positive: down).
    fn on_scroll(&mut self, _delta: i32) {}
}

/// The open modal.
#[derive(Debug)]
pub enum Modal {
    /// Usage and keys.
    Help(HelpModal),
    /// Confirm a destructive / consequential action.
    Confirm(ConfirmModal),
    /// Error or status.
    Message(MessageModal),
    /// Advanced search filters.
    Search(SearchModal),
    /// Command palette.
    Palette(PaletteModal),
    /// Pick / create a research project (and add to it).
    Research(ResearchModal),
    /// Pick a theme (F5).
    ThemePicker(Box<ThemePickerModal>),
    /// Pick a brand (the header's `«brand» ▾`).
    BrandPicker(Box<BrandPickerModal>),
    /// Read-only actions for one post (`p`).
    Post(PostMenu),
    /// Running and recent jobs.
    Job(JobModal),
    /// New post or reply. A text publish sends validate and save. Attachments are refused.
    Compose(ComposeModal),
    /// Name, slogan, and a star. The credits stay hidden.
    About(AboutModal),
    /// Sync › Rules. System rows are read-only.
    Rules(RulesModal),
    /// Saved searches. Choosing one re-runs it.
    History(HistoryModal),
    /// Action-history paths. Choosing one restores that path.
    Branches(BranchesModal),
    /// Organizations › Template. Source and a live preview.
    Template(TemplateModal),
    /// Organizations › Assets.
    Assets(AssetsModal),
    /// Organizations › Members. Roster and the local fingerprint.
    Members(MembersModal),
    /// Organizations › Roles. Built-ins, custom roles, and the two defaults.
    Roles(RolesModal),
    /// Organizations › Access. The gate, Open versus Closed, and pending registrations.
    Access(AccessModal),
}

impl Modal {
    /// The shared behaviour.
    pub fn state(&mut self) -> &mut dyn ModalState {
        match self {
            Modal::Help(m) => m, Modal::Confirm(m) => m,
            Modal::Message(m) => m, Modal::Search(m) => m, Modal::Palette(m) => m, Modal::Research(m) => m, Modal::ThemePicker(m) => m.as_mut(),
            Modal::BrandPicker(m) => m.as_mut(),
            Modal::Post(m) => m,
            Modal::Job(m) => m,
            Modal::Compose(m) => m,
            Modal::About(m) => m,
            Modal::Rules(m) => m,
            Modal::History(m) => m,
            Modal::Branches(m) => m,
            Modal::Template(m) => m,
            Modal::Assets(m) => m,
            Modal::Members(m) => m,
            Modal::Roles(m) => m,
            Modal::Access(m) => m,
        }
    }
}

/// Move a focus index by `d` within `0..n`, wrapping.
pub fn cycle(i: usize, n: usize, d: i32) -> usize {
    if n == 0 { return 0; }
    ((i as i64 + d as i64).rem_euclid(n as i64)) as usize
}
