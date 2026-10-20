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
fn site_attachments_are_one_based_and_capped_at_four() {
    let a = Args::try_parse_from(["x", "--new-post", "Title", "--site-attachments", "1,3,5,6"]).unwrap();
    assert_eq!(a.site_attachments.as_deref(), Some(&[0usize, 2, 4, 5][..]));
    let a = Args::try_parse_from(["x", "--reply", "T", "--site-attachments", " 1, 2 "]).unwrap();
    assert_eq!(a.site_attachments.as_deref(), Some(&[0usize, 1][..]));
    assert!(Args::try_parse_from(["x", "--new-post", "Title", "--site-attachments", "0"]).is_err());
    assert!(Args::try_parse_from(["x", "--new-post", "Title", "--site-attachments", "1,2,3,4,5"]).is_err());
    assert!(Args::try_parse_from(["x", "--new-post", "Title", "--site-attachments", "1,1"]).is_err());
    assert!(Args::try_parse_from(["x", "--new-post", "Title", "--site-attachments", "no"]).is_err());
    assert!(Args::try_parse_from(["x", "--new-post", "Title", "--site-attachments", ""]).is_err());
    assert!(Args::try_parse_from(["x", "--new-post", "Title", "--site-attachments", "1,,2"]).is_err());
    let help = Args::command().render_long_help().to_string();
    assert!(help.contains("--site-attachments"), "{help}");
    assert!(help.contains("1,3,5,6"), "{help}");
    assert!(!parse_with_mode(["x", "--site-attachments", "1"]).unwrap().1);
}

#[test]
fn post_limit_is_a_positive_nick_scrape_flag() {
    let a = Args::try_parse_from(["x", "--nick", "alice", "--limit", "100"]).unwrap();
    assert_eq!(a.limit, Some(100));
    assert_eq!(a.crawl_limit, crate::config::DEFAULT_CRAWL_LIMIT);
    assert!(Args::try_parse_from(["x", "--nick", "alice", "--limit", "0"]).is_err());
    assert!(Args::try_parse_from(["x", "--nick", "alice", "--limit", "no"]).is_err());
    assert!(Args::try_parse_from(["x", "--import", "f", "--limit", "10"]).is_err());
    let a = Args::try_parse_from(["x", "--nick", "alice"]).unwrap();
    assert_eq!(a.limit, None);
    assert_eq!(a.profile_feed, ProfileFeed::Newest);
    assert_eq!(a.order, ScrapeOrder::Newest);
}

#[test]
fn profile_feed_and_order_are_nick_scrape_flags() {
    let a = Args::try_parse_from(["x", "--nick", "alice", "--profile-feed", "top", "--order", "oldest", "--limit", "100"]).unwrap();
    assert_eq!(a.profile_feed, ProfileFeed::Top);
    assert_eq!(a.order, ScrapeOrder::Oldest);
    assert_eq!(a.limit, Some(100));
    assert_eq!(a.feed_mode(), FeedMode::Latest);
    let a = Args::try_parse_from(["x", "--nick", "alice", "--profile-feed", "Newest", "--order", "OLDEST"]).unwrap();
    assert_eq!((a.profile_feed, a.order), (ProfileFeed::Newest, ScrapeOrder::Oldest));
    assert!(Args::try_parse_from(["x", "--nick", "alice", "--profile-feed", "trending"]).is_err());
    assert!(Args::try_parse_from(["x", "--nick", "alice", "--order", "top"]).is_err());
    assert!(Args::try_parse_from(["x", "--import", "f", "--profile-feed", "top"]).is_err());
    assert!(Args::try_parse_from(["x", "--import", "f", "--order", "oldest"]).is_err());
    assert!(!parse_with_mode(["x", "--profile-feed", "top"]).unwrap().1);
    assert!(!parse_with_mode(["x", "--order", "oldest"]).unwrap().1);
}

