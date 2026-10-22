//! Sample posts for the theme editor's live preview (never the real database).

use crate::prelude::*;
use crate::tui::data::MemStore;

fn post(id: &str, parent: &str, reply_to: &str, nick: &str, at: &str, title: &str, content: &str) -> Post {
    Post { post_id: id.into(), parent_post_id: parent.into(), in_reply_to_id: reply_to.into(), nick: nick.into(),
        title: title.into(), content: content.into(), posted_at: at.into(), media: Vec::new(), truncated: false, body_json: String::new(), score: None }
}

/// Nicks in the theme preview. They are not members of the site.
pub const SAMPLE_NICKS: &[&str] = &["sample", "reader"];

/// A small made-up forum: two members, posts, a reply and a cross-link.
pub fn sample_store() -> MemStore {
    MemStore {
        posts: vec![
            post("SAMPLE001", "", "", "sample", "2026-05-01T09:00:00Z", "Field notes",
                "# A sample heading\n\nThis is **sample text** for the theme preview, with `code`, *emphasis*\nand a link to /rs/post/SAMPLE002.[^1]\n\n## Section\n\n- one\n- [x] two\n  1. nested\n\n> A quote\n\n---\n\n```sh\nacp-red-star --nick sample\n```\n\n| Item | Count |\n|---|--:|\n| Notebook | 12 |\n\n#### Heading 4\n\n[^1]: A footnote."),
            post("SAMPLE002", "", "", "reader", "2026-04-30T18:00:00Z", "Second note", "A short note for the preview."),
            post("SAMPLE003", "SAMPLE001", "SAMPLE001", "reader", "2026-05-01T10:00:00Z", "", "Thanks, that helps."),
            post("SAMPLE004", "", "", "sample", "2026-04-29T12:00:00Z", "Links", "A few links to try."),
        ],
        marks: vec!["SAMPLE002".into()],
        ..MemStore::default()
    }
}
