//! View › Spaces shows the saved list and asks the loaded service. It does not join a room.

use super::*;
use crate::spaces::SpaceRow;
use crate::tui::commands::Action;
use crate::tui::jobs::JobKind;
use crate::tui::state::Effect;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn spaces_opens_the_saved_list_and_shows_a_live_badge() {
    let mut store = fixture();
    store.posts[0].post_id = "ALICE0001".into();
    store.spaces.push(SpaceRow {
        slug: "strike-meeting".into(),
        post_id: "ALICE0001".into(),
        title: "Strike meeting".into(),
        state: "live".into(),
        live: true,
        scheduled_at: String::new(),
        ended_at: String::new(),
        recording_url: "https://example.test/recordings/strike".into(),
        fetched_at: "t".into(),
    });
    let mut a = App::new(Box::new(store), &no_args(), Theme::acp(), false);
    a.act(Action::OpenSpaces);
    assert_eq!(a.view().posts.iter().map(|p| p.post_id.as_str()).collect::<Vec<_>>(), ["ALICE0001"]);
    assert_eq!(a.view().site_key.as_deref(), Some("spaces"));
    assert!(a.spaces.get("ALICE0001").is_some_and(|row| row.live));
    let Some(Effect::RunJob(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--spaces"]);
    let mut t = Terminal::new(TestBackend::new(160, 32)).unwrap();
    t.draw(|f| crate::tui::widgets::draw(f, &mut a, std::time::Instant::now())).unwrap();
    let text: String = t.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();
    assert!(text.contains("LIVE"), "{text}");
    assert!(text.contains("https://example.test/recordings/strike"), "{text}");
    assert!(!text.contains("wss://"), "{text}");
}
