//! Screen geometry (pure, so it is unit-tested): the base layout, the post
//! list ↔ detail split (the "slide up"), and the nested detail stack (the
//! "push the previous detail up").

use crate::prelude::*;
use ratatui::layout::Rect;

/// Rows of a post card when no detail is open.
pub const CARD_ROWS: u16 = 4;

/// The base layout's areas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Areas {
    /// Top bar ([`TUI_HEADER_ROWS`] tall: logo, big "ACP", title, session).
    pub top: Rect,
    /// Menu bar.
    pub menu: Rect,
    /// Left nick bar.
    pub nicks: Rect,
    /// Body.
    pub body: Rect,
    /// Right action-history bar. One column when it is collapsed.
    pub actions: Rect,
    /// Status / log footer.
    pub status: Rect,
}

/// Split the screen: top bar, menu bar, nicks | body | actions, status (`log_rows` tall).
///
/// Nicks start open. The action bar starts collapsed to one column.
pub fn areas(r: Rect, log_rows: u16) -> Areas {
    areas_open(r, log_rows, true, false)
}

/// [`areas`] with the nick bar and the action bar open or collapsed.
pub fn areas_open(r: Rect, log_rows: u16, nicks_open: bool, actions_open: bool) -> Areas {
    let row = |y: u16, h: u16| Rect { x: r.x, y, width: r.width, height: h };
    let th = TUI_HEADER_ROWS.min(r.height);
    let top = row(r.y, th);
    let menu = row(r.y + th, 1.min(r.height.saturating_sub(th)));
    let status_h = log_rows.max(1).min(r.height.saturating_sub(th + 3));
    let main_y = r.y + th + 1;
    let main_h = r.height.saturating_sub(th + 1 + status_h);
    let status = row(main_y + main_h, status_h);
    let nw = if nicks_open { NICK_BAR_WIDTH.min(r.width / 3).max(1) } else { 1.min(r.width) };
    let rest = r.width.saturating_sub(nw);
    let aw = if actions_open { 24.min(rest / 3).max(1) } else { 1.min(rest) };
    let body_w = rest.saturating_sub(aw);
    Areas { top, menu,
        nicks: Rect { x: r.x, y: main_y, width: nw, height: main_h },
        body: Rect { x: r.x + nw, y: main_y, width: body_w, height: main_h },
        actions: Rect { x: r.x + nw + body_w, y: main_y, width: aw, height: main_h },
        status }
}

/// The post list with an open detail: compact rows above and below the detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailSplit {
    /// Compact rows above: (post index, rect).
    pub above: Vec<(usize, Rect)>,
    /// The detail box.
    pub detail: Rect,
    /// Compact rows below.
    pub below: Vec<(usize, Rect)>,
}

/// Lay out the list around the detail of post `cursor` (of `n`), `p` = slide progress.
/// Fully open, the list keeps [`DETAIL_RESIDUAL_ROWS`] rows: one above (when there
/// is a previous post) and the rest below; while sliding, the detail grows from
/// the selected row and the rows below are pushed down and out.
pub fn detail_split(area: Rect, n: usize, cursor: usize, p: f32) -> DetailSplit {
    let residual = DETAIL_RESIDUAL_ROWS.min(area.height / 3);
    let full = area.height.saturating_sub(residual);
    let h = ((full as f32) * p.clamp(0.0, 1.0)).round() as u16;
    let h = h.clamp(3.min(area.height), full.max(3.min(area.height)));
    let above_n = if cursor > 0 { 1.min(residual) } else { 0 } as usize;
    let mut above = Vec::new();
    let mut y = area.y;
    for k in 0..above_n {
        let i = cursor - above_n + k;
        above.push((i, Rect { x: area.x, y, width: area.width, height: 1 }));
        y += 1;
    }
    let detail = Rect { x: area.x, y, width: area.width, height: h.min(area.y + area.height - y) };
    y += detail.height;
    let mut below = Vec::new();
    let mut i = cursor + 1;
    while y < area.y + area.height && i < n {
        below.push((i, Rect { x: area.x, y, width: area.width, height: 1 }));
        y += 1;
        i += 1;
    }
    DetailSplit { above, detail, below }
}

/// Where each frame of a detail stack goes inside the root detail box (`inner`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackLayout {
    /// (depth, rect, collapsed) for every frame, root first.
    pub frames: Vec<(usize, Rect, bool)>,
    /// Border of the nested box holding frames 1.. (none with a single frame).
    pub nested: Option<Rect>,
}

fn lerp(a: u16, b: u16, p: f32) -> u16 { (a as f32 + (b as f32 - a as f32) * p.clamp(0.0, 1.0)).round() as u16 }

