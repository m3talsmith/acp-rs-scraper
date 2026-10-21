//! Trigger, condition, and action matching, system-rule immutability, priority, and dry-run.

use super::*;
use crate::prelude::*;
use std::fs;

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("acp-rules-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn db(tag: &str) -> (PathBuf, Connection) {
    let d = dir(tag);
    let path = d.join("rs.database");
    let c = open_rules(&path).unwrap();
    (path, c)
}

fn put(c: &Connection, id: &str, parent: &str, reply: &str, nick: &str, title: &str, content: &str) {
    c.execute(
        "INSERT INTO posts (post_id, parent_post_id, in_reply_to_id, nick, title, content, posted_at, body_state, body_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, '2026-10-01T00:00:00Z', 'full', '')",
        params![id, parent, reply, nick, title, content],
    ).unwrap();
}

fn user(name: &str, priority: i64, conditions: Vec<Condition>, actions: Vec<RuleAction>) -> NewRule {
    NewRule { name: name.into(), enabled: true, priority, cloned_from: None, template_key: None, trigger: Trigger::PostSynced, conditions, actions }
}

fn outcomes(v: &[Firing]) -> Vec<(&str, &str, &str)> {
    v.iter().map(|f| (f.rule_name.as_str(), f.post_id.as_str(), f.outcome.as_str())).collect()
}

#[test]
fn system_rule_ships_off_and_clone_is_a_user_rule() {
    let (_path, c) = db("system");
    let rules = list_rules(&c).unwrap();
    let sys = rules.iter().find(|r| r.system_key.as_deref() == Some(SYSTEM_SYNCED_POST)).unwrap();
    assert_eq!(sys.origin, Origin::System);
    assert!(!sys.enabled);
    assert_eq!(sys.priority, 100);
    assert!(sys.actions.iter().any(|a| matches!(a, RuleAction::Notify)));
    let err = set_enabled(&c, "Synced post", false).unwrap_err().to_string();
    assert!(err.contains("read-only"), "{err}");
    assert!(delete_user(&c, "Synced post").is_err());
    assert!(update_user(&c, sys.id, &RuleEdit {
        name: "nope".into(), enabled: true, priority: 1, trigger: Trigger::PostSynced, conditions: vec![], actions: vec![RuleAction::Notify],
    }).is_err());
    set_priority(&c, "Synced post", 7).unwrap();
    ensure_system_rules(&c).unwrap();
    let again = list_rules(&c).unwrap().into_iter().find(|r| r.system_key.as_deref() == Some(SYSTEM_SYNCED_POST)).unwrap();
    assert_eq!(again.priority, 7, "a later start does not overwrite priority");
    let clone = clone_system(&c, SYSTEM_SYNCED_POST).unwrap();
    assert_eq!(clone.origin, Origin::User);
    assert!(clone.enabled);
    assert_eq!(clone.cloned_from.as_deref(), Some(SYSTEM_SYNCED_POST));
    assert!(clone.system_key.is_none());
    set_enabled(&c, &clone.name, false).unwrap();
    delete_user(&c, &clone.name).unwrap();
    assert!(list_rules(&c).unwrap().iter().any(|r| r.system_key.as_deref() == Some(SYSTEM_SYNCED_POST)));
}

