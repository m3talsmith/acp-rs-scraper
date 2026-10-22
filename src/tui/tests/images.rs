//! Pictures: the 4-row header (halberd, then the title), inline post images, decoding.

use super::*;
use crate::tui::images::*;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

fn draw(a: &mut App, w: u16, h: u16) -> Buffer {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    t.backend().buffer().clone()
}

fn row(b: &Buffer, y: u16, x0: u16, x1: u16) -> String { (x0..x1).map(|x| b[(x, y)].symbol().to_string()).collect() }

fn with_images(store: MemStore) -> App {
    let mut a = App::new(Box::new(store), &no_args(), Theme::acp(), false);
    a.images = Some(Images::new(Protocol::Halfblocks, DEFAULT_CELL, false, [255, 255, 255], [0xdc, 0x26, 0x26]));
    a
}

fn png(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("acp-img-{}-{name}.png", std::process::id()));
    image::RgbaImage::from_pixel(40, 30, image::Rgba([10, 200, 30, 255])).save(&p).unwrap();
    p
}

#[test]
fn header_is_four_rows_logo_then_title() {
    let mut a = app();
    let b = draw(&mut a, 110, 30);
    assert_eq!(TUI_HEADER_ROWS, 4);
    // The logo occupies the left of the bar. Nothing is drawn in the columns the word used.
    for y in [0, 1, 3] {
        assert!(row(&b, y, 10, 34).chars().all(|c| c == ' '), "no word on row {y}: {:?}", row(&b, y, 10, 34));
    }
    let mid = row(&b, 2, 0, 110);
    assert!(mid.contains(TUI_SUBTITLE) && mid.contains(env!("CARGO_PKG_VERSION")) && mid.contains("logged out"), "{mid}");
    assert!(row(&b, 2, 10, 18).starts_with(TUI_SUBTITLE), "title sits against the logo: {}", row(&b, 2, 10, 40));
    assert_space_before_theme(&mid);
    for y in [0, 1, 3] { assert!(!row(&b, y, 10, 110).contains(TUI_SUBTITLE)); }
    assert!(row(&b, 4, 0, 110).contains("Scrape"), "menu under the header");
    assert_eq!(b[(40, 0)].bg, Theme::acp().accent, "red bar");

    // A full row still keeps a space before the theme marker.
    a.theme_name = "theme-name-that-fills-this-header".into();
    let tight = row(&draw(&mut a, 70, 24), 2, 0, 70);
    assert_space_before_theme(&tight);
}

fn assert_space_before_theme(mid: &str) {
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let Some(i) = mid.find(&version) else { panic!("version missing: {mid}"); };
    let after = &mid[i + version.len()..];
    let Some(rel) = after.find('‹') else { panic!("theme marker missing: {mid}"); };
    assert!(rel >= 1 && after.as_bytes()[rel - 1] == b' ', "version sits against the theme marker: {mid}");
}

#[test]
fn no_logo_mode_draws_a_word_beside_the_logo() {
    use crate::brands::LogoKind;
    for logo in [LogoKind::Halberd, LogoKind::Builtin, LogoKind::Text, LogoKind::File] {
        let mut a = app();
        a.brand.logo = logo;
        a.brand.word = "RED STAR".into();
        a.brand.logo_text = "RS".into();
        let b = draw(&mut a, 110, 30);
        for y in [0, 1, 3] {
            assert!(row(&b, y, 10, 40).chars().all(|c| c == ' '), "{logo:?} row {y}: {:?}", row(&b, y, 10, 40));
        }
        let mid = row(&b, 2, 10, 40);
        assert!(mid.starts_with(TUI_SUBTITLE), "{logo:?} title: {mid}");
        assert!(!mid.contains("RED"), "{logo:?} drew the word: {mid}");
    }
}

fn logo_rows(b: &Buffer) -> Vec<String> { (0..4).map(|y| row(b, y, 1, 9)).collect() }

#[test]
fn logo_is_the_png_silhouette_in_sextants_on_the_red_bar() {
    let t = Theme::acp();
    for mut a in [with_images(fixture()), app()] {
        a.brand.logo = crate::brands::LogoKind::Builtin;
        a.brand.word = "ACP".into();
        let b = draw(&mut a, 110, 30);
        let logo = logo_rows(&b);
        println!("{}", logo.join("\n"));
        assert!(logo.iter().all(|l| l.chars().count() == 8), "8x4 cells: {logo:?}");
        assert_eq!((b[(1, 0)].fg, b[(1, 0)].bg), (t.on_accent, t.accent), "white on the bar, no badge");
        let mut again = with_images(fixture());
        again.brand.logo = crate::brands::LogoKind::Builtin;
        again.brand.word = "ACP".into();
        assert_eq!(logo, logo_rows(&draw(&mut again, 110, 30)), "same with and without picture support");
    }
}

