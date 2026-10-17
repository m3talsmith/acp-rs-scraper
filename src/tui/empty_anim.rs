//! The animation in the empty Body: text frames played on a loop.
//!
//! Frames come from `~/.acp/animations/empty-body.acpanim(.gz)` (made with
//! `tools/acp-anim`), `[ui] empty_animation_path`, or the built-in clip
//! (`assets/empty-body.acpanim.gz`, see `assets/empty-body.NOTICE.md`); if that
//! can't be read, or `[ui] empty_animation = "star"`, a procedural red star is
//! drawn instead. `[ui] empty_animation = false` or `ACP_EMPTY_ANIMATION=off`
//! turns it off.
//!
//! Format (gzip or plain UTF-8): `ACPANIM 1`, `fps N`, `size COLS ROWS`, then
//! each frame as a `%%` line followed by up to ROWS lines.

use crate::prelude::*;
use std::io::Read;
use std::time::Duration;

/// The built-in clip.
const BUILTIN: &[u8] = include_bytes!("../../assets/empty-body.acpanim.gz");

/// Decoded frames.
#[derive(Clone, Debug, PartialEq)]
pub struct Frames {
    /// Frames per second (1–60).
    pub fps: u16,
    /// Columns.
    pub width: u16,
    /// Rows.
    pub height: u16,
    /// Each frame: `height` rows of chars, padded to `width`.
    pub frames: Vec<Vec<Vec<char>>>,
    /// Where they came from (shown nowhere; for logs and tests).
    pub source: String,
}

/// Which animation to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// None.
    Off,
    /// The local file, else the built-in clip, else the star.
    Clip,
    /// The procedural star only.
    Star,
}

/// The setting: `ACP_EMPTY_ANIMATION` wins over `[ui] empty_animation`.
pub fn choice(env: Option<&str>, pref: Option<&toml::Value>) -> Choice {
    let word = |s: &str| match s.trim().to_ascii_lowercase().as_str() {
        "off" | "false" | "no" | "0" | "none" => Some(Choice::Off),
        "star" | "minimal" | "procedural" => Some(Choice::Star),
        "on" | "true" | "yes" | "1" | "clip" | "" => Some(Choice::Clip),
        _ => None,
    };
    if let Some(c) = env.and_then(word) { return c; }
    match pref {
        Some(toml::Value::Boolean(false)) => Choice::Off,
        Some(toml::Value::String(s)) => word(s).unwrap_or(Choice::Clip),
        _ => Choice::Clip,
    }
}

/// Parse a frames file (gzip or plain).
pub fn parse(bytes: &[u8], source: &str) -> Result<Frames> {
    let text = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut s = String::new();
        flate2::read::GzDecoder::new(bytes).take(64 * 1024 * 1024).read_to_string(&mut s).context("gunzip")?;
        s
    } else {
        String::from_utf8(bytes.to_vec()).context("not UTF-8")?
    };
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("ACPANIM 1") { bail!("not an ACPANIM 1 file"); }
    let (mut fps, mut size) = (12u16, None);
    let mut frames: Vec<Vec<Vec<char>>> = Vec::new();
    for l in lines {
        if l == "%%" { frames.push(Vec::new()); continue; }
        match frames.last_mut() {
            Some(f) => f.push(l.chars().collect()),
            None => {
                let mut it = l.split_whitespace();
                match it.next() {
                    Some("fps") => fps = it.next().and_then(|v| v.parse().ok()).filter(|v| (1..=60).contains(v)).context("bad fps")?,
                    Some("size") => {
                        let w: u16 = it.next().and_then(|v| v.parse().ok()).context("bad size")?;
                        let h: u16 = it.next().and_then(|v| v.parse().ok()).context("bad size")?;
                        if !(1..=1000).contains(&w) || !(1..=1000).contains(&h) { bail!("size {w}x{h} out of range"); }
                        size = Some((w, h));
                    }
                    _ => {}
                }
            }
        }
    }
    let (w, h) = size.context("missing size line")?;
    if frames.is_empty() { bail!("no frames"); }
    for f in &mut frames {
        f.truncate(h as usize);
        f.resize(h as usize, Vec::new());
        for row in f.iter_mut() { row.truncate(w as usize); row.resize(w as usize, ' '); }
    }
    Ok(Frames { fps, width: w, height: h, frames, source: source.to_string() })
}

/// The built-in clip, or the star if it can't be decoded.
pub fn builtin() -> Frames { parse(BUILTIN, "built-in").unwrap_or_else(|_| star()) }