#[test]
fn conditions_match_and_a_bad_regex_is_an_error() {
    let (path, c) = db("match");
    put(&c, "POSTAAA1", "", "", "Haz", "Strike", "the docks are out");
    put(&c, "POSTAAA2", "", "", "bob", "Other", "nothing here");
    put(&c, "POSTAAA3", "POSTAAA1", "POSTAAA1", "carol", "", "reply");
    put(&c, "POSTAAA4", "", "", "haz", "Note", "see #Strike today");
    c.execute("INSERT INTO post_tags (post_id, tag) VALUES ('POSTAAA4', 'strike')", []).unwrap();
    insert_user(&c, &user("author", 10, vec![Condition::Author { nick: "@haz".into() }], vec![RuleAction::Bookmark])).unwrap();
    insert_user(&c, &user("word", 20, vec![Condition::Keyword { text: "DOCKS".into() }], vec![RuleAction::Tag { tag: "dock".into() }])).unwrap();
    insert_user(&c, &user("src", 30, vec![Condition::Source { source: "redstar".into() }], vec![RuleAction::Notify])).unwrap();
    insert_user(&c, &user("node", 40, vec![Condition::Source { source: "node".into() }], vec![RuleAction::Notify])).unwrap();
    insert_user(&c, &user("tag", 50, vec![Condition::Tag { tag: "#strike".into() }], vec![RuleAction::Notify])).unwrap();
    insert_user(&c, &user("saved", 60, vec![Condition::SavedSearch { keyword: "docks".into(), nicks: vec!["haz".into()] }], vec![RuleAction::Notify])).unwrap();
    insert_user(&c, &user("reply", 70, vec![Condition::RepliesTo { nick: "haz".into() }], vec![RuleAction::Notify])).unwrap();
    insert_user(&c, &user("badre", 80, vec![Condition::Regex { pattern: "(".into() }], vec![RuleAction::Notify])).unwrap();
    let posts = load_posts(&c).unwrap();
    let fired = evaluate(&c, &path, &posts, "site", false).unwrap();
    let names = |id: &str| fired.iter().filter(|f| f.post_id == id && f.outcome == "fired").map(|f| f.rule_name.as_str()).collect::<Vec<_>>();
    assert!(names("POSTAAA1").contains(&"author"), "{fired:?}");
    assert!(!names("POSTAAA2").contains(&"author"));
    assert!(names("POSTAAA1").contains(&"word"));
    assert!(names("POSTAAA1").contains(&"src"));
    assert!(!fired.iter().any(|f| f.rule_name == "node" && f.outcome == "fired"));
    assert!(names("POSTAAA4").contains(&"tag"));
    assert!(names("POSTAAA1").contains(&"saved"));
    assert!(!names("POSTAAA4").contains(&"saved"), "saved search also filters nicks");
    assert!(names("POSTAAA3").contains(&"reply"));
    assert!(!names("POSTAAA1").contains(&"reply"));
    assert!(fired.iter().any(|f| f.rule_name == "badre" && f.outcome == "error" && f.detail.contains("invalid regex")));
    let marked: i64 = c.query_row("SELECT count(*) FROM bookmarks WHERE post_id = 'POSTAAA1'", [], |r| r.get(0)).unwrap();
    assert_eq!(marked, 1);
    let tagged: i64 = c.query_row("SELECT count(*) FROM post_tags WHERE post_id = 'POSTAAA1' AND tag = 'dock'", [], |r| r.get(0)).unwrap();
    assert_eq!(tagged, 1);
}

#[test]
fn lower_priority_runs_first_and_a_post_fires_once() {
    let (path, c) = db("prio");
    put(&c, "POSTBBB1", "", "", "ada", "Hi", "hello");
    insert_user(&c, &user("later", 50, vec![], vec![RuleAction::Notify])).unwrap();
    insert_user(&c, &user("sooner", 5, vec![], vec![RuleAction::Notify])).unwrap();
    let posts = load_posts(&c).unwrap();
    let first = evaluate(&c, &path, &posts, "site", false).unwrap();
    let order: Vec<_> = first.iter().filter(|f| f.outcome == "fired").map(|f| f.rule_name.as_str()).collect();
    assert_eq!(order, ["sooner", "later"]);
    let second = evaluate(&c, &path, &posts, "site", false).unwrap();
    assert!(second.iter().all(|f| f.outcome == "skipped" && f.detail == "already fired"), "{second:?}");
}

