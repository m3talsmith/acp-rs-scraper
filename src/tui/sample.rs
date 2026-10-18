//! Sample posts for the theme editor's live preview (never the real database).

use crate::prelude::*;
use crate::tui::data::MemStore;

fn post(id: &str, parent: &str, reply_to: &str, nick: &str, at: &str, title: &str, content: &str) -> Post {
    Post { post_id: id.into(), parent_post_id: parent.into(), in_reply_to_id: reply_to.into(), nick: nick.into(),
        title: title.into(), content: content.into(), posted_at: at.into(), media: Vec::new(), truncated: false, body_json: String::new(), score: None }
}

/// A small made-up forum: two members, posts, a reply and a cross-link.
pub fn sample_store() -> MemStore {
    MemStore {
        posts: vec![
            post("SAMPLE001", "", "", "comrade", "2026-05-01T09:00:00Z", "May Day",
                "# Workers of the world\n\nThis is **sample text** for the theme preview, with `code`, *emphasis*\nand a link to /rs/post/SAMPLE002.[^1]\n\n## Agenda\n\n- one\n- [x] two\n  1. nested\n\n> A quote\n\n---\n\n```sh\nacp-rs-scraper --nick comrade\n```\n\n| Item | Votes |\n|---|--:|\n| Strike fund | 12 |\n\n#### Heading 4\n\n[^1]: A footnote."),
            post("SAMPLE002", "", "", "organizer", "2026-04-30T18:00:00Z", "Meeting notes", "Agenda: the strike fund, the reading group."),
            post("SAMPLE003", "SAMPLE001", "SAMPLE001", "organizer", "2026-05-01T10:00:00Z", "", "Solidarity! See you at the march."),
            post("SAMPLE004", "", "", "comrade", "2026-04-29T12:00:00Z", "Reading list", "Start with the pamphlets."),
        ],
        marks: vec!["SAMPLE002".into()],
        ..MemStore::default()
    }
}
