//! Post menu (`p`): read-only actions for the open or highlighted post.

use super::*;
use crossterm::event::KeyCode;

/// One row of the post menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostAction {
    /// `/rs/post/{id}/reposts`.
    Reposts,
    /// `/rs/post/{thread}/{id}/edits`, shown as a diff.
    Edits,
    /// Prepare `subscribe` with state `1`. Nothing is sent.
    Subscribe,
    /// Ask for `yes`, then prepare `delete-post`. Nothing is sent.
    Delete,
    /// Prepare an edit. The form is not built, and nothing is sent.
    Edit,
    /// Prepare a repost. The form is not built, and nothing is sent.
    Repost,
}

impl PostAction {
    /// The row label.
    pub fn label(self) -> &'static str {
        match self {
            PostAction::Reposts => "View reposts",
            PostAction::Edits => "View edits",
            PostAction::Subscribe => "Subscribe",
            PostAction::Delete => "Delete",
            PostAction::Edit => "Edit",
            PostAction::Repost => "Repost",
        }
    }
}

/// The post menu.
#[derive(Debug, Clone)]
pub struct PostMenu {
    /// The post the menu was opened on.
    pub post_id: String,
    /// Thread root (`post_id` for a root post). Kept for the edits row.
    pub thread_id: String,
    /// Rows.
    pub items: Vec<PostAction>,
    /// Selected row.
    pub cursor: usize,
}

impl PostMenu {
    /// Menu for `post_id` in thread `thread_id`.
    pub fn new(post_id: &str, thread_id: &str) -> Self {
        PostMenu { post_id: post_id.into(), thread_id: thread_id.into(), items: vec![PostAction::Reposts, PostAction::Edits, PostAction::Subscribe, PostAction::Delete, PostAction::Edit, PostAction::Repost], cursor: 0 }
    }
    /// The selected action.
    pub fn action(&self) -> PostAction { self.items[self.cursor.min(self.items.len().saturating_sub(1))] }
}

impl ModalState for PostMenu {
    fn title(&self) -> String { format!("Post {}", self.post_id) }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        let n = self.items.len();
        match k.code {
            KeyCode::Esc => Some(ModalResult::ClosedCancelled),
            KeyCode::Up | KeyCode::Char('k') => { self.cursor = cycle(self.cursor, n, -1); None }
            KeyCode::Down | KeyCode::Char('j') => { self.cursor = cycle(self.cursor, n, 1); None }
            KeyCode::Enter => Some(ModalResult::ClosedSubmitted),
            _ => None,
        }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h { ModalHit::Item(i) if i < self.items.len() => { self.cursor = i; Some(ModalResult::ClosedSubmitted) } _ => None }
    }
}
