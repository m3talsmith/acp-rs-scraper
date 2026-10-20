//! Theme files: the parser, colours, fallbacks, and save / load / download /
//! migration in temporary folders (never the real ~/.acp).

use super::*;
use ratatui::style::Color;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("acp-themes-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn repo_default() -> &'static str { include_str!("../../themes/default.toml") }

#[test]
fn colours_in_every_notation() {
    let c = |s: &str| parse_color_str(s).unwrap();
    assert_eq!(c("#dc2626"), ColorSpec::Value(Color::Rgb(0xdc, 0x26, 0x26)));
    assert_eq!(c("#F0a"), ColorSpec::Value(Color::Rgb(0xff, 0x00, 0xaa)));
    assert_eq!(c("208"), ColorSpec::Value(Color::Indexed(208)));
    assert_eq!(parse_color(&toml::Value::Integer(17)).unwrap(), ColorSpec::Value(Color::Indexed(17)));
    assert_eq!(c("red"), ColorSpec::Value(Color::Red));
    assert_eq!(c("white"), ColorSpec::Value(Color::Gray), "ANSI white is colour 7");
    assert_eq!(c("bright-white"), ColorSpec::Value(Color::White));
    assert_eq!(c("Bright_Black"), ColorSpec::Value(Color::DarkGray));
    assert_eq!(c("light_blue"), ColorSpec::Value(Color::LightBlue));
    assert_eq!(c("reset"), ColorSpec::Value(Color::Reset));
    assert_eq!(c("@accent"), ColorSpec::Ref("accent".into()));
    for bad in ["#12", "#gggggg", "256", "chartreuse", "@../x"] { assert!(parse_color_str(bad).is_err(), "{bad}"); }
    assert!(parse_color(&toml::Value::Integer(300)).is_err());
    assert!(parse_color(&toml::Value::Boolean(true)).is_err());
    assert_eq!(ansi_colors().len(), 16);
    for s in ["#dc2626", "208", "bright_cyan", "reset", "@gold", "white"] { assert_eq!(color_text(&c(s)), s); }
}

#[test]
fn the_shipped_default_parses_cleanly_and_round_trips() {
    let (f, w) = parse(repo_default(), "default").unwrap();
    assert!(w.is_empty(), "{w:?}");
    assert_eq!((f.version, f.name.as_str()), (1, "default"));
    assert_eq!(f.dark.color("accent"), Color::Rgb(0xdc, 0x26, 0x26));
    let again = parse(&to_toml(&f), "default").unwrap();
    assert_eq!(again.0, f);
    assert!(again.1.is_empty());
}

#[test]
fn unknown_keys_warn_one_by_one_and_missing_ones_fall_back() {
    let src = r##"version = 1
name = "x"
colour = "red"
[dark]
pallete = 1
[dark.palette]
accent = "#ff0000"
muted = "chartreuse"
"Bad Name" = "#000000"
[dark.styles.link]
fg = "@nope"
glow = true
bold = "yes"
[dark.styles.sparkles]
fg = "red"
[dark.styles.text]
border = "red"
[light.palette]
accent = "@accent2"
accent2 = "#0000ff"
"##;
    let (f, w) = parse(src, "x").unwrap();
    let has = |s: &str| assert!(w.iter().any(|x| x.contains(s)), "no warning about {s}: {w:#?}");
    for s in ["\"colour\"", "\"pallete\"", "muted: unknown colour", "\"Bad Name\"", "@nope", "\"glow\"", "bold: expected true or false", "\"sparkles\"", "text has no border"] { has(s); }
    assert_eq!(f.dark.color("accent"), Color::Rgb(255, 0, 0));
    assert_eq!(f.dark.color("muted"), Color::Reset, "skipped: the terminal's colour");
    assert_eq!(f.section(Variant::Light).color("accent"), Color::Rgb(0, 0, 255), "a reference through the palette");
    assert_eq!(f.section(Variant::Light).color("border"), Color::Reset);
}

#[test]
fn light_falls_back_to_dark_and_loops_resolve_to_reset() {
    let (f, w) = parse("version = 1\n[dark.palette]\naccent = \"#111111\"\na = \"@b\"\nb = \"@a\"\n", "x").unwrap();
    assert_eq!(f.section(Variant::Light).color("accent"), Color::Rgb(17, 17, 17));
    assert_eq!(f.dark.color("a"), Color::Reset);
    assert!(w.iter().any(|x| x.contains("loops")), "{w:?}");
}

#[test]
fn broken_newer_and_oversized_files_are_refused() {
    assert!(parse("version = [", "x").unwrap_err().contains("not valid TOML"));
    assert!(parse("version = 2\n", "x").unwrap_err().contains("newer"));
    assert!(parse("version = \"one\"\n", "x").is_err());
    let (_, w) = parse("[dark.palette]\naccent = \"red\"\n", "x").unwrap();
    assert!(w[0].contains("no version"));
    let d = tmp("big");
    std::fs::write(d.join("big.toml"), format!("version = 1\n#{}\n", "x".repeat(THEME_MAX_BYTES as usize))).unwrap();
    let e = load(&d, "big").unwrap_err();
    assert!(matches!(&e, LoadError::Broken(_, m) if m.contains("at most 64 KiB")), "{e:?}");
    std::fs::write(d.join("bad.toml"), "version = [").unwrap();
    let n = load(&d, "bad").unwrap_err().notice();
    assert!(n.contains("bad.toml") && n.contains("fix the file") && n.contains("never overwritten"), "{n}");
    let m = load(&d, "gone").unwrap_err();
    assert!(matches!(m, LoadError::Missing(_)) && m.notice().contains("gone.toml is missing"));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn names_cannot_escape_the_themes_folder() {
    let long_ok = "x".repeat(32);
    let long_bad = "x".repeat(33);
    for ok in ["default", "red-star", "a_1", long_ok.as_str()] { assert!(valid_name(ok), "{ok}"); }
    for bad in ["", "../etc", "a/b", "A", "x.toml", "with space", ".hidden", long_bad.as_str()] {
        assert!(!valid_name(bad), "{bad}");
        assert!(matches!(load(Path::new("/tmp"), bad), Err(LoadError::BadName(_))));
    }
}

#[test]
fn save_is_atomic_private_and_lists_back() {
    let d = tmp("save").join("themes");
    let (mut f, _) = parse(repo_default(), "default").unwrap();
    f.name = "mine".into();
    f.description = "Mine".into();
    let p = save(&d, &f).unwrap();
    assert_eq!(p, d.join("mine.toml"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&d).unwrap().permissions().mode() & 0o777, 0o700);
    }
    assert_eq!(load(&d, "mine").unwrap().file, f);
    std::fs::write(d.join("broken.toml"), "nope = [").unwrap();
    std::fs::write(d.join("Not-Valid.toml"), "").unwrap();
    let l = list(&d);
    assert_eq!(l.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["broken", "mine"]);
    assert!(l[0].error.is_some() && l[1].description == "Mine");
    assert!(std::fs::read_dir(&d).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")), "no temp files left");
    delete(&d, "mine").unwrap();
    assert!(!p.exists());
    assert!(!atomic_write_new(&d.join("broken.toml"), b"x").unwrap(), "never replaces a file");
    assert_eq!(std::fs::read_to_string(d.join("broken.toml")).unwrap(), "nope = [");
    let _ = std::fs::remove_dir_all(d.parent().unwrap());
}

#[test]
fn old_theme_prefs_become_a_custom_theme_file_once() {
    let d = tmp("migrate");
    std::fs::write(d.join("default.toml"), repo_default()).unwrap();
    let old = ThemePrefs { accent: Some("#00ff00".into()), muted: Some("oops".into()), ..Default::default() };
    assert!(legacy_set(&old) && !legacy_set(&ThemePrefs::default()));
    let (name, warn) = migrate_legacy(&d, &old).unwrap();
    assert_eq!(name, "custom");
    assert!(warn[0].contains("muted"));
    let l = load(&d, "custom").unwrap();
    assert_eq!(l.file.dark.color("accent"), Color::Rgb(0, 255, 0));
    assert_eq!(l.file.section(Variant::Light).color("accent"), Color::Rgb(0, 255, 0));
    assert_eq!(l.file.dark.color("muted"), Color::Rgb(0x9c, 0xa3, 0xaf), "the rest comes from the default");
    assert_eq!(migrate_legacy(&d, &old).unwrap().0, "custom-2", "an existing custom.toml is never replaced");
    // prepare() converts only while [ui] theme is unset.
    let d2 = tmp("prepare");
    std::fs::write(d2.join("default.toml"), repo_default()).unwrap();
    let mut prefs = Prefs { theme: Some(old.clone()), ..Default::default() };
    let (m, notes) = prepare(&d2, &prefs);
    assert_eq!(m.as_deref(), Some("custom"));
    assert!(notes.iter().any(|n| n.contains("no longer used")), "{notes:?}");
    prefs.ui = Some(UiPrefs { theme: Some("custom".into()), ..Default::default() });
    assert_eq!(prepare(&d2, &prefs).0, None);
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_dir_all(d2);
}

#[test]
fn mode_and_terminal_background() {
    assert_eq!(Mode::parse(Some("Light")), Mode::Light);
    assert_eq!(Mode::parse(Some("dark")), Mode::Dark);
    assert_eq!(Mode::parse(None), Mode::System);
    assert_eq!(Mode::System.variant(None), Variant::Dark, "dark when unknown");
    assert_eq!(Mode::System.variant(Some(false)), Variant::Light);
    assert_eq!(Mode::Dark.variant(Some(false)), Variant::Dark);
    assert_eq!(colorfgbg_dark(Some("15;0")), Some(true));
    assert_eq!(colorfgbg_dark(Some("0;default;15")), Some(false));
    assert_eq!(colorfgbg_dark(Some("junk")), None);
    assert_eq!(osc11_dark("\x1b]11;rgb:ffff/ffff/f0f0\x1b\\"), Some(false));
    assert_eq!(osc11_dark("\x1b]11;rgb:1c1c/1c1c/1c1c\x07\x1b[?62c"), Some(true));
    assert_eq!(osc11_dark("\x1b[?62c"), None);
}

#[test]
fn contrast_ratios() {
    assert!((contrast(Color::Black, Color::White).unwrap() - 21.0).abs() < 0.01);
    assert!((contrast(Color::Rgb(0xdc, 0x26, 0x26), Color::Rgb(255, 255, 255)).unwrap() - 4.83).abs() < 0.05);
    assert_eq!(contrast(Color::Reset, Color::White), None);
    assert_eq!(rgb(Color::Indexed(196)), Some((255, 0, 0)));
    assert_eq!(rgb(Color::Indexed(232)), Some((8, 8, 8)));
}

#[test]
fn acp_is_the_site_palette_and_body_text_meets_aa() {
    use ratatui::style::Color;
    let src = include_str!("../../themes/acp.toml");
    let (f, w) = parse(src, "acp").unwrap();
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(f.name, "acp");
    assert_eq!(f.description, "American Communist Party: ACP red, black and white on blush pink");
    assert!(SHIPPED_THEMES.contains(&"acp"));
    assert!(!src.contains("gold") && !src.contains("yellow"), "the site has no gold");
    let poster = parse(include_str!("../../themes/chinese-poster.toml"), "chinese-poster").unwrap().0;
    for side in ["dark", "light"] {
        let want = if side == "dark" { &poster.dark.styles } else { &poster.light.as_ref().unwrap().styles };
        let got = if side == "dark" { &f.dark.styles } else { &f.light.as_ref().unwrap().styles };
        for (name, _) in want {
            assert!(got.iter().any(|(n, _)| n == name), "{side} is missing [{name}]");
        }
    }
    let ratio = |a, b| contrast(a, b).unwrap();
    let dark = crate::tui::theme::Theme::from_file(&f, Variant::Dark);
    let light = crate::tui::theme::Theme::from_file(&f, Variant::Light);
    assert_eq!(light.bg, Color::Rgb(0xff, 0xf1, 0xf1));
    assert_eq!(light.fg, Color::Rgb(0x00, 0x00, 0x00));
    assert_eq!(light.muted, Color::Rgb(0x70, 0x62, 0x62));
    assert_eq!(light.accent, Color::Rgb(0xc0, 0x1c, 0x1c));
    assert_eq!(light.on_accent, Color::Rgb(0xff, 0xff, 0xff));
    assert_eq!(dark.bg, Color::Rgb(0x00, 0x00, 0x00));
    assert_eq!(dark.fg, Color::Rgb(0xff, 0xf1, 0xf1));
    assert_eq!(dark.muted, Color::Rgb(0xff, 0xcd, 0xcd));
    assert_eq!(dark.accent, Color::Rgb(0xc0, 0x1c, 0x1c));
    let h1 = ratatui::style::Style::from(&light.heading(1));
    assert_eq!((h1.fg, h1.bg), (Some(Color::Rgb(0xff, 0xff, 0xff)), Some(light.accent)), "h1 is white on red");
    let button = ratatui::style::Style::from(&light.button());
    assert_eq!((button.fg, button.bg), (Some(Color::Rgb(0, 0, 0)), Some(Color::Rgb(0xff, 0xff, 0xff))), "donate chip");
    let sel = ratatui::style::Style::from(&light.selected());
    assert_eq!(sel.bg, Some(light.accent), "the active indicator is the logo red");
    let body = [
        ("light fg/bg", ratio(light.fg, light.bg)),
        ("light muted/bg", ratio(light.muted, light.bg)),
        ("light accent/bg", ratio(light.accent, light.bg)),
        ("dark fg/bg", ratio(dark.fg, dark.bg)),
        ("dark muted/bg", ratio(dark.muted, dark.bg)),
        ("h1", ratio(h1.fg.unwrap(), h1.bg.unwrap())),
    ];
    for (name, r) in body {
        assert!(r >= 4.5, "{name} is {r:.2}:1");
    }
    let accent = ratio(dark.accent, dark.bg);
    assert!(accent >= 3.0, "dark accent/bg is {accent:.2}:1");
    assert!(ratio(dark.on_accent, dark.accent) >= 4.5, "white on the logo red");
}

#[test]
fn every_shipped_theme_loads_cleanly_with_both_variants_and_readable_text() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes");
    let files = toml_files(&dir);
    assert!(files.len() >= 8, "{files:?}");
    for f in files {
        let l = load_path(&f).unwrap_or_else(|e| panic!("{}", e.notice()));
        assert!(l.warnings.is_empty(), "{}: {:?}", f.display(), l.warnings);
        assert!(!l.file.description.is_empty() && l.file.light.is_some(), "{}", f.display());
        assert_eq!(to_toml(&parse(&to_toml(&l.file), "x").unwrap().0), to_toml(&l.file), "round trip");
        if l.file.name == DEFAULT_THEME { continue; }
        for v in [Variant::Dark, Variant::Light] {
            for r in crate::tui::editor::all_rows(&l.file, v) {
                assert!(crate::tui::editor::warning(&l.file, v, &r).is_none(), "{} {}: {r:?} below 3:1", l.file.name, v.name());
            }
        }
    }
}

// ---------------------------------------------------------------- downloads

/// A one-route HTTP server on localhost: answers every request with `status`, `body`
/// (sent without a Content-Length when `chunked`) after `delay`, and records the
/// request heads it saw.
async fn serve(status: &'static str, body: Vec<u8>, chunked: bool, delay: std::time::Duration) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/themes/", l.local_addr().unwrap());
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut c, _)) = l.accept().await else { break };
            let (body, seen) = (body.clone(), seen2.clone());
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut b = [0u8; 1024];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match c.read(&mut b).await { Ok(0) | Err(_) => return, Ok(n) => head.extend_from_slice(&b[..n]) }
                }
                seen.lock().unwrap().push(String::from_utf8_lossy(&head).to_string());
                tokio::time::sleep(delay).await;
                if chunked {
                    let _ = c.write_all(format!("HTTP/1.1 {status}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").as_bytes()).await;
                    for part in body.chunks(8192) {
                        let _ = c.write_all(format!("{:x}\r\n", part.len()).as_bytes()).await;
                        let _ = c.write_all(part).await;
                        let _ = c.write_all(b"\r\n").await;
                    }
                    let _ = c.write_all(b"0\r\n\r\n").await;
                } else {
                    let _ = c.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await;
                    let _ = c.write_all(&body).await;
                }
            });
        }
    });
    (url, seen)
}