/// Stack layout for `n` frames; `push` = progress of the latest push (1.0 = settled).
/// Frame 0 (the post opened from the list) fills `inner` alone; with more frames
/// it collapses to a [`DETAIL_HEADER_ROWS`] header and the rest nest in a box
/// below it, where every frame but the top one is a header too. A push animates
/// the previous top frame from its full height down to a header.
pub fn stack_layout(inner: Rect, n: usize, push: f32) -> StackLayout {
    if n == 0 { return StackLayout { frames: Vec::new(), nested: None }; }
    if n == 1 { return StackLayout { frames: vec![(0, inner, false)], nested: None }; }
    let hdr = DETAIL_HEADER_ROWS;
    let bottom = inner.y + inner.height;
    let clip = |y: u16, h: u16| h.min(bottom.saturating_sub(y));
    let mut frames = Vec::new();
    // The frame that was on top before the push shrinks from "everything left" to a header.
    let prev_full = |avail: u16| lerp(avail, hdr, push).max(hdr.min(avail));
    let h0 = if n == 2 { prev_full(inner.height.saturating_sub(3)) } else { hdr };
    frames.push((0, Rect { height: clip(inner.y, h0), ..inner }, true));
    let ny = inner.y + frames[0].1.height;
    let nested = Rect { x: inner.x + 1, y: ny, width: inner.width.saturating_sub(2), height: bottom.saturating_sub(ny) };
    let ni = Rect { x: nested.x + 1, y: nested.y + 1, width: nested.width.saturating_sub(2), height: nested.height.saturating_sub(2) };
    let nb = ni.y + ni.height;
    let mut y = ni.y;
    for d in 1..n {
        let top = d == n - 1;
        let h = if top { nb.saturating_sub(y) }
            else if d == n - 2 { prev_full(nb.saturating_sub(y).saturating_sub(2)) }
            else { hdr };
        let h = h.min(nb.saturating_sub(y));
        frames.push((d, Rect { x: ni.x, y, width: ni.width, height: h }, !top));
        y += h;
    }
    StackLayout { frames, nested: Some(nested) }
}

/// Centered rect of at most `w`×`h` inside `r` with a 1–2 cell margin.
pub fn modal_rect(r: Rect, w: u16, h: u16) -> Rect {
    let mw = r.width.saturating_sub(4).min(w).max(1);
    let mh = r.height.saturating_sub(2).min(h).max(1);
    Rect { x: r.x + (r.width - mw) / 2, y: r.y + (r.height - mh) / 2, width: mw, height: mh }
}

/// Word-wrap `text` to `width` columns, keeping its newlines (blank lines stay).
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;
    let width = width.max(1);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        let mut w = 0;
        for word in para.split(' ') {
            let ww = word.width();
            if w > 0 && w + 1 + ww > width {
                out.push(std::mem::take(&mut line));
                w = 0;
            }
            if ww > width {
                // Hard-break words longer than the line.
                for ch in word.chars() {
                    let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                    if w + cw > width { out.push(std::mem::take(&mut line)); w = 0; }
                    line.push(ch);
                    w += cw;
                }
                continue;
            }
            if w > 0 { line.push(' '); w += 1; }
            line.push_str(word);
            w += ww;
        }
        out.push(line);
    }
    out
}

/// Cut `s` to at most `width` columns, adding `…` when cut.
pub fn ellipsize(s: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut w = 0;
    let mut out = String::new();
    let total: usize = s.chars().map(|c| c.width().unwrap_or(0)).sum();
    if total <= width { return s.to_string(); }
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > width { break; }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

/// Whether `(x, y)` is inside `r`.
pub fn contains(r: Rect, x: u16, y: u16) -> bool { x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height }

/// Minimum width of a grid cell.
pub const GRID_CELL_MIN_W: u16 = 34;
/// Height of a grid cell.
pub const GRID_CELL_H: u16 = 8;

/// Grid view: columns that fit `r`, and the rect of each visible cell for posts
/// `first..` (row-major), with `n` posts in total.
pub fn grid_cells(r: Rect, n: usize, first: usize) -> (usize, Vec<(usize, Rect)>) {
    let cols = (r.width / GRID_CELL_MIN_W).max(1);
    let cw = r.width / cols;
    let rows = (r.height / GRID_CELL_H).max(1);
    let mut v = Vec::new();
    for k in 0..(cols * rows) as usize {
        let i = first + k;
        if i >= n { break; }
        let (row, col) = ((k / cols as usize) as u16, (k % cols as usize) as u16);
        let w = if col + 1 == cols { r.width - cw * col } else { cw };
        let h = GRID_CELL_H.min(r.height.saturating_sub(row * GRID_CELL_H));
        if h == 0 { break; }
        v.push((i, Rect { x: r.x + col * cw, y: r.y + row * GRID_CELL_H, width: w, height: h }));
    }
    (cols as usize, v)
}

/// Grid rows that fit `r`.
pub fn grid_rows(r: Rect) -> usize { (r.height / GRID_CELL_H).max(1) as usize }

/// Carousel view: the centre card and the strips of the previous / next posts peeking at its sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carousel {
    /// Previous post's strip.
    pub prev: Rect,
    /// The current post.
    pub center: Rect,
    /// Next post's strip.
    pub next: Rect,
}

/// Lay out a carousel in `r` (side strips ≈ 1/8 of the width, at least 6 cells when it fits).
pub fn carousel(r: Rect) -> Carousel {
    let side = (r.width / 8).max(6).min(r.width / 4);
    let cw = r.width.saturating_sub(2 * side);
    Carousel { prev: Rect { width: side, ..r }, center: Rect { x: r.x + side, width: cw, ..r }, next: Rect { x: r.x + side + cw, width: r.width - side - cw, ..r } }
}

/// Rows a full card for `text` takes at `width` (borders included), at most `max`.
pub fn card_height(text: &str, width: u16, max: u16) -> u16 {
    let n = wrap(text.trim(), width.saturating_sub(2) as usize).iter().filter(|l| !l.trim().is_empty()).count() as u16;
    (n + 2).clamp(3, max.max(3))
}
