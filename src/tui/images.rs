//! Pictures in the terminal: the header logo and post images.
//!
//! The graphics protocol is found once at start by querying the terminal
//! (kitty graphics reply, sixel in the DA1 attributes, cell size via
//! `CSI 16 t`), with iTerm2 inline images picked from the environment (iTerm2,
//! WezTerm). Anything that doesn't answer — the Linux console, tmux, many SSH,
//! waypipe and Windows-terminal setups — gets Unicode half blocks, which are
//! drawn straight into the ratatui buffer and work everywhere.
//!
//! Graphics protocols are drawn after ratatui's frame, like the OSC 8 links:
//! widgets reserve blank cells and record a [`Placement`]; [`Images::flush`]
//! emits the escapes only when the placements change (kitty deletes and
//! re-places; sixel / iTerm2 ask for a full repaint first so no fragments stay).
//! Images decode on a worker thread; AVIF/HEIC (the site's format) are
//! converted once to a cached PNG with `avifdec`, `magick` or `ffmpeg`.
//!
//! This is hand-rolled rather than ratatui-image because that crate's sixel
//! encoder pulls in a quantiser that needs Rust 1.90, above our MSRV (1.88).

use crate::prelude::*;
use image::{DynamicImage, RgbaImage};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// The red star logo (`assets/red-star.png`), embedded.
pub const LOGO_PNG: &[u8] = include_bytes!("../../assets/red-star.png");
/// Largest side of a decoded post image, in pixels (keeps scaling cheap).
pub const MAX_IMAGE_PX: u32 = 1024;
/// Rows an inline post image takes in a detail.
pub const IMAGE_ROWS: u16 = 14;
/// Cell size assumed when the terminal doesn't report one (pixels).
pub const DEFAULT_CELL: (u16, u16) = (10, 20);

/// How pictures reach the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    /// Unicode `▀` cells with true-colour fg/bg (works everywhere).
    Halfblocks,
    /// Kitty graphics protocol.
    Kitty,
    /// DEC sixel.
    Sixel,
    /// iTerm2 inline images (OSC 1337), also WezTerm.
    Iterm2,
}

impl Protocol {
    /// Name for the status line and `ACP_IMAGE_PROTOCOL`.
    pub fn name(self) -> &'static str {
        match self { Protocol::Halfblocks => "halfblocks", Protocol::Kitty => "kitty", Protocol::Sixel => "sixel", Protocol::Iterm2 => "iterm2" }
    }
    /// Parse `ACP_IMAGE_PROTOCOL`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "halfblocks" | "blocks" => Some(Protocol::Halfblocks), "kitty" => Some(Protocol::Kitty),
            "sixel" => Some(Protocol::Sixel), "iterm2" | "iterm" => Some(Protocol::Iterm2), _ => None,
        }
    }
}

/// A decoded picture.
pub struct Pic {
    /// Kitty image id (unique per picture).
    pub id: u32,
    /// Pixels.
    pub img: RgbaImage,
}

/// Where one image stands.
pub enum ImgState {
    /// Decoding on the worker.
    Loading,
    /// Ready to draw.
    Ready(Pic),
    /// Could not be decoded (the reason).
    Failed(String),
}

/// A graphics-protocol picture to draw after the frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// Picture id.
    pub id: u32,
    /// Cells it covers (already fitted to the aspect ratio).
    pub rect: Rect,
}

/// What [`Images::flush`] needs from the caller.
#[derive(Debug, PartialEq, Eq)]
pub enum Flush {
    /// Done.
    Done,
    /// Clear the terminal, redraw, and flush again (sixel / iTerm2 leftovers).
    Repaint,
}

type Decoded = (PathBuf, std::result::Result<DynamicImage, String>);

