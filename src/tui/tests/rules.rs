//! The rules editor opens, edits a name, and `?` scrolls Help to that control's topic.

use super::*;
use crate::tui::input::on_key;
use crate::tui::modals::{HelpModal, Modal, RulesModal};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;

fn key(a: &mut App, c: KeyCode) { on_key(a, KeyEvent::new(c, KeyModifiers::NONE)); }
fn typ(a: &mut App, s: &str) { for c in s.chars() { key(a, KeyCode::Char(c)); } }

fn open(tag: &str) -> (App, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("acp-rules-ui-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rs.database");
    let mut a = app();
    a.modal = Some(Modal::Rules(RulesModal::open(db.clone()).unwrap()));
    (a, db)
}

#[test]
fn the_editor_edits_a_name_and_question_mark_opens_the_trigger_topic() {
    let (mut a, _db) = open("edit");
    key(&mut a, KeyCode::Char('n'));
    typ(&mut a, "Watch");
    match &a.modal {
        Some(Modal::Rules(m)) => assert_eq!(m.name.value, "Watch"),
        other => panic!("expected the editor, got {other:?}"),
    }
    key(&mut a, KeyCode::Char('?'));
    match &a.modal {
        Some(Modal::Help(h)) => assert_eq!(h.lines[h.scroll], "[[rules-trigger]]"),
        other => panic!("expected help, got {other:?}"),
    }
    let help = crate::tui::modals::help_text().join("\n");
    for topic in ["rules-trigger", "rules-condition", "rules-action", "rules-priority", "rules-clone"] {
        assert!(help.contains(&format!("[[{topic}]]")), "{topic}");
    }
    let _ = HelpModal::topic("rules-condition");
}

#[test]
fn a_system_rule_stays_read_only_in_the_editor() {
    let (mut a, db) = open("system");
    key(&mut a, KeyCode::Enter);
    match &a.modal {
        Some(Modal::Rules(m)) => {
            assert!(m.id.is_some());
            assert_eq!(m.origin, crate::rules::Origin::System);
            assert_eq!(m.name.value, "Synced post");
        }
        other => panic!("{other:?}"),
    }
    typ(&mut a, "x");
    match &a.modal {
        Some(Modal::Rules(m)) => assert_eq!(m.name.value, "Synced post"),
        other => panic!("{other:?}"),
    }
    let rules = crate::rules::list_rules(&crate::rules::open_rules(&db).unwrap()).unwrap();
    assert!(rules.iter().any(|r| r.system_key.as_deref() == Some(crate::rules::SYSTEM_SYNCED_POST) && r.name == "Synced post"));
}
