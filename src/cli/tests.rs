//! Unit tests for argument parsing, nick lists, names and generators.

use super::*;
use clap::{CommandFactory, Parser};

#[test]
fn nicks_parse() {
    assert_eq!(parse_nicks("@hazaldin,@m3talsmith").unwrap().0, ["hazaldin", "m3talsmith"]);
    assert_eq!(parse_nicks(" @a , b,,@A, @b ,c").unwrap().0, ["a", "b", "c"]);
    assert_eq!(parse_nicks("solo").unwrap().0, ["solo"]);
    assert!(parse_nicks(" , @ ,").is_err());
    assert!(parse_nicks("a b").is_err());
}

#[test]
fn nicks_cli() {
    let a = Args::try_parse_from(["x", "--nicks", "@hazaldin, @m3talsmith,@hazaldin"]).unwrap();
    assert_eq!(a.nick_list().unwrap(), ["hazaldin", "m3talsmith"]);
    let a = Args::try_parse_from(["x", "--nick", "@solo"]).unwrap();
    assert_eq!(a.nick_list().unwrap(), ["solo"]);
    assert!(Args::try_parse_from(["x", "--nick", "a", "--nicks", "b"]).is_err());
    assert!(Args::try_parse_from(["x", "--import", "f", "--nicks", "b"]).is_err());
    assert!(Args::try_parse_from(["x", "--nicks", ","]).is_err());
    assert!(Args::try_parse_from(["x", "--export", "f", "--nicks", "a,b"]).is_ok());
}

#[test]
fn default_names() {
    assert_eq!(nicks_name(&["haz".into(), "m3talsmith".into()], "T"), "haz+m3talsmith-T");
    let many: Vec<String> = (0..5).map(|i| format!("n{i}")).collect();
    assert_eq!(nicks_name(&many, "T"), "multi-T");
}

#[test]
fn list_nicks_flag() {
    assert!(Args::try_parse_from(["x", "--list-nicks"]).is_ok());
    assert!(Args::try_parse_from(["x", "--full"]).is_ok());
}

#[test]
fn identity_flags_parse_and_do_not_open_the_tui() {
    let a = Args::try_parse_from(["x", "--identity-init"]).unwrap();
    assert!(a.identity_init);
    assert!(Args::try_parse_from(["x", "--acp-id"]).unwrap().acp_id);
    assert!(Args::try_parse_from(["x", "--node-init"]).unwrap().node_init);
    assert!(Args::try_parse_from(["x", "--node-id"]).unwrap().node_id);
    assert_eq!(
        Args::try_parse_from(["x", "--node-init", "--node-label", "laptop"]).unwrap().node_label.as_deref(),
        Some("laptop")
    );
    assert!(Args::try_parse_from(["x", "--node-label", "laptop"]).is_err());
    assert!(Args::try_parse_from(["x", "--node-init", "--node-id"]).is_err());
    assert!(Args::try_parse_from(["x", "--node-init", "--scrape"]).is_err());
    assert!(Args::try_parse_from(["x", "--node-id", "--identity-init"]).is_err());
    let m = Args::command().try_get_matches_from(["x", "--node-id"]).unwrap();
    assert!(!wants_tui(&m));
    assert!(Args::try_parse_from(["x", "--identity-init", "--scrape"]).is_err());
    assert!(Args::try_parse_from(["x", "--identity-init", "--acp-id"]).is_err());
    assert!(Args::try_parse_from(["x", "--shares-print"]).unwrap().shares_print);
    assert!(Args::try_parse_from(["x", "--shares-verify"]).unwrap().shares_verify);
    assert!(Args::try_parse_from(["x", "--shares-print", "--shares-verify"]).is_err());
    assert!(Args::try_parse_from(["x", "--shares-file", "s.txt"]).is_err());
    assert!(Args::try_parse_from(["x", "--shares-verify", "--shares-file", "s.txt"]).is_ok());
    assert!(Args::try_parse_from(["x", "--shares-print", "--scrape"]).is_err());
    let m = Args::command().try_get_matches_from(["x", "--acp-id"]).unwrap();
    assert!(!wants_tui(&m));
    let m = Args::command().try_get_matches_from(["x", "--shares-verify"]).unwrap();
    assert!(!wants_tui(&m));
    let a = Args::try_parse_from(["x", "--backup"]).unwrap();
    assert!(a.backup);
    assert!(Args::try_parse_from(["x", "--backup", "--output", "a.acpbackup"]).unwrap().output.unwrap().ends_with("a.acpbackup"));
    assert!(Args::try_parse_from(["x", "--restore", "a.acpbackup"]).is_ok());
    assert!(Args::try_parse_from(["x", "--backup", "--restore", "a.acpbackup"]).is_err());
    assert!(Args::try_parse_from(["x", "--backup-schedule", "weekly"]).is_err());
    let a = Args::try_parse_from(["x", "--backup-schedule", "daily", "--keep", "3", "--dest", "/tmp/b"]).unwrap();
    assert_eq!(a.backup_schedule.as_deref(), Some("daily"));
    assert_eq!(a.backup_keep, Some(3));
    let m = Args::command().try_get_matches_from(["x", "--backup"]).unwrap();
    assert!(!wants_tui(&m));
    assert!(Args::try_parse_from(["x", "--encrypt-db"]).unwrap().encrypt_db);
    assert!(Args::try_parse_from(["x", "--encrypt-db", "--scrape"]).is_err());
    assert!(Args::try_parse_from(["x", "--change-password"]).unwrap().change_password);
    assert!(Args::try_parse_from(["x", "--change-password", "--encrypt-db"]).is_err());
    assert!(Args::try_parse_from(["x", "--change-password", "--backup"]).is_err());
    let m = Args::command().try_get_matches_from(["x", "--change-password"]).unwrap();
    assert!(!wants_tui(&m));
    assert!(Args::try_parse_from(["x", "--show-phrase"]).unwrap().show_phrase);
    assert!(Args::try_parse_from(["x", "--show-phrase", "--encrypt-db"]).is_err());
    assert!(Args::try_parse_from(["x", "--show-phrase", "--identity-init"]).is_err());
    let m = Args::command().try_get_matches_from(["x", "--show-phrase"]).unwrap();
    assert!(!wants_tui(&m));
    let help = Args::command().render_long_help().to_string();
    assert!(help.contains("not the master key"), "{help}");
    assert!(help.contains("--show-phrase"));
    assert!(help.contains("Any 3 of those 5"));
    assert!(help.contains("--identity-init"));
}