/// Image support for the TUI.
pub struct Images {
    /// Protocol in use.
    pub protocol: Protocol,
    /// Cell size in pixels (width, height).
    pub cell: (u16, u16),
    logo: Option<Pic>,
    badge: [u8; 3],
    cache: HashMap<PathBuf, ImgState>,
    next_id: u32,
    tx: Option<std::sync::mpsc::Sender<PathBuf>>,
    rx: Option<std::sync::mpsc::Receiver<Decoded>>,
    /// Placements recorded this frame.
    pub frame: Vec<Placement>,
    last: Vec<Placement>,
    transmitted: HashSet<u32>,
    scaled: HashMap<(u32, u16, u16), RgbaImage>,
    encoded: HashMap<(u32, u16, u16), String>,
}

impl Images {
    /// Images drawn with `protocol`; `background` decodes on a worker thread (else
    /// inline, for tests). The logo is laid on a `badge` colour so the red star
    /// stays visible on the red header.
    pub fn new(protocol: Protocol, cell: (u16, u16), background: bool, badge: [u8; 3]) -> Self {
        let logo = logo_on(badge).map(|i| Pic { id: 1, img: i.to_rgba8() });
        let (tx, rx) = if background {
            let (tx, rx_req) = std::sync::mpsc::channel::<PathBuf>();
            let (tx_res, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                while let Ok(p) = rx_req.recv() {
                    let r = decode(&p).map_err(|e| format!("{e:#}"));
                    if tx_res.send((p, r)).is_err() { break; }
                }
            });
            (Some(tx), Some(rx))
        } else { (None, None) };
        let cell = (cell.0.max(1), cell.1.max(2));
        Images { protocol, cell, logo, badge, cache: HashMap::new(), next_id: 2, tx, rx, frame: Vec::new(), last: Vec::new(),
            transmitted: HashSet::new(), scaled: HashMap::new(), encoded: HashMap::new() }
    }

    /// Detect the terminal's graphics support (call in raw mode, before the input
    /// thread reads stdin). `ACP_IMAGES=off` disables pictures;
    /// `ACP_IMAGE_PROTOCOL=halfblocks|sixel|kitty|iterm2` forces a protocol.
    pub fn detect(badge: [u8; 3]) -> Option<Self> {
        if std::env::var("ACP_IMAGES").is_ok_and(|v| matches!(v.as_str(), "off" | "0" | "no")) { return None; }
        let forced = std::env::var("ACP_IMAGE_PROTOCOL").ok().and_then(|s| Protocol::parse(&s));
        let in_tmux = std::env::var_os("TMUX").is_some();
        let reply = if forced == Some(Protocol::Halfblocks) || in_tmux { String::new() } else { query_terminal() };
        let mut p = parse_reply(&reply);
        if p.0 == Protocol::Halfblocks && !in_tmux && iterm2_env() { p.0 = Protocol::Iterm2; }
        let cell = p.1.or_else(cell_from_winsize).unwrap_or(DEFAULT_CELL);
        Some(Images::new(forced.unwrap_or(p.0), cell, true, badge))
    }

    /// The state of `path`, starting its decode on first use.
    pub fn get(&mut self, path: &Path) -> &mut ImgState {
        if !self.cache.contains_key(path) {
            let st = match &self.tx {
                Some(tx) => { let _ = tx.send(path.to_path_buf()); ImgState::Loading }
                None => match decode(path) {
                    Ok(i) => { self.next_id += 1; ImgState::Ready(Pic { id: self.next_id, img: i.to_rgba8() }) }
                    Err(e) => ImgState::Failed(format!("{e:#}")),
                },
            };
            self.cache.insert(path.to_path_buf(), st);
        }
        self.cache.get_mut(path).expect("just inserted")
    }

    /// Take finished decodes from the worker; true when something changed.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        let mut done = Vec::new();
        while let Ok(d) = rx.try_recv() { done.push(d); }
        let changed = !done.is_empty();
        for (p, r) in done {
            let st = match r {
                Ok(i) => { self.next_id += 1; ImgState::Ready(Pic { id: self.next_id, img: i.to_rgba8() }) }
                Err(e) => ImgState::Failed(e),
            };
            self.cache.insert(p, st);
        }
        changed
    }

    /// Draw the logo into `area`. With half blocks it is drawn in two colours
    /// with sextant cells instead (2×3 dots per cell), which keeps the star's
    /// points at header size.
    pub fn draw_logo(&mut self, buf: &mut Buffer, area: Rect) -> bool {
        let Some(pic) = self.logo.take() else { return false };
        if self.protocol == Protocol::Halfblocks {
            let (cols, rows) = fit(pic.img.width(), pic.img.height(), self.cell, area.width, area.height);
            let rect = Rect { x: area.x + (area.width - cols) / 2, y: area.y + (area.height - rows) / 2, width: cols, height: rows };
            let small = self.scaled.entry((pic.id, cols, rows)).or_insert_with(|| {
                image::imageops::resize(&pic.img, u32::from(cols) * 2, u32::from(rows) * 3, image::imageops::FilterType::Triangle)
            });
            let b = self.badge;
            let star = |p: &image::Rgba<u8>| {
                let d = |i: usize| (i32::from(p.0[i]) - i32::from(b[i])).abs();
                d(0) + d(1) + d(2) > 120
            };
            sextants(buf, rect, small, star, Color::Rgb(193, 22, 22), Color::Rgb(b[0], b[1], b[2]));
        } else {
            self.place(buf, area, &pic, true, true);
        }
        self.logo = Some(pic);
        true
    }

    /// Draw the picture at `path` into `area` (left-aligned, aspect kept).
    /// `graphics = false` forces half blocks (e.g. under a modal). Err = what to show instead.
    pub fn draw_file(&mut self, buf: &mut Buffer, area: Rect, path: &Path, graphics: bool) -> std::result::Result<(), String> {
        let pic = match self.get(path) {
            ImgState::Loading => return Err("… loading picture".into()),
            ImgState::Failed(e) => return Err(format!("▣ picture unavailable: {e}")),
            ImgState::Ready(_) => match self.cache.remove(path) { Some(ImgState::Ready(p)) => p, _ => unreachable!() },
        };
        self.place(buf, area, &pic, graphics, false);
        self.cache.insert(path.to_path_buf(), ImgState::Ready(pic));
        Ok(())
    }

    fn place(&mut self, buf: &mut Buffer, area: Rect, pic: &Pic, graphics: bool, centre: bool) {
        let (cols, rows) = fit(pic.img.width(), pic.img.height(), self.cell, area.width, area.height);
        if cols == 0 || rows == 0 { return; }
        let x = if centre { area.x + (area.width - cols) / 2 } else { area.x };
        let y = if centre { area.y + (area.height - rows) / 2 } else { area.y };
        let rect = Rect { x, y, width: cols, height: rows };
        if graphics && self.protocol != Protocol::Halfblocks {
            for cy in rect.top()..rect.bottom() { for cx in rect.left()..rect.right() {
                if let Some(c) = buf.cell_mut((cx, cy)) { c.set_symbol(" "); }
            } }
            self.frame.push(Placement { id: pic.id, rect });
            return;
        }
        let key = (pic.id, cols, rows);
        let small = self.scaled.entry(key).or_insert_with(|| {
            image::imageops::resize(&pic.img, u32::from(cols), u32::from(rows) * 2, image::imageops::FilterType::Triangle)
        });
        halfblocks(buf, rect, small);
    }

    /// Emit this frame's graphics (after ratatui drew). Writes nothing when the
    /// placements didn't change.
    pub fn flush(&mut self, out: &mut impl Write) -> Flush {
        let frame = std::mem::take(&mut self.frame);
        if frame == self.last { return Flush::Done; }
        if matches!(self.protocol, Protocol::Sixel | Protocol::Iterm2) && !self.last.is_empty() {
            self.last.clear();
            return Flush::Repaint;
        }
        let mut s = String::new();
        if self.protocol == Protocol::Kitty { s.push_str("\x1b_Ga=d,d=a,q=2\x1b\\"); }
        for (n, p) in frame.iter().enumerate() {
            s.push_str(&format!("\x1b7\x1b[{};{}H", p.rect.y + 1, p.rect.x + 1));
            match self.protocol {
                Protocol::Kitty => {
                    if !self.transmitted.contains(&p.id)
                        && let Some(img) = self.pixels(p.id) {
                            s.push_str(&kitty_transmit(p.id, &png_bytes(img)));
                            self.transmitted.insert(p.id);
                        }
                    s.push_str(&format!("\x1b_Ga=p,i={},p={},c={},r={},C=1,q=2\x1b\\", p.id, n + 1, p.rect.width, p.rect.height));
                }
                Protocol::Sixel => {
                    let key = (p.id, p.rect.width, p.rect.height);
                    if !self.encoded.contains_key(&key) {
                        let (w, h) = (u32::from(p.rect.width) * u32::from(self.cell.0), u32::from(p.rect.height) * u32::from(self.cell.1));
                        if let Some(img) = self.pixels(p.id) {
                            let small = image::imageops::resize(img, w, h, image::imageops::FilterType::Triangle);
                            self.encoded.insert(key, sixel(&small));
                        }
                    }
                    if let Some(e) = self.encoded.get(&key) { s.push_str(e); }
                }
                Protocol::Iterm2 => {
                    if let Some(img) = self.pixels(p.id) {
                        let png = png_bytes(img);
                        s.push_str(&format!("\x1b]1337;File=inline=1;size={};width={};height={};preserveAspectRatio=1;doNotMoveCursor=1:{}\x07",
                            png.len(), p.rect.width, p.rect.height, base64(&png)));
                    }
                }
                Protocol::Halfblocks => {}
            }
            s.push_str("\x1b8");
        }
        let _ = out.write_all(s.as_bytes());
        let _ = out.flush();
        self.last = frame;
        Flush::Done
    }

    /// Forget what is on screen (after a resize or clear), so the next flush re-places everything.
    pub fn invalidate(&mut self) { self.last.clear(); }

    /// Remove every kitty picture (on exit).
    pub fn clear_screen_images(&self, out: &mut impl Write) {
        if self.protocol == Protocol::Kitty { let _ = out.write_all(b"\x1b_Ga=d,d=A,q=2\x1b\\"); let _ = out.flush(); }
    }

    fn pixels(&self, id: u32) -> Option<&RgbaImage> {
        if let Some(l) = &self.logo && l.id == id { return Some(&l.img); }
        self.cache.values().find_map(|s| match s { ImgState::Ready(p) if p.id == id => Some(&p.img), _ => None })
    }
}

