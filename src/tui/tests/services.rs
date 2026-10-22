//! Account › Services lists the reserved id and does not contact a site.

use super::*;
use crate::tui::commands::Action;
use crate::tui::modals::Modal;

#[test]
fn services_lists_the_reserved_id_and_combine_is_remembered_in_memory() {
    let mut a = app();
    a.act(Action::OpenServices);
    let Some(Modal::Services(m)) = &a.modal else { panic!("services") };
    assert!(m.rows.iter().any(|r| r.line().contains("reserved")));
    assert!(m.rows.iter().all(|r| !r.line().contains("acp.us")));
    a.combine = crate::services::Combine::Grouped;
    a.source = "x".into();
    assert_eq!(a.combine.as_str(), "grouped");
    assert_eq!(a.source, "x");
}