#[test]
fn default_header_is_the_halberd_in_the_bar_colours() {
    let t = Theme::acp();
    let mut a = app();
    assert_eq!(a.brand.logo, crate::brands::LogoKind::Halberd);
    assert_eq!(a.brand.word, "RED STAR");
    let b = draw(&mut a, 110, 30);
    let logo = logo_rows(&b);
    let star = {
        let mut s = app();
        s.brand.logo = crate::brands::LogoKind::Builtin;
        s.brand.word = "ACP".into();
        logo_rows(&draw(&mut s, 110, 30))
    };
    assert_ne!(logo, star, "the halberd is not the old star");
    let style = ratatui::style::Style::from(t.bar());
    assert_eq!((b[(1, 0)].fg, b[(1, 0)].bg), (style.fg.unwrap(), style.bg.unwrap()));
    assert_eq!((b[(4, 1)].fg, b[(4, 1)].bg), (t.on_accent, t.accent));
    let at = crate::tui::images::halberd_dot;
    assert!(at(6, 0) && at(7, 0), "tip");
    assert!(at(1, 5), "fluke");
    assert!(at(14, 3), "blade edge");
    assert!(!at(11, 3) && at(10, 3) && at(12, 3), "star top gap");
    assert!(!at(11, 5) && at(9, 5) && at(13, 5), "star centre");
}

#[test]
fn logo_mask_keeps_the_ring_and_the_star() {
    let m = logo_mask(16, 12).unwrap();
    let at = |x: usize, y: usize| m[y * 16 + x];
    let txt: String = (0..12).map(|y| (0..16).map(|x| if at(x, y) { '#' } else { '.' }).collect::<String>() + "\n").collect();
    // The ring: left and right edges at mid height, top and bottom in the middle.
    assert!(at(0, 6) && at(15, 6) && at(8, 0) && at(8, 11), "ring\n{txt}");
    // Just inside the ring is empty; the star's arms cross the middle row and its top point is above.
    assert!(!at(2, 6) && !at(13, 6), "gap inside the ring\n{txt}");
    assert!((4..12).filter(|&x| at(x, 5)).count() >= 6, "star arms\n{txt}");
    assert!(at(7, 3) || at(8, 3), "star top point\n{txt}");
    assert!((4..12).any(|x| at(x, 8)), "star legs\n{txt}");
}

#[test]
fn logo_image_is_the_shape_in_fg_over_bg() {
    let img = logo_image([255, 255, 255], [0xdc, 0x26, 0x26]).unwrap();
    assert_eq!(img.width(), img.height());
    assert_eq!(img.get_pixel(0, 0).0, [0xdc, 0x26, 0x26, 255], "transparent corner = the bar");
    assert_eq!(img.get_pixel(256, 3).0, [255, 255, 255, 255], "the ring = on-accent");
}

#[test]
fn post_images_show_inline_in_the_detail() {
    let p = png("inline");
    let mut store = fixture();
    store.atts[0].local_path = p.clone();
    store.atts[0].mime_type = "image/png".into();
    let mut a = with_images(store);
    a.select_only(nick_idx(&a, "alice"));
    let i = a.view().posts.iter().position(|p| p.post_id == "ALICE0001").unwrap();
    a.open_post(i);
    let b = draw(&mut a, 120, 50);
    let name = p.file_name().unwrap().to_string_lossy().to_string();
    let y = (0..50).find(|y| row(&b, *y, 0, 120).contains(&format!("🖼 {name}"))).expect("caption");
    let pic: String = (y + 1..y + 1 + IMAGE_ROWS).map(|y| row(&b, y, 0, 120)).collect();
    assert!(pic.contains('▀') || pic.contains('▄') || pic.contains('█'), "picture cells");
    assert!(matches!(a.images.as_mut().unwrap().get(&p), ImgState::Ready(_)));
    std::fs::remove_file(p).ok();
}

#[test]
fn missing_or_broken_images_fail_softly() {
    let mut im = Images::new(Protocol::Halfblocks, DEFAULT_CELL, false, [255, 255, 255], [0, 0, 0]);
    assert!(matches!(im.get(Path::new("/nonexistent/a.png")), ImgState::Failed(e) if e.contains("missing")));
    let bad = std::env::temp_dir().join(format!("acp-img-{}-bad.png", std::process::id()));
    std::fs::write(&bad, b"not a png").unwrap();
    assert!(decode(&bad).is_err());
    std::fs::remove_file(bad).ok();
}

