//! Pictures: the 3-row header (logo + big "ACP"), inline post images, decoding.

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
    let mut a = App::new(Box::new(store), &no_args(), Theme::default(), false);
    a.images = Some(Images::new(Protocol::Halfblocks, DEFAULT_CELL, false, [255, 255, 255]));
    a
}

fn png(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("acp-img-{}-{name}.png", std::process::id()));
    image::RgbaImage::from_pixel(40, 30, image::Rgba([10, 200, 30, 255])).save(&p).unwrap();
    p
}

#[test]
fn header_is_three_rows_with_big_acp_centred() {
    let mut a = app();
    let b = draw(&mut a, 110, 30);
    // The big "ACP" spans all three header rows; the title and session sit on the middle one.
    for y in 0..TUI_HEADER_ROWS { assert!(row(&b, y, 8, 32).chars().any(|c| c != ' '), "row {y}: {:?}", row(&b, y, 8, 32)); }
    let mid = row(&b, 1, 0, 110);
    assert!(mid.contains(TUI_SUBTITLE) && mid.contains(env!("CARGO_PKG_VERSION")) && mid.contains("logged out"), "{mid}");
    assert!(!row(&b, 0, 34, 110).contains(TUI_SUBTITLE) && !row(&b, 2, 34, 110).contains(TUI_SUBTITLE));
    assert!(row(&b, 3, 0, 110).contains("Scrape"), "menu under the header");
}

#[test]
fn logo_is_drawn_from_the_png_in_sextants() {
    let mut a = with_images(fixture());
    let b = draw(&mut a, 110, 30);
    let logo: Vec<String> = (0..3).map(|y| row(&b, y, 1, 7)).collect();
    println!("{}", logo.join("\n"));
    assert!(logo.iter().all(|l| l.chars().count() == 6 && l.trim() != ""), "{logo:?}");
    assert_eq!(b[(4, 1)].bg, ratatui::style::Color::Rgb(255, 255, 255), "on the badge");
    // Without image support the text star stands in.
    let mut plain = app();
    let b = draw(&mut plain, 110, 30);
    assert!(row(&b, 1, 1, 7).contains('█'));
}

#[test]
fn logo_badge_keeps_the_star_visible() {
    let img = logo_on([255, 255, 255]).unwrap().to_rgba8();
    assert_eq!(img.width(), img.height());
    assert_eq!(img.get_pixel(1, 1).0, [255, 255, 255, 255]);
    let c = img.get_pixel(img.width() / 2, img.height() / 2).0;
    assert!(c[0] > 150 && c[1] < 80, "red centre: {c:?}");
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
    let mut im = Images::new(Protocol::Halfblocks, DEFAULT_CELL, false, [0, 0, 0]);
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
    let mut a = App::new(Box::new(fixture()), &no_args(), Theme::default(), false);
    a.images = Some(Images::new(Protocol::Kitty, DEFAULT_CELL, false, [255, 255, 255]));
    let b = draw(&mut a, 110, 30);
    assert_eq!(row(&b, 1, 1, 7), "      ", "cells left blank for the picture");
    let im = a.images.as_mut().unwrap();
    assert_eq!(im.frame.len(), 1);
    let mut out = Vec::new();
    assert_eq!(im.flush(&mut out), Flush::Done);
    let s = String::from_utf8_lossy(&out).to_string();
    assert!(s.contains("a=t,f=100,i=1") && s.contains("a=p,i=1,p=1,c=6,r=3"), "{}", &s[..80.min(s.len())]);
    draw(&mut a, 110, 30);
    let mut again = Vec::new();
    a.images.as_mut().unwrap().flush(&mut again);
    assert!(again.is_empty(), "unchanged frame writes nothing");
    // Sixel asks for a repaint when the pictures change.
    let mut s = App::new(Box::new(fixture()), &no_args(), Theme::default(), false);
    s.images = Some(Images::new(Protocol::Sixel, DEFAULT_CELL, false, [255, 255, 255]));
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
