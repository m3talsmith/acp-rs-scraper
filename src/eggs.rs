//! Hidden surprises. The write-up is `docs/.secrets/EASTER_EGGS.md`.
//!
//! Nothing here talks to the network. Each clip lasts two seconds.

use crossterm::event::KeyCode;
use std::sync::LazyLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Lines from the roar clip.
pub const ROAR_QUOTES: &[&str] = &[
    "What a man is made of! Where's the passion in your soul?",
    "Wake the fuck up!",
    "We get one life. Why not be an explosion?",
    "You have one life to live. Be worthy of the cosmic catastrophes all around us.",
    "You have one fucking life",
];

/// Who the roar lines belong to.
pub const ATTRIBUTION: &str = "— Haz al Din (@hazaldin)";

/// The line under the star burst.
pub const BURST_LINE: &str = "Be an explosive fucking nebula";

/// The search phrase that flashes the starfield. Matched trimmed, ignoring case.
pub const COSMOS_QUERY: &str = "one fucking life";

/// Presses on a star before the credits open.
pub const STAR_TAPS: u8 = 5;

/// A longer gap than this starts the star count over.
pub const TAP_GAP: Duration = Duration::from_secs(4);

/// How wide a terminal must be before the art (rather than one asterisk) is drawn.
const MIN_COLS: u16 = 20;

/// How tall a terminal must be before the art is drawn.
const MIN_ROWS: u16 = 6;

const STAR: &str = "\
       *\n\
      ***\n\
   *********\n\
      ***\n\
       *";

/// `true` when this process is drawing plain text.
///
/// `--color never` is off. `--color always` is on. `auto` follows a non-empty `NO_COLOR`.
/// Call [`crate::cli::apply_color_mode`] first so the flag is the one in force.
pub fn colors_off() -> bool {
    crate::cli::colors_suppressed(crate::cli::current_color_mode(), crate::cli::no_color_set())
}

/// `true` when `q` is exactly the cosmos phrase.
pub fn is_cosmos_query(q: &str) -> bool {
    q.trim().eq_ignore_ascii_case(COSMOS_QUERY)
}

/// Which quote to print, from the clock. Tests pass an index to [`roar_text`] instead.
pub fn roar_index() -> usize {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    n as usize % ROAR_QUOTES.len()
}

/// Columns for the roar art: `COLUMNS`, then the terminal, then 80.
pub fn columns() -> u16 {
    if let Ok(s) = std::env::var("COLUMNS")
        && let Ok(n) = s.parse::<u16>()
        && n > 0
    {
        return n;
    }
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80)
}

/// The roar: an ASCII star, one quote, and the attribution.
///
/// `plain` skips the red ANSI colour. Fewer than [`MIN_COLS`] columns is a single asterisk.
pub fn roar_text(index: usize, plain: bool, columns: u16) -> String {
    let quote = ROAR_QUOTES[index % ROAR_QUOTES.len()];
    let star = if columns < MIN_COLS { "*".to_string() } else { STAR.to_string() };
    let star = if plain { star } else { format!("\u{1b}[31m{star}\u{1b}[0m") };
    format!("{star}\n\n\"{quote}\"\n{ATTRIBUTION}")
}

/// Count one star press. `true` when this press reaches [`STAR_TAPS`].
///
/// A gap longer than [`TAP_GAP`] since `last` starts the count over.
pub fn note_tap(taps: &mut u8, last: &mut Option<Instant>, now: Instant) -> bool {
    if last.is_some_and(|t| now.saturating_duration_since(t) > TAP_GAP) {
        *taps = 0;
    }
    *taps = taps.saturating_add(1);
    *last = Some(now);
    if *taps >= STAR_TAPS {
        *taps = 0;
        *last = None;
        true
    } else {
        false
    }
}

/// Which clip is playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EggKind {
    /// The star burst and its line.
    Burst,
    /// The starfield in front of search results.
    Cosmos,
}

