//! Theme files: the parser, colours, fallbacks, and save / load / install /
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
fn first_run_copies_the_shipped_themes_only_into_an_empty_folder() {
    let root = tmp("install");
    let (none, src, dest) = (root.join("none"), root.join("src"), root.join("home/.acp/themes"));
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("default.toml"), repo_default()).unwrap();
    std::fs::write(src.join("broken.toml"), "x = [").unwrap();
    let (from, names) = install_defaults(&dest, &[none.clone(), src.clone()]).unwrap().unwrap();
    assert_eq!((from, names), (src.clone(), vec!["default".to_string()]), "broken shipped files are skipped");
    assert_eq!(std::fs::read_to_string(dest.join("default.toml")).unwrap(), repo_default());
    // A user's (even broken) file means the folder is in use: nothing is copied or replaced.
    std::fs::write(dest.join("default.toml"), "broken = [").unwrap();
    assert_eq!(install_defaults(&dest, std::slice::from_ref(&src)).unwrap(), None);
    assert_eq!(std::fs::read_to_string(dest.join("default.toml")).unwrap(), "broken = [");
    assert_eq!(install_defaults(&root.join("empty"), &[none]).unwrap(), None);
    assert!(source_dirs().first().is_some_and(|p| p == Path::new(THEMES_SYSTEM_DIR)));
    assert!(source_dirs().last().unwrap().join("default.toml").exists(), "the repo's themes/ in development");
    let _ = std::fs::remove_dir_all(root);
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
fn every_shipped_theme_loads_cleanly_with_both_variants_and_readable_text() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes");
    let files = toml_files(&dir);
    assert!(files.len() >= 7, "{files:?}");
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
