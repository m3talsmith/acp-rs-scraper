use crate::config::*;
use crate::tui::layout::*;
use ratatui::layout::Rect;

#[test]
fn base_layout_tiles_the_screen() {
    let r = Rect::new(0, 0, 100, 30);
    let a = areas(r, 1);
    assert_eq!((a.top.y, a.menu.y), (0, 1));
    assert_eq!(a.nicks.width, NICK_BAR_WIDTH);
    assert_eq!(a.nicks.width + a.body.width, 100);
    assert_eq!(a.body.y + a.body.height, a.status.y);
    assert_eq!(a.status.y + a.status.height, 30);
    let big_log = areas(r, 10);
    assert_eq!(big_log.status.height, 10);
}

#[test]
fn detail_slides_up_leaving_a_residual_strip() {
    let r = Rect::new(0, 0, 60, 24);
    let s = detail_split(r, 10, 3, 1.0);
    assert_eq!(s.above.iter().map(|x| x.0).collect::<Vec<_>>(), [2]);
    assert_eq!(s.detail.height, 24 - DETAIL_RESIDUAL_ROWS);
    assert_eq!(s.below.len() as u16, DETAIL_RESIDUAL_ROWS - 1);
    assert_eq!(s.below[0].0, 4);
    // First post: nothing above.
    assert!(detail_split(r, 10, 0, 1.0).above.is_empty());
    // Mid-slide the detail is smaller and more rows show below.
    let mid = detail_split(r, 30, 3, 0.3);
    assert!(mid.detail.height < s.detail.height && mid.below.len() > s.below.len());
    // Everything stays inside.
    for x in mid.above.iter().chain(&mid.below) { assert!(x.1.y + x.1.height <= 24); }
}

#[test]
fn nested_stack_collapses_previous_frames_to_headers() {
    let inner = Rect::new(0, 0, 50, 20);
    assert_eq!(stack_layout(inner, 1, 1.0).frames, [(0, inner, false)]);
    let two = stack_layout(inner, 2, 1.0);
    assert_eq!(two.frames[0].1.height, DETAIL_HEADER_ROWS);
    assert!(two.frames[0].2 && !two.frames[1].2);
    let nested = two.nested.unwrap();
    assert!(nested.x > inner.x && nested.y == DETAIL_HEADER_ROWS);
    let three = stack_layout(inner, 3, 1.0);
    assert_eq!(three.frames[1].1.height, DETAIL_HEADER_ROWS, "linked post collapses inside the nest");
    assert!(three.frames[2].1.height > DETAIL_HEADER_ROWS);
    assert_eq!(three.frames[2].1.y, three.frames[1].1.y + DETAIL_HEADER_ROWS);
}

#[test]
fn push_animation_shrinks_previous_frame() {
    let inner = Rect::new(0, 0, 50, 30);
    let start = stack_layout(inner, 2, 0.0);
    let mid = stack_layout(inner, 2, 0.5);
    let end = stack_layout(inner, 2, 1.0);
    assert!(start.frames[0].1.height > mid.frames[0].1.height);
    assert!(mid.frames[0].1.height > end.frames[0].1.height);
    let s3 = stack_layout(inner, 3, 0.0);
    let e3 = stack_layout(inner, 3, 1.0);
    assert!(s3.frames[1].1.height > e3.frames[1].1.height);
}

#[test]
fn wrap_keeps_paragraphs_and_breaks_long_words() {
    assert_eq!(wrap("one two three\n\nfour", 7), ["one two", "three", "", "four"]);
    assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
    assert_eq!(ellipsize("hello world", 6), "hello…");
    assert_eq!(ellipsize("hi", 6), "hi");
}

#[test]
fn modal_rect_keeps_margin() {
    let r = modal_rect(Rect::new(0, 0, 80, 24), 200, 200);
    assert_eq!((r.x, r.y, r.width, r.height), (2, 1, 76, 22));
}