/// Cells (cols, rows) a `w`×`h` picture fills inside `max_cols`×`max_rows`, keeping its aspect.
pub fn fit(w: u32, h: u32, cell: (u16, u16), max_cols: u16, max_rows: u16) -> (u16, u16) {
    if w == 0 || h == 0 || max_cols == 0 || max_rows == 0 { return (0, 0); }
    let (cw, ch) = (f64::from(cell.0), f64::from(cell.1));
    let scale = (f64::from(max_cols) * cw / f64::from(w)).min(f64::from(max_rows) * ch / f64::from(h));
    let cols = ((f64::from(w) * scale / cw).round() as u16).clamp(1, max_cols);
    let rows = ((f64::from(h) * scale / ch).round() as u16).clamp(1, max_rows);
    (cols, rows)
}

/// Draw `img` (cols × 2·rows pixels) as `▀` / `▄` cells; transparent pixels leave the cell's background.
pub fn halfblocks(buf: &mut Buffer, rect: Rect, img: &RgbaImage) {
    let px = |x: u32, y: u32| img.get_pixel_checked(x, y).filter(|p| p.0[3] >= 128).map(|p| Color::Rgb(p.0[0], p.0[1], p.0[2]));
    for row in 0..rect.height {
        for col in 0..rect.width {
            let (top, bot) = (px(u32::from(col), u32::from(row) * 2), px(u32::from(col), u32::from(row) * 2 + 1));
            let Some(c) = buf.cell_mut((rect.x + col, rect.y + row)) else { continue };
            match (top, bot) {
                (Some(t), Some(b)) => { c.set_symbol("▀").set_fg(t).set_bg(b); }
                (Some(t), None) => { c.set_symbol("▀").set_fg(t); }
                (None, Some(b)) => { c.set_symbol("▄").set_fg(b); }
                (None, None) => {}
            }
        }
    }
}

