//! Confirm modal: destructive Maintain actions, and quitting while a job runs.

use super::*;
use crate::tui::widgets::text::TextField;
use crossterm::event::KeyCode;

/// What to do once confirmed.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    /// Run this headless job (argv incl. `argv[0]`).
    Job(crate::tui::jobs::JobKind, Vec<String>),
    /// Quit; button 0 detaches the job, button 1 cancels it first.
    Quit,
    /// Delete a post. The send stays gated.
    Delete(String),
    /// Upvote, clear, or downvote. The send stays gated.
    Vote(String, String),
    /// Subscribe. The send stays gated.
    Subscribe(String),
    /// Edit a top-level post. The form is not built. The send stays gated.
    Edit(String),
    /// Edit a reply. Confirm sends validate, then save, once.
    EditReply {
        /// Reply id.
        id: String,
        /// Markdown body that replaces the document.
        body: String,
    },
    /// Repost. The form is not built. The send stays gated.
    Repost(String),
    /// Send one outbox row. The id is the `Q-` id.
    SendOutbox(String),
    /// Create the recovery phrase. The event loop leaves the screen first.
    CreatePhrase,
}

/// Question with buttons; `strict` also requires typing `yes`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmModal {
    /// Title.
    pub title: String,
    /// Explanation lines.
    pub lines: Vec<String>,
    /// Button labels; the last one cancels.
    pub buttons: Vec<String>,
    /// Focused button.
    pub focus: usize,
    /// Require typing `yes` (clear-all).
    pub strict: bool,
    /// The `yes` field (strict only).
    pub typed: TextField,
    /// Button that closed it with Submitted.
    pub chosen: usize,
    /// What to do.
    pub action: ConfirmAction,
    /// Shown when Submit was refused (strict without `yes`).
    pub error: Option<String>,
}

impl ConfirmModal {
    /// Confirm / Cancel.
    pub fn new(title: &str, lines: Vec<String>, confirm: &str, action: ConfirmAction) -> Self {
        ConfirmModal { title: title.into(), lines, buttons: vec![confirm.into(), "Cancel".into()], focus: 1,
            strict: false, typed: TextField::default(), chosen: 0, action, error: None }
    }
    fn press(&mut self, i: usize) -> Option<ModalResult> {
        if i + 1 >= self.buttons.len() { return Some(ModalResult::ClosedCancelled); }
        if self.strict && self.typed.value.trim() != "yes" {
            self.error = Some("type yes to confirm".into());
            return None;
        }
        self.chosen = i;
        Some(ModalResult::ClosedSubmitted)
    }
}

impl ModalState for ConfirmModal {
    fn title(&self) -> String { self.title.clone() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc => return Some(ModalResult::ClosedCancelled),
            KeyCode::Tab | KeyCode::Right => self.focus = cycle(self.focus, self.buttons.len(), 1),
            KeyCode::BackTab | KeyCode::Left => self.focus = cycle(self.focus, self.buttons.len(), -1),
            KeyCode::Enter => {
                // In strict mode Enter in the field means the first (confirm) button.
                let i = if self.strict && self.typed.value.trim() == "yes" { 0 } else { self.focus };
                return self.press(i);
            }
            KeyCode::Char('y') if !self.strict => return self.press(0),
            KeyCode::Char('n') if !self.strict => return Some(ModalResult::ClosedCancelled),
            _ if self.strict => { self.typed.on_key(k); self.error = None; }
            _ => {}
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h { ModalHit::Button(i) => { self.focus = i; self.press(i) } _ => None }
    }
}
