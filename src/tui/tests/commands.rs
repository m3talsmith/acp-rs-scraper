use super::*;
use crate::tui::commands::*;
use crate::tui::data::SearchFilter;
use crate::tui::state::Effect;

fn parsed(argv: &[String]) -> Args { parse_with_mode(argv).unwrap().0 }

#[test]
fn bare_and_logging_only_open_tui_flags_do_not() {
    let t = |v: &[&str]| { let mut a = vec!["x"]; a.extend(v); parse_with_mode(a).unwrap().1 };
    assert!(t(&[]));
    assert!(t(&["-v"]));
    assert!(t(&["--debug", "--enable-logging"]));
    assert!(t(&["--tui", "--nick", "a"]));
    assert!(!t(&["--nick", "a"]));
    assert!(!t(&["--scrape"]));
    assert!(!t(&["--no-mentions"]));
    assert!(!t(&["--search", "x"]));
    assert!(!t(&["--list-nicks"]));
    assert!(!t(&["--logout"]));
    assert!(!t(&["--trending"]));
}

#[test]
fn scrape_defaults_map_to_cli_defaults() {
    let m = Menu::new(&no_args());
    let argv = m.scrape_argv(&["alice".into()]).unwrap();
    assert_eq!(argv, ["acp-red-star", "--nicks", "alice", "--latest"]);
    let a = parsed(&argv);
    assert_eq!(a.nick_list().unwrap(), ["alice"]);
    assert!(a.mentions && !a.said_by && !a.full && !a.no_attachments && !a.trending);
    assert_eq!(a.crawl_limit, DEFAULT_CRAWL_LIMIT);
    assert_eq!(a.limit, None);
    assert_eq!(a.profile_feed, ProfileFeed::Newest);
    assert_eq!(a.order, ScrapeOrder::Newest);
}

#[test]
fn scrape_every_option_maps_to_its_flag() {
    let mut m = Menu::new(&no_args());
    use CommandId::Scrape as S;
    m.set_choice(S, "mode", "Trending");
    m.set_text(S, "nicks", "@a, b");
    m.set_check(S, "mentions", false);
    m.set_check(S, "said_by", true);
    m.set_choice(S, "profile_feed", "Top");
    m.set_choice(S, "order", "Oldest");
    m.set_text(S, "limit", "100");
    m.set_text(S, "crawl_limit", "200");
    m.set_check(S, "attachments", false);
    m.set_check(S, "full", true);
    m.set_check(S, "refetch", true);
    m.set_text(S, "output", "/tmp/out.json");
    m.set_choice(S, "format", "json,html");
    m.set_check(CommandId::Debug, "verbose", true);
    m.set_check(CommandId::Debug, "logging", true);
    let argv = m.scrape_argv(&["ignored".into()]).unwrap();
    let a = parsed(&argv);
    assert_eq!(a.nick_list().unwrap(), ["a", "b"], "the Nicks field wins over the selection");
    assert!(a.trending && !a.mentions && a.said_by && a.no_attachments && a.full && a.refetch_truncated);
    assert_eq!(a.crawl_limit, 200);
    assert_eq!(a.limit, Some(100));
    assert_eq!(a.profile_feed, ProfileFeed::Top);
    assert_eq!(a.order, ScrapeOrder::Oldest);
    assert_eq!(a.output.as_deref(), Some(Path::new("/tmp/out.json")));
    assert_eq!(a.formats(), [Format::Json, Format::Html]);
    assert!(a.verbose && a.enable_logging && !a.debug);
}

#[test]
fn scrape_errors() {
    let mut m = Menu::new(&no_args());
    assert!(m.scrape_argv(&[]).unwrap_err().contains("at least one nick"));
    m.set_text(CommandId::Scrape, "crawl_limit", "lots");
    assert!(m.scrape_argv(&["a".into()]).unwrap_err().contains("Crawl limit"));
    m.set_text(CommandId::Scrape, "crawl_limit", "");
    m.set_text(CommandId::Scrape, "limit", "0");
    assert!(m.scrape_argv(&["a".into()]).unwrap_err().contains("Post limit"));
    m.set_text(CommandId::Scrape, "limit", "");
    m.set_text(CommandId::Scrape, "nicks", "a b");
    assert!(m.scrape_argv(&[]).is_err());
}