#[test]
fn color_flag_parses_and_resolution_follows_the_flag_then_the_preference() {
    assert_eq!(ColorMode::parse("Always").unwrap(), ColorMode::Always);
    assert_eq!(ColorMode::parse("NEVER").unwrap(), ColorMode::Never);
    assert!(ColorMode::parse("on").is_err());
    let a = Args::try_parse_from(["x", "--color", "always"]).unwrap();
    assert_eq!(a.color, Some(ColorMode::Always));
    assert!(parse_with_mode(["x", "--color", "never"]).unwrap().1, "--color alone still opens the terminal UI");
    assert!(Args::try_parse_from(["x", "--color", "sometimes"]).is_err());
    assert_eq!(Args::try_parse_from(["x"]).unwrap().color, None);
    assert_eq!(resolve_color_mode(Some(ColorMode::Never), Some("always")), ColorMode::Never);
    assert_eq!(resolve_color_mode(None, Some("always")), ColorMode::Always);
    assert_eq!(resolve_color_mode(None, Some("nope")), ColorMode::Auto);
    assert_eq!(resolve_color_mode(None, None), ColorMode::Auto);
    assert!(!colors_suppressed(ColorMode::Always, true));
    assert!(colors_suppressed(ColorMode::Never, false));
    assert!(colors_suppressed(ColorMode::Auto, true));
    assert!(!colors_suppressed(ColorMode::Auto, false));
    assert_eq!(no_color_hint(ColorMode::Auto, true), Some(NO_COLOR_HINT));
    assert_eq!(no_color_hint(ColorMode::Always, true), None);
    assert_eq!(no_color_hint(ColorMode::Never, false), None);
    assert!(no_color_hint(ColorMode::Never, true).unwrap().contains("--color always"));
    apply_color_mode(ColorMode::Never);
    assert!(crossterm::style::Colored::ansi_color_disabled_memoized());
    apply_color_mode(ColorMode::Always);
    assert!(!crossterm::style::Colored::ansi_color_disabled_memoized());
    apply_color_mode(ColorMode::Auto);
    assert_eq!(current_color_mode(), ColorMode::Auto);
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
    for g in [Generate::AcprsMan, Generate::AcprsBash, Generate::AcprsZsh, Generate::AcprsFish] {
        let mut v = Vec::new();
        generate(g, &mut v).unwrap();
        let t = String::from_utf8(v).unwrap().replace("\\-", "-");
        assert!(t.contains("acprs"), "{g:?}: {t}");
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
fn write_flags_are_headless_and_exclusive() {
    let a = Args::try_parse_from(["x", "--new-post", "Title", "--body", "hello", "--dry-run"]).unwrap();
    assert_eq!(a.new_post.as_deref(), Some("Title"));
    assert!(a.dry_run && !a.yes);
    assert!(!parse_with_mode(["x", "--new-post", "Title"]).unwrap().1);
    assert!(Args::try_parse_from(["x", "--new-post", "Title", "--reply", "T1"]).is_err());
    assert!(Args::try_parse_from(["x", "--reply", "T1", "--tag", "strike"]).is_err());
    assert!(Args::try_parse_from(["x", "--in-reply-to", "C1"]).is_err());
    let a = Args::try_parse_from(["x", "--reply", "T1", "--in-reply-to", "C1", "--quote", "lines", "--yes"]).unwrap();
    assert_eq!((a.reply.as_deref(), a.in_reply_to.as_deref(), a.quote.as_deref()), (Some("T1"), Some("C1"), Some("lines")));
    assert!(a.yes);
    let help = Args::command().render_long_help().to_string();
    assert!(help.contains("--new-post") && help.contains("--dry-run"));
    assert!(!help.contains("--capture-reply"), "the capture flag stays in the architecture notes");
    assert!(!help.contains("--roar"));
    let (args, tui) = parse_with_mode(["x", "--roar"]).unwrap();
    assert!(args.roar && !tui);
    let (args, tui) = parse_with_mode(["x", "--roar", "--tui"]).unwrap();
    assert!(args.roar && tui);
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
