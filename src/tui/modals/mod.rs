//! Modals: floating over the layout (small margin, dimmed background), each
//! closing with [`ModalResult::ClosedCancelled`] or [`ModalResult::ClosedSubmitted`].

pub mod about;
pub mod compose;
pub mod confirm;
pub mod help;
pub mod job;
pub mod login;
pub mod message;
pub mod palette;
pub mod post;
pub mod research;
pub mod search;
pub mod theme_picker;

use crossterm::event::KeyEvent;

pub use about::*;
pub use compose::*;
pub use confirm::*;
pub use help::*;
pub use job::JobModal;
pub use login::*;
pub use message::*;
pub use palette::*;
pub use post::*;
pub use research::*;
pub use search::*;
pub use theme_picker::*;

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
    /// Email + one-time code.
    Login(LoginModal),
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
    /// Read-only actions for one post (`p`).
    Post(PostMenu),
    /// Running and recent jobs.
    Job(JobModal),
    /// New post or reply. Publish does not open a connection.
    Compose(ComposeModal),
    /// Name, slogan, and a star. The credits stay hidden.
    About(AboutModal),
}

impl Modal {
    /// The shared behaviour.
    pub fn state(&mut self) -> &mut dyn ModalState {
        match self {
            Modal::Login(m) => m, Modal::Help(m) => m, Modal::Confirm(m) => m,
            Modal::Message(m) => m, Modal::Search(m) => m, Modal::Palette(m) => m, Modal::Research(m) => m, Modal::ThemePicker(m) => m.as_mut(),
            Modal::Post(m) => m,
            Modal::Job(m) => m,
            Modal::Compose(m) => m,
            Modal::About(m) => m,
        }
    }
}

/// Move a focus index by `d` within `0..n`, wrapping.
pub fn cycle(i: usize, n: usize, d: i32) -> usize {
    if n == 0 { return 0; }
    ((i as i64 + d as i64).rem_euclid(n as i64)) as usize
}