#[test]
fn whole_site_scrape_and_sync_map_to_scrape_all() {
    let mut m = Menu::new(&no_args());
    m.set_check(CommandId::Scrape, "whole_site", true);
    m.set_text(CommandId::Scrape, "budget", "90");
    m.set_text(CommandId::Scrape, "limit", "10");
    let argv = m.scrape_argv(&[]).unwrap();
    let a = parsed(&argv);
    assert!(a.scrape_all && !a.full);
    assert_eq!(a.scrape_budget, Some(90));
    assert_eq!(a.limit, Some(10));
    assert!(a.nick_list().unwrap().is_empty());
    m.set_text(CommandId::Scrape, "budget", "0");
    assert!(m.scrape_argv(&[]).unwrap_err().contains("at least 1"));
    m.set_check(CommandId::Scrape, "whole_site", false);
    m.set_text(CommandId::Scrape, "budget", "");
    m.set_text(CommandId::Scrape, "limit", "");
    m.set_check(CommandId::Sync, "whole_site", true);
    m.set_check(CommandId::Sync, "bookmarks", false);
    let sync = m.sync_argv(&[]).unwrap();
    let s = parsed(&sync);
    assert!(s.sync && s.scrape_all && s.no_bookmarks);
}

#[test]
fn export_bulk_from_selection_view_or_everything() {
    let mut m = Menu::new(&no_args());
    use CommandId::Export as E;
    assert!(m.export_argv(&[], None).unwrap_err().contains("Path"));
    m.set_text(E, "path", "/tmp/sel.tar.gz");
    assert!(m.export_argv(&[], None).unwrap_err().contains("no nick is selected"));
    m.set_choice(E, "format", "json,html");
    m.set_check(E, "compress", true);
    m.set_text(E, "name", "sel");
    let a = parsed(&m.export_argv(&["alice".into(), "bob".into()], None).unwrap());
    assert_eq!(a.export.as_deref(), Some(Path::new("/tmp/sel.tar.gz")));
    assert_eq!(a.nick_list().unwrap(), ["alice", "bob"]);
    assert!(a.compress && a.tar_gz);
    assert_eq!(a.name.as_deref(), Some("sel"));
    m.set_choice(E, "scope", "Current view");
    let f = SearchFilter { keyword: "strike".into(), nicks: vec!["bob".into()], ..Default::default() };
    let a = parsed(&m.export_argv(&["alice".into()], Some(&f)).unwrap());
    assert_eq!(a.search.as_deref(), Some("strike"));
    assert_eq!(a.nick_list().unwrap(), ["bob"]);
    m.set_choice(E, "scope", "Everything");
    let a = parsed(&m.export_argv(&["alice".into()], None).unwrap());
    assert!(a.nick_list().unwrap().is_empty() && a.search.is_none());
}

#[test]
fn import_remove_clear_argv() {
    let mut m = Menu::new(&no_args());
    assert!(m.import_argv().is_err());
    m.set_text(CommandId::Import, "path", "/tmp/a.tar.gz");
    m.set_check(CommandId::Import, "compress", true);
    let a = parsed(&m.import_argv().unwrap());
    assert!(a.import.is_some() && a.compress && a.tar_gz);

    assert!(m.remove_argv(&[]).is_err());
    m.set_check(CommandId::Maintain, "keep_replies", true);
    m.set_check(CommandId::Maintain, "dry_run", true);
    let a = parsed(&m.remove_argv(&["bob".into()]).unwrap());
    assert_eq!(a.remove_nick.unwrap().0, ["bob"]);
    assert!(a.keep_replies && a.dry_run && a.yes);
    let a = parsed(&m.clear_all_argv().unwrap());
    assert!(a.clear_all && a.yes && a.dry_run);
}

#[test]
fn debug_menu_seeded_from_args() {
    use clap::Parser;
    let m = Menu::new(&Args::parse_from(["x", "--debug"]));
    assert_eq!(m.debug_args(), ["--debug", "--enable-logging"].map(String::from)[..1].to_vec());
    let m = Menu::new(&Args::parse_from(["x", "-v", "--enable-logging"]));
    assert_eq!(m.debug_args(), ["-v", "--enable-logging"]);
}