/// A clip in progress.
#[derive(Clone, Copy, Debug)]
pub struct EggPlay {
    /// Which clip.
    pub kind: EggKind,
    /// When it started.
    pub start: Instant,
    /// Draw with the terminal's own colour.
    pub plain: bool,
}

impl EggPlay {
    /// Start `kind` now. `plain` follows [`colors_off`].
    pub fn start(kind: EggKind) -> Self {
        Self { kind, start: Instant::now(), plain: colors_off() }
    }
    fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.start)
    }
    /// The clip has reached its last frame's end. It does not loop.
    pub fn finished(&self, now: Instant) -> bool {
        self.elapsed(now) >= clip(self.kind).duration()
    }
    /// How long until the next frame is due.
    pub fn until_next(&self, now: Instant) -> Duration {
        let frame = Duration::from_millis(1000 / clip(self.kind).fps as u64);
        let step = frame.as_millis().max(1) as u64;
        let into = self.elapsed(now).as_millis() as u64 % step;
        frame.saturating_sub(Duration::from_millis(into)).max(Duration::from_millis(1))
    }
}

/// How long `kind` plays.
pub fn clip_duration(kind: EggKind) -> Duration {
    clip(kind).duration()
}

/// One frame of `kind` at `elapsed`, fitted to `width` × `height`.
///
/// Below [`MIN_COLS`] × [`MIN_ROWS`] this is a single asterisk (and, for the burst, the line).
pub fn frame_lines(kind: EggKind, elapsed: Duration, width: u16, height: u16) -> Vec<String> {
    if width < MIN_COLS || height < MIN_ROWS {
        return match kind {
            EggKind::Burst => {
                let mut v = vec!["*".to_string()];
                if height > 1 {
                    v.push(clip_to(BURST_LINE, width));
                }
                v
            }
            EggKind::Cosmos => vec!["*".to_string()],
        };
    }
    let c = clip(kind);
    let frame = &c.frames[c.index(elapsed)];
    let mut rows: Vec<String> = frame.iter().map(|row| clip_to(row.trim_end(), width)).filter(|s| !s.is_empty()).collect();
    if rows.len() as u16 > height {
        rows.truncate(height as usize);
    }
    rows
}

/// The public About page: the name, the version, and a star.
pub fn about_lines() -> Vec<String> {
    vec![
        String::new(),
        "ACP Red Star".into(),
        format!("v{}", env!("CARGO_PKG_VERSION")),
        String::new(),
        "★".into(),
        String::new(),
        "Esc closes.".into(),
    ]
}

/// The hidden credits.
pub fn credits_lines() -> Vec<String> {
    vec![
        "Neon Lust".into(),
        "Michael's band. The demo music is a Neon Lust song he is writing. Guitar by him.".into(),
        String::new(),
        "Haz al Din (@hazaldin)".into(),
        "The roar clip, from IGG | Qizilbash.".into(),
        "https://www.youtube.com/@IGG-QZBSH".into(),
        String::new(),
        "Contributors".into(),
        "The people who built this client.".into(),
        String::new(),
        "For the working class,".into(),
        "~ @m3talsmith".into(),
    ]
}

/// Progress through ↑↑↓↓←→←→ b a.
#[derive(Clone, Copy, Debug, Default)]
pub struct Konami {
    pos: usize,
}