/// The sextant character for 6 dots (bit 0 top-left, 1 top-right, 2 middle-left, …, 5 bottom-right).
pub fn sextant(bits: u8) -> char {
    match bits & 63 {
        0 => ' ',
        63 => '█',
        21 => '▌',
        42 => '▐',
        n => {
            let off = if n < 21 { n - 1 } else if n < 42 { n - 2 } else { n - 3 };
            char::from_u32(0x1FB00 + u32::from(off)).unwrap_or('█')
        }
    }
}

/// Draw `img` (2·cols × 3·rows pixels) in two colours: `on` pixels in `fg`, the rest `bg`.
pub fn sextants(buf: &mut Buffer, rect: Rect, img: &RgbaImage, on: impl Fn(&image::Rgba<u8>) -> bool, fg: Color, bg: Color) {
    for row in 0..rect.height {
        for col in 0..rect.width {
            let mut bits = 0u8;
            for dy in 0..3 { for dx in 0..2 {
                let (x, y) = (u32::from(col) * 2 + dx, u32::from(row) * 3 + dy);
                if img.get_pixel_checked(x, y).is_some_and(&on) { bits |= 1 << (dy * 2 + dx); }
            } }
            if let Some(c) = buf.cell_mut((rect.x + col, rect.y + row)) {
                c.set_char(sextant(bits)).set_fg(fg).set_bg(bg);
            }
        }
    }
}