#[test]
fn bar_categories_pin_scrape_and_tuck_library_children() {
    use CommandId::*;
    let m = Menu::new(&no_args());
    let labels: Vec<_> = m.bar_indices().iter().map(|&i| m.items[i].label).collect();
    assert_eq!(labels, ["View", "Search", "Compose", "Library", "Bookmarks", "Research", "Sync", "Discover", "Account", "Debug", "Help", "About", "Quit", "Scrape"]);
    let tucked: Vec<_> = m.items.iter().filter(|c| !c.on_bar).map(|c| c.id).collect();
    assert_eq!(tucked, [Export, Import, Basket, List, Maintain]);
    assert!(m.get(Scrape).pin_right);
    assert_eq!(m.bar_by_hint('x'), None);
    assert_eq!(m.bar_by_hint('S').map(|i| m.items[i].id), Some(Scrape));
    assert!(m.get(Export).opts.iter().all(|o| o.key != "basket_add"));
    assert!(m.get(Basket).opts.iter().any(|o| o.key == "basket_add"));
}

#[test]
fn account_security_queues_the_recovery_screen() {
    let m = Menu::new(&no_args());
    let security = m.get(CommandId::Session).opts.iter().find(|o| o.key == "security").unwrap();
    assert_eq!(security.label, "Security");
    assert!(security.flag.contains("--show-recovery"));
    let mut a = app();
    a.act(Action::ShowRecovery);
    assert!(matches!(a.effects.as_slice(), [Effect::ShowRecovery]));
    assert!(a.status.is_empty());
}

#[test]
fn every_menu_item_has_a_hint_and_flags_exist() {
    use clap::CommandFactory;
    let m = Menu::new(&no_args());
    let hints: HashSet<char> = m.items.iter().map(|i| i.hint).collect();
    assert_eq!(hints.len(), m.items.len(), "hint letters are unique");
    let longs: HashSet<String> = Args::command().get_arguments().filter_map(|a| a.get_long().map(|l| format!("--{l}"))).collect();
    for i in &m.items {
        for o in &i.opts {
            for f in o.flag.split([' ', '/']).filter(|f| f.starts_with("--")) {
                assert!(longs.contains(f), "{} › {} maps to unknown flag {f}", i.label, o.label);
            }
        }
    }
}

#[test]
fn export_import_and_output_paths_expand_a_tilde() {
    let mut m = Menu::new(&no_args());
    let home = std::env::var("HOME").expect("HOME");
    m.set_text(CommandId::Export, "path", "~/out.json");
    m.set_choice(CommandId::Export, "scope", "Everything");
    let a = parsed(&m.export_argv(&[], None).unwrap());
    assert_eq!(a.export.as_deref(), Some(Path::new(&format!("{home}/out.json"))));
    m.set_text(CommandId::Export, "path", "~/resonance-cosmos.tar.gz");
    let a = parsed(&m.export_argv(&[], None).unwrap());
    assert_eq!(a.export.as_deref(), Some(Path::new(&format!("{home}/resonance-cosmos.tar.gz"))));
    assert!(!a.export.as_deref().unwrap().to_string_lossy().contains("/~/"), "a leading ~ is not a relative directory");
    m.set_text(CommandId::Import, "path", "~/in.tar.gz");
    let a = parsed(&m.import_argv().unwrap());
    assert_eq!(a.import.as_deref(), Some(Path::new(&format!("{home}/in.tar.gz"))));
    m.set_text(CommandId::Research, "export_path", "~/proj.json");
    let a = parsed(&m.research_export_argv("strike").unwrap());
    assert_eq!(a.export.as_deref(), Some(Path::new(&format!("{home}/proj.json"))));
    m.set_text(CommandId::Scrape, "output", "~/scan.html");
    let argv = m.scrape_argv(&["alice".into()]).unwrap();
    assert!(argv.iter().any(|s| s == &format!("{home}/scan.html")), "{argv:?}");
}

#[test]
fn tab_completes_an_export_path() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let dir = std::env::temp_dir().join(format!("acp-export-path-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.json"), b"x").unwrap();
    let mut a = app();
    let export = a.menu.menu.items.iter().position(|i| i.id == CommandId::Export).unwrap();
    a.open_menu(export);
    let path = a.menu.menu.items[export].opts.iter().position(|o| o.key == "path").unwrap();
    a.menu.opt = path;
    a.menu.editing = true;
    a.menu.menu.set_text(CommandId::Export, "path", &format!("{}/n", dir.display()));
    crate::tui::input::on_key(&mut a, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(a.menu.menu.text(CommandId::Export, "path"), format!("{}/notes.json", dir.display()));
    assert!(a.menu.editing, "a completed path stays in the field");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shell_line_quotes() {
    assert_eq!(shell_line(&["a".into(), "b c".into(), "it's".into()]), "a 'b c' 'it'\\''s'");
}