#[test]
fn new_rows_only_and_dry_run_applies_nothing() {
    let (path, c) = db("dry");
    put(&c, "POSTCCC1", "", "", "ada", "", "one");
    put(&c, "POSTCCC2", "", "", "ada", "", "two");
    insert_user(&c, &user("mark", 1, vec![Condition::Author { nick: "ada".into() }], vec![RuleAction::Bookmark])).unwrap();
    let posts = load_posts(&c).unwrap();
    let only_new: Vec<_> = posts.iter().filter(|p| p.post_id == "POSTCCC2").cloned().collect();
    let live = evaluate(&c, &path, &only_new, "site", false).unwrap();
    assert_eq!(outcomes(&live), vec![("mark", "POSTCCC2", "fired")]);
    let marked: Vec<String> = {
        let mut st = c.prepare("SELECT post_id FROM bookmarks").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap()
    };
    assert_eq!(marked, vec!["POSTCCC2".to_string()]);
    let previewed = preview(&path).unwrap();
    assert!(previewed.iter().any(|f| f.post_id == "POSTCCC1" && f.detail == "dry-run"), "{previewed:?}");
    assert!(previewed.iter().any(|f| f.post_id == "POSTCCC2" && f.detail == "already fired"));
    let marked_after: i64 = c.query_row("SELECT count(*) FROM bookmarks", [], |r| r.get(0)).unwrap();
    assert_eq!(marked_after, 1, "preview writes no bookmark");
}

#[test]
fn notify_is_not_capped() {
    let (path, c) = db("cap");
    for i in 0..9 {
        put(&c, &format!("POSTD{i:04}"), "", "", "ada", "", "x");
    }
    insert_user(&c, &user("ping", 1, vec![], vec![RuleAction::Notify])).unwrap();
    let posts = load_posts(&c).unwrap();
    let fired = evaluate(&c, &path, &posts, "site", false).unwrap();
    let notifies: Vec<_> = fired.iter().filter(|f| f.action == "notify").collect();
    assert_eq!(notifies.len(), 9);
    assert!(notifies.iter().all(|f| f.outcome == "fired"), "{notifies:?}");
    assert!(notifies.iter().all(|f| f.detail != "rate limit"));
}

#[test]
fn import_and_export_round_trip_toml_and_json() {
    let (path, c) = db("io");
    insert_user(&c, &user(
        "Watch",
        50,
        vec![Condition::Author { nick: "haz".into() }],
        vec![RuleAction::Notify],
    )).unwrap();
    let toml_path = path.parent().unwrap().join("rules.toml");
    let json_path = path.parent().unwrap().join("rules.json");
    export_file(&c, &toml_path, false).unwrap();
    export_file(&c, &json_path, true).unwrap();
    let toml_text = fs::read_to_string(&toml_path).unwrap();
    assert!(toml_text.contains("name = \"Watch\""));
    assert!(toml_text.contains("trigger = \"post_synced\""));
    assert!(toml_text.contains("kind = \"author\""));
    assert!(toml_text.contains("kind = \"notify\""));
    let (path2, c2) = db("io-toml");
    assert!(import_file(&c2, &toml_path, Some(false)).unwrap() >= 1);
    let watch = list_rules(&c2).unwrap().into_iter().find(|r| r.name == "Watch").unwrap();
    assert_eq!(watch.origin, Origin::User);
    assert!(watch.system_key.is_none());
    assert_eq!(watch.priority, 50);
    assert_eq!(watch.conditions, vec![Condition::Author { nick: "haz".into() }]);
    assert_eq!(watch.actions, vec![RuleAction::Notify]);
    let (path3, c3) = db("io-json");
    import_file(&c3, &json_path, Some(true)).unwrap();
    let watch = list_rules(&c3).unwrap().into_iter().find(|r| r.name == "Watch").unwrap();
    assert_eq!(watch.conditions, vec![Condition::Author { nick: "haz".into() }]);
    assert_eq!(watch.actions, vec![RuleAction::Notify]);
    let _ = (path2, path3);
}

#[test]
fn clap_accepts_the_rules_flags() {
    use clap::Parser;
    let a = Args::parse_from(["acp-red-star", "--rules-import", "rules.toml", "--rules-format", "toml"]);
    assert!(a.rules_import.is_some());
    assert!(matches!(a.rules_format, Some(RulesFormat::Toml)));
    assert!(Args::try_parse_from(["acp-red-star", "--rules-list", "--search", "x"]).is_err());
    assert!(Args::try_parse_from(["acp-red-star", "--dry-run"]).is_err(), "--dry-run stays on confirmable commands");
}