#[test]
fn large_images_are_shrunk_and_types_recognised() {
    let p = std::env::temp_dir().join(format!("acp-img-{}-big.png", std::process::id()));
    image::RgbImage::new(3000, 1500).save(&p).unwrap();
    let d = decode(&p).unwrap();
    assert_eq!((d.width(), d.height()), (1024, 512));
    std::fs::remove_file(p).ok();
    assert!(is_image("image/avif", Path::new("a.bin")));
    assert!(is_image("", Path::new("a.WEBP")));
    assert!(!is_image("video/mp4", Path::new("a.mp4")));
}

#[test]
fn fit_keeps_the_aspect_ratio() {
    // 10×20 px cells: a square picture is twice as wide in cells as tall.
    assert_eq!(fit(512, 512, (10, 20), 6, 3), (6, 3));
    assert_eq!(fit(1000, 500, (10, 20), 80, 14), (56, 14));
    assert_eq!(fit(100, 1000, (10, 20), 80, 14), (3, 14));
    assert_eq!(fit(0, 10, (10, 20), 10, 10), (0, 0));
}

#[test]
fn terminal_replies_pick_the_protocol_and_cell_size() {
    assert_eq!(parse_reply("\x1b_Gi=31;OK\x1b\\\x1b[6;18;9t\x1b[?62;22c"), (Protocol::Kitty, Some((9, 18))));
    assert_eq!(parse_reply("\x1b[?62;4;6;22c"), (Protocol::Sixel, None));
    assert_eq!(parse_reply("\x1b[?1;2c"), (Protocol::Halfblocks, None));
    assert_eq!(parse_reply(""), (Protocol::Halfblocks, None));
    assert_eq!(Protocol::parse("Kitty"), Some(Protocol::Kitty));
    assert_eq!(Protocol::parse("nope"), None);
}

#[test]
fn base64_and_sixel_encoders() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    let img = image::RgbaImage::from_pixel(8, 7, image::Rgba([255, 0, 0, 255]));
    let s = sixel(&img);
    assert!(s.starts_with("\x1bP0;1;0q\"1;1;8;7") && s.ends_with("\x1b\\"));
    assert!(s.contains("#180;2;100;0;0"), "red register: {s}");
    assert!(s.contains("!8~") && s.contains("!8@"), "rle bands: {s}");
    let k = kitty_transmit(7, &vec![0u8; 5000]);
    assert!(k.starts_with("\x1b_Ga=t,f=100,i=7,q=2,m=1;") && k.contains("\x1b_Gm=0;"));
}

#[test]
fn graphics_protocols_place_after_the_frame_and_only_on_change() {
    let mut a = App::new(Box::new(fixture()), &no_args(), Theme::acp(), false);
    a.brand.logo = crate::brands::LogoKind::Builtin;
    a.brand.word = "ACP".into();
    a.images = Some(Images::new(Protocol::Kitty, DEFAULT_CELL, false, [255, 255, 255], [0xdc, 0x26, 0x26]));
    let b = draw(&mut a, 110, 30);
    assert_eq!(row(&b, 1, 1, 9), "        ", "cells left blank for the picture");
    let im = a.images.as_mut().unwrap();
    assert_eq!(im.frame.len(), 1);
    let mut out = Vec::new();
    assert_eq!(im.flush(&mut out), Flush::Done);
    let s = String::from_utf8_lossy(&out).to_string();
    assert!(s.contains("a=t,f=100,i=1") && s.contains("a=p,i=1,p=1,c=8,r=4"), "{}", &s[..80.min(s.len())]);
    draw(&mut a, 110, 30);
    let mut again = Vec::new();
    a.images.as_mut().unwrap().flush(&mut again);
    assert!(again.is_empty(), "unchanged frame writes nothing");
    // Sixel asks for a repaint when the pictures change.
    let mut s = App::new(Box::new(fixture()), &no_args(), Theme::acp(), false);
    s.brand.logo = crate::brands::LogoKind::Builtin;
    s.brand.word = "ACP".into();
    s.images = Some(Images::new(Protocol::Sixel, DEFAULT_CELL, false, [255, 255, 255], [0xdc, 0x26, 0x26]));
    draw(&mut s, 110, 30);
    let mut o = Vec::new();
    assert_eq!(s.images.as_mut().unwrap().flush(&mut o), Flush::Done);
    assert!(String::from_utf8_lossy(&o).contains("\x1bP0;1;0q"));
    s.images.as_mut().unwrap().frame.clear();
    assert_eq!(s.images.as_mut().unwrap().flush(&mut Vec::new()), Flush::Repaint);
}

#[test]
fn sextant_table_covers_every_pattern() {
    assert_eq!(sextant(0), ' ');
    assert_eq!(sextant(63), '█');
    assert_eq!(sextant(1), '\u{1FB00}');
    assert_eq!(sextant(20), '\u{1FB13}');
    assert_eq!(sextant(22), '\u{1FB14}');
    assert_eq!(sextant(62), '\u{1FB3B}');
    assert_eq!(sextant(21), '▌');
}