/// Standard base64 (no line breaks).
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for ch in data.chunks(3) {
        let b = [ch[0], *ch.get(1).unwrap_or(&0), *ch.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= ch.len() { s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char); } else { s.push('='); }
        }
    }
    s
}

fn png_bytes(img: &RgbaImage) -> Vec<u8> {
    let mut v = std::io::Cursor::new(Vec::new());
    let _ = DynamicImage::ImageRgba8(img.clone()).write_to(&mut v, image::ImageFormat::Png);
    v.into_inner()
}

/// Kitty: transmit a PNG as image `id`, in 4096-byte chunks.
pub fn kitty_transmit(id: u32, png: &[u8]) -> String {
    let b = base64(png);
    let chunks: Vec<&[u8]> = b.as_bytes().chunks(4096).collect();
    let mut s = String::new();
    for (i, c) in chunks.iter().enumerate() {
        let more = u8::from(i + 1 < chunks.len());
        let c = std::str::from_utf8(c).unwrap_or_default();
        if i == 0 { s.push_str(&format!("\x1b_Ga=t,f=100,i={id},q=2,m={more};{c}\x1b\\")); }
        else { s.push_str(&format!("\x1b_Gm={more};{c}\x1b\\")); }
    }
    s
}

/// Encode RGBA as sixel (6×6×6 colour cube; transparent pixels left alone).
pub fn sixel(img: &RgbaImage) -> String {
    let (w, h) = img.dimensions();
    let q = |v: u8| (u16::from(v) * 5 + 127) / 255;
    let idx = |x: u32, y: u32| -> Option<u16> {
        let p = img.get_pixel(x, y).0;
        (p[3] >= 128).then(|| q(p[0]) * 36 + q(p[1]) * 6 + q(p[2]))
    };
    let mut s = format!("\x1bP0;1;0q\"1;1;{w};{h}");
    let mut used = [false; 216];
    for y in 0..h { for x in 0..w { if let Some(i) = idx(x, y) { used[i as usize] = true; } } }
    for (i, u) in used.iter().enumerate() {
        if *u { let c = |k: usize| k * 100 / 5; s.push_str(&format!("#{i};2;{};{};{}", c(i / 36), c(i / 6 % 6), c(i % 6))); }
    }
    let mut band = 0;
    while band < h {
        let mut cols: Vec<u16> = Vec::new();
        let mut bits = vec![0u8; (w as usize) * 216];
        for dy in 0..6 {
            let y = band + dy;
            if y >= h { break; }
            for x in 0..w {
                if let Some(i) = idx(x, y) {
                    let b = &mut bits[i as usize * w as usize + x as usize];
                    if *b == 0 && !cols.contains(&i) { cols.push(i); }
                    *b |= 1 << dy;
                }
            }
        }
        for (n, c) in cols.iter().enumerate() {
            if n > 0 { s.push('$'); }
            s.push_str(&format!("#{c}"));
            let row = &bits[*c as usize * w as usize..(*c as usize + 1) * w as usize];
            let mut x = 0;
            while x < row.len() {
                let v = row[x];
                let mut run = 1;
                while x + run < row.len() && row[x + run] == v { run += 1; }
                let ch = (63 + v) as char;
                if run > 3 { s.push_str(&format!("!{run}{ch}")); } else { for _ in 0..run { s.push(ch); } }
                x += run;
            }
        }
        s.push('-');
        band += 6;
    }
    s.push_str("\x1b\\");
    s
}