#[test]
fn quiet_verbose_conflict() {
    assert!(Args::try_parse_from(["x", "-q", "-v"]).is_err());
    assert!(Args::try_parse_from(["x", "--quiet", "--debug"]).is_err());
    assert!(Args::try_parse_from(["x", "-v", "--debug"]).is_ok());
    assert!(Args::try_parse_from(["x", "-q"]).is_ok());
}

#[test]
fn generators_render() {
    for g in [Generate::Man, Generate::Bash, Generate::Zsh, Generate::Fish] {
        let mut v = Vec::new();
        generate(g, &mut v).unwrap();
        let t = String::from_utf8(v).unwrap().replace("\\-", "-");
        assert!(t.contains("remove-nick"), "{g:?}");
    }
}

#[test]
fn maintenance_flags() {
    let a = Args::try_parse_from(["x", "--remove-nick", "@a, b,@A", "--dry-run", "-y", "--keep-replies"]).unwrap();
    assert_eq!(a.remove_nick.unwrap().0, vec!["a", "b"]);
    assert!(a.dry_run && a.yes && a.keep_replies);
    assert!(Args::try_parse_from(["x", "--clear-all", "--yes"]).unwrap().clear_all);
    for bad in [
        &["x", "--remove-nick", "a", "--clear-all"][..],
        &["x", "--remove-nick", "a", "--nick", "b"],
        &["x", "--remove-nick", "a", "--export", "f.json"],
        &["x", "--clear-all", "--import", "f.json"],
        &["x", "--clear-all", "--search", "q"],
        &["x", "--clear-all", "--keep-replies"],
        &["x", "--dry-run"],
        &["x", "--yes"],
    ] {
        assert!(Args::try_parse_from(bad).is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn feed_mode_flags() {
    let a = Args::try_parse_from(["x"]).unwrap();
    assert_eq!(a.feed_mode(), FeedMode::Latest);
    let a = Args::try_parse_from(["x", "--latest"]).unwrap();
    assert_eq!(a.feed_mode(), FeedMode::Latest);
    let a = Args::try_parse_from(["x", "--trending"]).unwrap();
    assert_eq!(a.feed_mode(), FeedMode::Trending);
    assert!(Args::try_parse_from(["x", "--latest", "--trending"]).is_err());
    let a = Args::try_parse_from(["x", "--top"]).unwrap();
    assert_eq!(a.top, Some(TopPeriod::Week));
    assert!(!a.top_replies);
    let a = Args::try_parse_from(["x", "--top", "all", "--top-replies"]).unwrap();
    assert_eq!((a.top, a.top_replies), (Some(TopPeriod::All), true));
    assert!(Args::try_parse_from(["x", "--top", "hour"]).is_err());
    assert!(Args::try_parse_from(["x", "--top", "week", "--nick", "alice"]).is_err());
    assert!(!parse_with_mode(["x", "--top", "day"]).unwrap().1);
    let a = Args::try_parse_from(["x", "--eb", "--eb-replies"]).unwrap();
    assert!(a.eb && a.eb_replies);
    assert!(Args::try_parse_from(["x", "--eb", "--top"]).is_err());
    assert!(Args::try_parse_from(["x", "--eb-replies"]).is_err());
    assert!(!parse_with_mode(["x", "--eb"]).unwrap().1);
    assert!(Args::try_parse_from(["x", "--explore"]).unwrap().explore);
    assert!(Args::try_parse_from(["x", "--explore-latest"]).unwrap().explore_latest);
    assert!(Args::try_parse_from(["x", "--explore", "--explore-latest"]).is_err());
    assert_eq!(Args::try_parse_from(["x", "--site-search", "strike"]).unwrap().site_search.as_deref(), Some("strike"));
    assert!(Args::try_parse_from(["x", "--site-search", "strike", "--search", "x"]).is_err());
    assert_eq!(Args::try_parse_from(["x", "--tag", "strike"]).unwrap().tag.as_deref(), Some("strike"));
    assert!(Args::try_parse_from(["x", "--tag", "strike", "--top"]).is_err());
    assert!(Args::try_parse_from(["x", "--tag", "strike", "--nick", "alice"]).is_err());
    let a = Args::try_parse_from(["x", "--backfill-tags", "--nick", "alice"]).unwrap();
    assert!(a.backfill_tags && a.nick.as_deref() == Some("alice"));
    assert!(Args::try_parse_from(["x", "--backfill-tags", "--tag", "strike"]).is_err());
    assert!(!parse_with_mode(["x", "--tag", "strike"]).unwrap().1);
    let a = Args::try_parse_from(["x", "--sort-thread", "T1", "--thread-sort", "top"]).unwrap();
    assert_eq!((a.sort_thread.as_deref(), a.thread_sort.as_deref()), (Some("T1"), Some("top")));
    assert!(Args::try_parse_from(["x", "--sort-thread", "T1"]).is_err());
    assert!(Args::try_parse_from(["x", "--thread-sort", "oldest"]).is_err());
    assert!(Args::try_parse_from(["x", "--sort-thread", "T1", "--thread-sort", "newest"]).is_err());
    assert!(Args::try_parse_from(["x", "--sort-thread", "T1", "--thread-sort", "oldest", "--tag", "x"]).is_err());
    assert_eq!(Args::try_parse_from(["x", "--reposts", "T1"]).unwrap().reposts.as_deref(), Some("T1"));
    assert!(Args::try_parse_from(["x", "--reposts", "T1", "--tag", "x"]).is_err());
    assert_eq!(Args::try_parse_from(["x", "--edits", "T1/P1"]).unwrap().edits.as_deref(), Some("T1/P1"));
    assert!(Args::try_parse_from(["x", "--edits", "T1/P1", "--reposts", "T1"]).is_err());
    assert!(!parse_with_mode(["x", "--edits", "P1"]).unwrap().1);
    assert_eq!(FeedMode::Latest.site_feed_url(), feed_latest_url());
    assert_eq!(FeedMode::Trending.site_feed_url(), feed_trending_url());
    assert!(FeedMode::Latest.allow_incremental_stop());
    assert!(!FeedMode::Trending.allow_incremental_stop());
}

#[test]
fn multiple_formats() {
    use crate::export::Format::{Html, Json};
    assert_eq!(Args::try_parse_from(["x"]).unwrap().formats(), [Json]);
    assert_eq!(Args::try_parse_from(["x", "--format", "html"]).unwrap().formats(), [Html]);
    assert_eq!(Args::try_parse_from(["x", "--format", "json,html"]).unwrap().formats(), [Json, Html]);
    assert_eq!(Args::try_parse_from(["x", "--format=html,json"]).unwrap().formats(), [Html, Json]);
    assert_eq!(Args::try_parse_from(["x", "--format", "html", "--format", "json", "--format", "html"]).unwrap().formats(), [Html, Json]);
    assert!(Args::try_parse_from(["x", "--format", "json,pdf"]).is_err());
}
