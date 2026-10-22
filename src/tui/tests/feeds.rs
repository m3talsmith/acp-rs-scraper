//! Library › Feeds. Refresh asks first. Cancel is the default.

use super::*;
use crate::tui::commands::{Action, CommandId};
use crate::tui::jobs::JobKind;
use crate::tui::modals::Modal;
use crate::tui::state::Effect;

#[test]
fn refresh_asks_and_cancel_is_the_default() {
    let mut a = app();
    a.menu.menu.set_text(CommandId::Feeds, "name", "Theory");
    a.act(Action::RefreshFeed);
    let Some(Modal::Confirm(m)) = &a.modal else { panic!("{:?}", a.modal) };
    assert_eq!(m.focus, 1, "Cancel is the default");
    assert!(m.lines.iter().any(|line| line.contains("Refresh this feed from the site? [y/N]")));
    a.close_modal(crate::tui::modals::ModalResult::ClosedSubmitted);
    let Some(Effect::RunJob(JobKind::Read, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    assert_eq!(argv[1..], ["--feed-refresh", "Theory", "--yes"]);
}
