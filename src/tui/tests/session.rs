//! An encrypted database, already unlocked, stays open for every screen.
//! `ACP_PASSWORD` is not set here. Job children receive the cached key on stdin.

use super::*;
use crate::crypto::atrest;
use crate::tui::commands::{Action, CommandId};
use crate::tui::data::{SearchFilter, SqliteStore, Store};
use crate::tui::modals::Modal;
use crate::tui::state::SessionState;

const PASSWORD: &str = "correct horse battery staple";

fn quiet(a: &App, what: &str) {
    let hit = a.status.iter().any(|l| {
        let l = l.to_ascii_lowercase();
        l.contains("needs a password") || l.contains("acp_password")
    });
    assert!(!hit, "{what} asked for a password: {:?}", a.status);
}

#[test]
fn every_screen_uses_the_unlocked_database_without_acp_password() {
    let home = std::env::temp_dir().join(format!(
        "acp-tui-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let db = home.join(DB_REL_PATH);
    atrest::create_fast(&db, PASSWORD).unwrap();
    let key = atrest::cached_key(&db).expect("the new database is unlocked");

    let conn = open_db(&db).unwrap();
    ensure_posts_schema(&conn).unwrap();
    conn.execute(
        "INSERT INTO posts (post_id,parent_post_id,in_reply_to_id,nick,title,content,posted_at,body_state)
         VALUES ('P1','','','ada','','general strike','2026-10-01T00:00:00Z','full')",
        [],
    ).unwrap();
    conn.execute_batch(BOOKMARKS_SCHEMA).unwrap();
    conn.execute("INSERT INTO bookmarks (post_id, created_at) VALUES ('P1', '2026-10-01T00:00:00Z')", []).unwrap();
    drop(conn);
    let research = crate::research::research_conn(&db).unwrap();
    crate::research::create_project(&research, "Notes", "local").unwrap();
    drop(research);

    atrest::forget(&db);
    let err = atrest::prepare(&db, None).unwrap_err().to_string();
    assert!(err.contains("password"), "{err}");
    atrest::install_cached_key(&db, &key);
    assert!(std::env::var_os("ACP_PASSWORD").is_none());
    crate::crypto::cmd::prepare_path(&db).unwrap();

    let store = SqliteStore::new(db);
    assert_eq!(store.nicks().unwrap().iter().map(|n| n.nick.clone()).collect::<Vec<_>>(), ["ada"]);
    assert_eq!(store.posts_for(&["ada".into()]).unwrap().len(), 1);
    assert_eq!(store.bookmarks().unwrap().len(), 1);
    assert_eq!(store.research_projects().unwrap().len(), 1);
    assert!(store.research_items("Notes").unwrap().is_empty());

    let mut a = App::new(Box::new(store), &no_args(), Theme::acp(), false);
    quiet(&a, "startup");
    assert_eq!(a.nicks.len(), 1);
    a.select_only(0);
    quiet(&a, "scrape posts");
    assert_eq!(a.base.posts.len(), 1);

    let tabs: Vec<(usize, String)> = a.menu.menu.items.iter().enumerate().map(|(i, it)| (i, it.label.to_string())).collect();
    for (i, label) in tabs {
        a.needs_redraw = false;
        a.open_menu(i);
        assert!(a.needs_redraw, "{label} did not ask for a full redraw");
        quiet(&a, &label);
        a.modal = None;
        a.close_menu();
        a.effects.clear();
    }

    a.session = SessionState::Valid;
    a.act(Action::StartScan);
    quiet(&a, "scrape");
    assert!(a.effects.is_empty(), "{:?}", a.effects);
    assert!(a.status.iter().any(|s| s.contains("not in this binary")), "{:?}", a.status);
    a.effects.clear();

    a.run_search(SearchFilter { keyword: "strike".into(), ..Default::default() });
    quiet(&a, "search");
    assert_eq!(a.search.as_ref().unwrap().posts.len(), 1);

    a.act(Action::ShowBookmarks);
    quiet(&a, "bookmarks");
    assert!(a.search.as_ref().unwrap().title.contains("Bookmarks"));

    a.menu.menu.set_text(CommandId::Research, "project", "Notes");
    a.act(Action::ShowResearch);
    quiet(&a, "research");
    assert!(a.search.as_ref().unwrap().title.contains("Notes"));
    a.act(Action::ResearchProjects);
    quiet(&a, "research projects");
    assert!(matches!(a.modal, Some(Modal::Research(_))));
    a.modal = None;

    for action in [Action::OpenEb, Action::OpenExplore, Action::OpenTop, Action::SyncNow] {
        a.effects.clear();
        a.act(action);
        quiet(&a, &format!("{action:?}"));
        assert!(a.effects.is_empty(), "{action:?} {:?}", a.effects);
        assert!(a.status.iter().any(|s| s.contains("not in this binary")), "{action:?} {:?}", a.status);
    }

    atrest::forget(&home.join(DB_REL_PATH));
    let _ = std::fs::remove_dir_all(&home);
}
