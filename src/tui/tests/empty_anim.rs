//! The empty-Body animation: frame files, the setting, looping, fitting and the render.

use super::*;
use crate::tui::empty_anim::{Choice, builtin, choice, load, parse, star};
use ratatui::{Terminal, backend::TestBackend};
use std::io::Write;
use std::time::Duration;

const WORD: &str = "█████  █      █████";
const SMALL: &str = "ACPANIM 1\nfps 4\nsize 4 2\n%%\nabcd\nefgh\n%%\nAB\n%%\n12345\n1\n2\n";

#[test]
fn frames_parse_and_pad_to_the_grid() {
    let f = parse(SMALL.as_bytes(), "t").unwrap();
    assert_eq!((f.fps, f.width, f.height, f.frames.len()), (4, 4, 2, 3));
    assert_eq!(f.frames[1], vec![vec!['A', 'B', ' ', ' '], vec![' '; 4]], "short frames are padded");
    assert_eq!(f.frames[2], vec![vec!['1', '2', '3', '4'], vec!['1', ' ', ' ', ' ']], "long ones cut");
}

#[test]
fn gzip_frames_parse_too() {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(SMALL.as_bytes()).unwrap();
    let f = parse(&gz.finish().unwrap(), "t.gz").unwrap();
    assert_eq!(f.frames.len(), 3);
}

#[test]
fn bad_files_are_refused() {
    assert!(parse(b"hello", "x").is_err());
    assert!(parse(b"ACPANIM 1\nfps 12\n%%\nx\n", "x").unwrap_err().to_string().contains("size"));
    assert!(parse(b"ACPANIM 1\nsize 2 2\n", "x").unwrap_err().to_string().contains("no frames"));
    assert!(parse(b"ACPANIM 1\nfps 0\nsize 2 2\n%%\n", "x").is_err());
}

#[test]
fn the_built_in_clip_decodes() {
    let f = builtin();
    assert_eq!(f.source, "built-in");
    assert_eq!((f.fps, f.width, f.height, f.frames.len()), (12, 120, 34, 120));
}

#[test]
fn the_star_is_procedural_and_turns() {
    let s = star();
    assert_eq!((s.fps, s.width, s.height, s.frames.len()), (12, 72, 30, 48));
    let txt = |i: usize| s.frames[i].iter().map(|r| r.iter().collect::<String>()).collect::<Vec<_>>().join("\n");
    assert!(txt(0).contains('@') && txt(0).contains('█'), "star and wordmark");
    assert_ne!(txt(0), txt(6), "it turns");
}

#[test]
fn playback_loops_at_the_file_fps() {
    let f = parse(SMALL.as_bytes(), "t").unwrap();
    assert_eq!(f.index(Duration::ZERO), 0);
    assert_eq!(f.index(Duration::from_millis(260)), 1);
    assert_eq!(f.index(Duration::from_millis(500)), 2);
    assert_eq!(f.index(Duration::from_millis(750)), 0, "loops after the last frame");
    assert_eq!(f.until_next(Duration::from_millis(100)), Duration::from_millis(150));
}

#[test]
fn frames_shrink_to_fit_and_pause_when_too_small() {
    let f = builtin();
    let big = f.fit(0, 200, 60).unwrap();
    assert_eq!(big.len(), 34, "never enlarged");
    let half = f.fit(0, 60, 40).unwrap();
    assert_eq!(half.len(), 17, "aspect kept: 120x34 into 60 columns is 17 rows");
    assert!(half.iter().all(|l| l.chars().count() <= 60));
    let tall = f.fit(0, 200, 17).unwrap();
    assert_eq!(tall.len(), 17);
    assert!(tall.iter().all(|l| l.chars().count() <= 60));
    assert!(f.fit(0, 23, 40).is_none() && f.fit(0, 80, 7).is_none(), "paused when too small");
}

#[test]
fn the_setting_env_and_pref() {
    use toml::Value;
    assert_eq!(choice(None, None), Choice::Clip);
    assert_eq!(choice(None, Some(&Value::Boolean(false))), Choice::Off);
    assert_eq!(choice(None, Some(&Value::String("minimal".into()))), Choice::Star);
    assert_eq!(choice(Some("off"), Some(&Value::Boolean(true))), Choice::Off, "env wins");
    assert_eq!(choice(Some("on"), Some(&Value::Boolean(false))), Choice::Clip);
    assert_eq!(choice(Some("bogus"), Some(&Value::String("star".into()))), Choice::Star, "unknown env falls back to the pref");
    assert!(load(Choice::Off, None, None).is_none());
    assert_eq!(load(Choice::Star, None, None).unwrap().source, "star");
}

#[test]
fn a_local_file_wins_and_a_broken_one_falls_back() {
    let home = std::env::temp_dir().join(format!("acp-anim-test-{}", std::process::id()));
    let dir = home.join(".acp/animations");
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(load(Choice::Clip, None, Some(&home)).unwrap().source, "built-in");
    std::fs::write(dir.join("empty-body.acpanim"), SMALL).unwrap();
    assert!(load(Choice::Clip, None, Some(&home)).unwrap().source.ends_with("empty-body.acpanim"));
    std::fs::write(home.join("mine.acpanim"), SMALL).unwrap();
    assert!(load(Choice::Clip, Some("~/mine.acpanim"), Some(&home)).unwrap().source.ends_with("mine.acpanim"));
    std::fs::write(dir.join("empty-body.acpanim"), "junk").unwrap();
    assert_eq!(load(Choice::Clip, None, Some(&home)).unwrap().source, "built-in");
    std::fs::remove_dir_all(&home).unwrap();
}

fn render(a: &mut App, w: u16, h: u16, at: Duration) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = a.empty_anim_start + at;
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

#[test]
fn the_empty_body_plays_it_and_content_replaces_it() {
    let mut a = app();
    a.empty_anim = Some(star());
    let s0 = render(&mut a, 120, 50, Duration::ZERO);
    assert!(s0.contains("Click a nick"), "the hint stays");
    assert!(s0.contains(WORD) && s0.contains('@'), "star and wordmark drawn\n{s0}");
    assert_eq!(a.empty_anim_frame, Some(0));
    let s1 = render(&mut a, 120, 50, Duration::from_millis(500));
    assert_eq!(a.empty_anim_frame, Some(6));
    assert_ne!(s0, s1, "next frames differ");
    // Too small: paused, nothing drawn.
    render(&mut a, 60, 18, Duration::ZERO);
    assert_eq!(a.empty_anim_frame, None);
    // Selecting a nick shows posts instead.
    a.select_only(nick_idx(&a, "alice"));
    let s = render(&mut a, 120, 50, Duration::ZERO);
    assert_eq!(a.empty_anim_frame, None);
    assert!(!s.contains(WORD));
    // Off: nothing.
    let mut b = app();
    render(&mut b, 120, 50, Duration::ZERO);
    assert_eq!(b.empty_anim_frame, None);
}