/// What to play, given the setting, an optional path override and `$HOME`.
pub fn load(c: Choice, path: Option<&str>, home: Option<&Path>) -> Option<Frames> {
    match c {
        Choice::Off => return None,
        Choice::Star => return Some(star()),
        Choice::Clip => {}
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = path.filter(|p| !p.trim().is_empty()) {
        let p = p.trim();
        candidates.push(match (p.strip_prefix("~/"), home) { (Some(rest), Some(h)) => h.join(rest), _ => PathBuf::from(p) });
    } else if let Some(h) = home {
        let base = h.join(ANIMATION_REL_PATH);
        candidates.push(PathBuf::from(format!("{}.gz", base.display())));
        candidates.push(base);
    }
    for p in candidates {
        if let Ok(b) = std::fs::read(&p) {
            match parse(&b, &p.display().to_string()) {
                Ok(f) => return Some(f),
                Err(e) => info!("warning: empty-Body animation {}: {e:#}; using the built-in one", p.display()),
            }
        }
    }
    Some(builtin())
}

/// From the environment and `[ui]` prefs.
pub fn from_prefs(ui: Option<&UiPrefs>) -> (Option<Frames>, bool) {
    let env = std::env::var("ACP_EMPTY_ANIMATION").ok();
    let c = choice(env.as_deref(), ui.and_then(|u| u.empty_animation.as_ref()));
    let accent = ui.and_then(|u| u.empty_animation_color.as_deref()).is_some_and(|s| s.eq_ignore_ascii_case("accent"));
    (load(c, ui.and_then(|u| u.empty_animation_path.as_deref()), home().ok().as_deref()), accent)
}

impl Frames {
    /// Duration of one frame.
    pub fn frame_time(&self) -> Duration { Duration::from_micros(1_000_000 / self.fps.max(1) as u64) }
    /// The frame shown `elapsed` after the start (loops).
    pub fn index(&self, elapsed: Duration) -> usize {
        ((elapsed.as_micros() * self.fps as u128 / 1_000_000) % self.frames.len().max(1) as u128) as usize
    }
    /// Time from `elapsed` until the next frame starts.
    pub fn until_next(&self, elapsed: Duration) -> Duration {
        let ft = self.frame_time().as_micros().max(1);
        let into = elapsed.as_micros() % ft;
        Duration::from_micros((ft - into) as u64)
    }
    /// Frame `i` fitted into `w`×`h`: shrunk (never enlarged) keeping its aspect,
    /// by nearest-neighbour sampling. `None` when the area is too small (paused).
    pub fn fit(&self, i: usize, w: u16, h: u16) -> Option<Vec<String>> {
        if w < EMPTY_ANIM_MIN.0 || h < EMPTY_ANIM_MIN.1 || self.frames.is_empty() { return None; }
        let f = &self.frames[i % self.frames.len()];
        let s = (w as f64 / self.width as f64).min(h as f64 / self.height as f64).min(1.0);
        let ow = ((self.width as f64 * s).round() as usize).clamp(1, w as usize);
        let oh = ((self.height as f64 * s).round() as usize).clamp(1, h as usize);
        Some((0..oh).map(|y| {
            let sy = (y * self.height as usize / oh).min(self.height as usize - 1);
            (0..ow).map(|x| f[sy][(x * self.width as usize / ow).min(self.width as usize - 1)]).collect::<String>().trim_end().to_string()
        }).collect())
    }
}

/// A five-pointed red star turning slowly about its vertical axis, over an
/// "ACP" wordmark: 48 frames at 12 fps, 72×30, drawn from geometry alone.
pub fn star() -> Frames {
    const W: usize = 72;
    const H: usize = 30;
    const N: usize = 48;
    const RAMP: &[char] = &['.', ':', '-', '=', '+', '*', '#', '%', '@'];
    let word = ["  ███    ████  █████ ", " █   █  █      █    █", " █████  █      █████ ", " █   █  █      █     ", " █   █   ████  █     "];
    let (cx, cy, r_out) = (W as f64 / 2.0, 11.0, 10.5);
    let pts: Vec<(f64, f64)> = (0..10).map(|k| {
        let a = -std::f64::consts::FRAC_PI_2 + k as f64 * std::f64::consts::PI / 5.0;
        let r = if k % 2 == 0 { 1.0 } else { 0.382 };
        (r * a.cos(), r * a.sin())
    }).collect();
    let inside = |x: f64, y: f64| {
        let mut c = false;
        let mut j = pts.len() - 1;
        for i in 0..pts.len() {
            let (xi, yi) = pts[i];
            let (xj, yj) = pts[j];
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi { c = !c; }
            j = i;
        }
        c
    };
    let frames = (0..N).map(|n| {
        let th = n as f64 / N as f64 * std::f64::consts::TAU;
        let (c, sn) = (th.cos(), th.sin());
        let mut g = vec![vec![' '; W]; H];
        for (y, row) in g.iter_mut().enumerate().take(21) {
            for (x, cell) in row.iter_mut().enumerate() {
                // Cells are about twice as tall as wide: x spans twice the units.
                let u = (x as f64 + 0.5 - cx) / (2.0 * r_out);
                let v = (y as f64 + 0.5 - cy) / r_out;
                if c.abs() < 0.04 { if u.abs() < 0.03 && v.abs() < 1.0 { *cell = '|'; } continue; }
                let su = u / c;
                if !inside(su, v) { continue; }
                // Light from the upper left; brighter towards the centre and facing us.
                let shade = (0.55 + 0.45 * c.abs()) * (1.0 - 0.35 * (su * su + v * v).sqrt()) + 0.1 * (-su * sn.signum() - v) * 0.5;
                let k = ((shade.clamp(0.0, 0.999)) * RAMP.len() as f64) as usize;
                *cell = RAMP[k.min(RAMP.len() - 1)];
            }
        }
        for (i, l) in word.iter().enumerate() {
            let chars: Vec<char> = l.chars().collect();
            let x0 = (W - chars.len()) / 2;
            for (k, ch) in chars.into_iter().enumerate() { g[23 + i][x0 + k] = ch; }
        }
        g
    }).collect();
    Frames { fps: 12, width: W as u16, height: H as u16, frames, source: "star".into() }
}

