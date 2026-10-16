//! TUI unit tests: state transitions, menu → argv mapping, layout geometry,
//! the read-only store, modals and headless renders (ratatui `TestBackend`).

mod commands;
mod bookmarks;
mod data;
mod jobs;
mod layout;
mod modals;
mod render;
mod state;
mod sync;
mod views;

use crate::prelude::*;
use crate::tui::data::MemStore;
use crate::tui::state::App;
use crate::tui::theme::Theme;

/// A post.
pub fn post(id: &str, parent: &str, reply_to: &str, nick: &str, at: &str, content: &str) -> Post {
    Post { post_id: id.into(), parent_post_id: parent.into(), in_reply_to_id: reply_to.into(), nick: nick.into(),
        title: String::new(), content: content.into(), posted_at: at.into(), media: Vec::new(), truncated: false }
}

/// alice: 2 roots + 1 reply in bob's thread; bob: 1 root linking alice's first post + 1 reply to alice.
pub fn fixture() -> MemStore {
    MemStore { posts: vec![
        post("ALICE0001", "", "", "alice", "2026-10-01T10:00:00Z", "First strike post.\n\nSecond paragraph."),
        post("ALICE0002", "", "", "alice", "2026-10-03T10:00:00Z", "General strike planning"),
        post("BOB000001", "", "", "bob", "2026-10-02T10:00:00Z", "See https://acp.us/rs/post/ALICE0001 and /rs/post/MISSING99"),
        post("ALICE0003", "BOB000001", "BOB000001", "alice", "2026-10-02T11:00:00Z", "Agreed with bob"),
        post("BOB000002", "ALICE0001", "ALICE0001", "bob", "2026-10-01T12:00:00Z", "Reply to alice"),
    ], atts: vec![Attachment { post_id: "ALICE0001".into(), url: "https://media.acp.us/x.jpg".into(), kind: "image".into(),
        local_path: PathBuf::from("/nonexistent/x.jpg"), mime_type: "image/jpeg".into() }], marks: vec!["BOB000002".into()] }
}

/// Default parsed args (no flags).
pub fn no_args() -> Args { use clap::Parser; Args::parse_from(["acp-rs-scraper"]) }

/// App over the fixture.
pub fn app() -> App { App::new(Box::new(fixture()), &no_args(), Theme::default(), false) }

/// Index of `nick` in the nick bar.
pub fn nick_idx(a: &App, nick: &str) -> usize { a.nicks.iter().position(|n| n.nick == nick).unwrap() }