impl Konami {
    /// No keys yet.
    pub fn new() -> Self { Self { pos: 0 } }
    /// Forget any partial sequence.
    pub fn reset(&mut self) { self.pos = 0; }
    /// Feed one key. `true` when the sequence just completed (that key should be swallowed).
    pub fn feed(&mut self, code: KeyCode) -> bool {
        let Some(step) = step_of(code) else {
            self.pos = 0;
            return false;
        };
        if step == SEQ[self.pos] {
            self.pos += 1;
            if self.pos == SEQ.len() {
                self.pos = 0;
                return true;
            }
            return false;
        }
        self.pos = if step == SEQ[0] { 1 } else { 0 };
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step { Up, Down, Left, Right, B, A }

const SEQ: [Step; 10] = [
    Step::Up, Step::Up, Step::Down, Step::Down, Step::Left, Step::Right, Step::Left, Step::Right, Step::B, Step::A,
];

fn step_of(code: KeyCode) -> Option<Step> {
    match code {
        KeyCode::Up => Some(Step::Up),
        KeyCode::Down => Some(Step::Down),
        KeyCode::Left => Some(Step::Left),
        KeyCode::Right => Some(Step::Right),
        KeyCode::Char('b') | KeyCode::Char('B') => Some(Step::B),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(Step::A),
        _ => None,
    }
}

struct Clip {
    fps: u16,
    frames: Vec<Vec<String>>,
}

impl Clip {
    fn duration(&self) -> Duration {
        Duration::from_millis(self.frames.len() as u64 * 1000 / self.fps as u64)
    }
    fn index(&self, elapsed: Duration) -> usize {
        let i = (elapsed.as_millis() as u64 * self.fps as u64 / 1000) as usize;
        i.min(self.frames.len().saturating_sub(1))
    }
}

fn clip(kind: EggKind) -> &'static Clip {
    match kind {
        EggKind::Burst => &BURST,
        EggKind::Cosmos => &COSMOS,
    }
}

fn clip_to(s: &str, width: u16) -> String {
    s.chars().take(width as usize).collect()
}

fn center_in(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width { return s.chars().take(width).collect(); }
    format!("{}{s}", " ".repeat((width - n) / 2))
}

static BURST: LazyLock<Clip> = LazyLock::new(|| {
    let (w, h) = (31usize, 9usize);
    let (cx, cy) = (15.0_f32, 4.0_f32);
    let mut frames = Vec::with_capacity(16);
    for f in 0..16 {
        let radius = 0.6 + f as f32 * 0.55;
        let mut rows = Vec::with_capacity(h + 1);
        for y in 0..h {
            let mut row = String::with_capacity(w);
            for x in 0..w {
                let dx = (x as f32 - cx) * 0.45;
                let dy = y as f32 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                let ray = (dy.atan2(dx) * 8.0 / std::f32::consts::PI).sin().abs();
                let ch = if d < 0.7 {
                    '+'
                } else if (d - radius).abs() < 0.5 && ray > 0.45 {
                    '*'
                } else if f > 2 && (d - radius * 0.55).abs() < 0.35 && ray > 0.8 {
                    '.'
                } else {
                    ' '
                };
                row.push(ch);
            }
            rows.push(row);
        }
        rows.push(center_in(BURST_LINE, w));
        frames.push(rows);
    }
    Clip { fps: 8, frames }
});

static COSMOS: LazyLock<Clip> = LazyLock::new(|| {
    const SEEDS: [(u16, u16, u16); 20] = [
        (2, 1, 1), (8, 3, 2), (14, 0, 1), (20, 5, 3), (27, 2, 1),
        (33, 8, 2), (5, 10, 1), (11, 6, 2), (18, 11, 1), (24, 4, 3),
        (30, 9, 1), (36, 1, 2), (1, 7, 1), (9, 9, 2), (16, 3, 1),
        (22, 7, 2), (28, 11, 1), (35, 5, 3), (7, 2, 1), (13, 8, 2),
    ];
    let (w, h) = (40usize, 12usize);
    let mut frames = Vec::with_capacity(16);
    for f in 0..16 {
        let mut grid = vec![vec![' '; w]; h];
        for (i, (x, y, speed)) in SEEDS.iter().enumerate() {
            let xx = (*x as usize + f * (*speed as usize)) % w;
            let yy = *y as usize % h;
            grid[yy][xx] = match i % 3 { 0 => '*', 1 => '.', _ => '+' };
        }
        frames.push(grid.into_iter().map(|row| row.into_iter().collect()).collect());
    }
    Clip { fps: 8, frames }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[test]
    fn quotes_are_the_roar_lines_and_each_can_be_printed() {
        assert_eq!(ROAR_QUOTES.len(), 5);
        for (i, q) in ROAR_QUOTES.iter().enumerate() {
            let plain = roar_text(i, true, 80);
            assert!(plain.contains(q), "{plain}");
            assert!(plain.contains(ATTRIBUTION));
            assert!(!plain.contains('\u{1b}'));
            assert!(plain.contains("***"));
            let red = roar_text(i, false, 80);
            assert!(red.contains("\u{1b}[31m") && red.contains(q));
            let tiny = roar_text(i, true, 10);
            assert!(tiny.starts_with("*\n"));
            assert!(!tiny.contains("***"));
        }
    }

    #[test]
    fn cosmos_phrase_is_exact() {
        assert!(is_cosmos_query("one fucking life"));
        assert!(is_cosmos_query("  One Fucking Life  "));
        assert!(!is_cosmos_query("one fucking life."));
        assert!(!is_cosmos_query("one fucking"));
        assert!(!is_cosmos_query(""));
    }

    #[test]
    fn clips_are_short_and_do_not_loop() {
        for kind in [EggKind::Burst, EggKind::Cosmos] {
            let d = clip_duration(kind);
            assert!(d < Duration::from_secs(3) && d >= Duration::from_millis(1500), "{d:?}");
            let early = frame_lines(kind, Duration::ZERO, 80, 24);
            let late = frame_lines(kind, Duration::from_secs(5), 80, 24);
            assert!(!early.is_empty() && !late.is_empty());
            assert_ne!(early, late, "the last frame is not the first");
        }
        let burst = frame_lines(EggKind::Burst, Duration::ZERO, 80, 24);
        assert!(burst.iter().any(|l| l.contains(BURST_LINE)));
        let cosmos = frame_lines(EggKind::Cosmos, Duration::ZERO, 80, 24);
        assert!(cosmos.iter().any(|l| l.contains('*')));
        assert!(!cosmos.iter().any(|l| l.contains(BURST_LINE)));
    }

    #[test]
    fn small_terminals_get_one_star() {
        let burst = frame_lines(EggKind::Burst, Duration::ZERO, 12, 4);
        assert!(burst.iter().any(|l| l.contains('*')));
        assert!(burst.iter().any(|l| l.contains("Be an")));
        assert!(burst.iter().all(|l| l.chars().count() <= 12));
        let cosmos = frame_lines(EggKind::Cosmos, Duration::ZERO, 8, 3);
        assert_eq!(cosmos, vec!["*".to_string()]);
    }

    #[test]
    fn konami_completes_only_the_full_sequence() {
        let mut k = Konami::new();
        let seq = [
            KeyCode::Up, KeyCode::Up, KeyCode::Down, KeyCode::Down,
            KeyCode::Left, KeyCode::Right, KeyCode::Left, KeyCode::Right,
            KeyCode::Char('B'), KeyCode::Char('a'),
        ];
        for (i, code) in seq.iter().enumerate() {
            assert_eq!(k.feed(*code), i + 1 == seq.len());
        }
        let mut k = Konami::new();
        assert!(!k.feed(KeyCode::Up));
        assert!(!k.feed(KeyCode::Char('x')));
        assert!(!k.feed(KeyCode::Up));
        assert_eq!(k.pos, 1);
        k.reset();
        assert_eq!(k.pos, 0);
    }

    #[test]
    fn five_taps_within_the_gap_open_it_and_a_pause_does_not() {
        let mut taps = 0;
        let mut last = None;
        let t0 = Instant::now();
        for i in 1..STAR_TAPS {
            assert!(!note_tap(&mut taps, &mut last, t0 + Duration::from_millis(i as u64 * 10)));
        }
        assert!(note_tap(&mut taps, &mut last, t0 + Duration::from_millis(100)));
        taps = 2;
        last = Some(t0);
        assert!(!note_tap(&mut taps, &mut last, t0 + TAP_GAP + Duration::from_millis(1)));
        assert_eq!(taps, 1);
    }
}