const QUICK: std::time::Duration = std::time::Duration::from_millis(400);

#[tokio::test]
async fn a_download_is_checked_written_0600_and_sends_no_cookies() {
    use download::*;
    let d = tmp("dl-ok");
    let (url, seen) = serve("200 OK", repo_default().as_bytes().to_vec(), false, std::time::Duration::ZERO).await;
    let f = download_from(&d, "default", &format!("{url}default.toml"), QUICK).await.unwrap();
    assert_eq!(f, Fetched::Written(d.join("default.toml")));
    assert_eq!(std::fs::read_to_string(d.join("default.toml")).unwrap(), repo_default());
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(d.join("default.toml")).unwrap().permissions().mode() & 0o777, 0o600); }
    assert!(load(&d, "default").unwrap().warnings.is_empty());
    let heads = seen.lock().unwrap().clone();
    assert_eq!(heads.len(), 1);
    assert!(heads[0].starts_with("GET /themes/default.toml "), "{}", heads[0]);
    assert!(!heads[0].to_ascii_lowercase().contains("\ncookie:"), "no cookies: {}", heads[0]);
    // A second download keeps the file and doesn't even ask the server.
    assert_eq!(download_from(&d, "default", &format!("{url}default.toml"), QUICK).await.unwrap(), Fetched::Kept(d.join("default.toml")));
    assert_eq!(seen.lock().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn an_existing_file_is_never_replaced_even_a_broken_one() {
    use download::*;
    let d = tmp("dl-keep");
    std::fs::write(d.join("default.toml"), "mine = [").unwrap();
    let (url, seen) = serve("200 OK", repo_default().as_bytes().to_vec(), false, std::time::Duration::ZERO).await;
    assert_eq!(download_from(&d, "default", &format!("{url}default.toml"), QUICK).await.unwrap(), Fetched::Kept(d.join("default.toml")));
    assert_eq!(std::fs::read_to_string(d.join("default.toml")).unwrap(), "mine = [");
    assert!(seen.lock().unwrap().is_empty());
    // store() itself also refuses to replace a file that appeared meanwhile.
    assert_eq!(store(&d, "default", repo_default().as_bytes()).unwrap(), Fetched::Kept(d.join("default.toml")));
    assert_eq!(std::fs::read_to_string(d.join("default.toml")).unwrap(), "mine = [");
    let _ = std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn oversize_invalid_missing_slow_and_offline_downloads_write_nothing() {
    use download::*;
    let d = tmp("dl-bad");
    let big = format!("version = 1\n#{}\n", "x".repeat(THEME_MAX_BYTES as usize)).into_bytes();
    type Case = (&'static str, &'static str, Vec<u8>, bool, std::time::Duration, &'static str);
    let cases: Vec<Case> = vec![
        ("big", "200 OK", big.clone(), false, std::time::Duration::ZERO, "KiB"),
        ("chunked", "200 OK", big, true, std::time::Duration::ZERO, "KiB"),
        ("bad", "200 OK", b"version = [oops".to_vec(), false, std::time::Duration::ZERO, "not a valid theme"),
        ("newer", "200 OK", b"version = 99\n".to_vec(), false, std::time::Duration::ZERO, "not a valid theme"),
        ("missing", "404 Not Found", b"nope".to_vec(), false, std::time::Duration::ZERO, "404"),
        ("slow", "200 OK", repo_default().as_bytes().to_vec(), false, std::time::Duration::from_secs(3), "timed out"),
    ];
    for (name, status, body, chunked, delay, want) in cases {
        let (url, _) = serve(status, body, chunked, delay).await;
        let e = format!("{:#}", download_from(&d, name, &format!("{url}{name}.toml"), QUICK).await.unwrap_err());
        assert!(e.contains(want), "{name}: {e}");
        assert!(!d.join(format!("{name}.toml")).exists(), "{name}");
    }
    let e = format!("{:#}", download_from(&d, "default", "http://127.0.0.1:9/themes/default.toml", QUICK).await.unwrap_err());
    assert!(!d.join("default.toml").exists(), "offline: {e}");
    assert!(std::fs::read_dir(&d).unwrap().next().is_none(), "no temp files left");
    assert!(download_from(&d, "../x", "http://127.0.0.1:9/x", QUICK).await.is_err(), "names can't leave the folder");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_download_can_be_turned_off_and_its_notices_say_how_to_fix_things() {
    use download::*;
    let on = Prefs::default();
    let off: Prefs = toml::from_str("[ui]\ntheme_download = false\n").unwrap();
    assert!(auto_enabled(None, &on));
    assert!(auto_enabled(Some("on"), &on));
    for v in ["off", "OFF", "0", "false", "no"] { assert!(!auto_enabled(Some(v), &on), "{v}"); }
    assert!(!auto_enabled(None, &off));
    let d = Path::new("/h/.acp/themes");
    let n = failure_notice(d, "default", "offline");
    assert!(n.contains("--theme-init") && n.contains("/h/.acp/themes/default.toml") && n.contains("default.toml"), "{n}");
    assert!(off_notice(d).contains("ACP_THEME_DOWNLOAD") && off_notice(d).contains("--theme-init"));
    assert!(THEME_DOWNLOAD_BASE.ends_with(&format!("/v{}/themes/", env!("CARGO_PKG_VERSION"))));
    assert_eq!(DEFAULT_THEME_URL, format!("{THEME_DOWNLOAD_BASE}default.toml"));
    assert!(THEME_DOWNLOAD_BASE.starts_with("https://raw.githubusercontent.com/m3talsmith/acp-red-star/v"));
    assert!(SHIPPED_THEMES.iter().all(|n| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes").join(format!("{n}.toml")).exists()));
    assert!(!has_themes(Path::new("/nonexistent/acp/themes")));
}

#[test]
fn the_starter_is_minimal_valid_written_0600_and_never_over_a_file() {
    use download::*;
    let f = starter("mine");
    assert!(f.dark.styles.is_empty() && f.light.is_none() && f.dark.palette.len() == PALETTE_KEYS.len());
    let (back, warn) = parse(&to_toml(&f), "mine").unwrap();
    assert!(warn.is_empty());
    assert_eq!(to_toml(&back), to_toml(&f));
    assert_ne!(to_toml(&f), repo_default(), "generated, not the shipped default");
    let d = tmp("starter");
    assert_eq!(write_starter(&d, "mine").unwrap(), Fetched::Written(d.join("mine.toml")));
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(d.join("mine.toml")).unwrap().permissions().mode() & 0o777, 0o600); }
    std::fs::write(d.join("mine.toml"), "x = [").unwrap();
    assert_eq!(write_starter(&d, "mine").unwrap(), Fetched::Kept(d.join("mine.toml")));
    assert_eq!(std::fs::read_to_string(d.join("mine.toml")).unwrap(), "x = [");
    assert!(write_starter(&d, "../x").is_err());
    assert_eq!(free_name(&d, "mine"), "mine-2");
    assert_eq!(free_name(&d, "starter"), "starter");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn warnings_of_one_file_are_one_notice() {
    let p = Path::new("/t/x.toml");
    assert_eq!(warning_notice(p, &[]), None);
    assert_eq!(warning_notice(p, &["a".into()]).unwrap(), "warning: /t/x.toml: a");
    let n = warning_notice(p, &["a".into(), "b".into(), "c".into()]).unwrap();
    assert_eq!(n, "warning: /t/x.toml: 3 keys skipped: a; b; c");
}

/// The shipped default, byte for byte: the old look (ACP red), the header, both variants.
/// Changing themes/default.toml must change this test too.
const EXPECTED_DEFAULT: &str = r##"# acp-red-star theme, format v1. See docs/USAGE.md (Themes).
#
# The look acp-red-star always had: ACP red (#dc2626) on the terminal's own
# text and background colours. Copy this file to make your own theme, or use
# the theme editor (Shift-F5 in the terminal UI).
#
# Colours: 0-255, "#rrggbb" or "#rgb", an ANSI name ("black" ... "white",
# "bright_black" ... "bright_white"), "reset" (the terminal's colour), or
# "@name" for an entry of the same palette. Per-element overrides go under
# [dark.styles.<element>] with fg, bg, border, bold, italic, underline,
# reverse and dim.
version = 1
name = "default"
description = "ACP red on your terminal's own colours"

[dark.palette]
accent = "#dc2626"
muted = "#9ca3af"
border = "#4b5563"
on_accent = "#ffffff"
fg = "reset"
bg = "reset"

# Markdown in post bodies ([light] uses these too unless it sets its own)
[dark.styles.h1]
fg = "@on_accent"
bg = "@accent"
bold = true

[dark.styles.h2]
bold = true

[dark.styles.h3]
fg = "@accent"
bold = true

[dark.styles.h4]
bold = true
italic = true

[dark.styles.h5]
fg = "@muted"
bold = true

[dark.styles.h6]
fg = "@muted"
italic = true

[dark.styles.quote]
fg = "@muted"
border = "@accent"
italic = true

[dark.styles.code_block]
border = "@border"

[dark.styles.list_marker]
fg = "@accent"

[dark.styles.task]
fg = "@accent"
bold = true

[dark.styles.table]
border = "@border"

[dark.styles.table_header]
fg = "@accent"
bold = true

[dark.styles.rule]
fg = "@muted"

[dark.styles.footnote]
fg = "@muted"

[light.palette]
accent = "#dc2626"
muted = "#9ca3af"
border = "#4b5563"
on_accent = "#ffffff"
fg = "reset"
bg = "reset"
"##;

#[test]
fn the_shipped_default_theme_has_not_drifted() {
    assert_eq!(repo_default(), EXPECTED_DEFAULT);
    let (f, w) = parse(EXPECTED_DEFAULT, "default").unwrap();
    assert!(w.is_empty());
    for v in [Variant::Dark, Variant::Light] {
        let s = f.section(v);
        assert_eq!(s.get("accent"), Some(&ColorSpec::Value(Color::Rgb(0xdc, 0x26, 0x26))), "{v:?}");
    }
}