/// Read what the terminal says to: a kitty graphics query, the cell-size
/// report (`CSI 16 t`) and the primary device attributes (DA1, answered by
/// everything, so it ends the wait). Gives up after 400 ms.
#[cfg(unix)]
fn query_terminal() -> String {
    let q = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[16t\x1b[c";
    let mut out = std::io::stdout();
    if out.write_all(q).and_then(|_| out.flush()).is_err() { return String::new(); }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(400);
    let mut got = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() { break; }
        let mut fds = libc::pollfd { fd: libc::STDIN_FILENO, events: libc::POLLIN, revents: 0 };
        // SAFETY: one valid pollfd for the duration of the call.
        let r = unsafe { libc::poll(&mut fds, 1, left.as_millis() as libc::c_int) };
        if r <= 0 { break; }
        let mut b = [0u8; 256];
        // SAFETY: reading into a local buffer of the given length.
        let n = unsafe { libc::read(libc::STDIN_FILENO, b.as_mut_ptr().cast(), b.len()) };
        if n <= 0 { break; }
        got.extend_from_slice(&b[..n as usize]);
        let s = String::from_utf8_lossy(&got);
        if let Some(i) = s.find("\x1b[?") && s[i..].contains('c') { break; }
    }
    String::from_utf8_lossy(&got).into_owned()
}

#[cfg(not(unix))]
fn query_terminal() -> String { String::new() }

/// The protocol and cell size a terminal's replies announce.
pub fn parse_reply(r: &str) -> (Protocol, Option<(u16, u16)>) {
    let cell = r.find("\x1b[6;").and_then(|i| {
        let rest = &r[i + 4..];
        let end = rest.find('t')?;
        let mut it = rest[..end].split(';');
        let h: u16 = it.next()?.parse().ok()?;
        let w: u16 = it.next()?.parse().ok()?;
        (w > 0 && h > 0).then_some((w, h))
    });
    if r.contains("\x1b_Gi=31;OK") { return (Protocol::Kitty, cell); }
    let sixel = r.find("\x1b[?").and_then(|i| {
        let rest = &r[i + 3..];
        let end = rest.find('c')?;
        Some(rest[..end].split(';').any(|p| p == "4"))
    }).unwrap_or(false);
    (if sixel { Protocol::Sixel } else { Protocol::Halfblocks }, cell)
}

fn iterm2_env() -> bool {
    matches!(std::env::var("TERM_PROGRAM").as_deref(), Ok("iTerm.app") | Ok("WezTerm"))
        || std::env::var("LC_TERMINAL").is_ok_and(|v| v == "iTerm2")
}

fn cell_from_winsize() -> Option<(u16, u16)> {
    let ws = crossterm::terminal::window_size().ok()?;
    (ws.width > 0 && ws.columns > 0 && ws.rows > 0).then(|| (ws.width / ws.columns, ws.height / ws.rows)).filter(|c| c.0 > 0 && c.1 > 0)
}

/// The logo composited on a square of `badge` colour (with a small margin).
pub fn logo_on(badge: [u8; 3]) -> Option<DynamicImage> {
    let star = image::load_from_memory(LOGO_PNG).ok()?.to_rgba8();
    let m = star.width() / 10;
    let side = star.width().max(star.height()) + 2 * m;
    let mut bg = RgbaImage::from_pixel(side, side, image::Rgba([badge[0], badge[1], badge[2], 255]));
    image::imageops::overlay(&mut bg, &star, i64::from(m), i64::from(m));
    Some(DynamicImage::ImageRgba8(bg))
}

/// Whether a stored attachment is a picture the TUI can show.
pub fn is_image(mime: &str, path: &Path) -> bool {
    mime.starts_with("image/") || matches!(ext(path).as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "heic")
}

fn ext(p: &Path) -> String { p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default() }

/// Decode `path` (converting AVIF/HEIC first), shrunk to [`MAX_IMAGE_PX`].
pub fn decode(path: &Path) -> Result<DynamicImage> {
    if !path.exists() { bail!("file missing"); }
    let src = if matches!(ext(path).as_str(), "avif" | "heic" | "heif") { convert_to_png(path)? } else { path.to_path_buf() };
    let img = image::ImageReader::open(&src)?.with_guessed_format()?.decode()
        .with_context(|| format!("decoding {}", src.display()))?;
    Ok(if img.width() > MAX_IMAGE_PX || img.height() > MAX_IMAGE_PX { img.thumbnail(MAX_IMAGE_PX, MAX_IMAGE_PX) } else { img })
}

/// Cached PNG for a format the image crate can't read: `~/.acp/cache/thumbs/<hash>.png`.
pub fn convert_to_png(path: &Path) -> Result<PathBuf> {
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok()).hash(&mut h);
    let dir = home()?.join(THUMBS_REL_DIR);
    std::fs::create_dir_all(&dir)?;
    let out = dir.join(format!("{:016x}.png", h.finish()));
    if out.exists() { return Ok(out); }
    let tmp = out.with_extension("tmp.png");
    let size = format!("{MAX_IMAGE_PX}x{MAX_IMAGE_PX}>");
    let scale = format!("scale='min({MAX_IMAGE_PX},iw)':-2");
    let tries: [(&str, Vec<&std::ffi::OsStr>); 3] = [
        ("avifdec", vec![path.as_os_str(), tmp.as_os_str()]),
        ("magick", vec![path.as_os_str(), "-resize".as_ref(), size.as_ref(), tmp.as_os_str()]),
        ("ffmpeg", vec!["-loglevel".as_ref(), "error".as_ref(), "-y".as_ref(), "-i".as_ref(), path.as_os_str(), "-vf".as_ref(), scale.as_ref(), "-frames:v".as_ref(), "1".as_ref(), tmp.as_os_str()]),
    ];
    let mut tried = Vec::new();
    for (cmd, args) in tries {
        let r = std::process::Command::new(cmd).args(&args)
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        match r {
            Ok(s) if s.success() && tmp.exists() => { std::fs::rename(&tmp, &out)?; return Ok(out); }
            Ok(_) => tried.push(format!("{cmd} failed")),
            Err(_) => tried.push(format!("{cmd} not found")),
        }
    }
    let _ = std::fs::remove_file(&tmp);
    bail!("cannot convert {} to PNG ({}); install libavif (avifdec), ImageMagick or ffmpeg", path.display(), tried.join(", "))
}
