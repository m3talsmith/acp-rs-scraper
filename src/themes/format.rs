//! The theme file format, v1 (TOML):
//!
//! ```toml
//! version = 1
//! name = "default"
//! description = "ACP red on your terminal's colours"
//!
//! [dark.palette]            # colours; any extra name can be referenced as "@name"
//! accent = "#dc2626"
//! muted = "#9ca3af"
//!
//! [dark.styles.link]        # optional per-element overrides
//! fg = "@accent"
//! underline = true
//!
//! [light.palette]           # optional; missing keys fall back to [dark]
//! ```
//!
//! A colour is `0`–`255` (the 256-colour palette), `#rrggbb` / `#rgb`, one of
//! the 16 ANSI names (`black` … `white`, `bright_black` … `bright_white`),
//! `reset` (the terminal's own colour), or `@name` for a palette entry.
//! Borders and padding are fixed in code; a style sets colours and attributes.

use crate::prelude::*;
use ratatui::style::Color;

/// Palette entries every widget uses (other names are only for `@` references).
pub const PALETTE_KEYS: [(&str, &str); 6] = [
    ("accent", "Accent: links, selection, focused borders, the header bar"),
    ("muted", "Secondary text: dates, ids, hints"),
    ("border", "Borders of unfocused panes; the Post badge"),
    ("on_accent", "Text on the accent colour (header, selection, buttons)"),
    ("fg", "Base text colour (reset: the terminal's)"),
    ("bg", "Base background (reset: the terminal's)"),
];

/// Style keys an element may set.
pub const STYLE_KEYS: [&str; 8] = ["fg", "bg", "border", "bold", "italic", "underline", "reverse", "dim"];

macro_rules! elements {
    ($($v:ident = $n:literal, $box:literal, $about:literal;)*) => {
        /// Every styled UI element a theme can override.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[allow(missing_docs)]
        pub enum Element { $($v),* }
        impl Element {
            /// All of them, in editor order.
            pub const ALL: &'static [Element] = &[$(Element::$v),*];
            /// The key under `[dark.styles]`.
            pub fn name(self) -> &'static str { match self { $(Element::$v => $n),* } }
            /// A box (its `border` key colours the border).
            pub fn is_box(self) -> bool { match self { $(Element::$v => $box),* } }
            /// What it is.
            pub fn about(self) -> &'static str { match self { $(Element::$v => $about),* } }
        }
    };
}

elements! {
    Text = "text", false, "Plain text";
    Bold = "bold", false, "Headings, titles, nicks";
    Accent = "accent", false, "Accent text";
    AccentBold = "accent_bold", false, "Bold accent text (focused labels, headings)";
    Link = "link", false, "Links";
    Muted = "muted", false, "Secondary text";
    Gone = "gone", false, "Something missing (a deleted file)";
    Code = "code", false, "Inline code";
    Selected = "selected", false, "The highlighted row";
    Field = "field", false, "A text field's value";
    Label = "label", false, "A form label";
    LabelFocused = "label_focused", false, "The label of the focused field";
    Bar = "bar", false, "The header bar";
    Button = "button", false, "Filled buttons (Start Scanning)";
    PostTag = "post_tag", false, "The Post badge";
    ReplyTag = "reply_tag", false, "The Reply badge";
    Dimmed = "dimmed", false, "The screen under an open dialog";
    Border = "border", false, "Border lines drawn by hand";
    BorderFocused = "border_focused", false, "Border lines of the focused pane";
    Pane = "pane", true, "Nick bar and other panes";
    PaneFocused = "pane_focused", true, "The focused pane";
    Card = "card", true, "A post card";
    CardSelected = "card_selected", true, "The selected card";
    CardFocused = "card_focused", true, "The selected card while the Body has focus";
    Detail = "detail", true, "A post detail";
    DetailFocused = "detail_focused", true, "The focused post detail";
    Dialog = "dialog", true, "Dialogs, dropdowns, nested details";
    H1 = "h1", false, "Markdown heading 1 (a ━ rule in its colour under it)";
    H2 = "h2", false, "Markdown heading 2 (a ─ rule in its colour, else the accent)";
    H3 = "h3", false, "Markdown heading 3";
    H4 = "h4", false, "Markdown heading 4";
    H5 = "h5", false, "Markdown heading 5";
    H6 = "h6", false, "Markdown heading 6";
    Quote = "quote", true, "Markdown block quote text (border: its ┃ bar)";
    CodeBlock = "code_block", true, "Markdown code block text (border: its box)";
    ListMarker = "list_marker", false, "Markdown list bullets and numbers";
    Task = "task", false, "Markdown task boxes ☐ ☑";
    Table = "table", true, "Markdown table (border: its grid)";
    TableHeader = "table_header", false, "Markdown table header cells";
    Rule = "rule", false, "Markdown horizontal rule";
    Footnote = "footnote", false, "Markdown footnote marks and notes";
}

impl Element {
    /// By style key.
    pub fn by_name(s: &str) -> Option<Element> { Element::ALL.iter().copied().find(|e| e.name() == s) }
    /// Index in [`Element::ALL`].
    pub fn index(self) -> usize { Element::ALL.iter().position(|e| *e == self).unwrap_or(0) }
}

/// A colour as written: a value or a palette reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorSpec {
    /// A colour.
    Value(Color),
    /// `@name`.
    Ref(String),
}

/// Overrides for one element; `None` keeps the built-in look.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StyleSpec {
    /// Text colour.
    pub fg: Option<ColorSpec>,
    /// Background.
    pub bg: Option<ColorSpec>,
    /// Border colour (boxes only).
    pub border: Option<ColorSpec>,
    /// Bold.
    pub bold: Option<bool>,
    /// Italic.
    pub italic: Option<bool>,
    /// Underlined.
    pub underline: Option<bool>,
    /// Foreground and background swapped.
    pub reverse: Option<bool>,
    /// Dim.
    pub dim: Option<bool>,
}

impl StyleSpec {
    /// Nothing set.
    pub fn is_empty(&self) -> bool { *self == StyleSpec::default() }
    /// A colour key (`fg`, `bg`, `border`).
    pub fn color(&self, k: &str) -> Option<&ColorSpec> {
        match k { "fg" => self.fg.as_ref(), "bg" => self.bg.as_ref(), "border" => self.border.as_ref(), _ => None }
    }
    /// Set a colour key.
    pub fn set_color(&mut self, k: &str, c: Option<ColorSpec>) {
        match k { "fg" => self.fg = c, "bg" => self.bg = c, "border" => self.border = c, _ => {} }
    }
    /// An attribute key (`bold` … `dim`).
    pub fn flag(&self, k: &str) -> Option<bool> {
        match k { "bold" => self.bold, "italic" => self.italic, "underline" => self.underline, "reverse" => self.reverse, "dim" => self.dim, _ => None }
    }
    /// Set an attribute key.
    pub fn set_flag(&mut self, k: &str, v: Option<bool>) {
        match k { "bold" => self.bold = v, "italic" => self.italic = v, "underline" => self.underline = v, "reverse" => self.reverse = v, "dim" => self.dim = v, _ => {} }
    }
}

/// `[dark]` or `[light]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Section {
    /// Colours, in file order.
    pub palette: Vec<(String, ColorSpec)>,
    /// Element overrides, in file order.
    pub styles: Vec<(String, StyleSpec)>,
}

impl Section {
    /// A palette entry as written.
    pub fn get(&self, name: &str) -> Option<&ColorSpec> { self.palette.iter().find(|(n, _)| n == name).map(|(_, c)| c) }
    /// Set (or add) a palette entry.
    pub fn set(&mut self, name: &str, c: ColorSpec) {
        match self.palette.iter_mut().find(|(n, _)| n == name) { Some(e) => e.1 = c, None => self.palette.push((name.into(), c)) }
    }
    /// An element's overrides (empty when none).
    pub fn style(&self, e: Element) -> StyleSpec {
        self.styles.iter().find(|(n, _)| n == e.name()).map(|(_, s)| s.clone()).unwrap_or_default()
    }
    /// Replace an element's overrides (removed when empty).
    pub fn set_style(&mut self, e: Element, s: StyleSpec) {
        self.styles.retain(|(n, _)| n != e.name());
        if !s.is_empty() { self.styles.push((e.name().into(), s)); self.styles.sort_by_key(|(n, _)| Element::by_name(n).map(Element::index)); }
    }
    /// Follow `@` references to a colour; unknown or circular ones give `Color::Reset`.
    pub fn resolve(&self, c: &ColorSpec) -> Color {
        let mut cur = c;
        for _ in 0..8 {
            match cur {
                ColorSpec::Value(v) => return *v,
                ColorSpec::Ref(r) => match self.get(r) { Some(n) => cur = n, None => return Color::Reset },
            }
        }
        Color::Reset
    }
    /// A palette entry resolved (`Color::Reset` when missing).
    pub fn color(&self, name: &str) -> Color { self.get(name).map(|c| self.resolve(c)).unwrap_or(Color::Reset) }
}

/// Dark or light variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// For dark terminal backgrounds.
    Dark,
    /// For light terminal backgrounds.
    Light,
}

impl Variant {
    /// `dark` / `light`.
    pub fn name(self) -> &'static str { match self { Variant::Dark => "dark", Variant::Light => "light" } }
    /// The other one.
    pub fn flip(self) -> Variant { match self { Variant::Dark => Variant::Light, Variant::Light => Variant::Dark } }
}

/// Logo, title, and motto carried by a theme (`[brand]`).
///
/// Missing `[brand]` means the theme file does not set branding. A user file in
/// `~/.acp/brands` or `--brand` can still supply it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeBrand {
    /// Header, window title, About, status, `--help`, `--version`.
    pub title: String,
    /// Status bar and About. Empty hides it.
    pub motto: String,
    /// Big letters beside the logo. Empty draws none.
    pub word: String,
    /// `halberd`, `builtin`, `text`, or `file`.
    pub logo: String,
    /// Lines drawn when [`ThemeBrand::logo`] is `text`.
    pub logo_text: String,
    /// Path relative to `~/.acp/brands` when [`ThemeBrand::logo`] is `file`.
    pub logo_file: String,
}

impl ThemeBrand {
    /// ACP Red Star: the title, the slogan, the word RED STAR, and the star-blade halberd.
    pub fn red_star() -> Self {
        Self {
            title: crate::config::TUI_TITLE.into(),
            motto: crate::branding::SLOGAN.into(),
            word: "RED STAR".into(),
            logo: "halberd".into(),
            logo_text: String::new(),
            logo_file: String::new(),
        }
    }
    /// Neutral demo branding: title Red Star, a star, no motto, no big word.
    pub fn plain() -> Self {
        Self {
            title: "Red Star".into(),
            motto: String::new(),
            word: String::new(),
            logo: "text".into(),
            logo_text: "★".into(),
            logo_file: String::new(),
        }
    }
}

/// A parsed theme file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeFile {
    /// Format version (1).
    pub version: i64,
    /// Theme name (the file stem).
    pub name: String,
    /// One line about it.
    pub description: String,
    /// Logo, title, and motto. `None`: the file does not set them.
    pub brand: Option<ThemeBrand>,
    /// The dark variant.
    pub dark: Section,
    /// The light variant (`None`: same as dark).
    pub light: Option<Section>,
}

impl ThemeFile {
    /// An empty theme (every colour the terminal's own).
    pub fn empty(name: &str) -> Self {
        ThemeFile { version: THEME_FORMAT_VERSION, name: name.into(), description: String::new(), brand: None, dark: Section::default(), light: None }
    }
    /// The variant as it applies: `[light]` with missing entries taken from `[dark]`.
    pub fn section(&self, v: Variant) -> Section {
        match (v, &self.light) {
            (Variant::Light, Some(l)) => {
                let mut s = self.dark.clone();
                for (n, c) in &l.palette { s.set(n, c.clone()); }
                for (n, st) in &l.styles { if let Some(e) = Element::by_name(n) { s.set_style(e, st.clone()); } }
                s
            }
            _ => self.dark.clone(),
        }
    }
    /// The variant to edit (`[light]` is created from `[dark]` on first use).
    pub fn section_mut(&mut self, v: Variant) -> &mut Section {
        match v {
            Variant::Dark => &mut self.dark,
            Variant::Light => {
                if self.light.is_none() { self.light = Some(self.dark.clone()); }
                self.light.as_mut().expect("just set")
            }
        }
    }
}

/// A theme name: 1–32 of `a-z`, `0-9`, `-`, `_` (so it can't leave the themes folder).
pub fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

const ANSI: [(&str, Color); 16] = [
    ("black", Color::Black), ("red", Color::Red), ("green", Color::Green), ("yellow", Color::Yellow),
    ("blue", Color::Blue), ("magenta", Color::Magenta), ("cyan", Color::Cyan), ("white", Color::Gray),
    ("bright_black", Color::DarkGray), ("bright_red", Color::LightRed), ("bright_green", Color::LightGreen),
    ("bright_yellow", Color::LightYellow), ("bright_blue", Color::LightBlue), ("bright_magenta", Color::LightMagenta),
    ("bright_cyan", Color::LightCyan), ("bright_white", Color::White),
];

/// The 16 ANSI colours by name, in palette order (0–15).
pub fn ansi_colors() -> &'static [(&'static str, Color); 16] { &ANSI }

/// Parse a colour value (see the module docs).
pub fn parse_color(v: &toml::Value) -> std::result::Result<ColorSpec, String> {
    match v {
        toml::Value::Integer(n) if (0..=255).contains(n) => Ok(ColorSpec::Value(Color::Indexed(*n as u8))),
        toml::Value::Integer(n) => Err(format!("{n} is not 0–255")),
        toml::Value::String(s) => parse_color_str(s),
        other => Err(format!("expected a colour, found {}", other.type_str())),
    }
}

/// Parse a colour written as text (also what the editor's hex field takes).
pub fn parse_color_str(s: &str) -> std::result::Result<ColorSpec, String> {
    let t = s.trim().to_ascii_lowercase().replace('-', "_");
    if t == "reset" || t == "default" || t == "none" { return Ok(ColorSpec::Value(Color::Reset)); }
    if let Some(r) = t.strip_prefix('@') {
        return if valid_name(r) { Ok(ColorSpec::Ref(r.into())) } else { Err(format!("bad palette reference {s:?}")) };
    }
    if let Some(h) = t.strip_prefix('#') {
        let ok = h.chars().all(|c| c.is_ascii_hexdigit());
        let p = |x: &str| u8::from_str_radix(x, 16).ok();
        return match (h.len(), ok) {
            (6, true) => Ok(ColorSpec::Value(Color::Rgb(p(&h[0..2]).unwrap_or(0), p(&h[2..4]).unwrap_or(0), p(&h[4..6]).unwrap_or(0)))),
            (3, true) => {
                let d = |i: usize| p(&h[i..i + 1]).unwrap_or(0) * 17;
                Ok(ColorSpec::Value(Color::Rgb(d(0), d(1), d(2))))
            }
            _ => Err(format!("bad hex colour {s:?} (use #rrggbb or #rgb)")),
        };
    }
    if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
        return t.parse::<u8>().map(|n| ColorSpec::Value(Color::Indexed(n))).map_err(|_| format!("{s} is not 0–255"));
    }
    let key = match t.as_str() {
        "gray" | "grey" => "white".to_string(),
        "dark_gray" | "dark_grey" => "bright_black".into(),
        "light_gray" | "light_grey" => "bright_white".into(),
        x => x.strip_prefix("light_").map(|r| format!("bright_{r}")).unwrap_or_else(|| x.to_string()),
    };
    ANSI.iter().find(|(n, _)| *n == key).map(|(_, c)| ColorSpec::Value(*c))
        .ok_or_else(|| format!("unknown colour {s:?} (use 0-255, #rrggbb, an ANSI name, reset or @name)"))
}

/// How a colour is written back.
pub fn color_text(c: &ColorSpec) -> String {
    match c {
        ColorSpec::Ref(r) => format!("@{r}"),
        ColorSpec::Value(v) => match v {
            Color::Reset => "reset".into(),
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            Color::Indexed(n) => n.to_string(),
            other => ANSI.iter().find(|(_, c)| c == other).map(|(n, _)| n.to_string()).unwrap_or_else(|| "reset".into()),
        },
    }
}

fn color_toml(c: &ColorSpec) -> String {
    match c {
        ColorSpec::Value(Color::Indexed(n)) => n.to_string(),
        other => toml::Value::String(color_text(other)).to_string(),
    }
}

/// Parse a theme file. `stem` is the file name without `.toml` (the name when the
/// file has none). Errors refuse the whole file; warnings name keys that were skipped.
pub fn parse(src: &str, stem: &str) -> std::result::Result<(ThemeFile, Vec<String>), String> {
    let table: toml::Table = toml::from_str(src).map_err(|e| format!("not valid TOML: {}", e.to_string().trim()))?;
    let mut warn = Vec::new();
    let version = match table.get("version") {
        None => { warn.push(format!("no version; reading it as version {THEME_FORMAT_VERSION}")); THEME_FORMAT_VERSION }
        Some(toml::Value::Integer(v)) if *v > THEME_FORMAT_VERSION => return Err(format!(
            "format version {v} is newer than this acp-red-star reads ({THEME_FORMAT_VERSION}); update acp-red-star")),
        Some(toml::Value::Integer(v)) if *v >= 1 => *v,
        Some(v) => return Err(format!("version must be {THEME_FORMAT_VERSION}, found {v}")),
    };
    let text = |k: &str, w: &mut Vec<String>| match table.get(k) {
        None => None,
        Some(toml::Value::String(s)) => Some(s.clone()),
        Some(v) => { w.push(format!("{k}: expected text, found {} (ignored)", v.type_str())); None }
    };
    let name = text("name", &mut warn).unwrap_or_else(|| stem.to_string());
    let description = text("description", &mut warn).unwrap_or_default();
    let sec = |k: &str, w: &mut Vec<String>| -> Option<Section> {
        match table.get(k) {
            None => None,
            Some(toml::Value::Table(t)) => Some(parse_section(k, t, w)),
            Some(v) => { w.push(format!("[{k}]: expected a table, found {} (ignored)", v.type_str())); None }
        }
    };
    let dark = sec("dark", &mut warn);
    let light = sec("light", &mut warn);
    let brand = parse_brand(&table, &mut warn);
    if dark.is_none() { warn.push("no [dark] section: using the terminal's colours".into()); }
    for k in table.keys() {
        if !["version", "name", "description", "brand", "dark", "light"].contains(&k.as_str()) { warn.push(format!("unknown key {k:?} (ignored)")); }
    }
    let f = ThemeFile { version, name, description, brand, dark: dark.unwrap_or_default(), light };
    for v in [Variant::Dark, Variant::Light] {
        if v == Variant::Light && f.light.is_none() { continue; }
        let s = f.section(v);
        let refs = s.palette.iter().map(|(n, c)| (format!("palette.{n}"), c))
            .chain(s.styles.iter().flat_map(|(n, st)| ["fg", "bg", "border"].into_iter().filter_map(move |k| st.color(k).map(|c| (format!("styles.{n}.{k}"), c)))));
        for (at, c) in refs {
            if let ColorSpec::Ref(r) = c && !is_reset_chain(&s, r) {
                warn.push(format!("[{}.{at}]: @{r} is not in the palette (or loops); using the terminal's colour", v.name()));
            }
        }
    }
    Ok((f, warn))
}

/// Whether `@r` resolves through the palette to a value (possibly `reset`) without a loop.
fn is_reset_chain(s: &Section, r: &str) -> bool {
    let mut cur = r.to_string();
    for _ in 0..8 {
        match s.get(&cur) { Some(ColorSpec::Value(_)) => return true, Some(ColorSpec::Ref(n)) => cur = n.clone(), None => return false }
    }
    false
}

fn brand_text(t: &toml::Table, k: &str, w: &mut Vec<String>) -> String {
    match t.get(k) {
        None => String::new(),
        Some(toml::Value::String(s)) => s.clone(),
        Some(v) => {
            w.push(format!("[brand] {k}: expected text, found {} (ignored)", v.type_str()));
            String::new()
        }
    }
}

fn parse_brand(table: &toml::Table, w: &mut Vec<String>) -> Option<ThemeBrand> {
    let v = table.get("brand")?;
    let toml::Value::Table(t) = v else {
        w.push(format!("[brand]: expected a table, found {} (ignored)", v.type_str()));
        return None;
    };
    let mut logo = brand_text(t, "logo", w);
    if logo.is_empty() { logo = "builtin".into(); }
    if !["halberd", "builtin", "text", "file"].contains(&logo.as_str()) {
        w.push(format!("[brand] logo: expected halberd, builtin, text, or file, found {logo:?} (using builtin)"));
        logo = "builtin".into();
    }
    for k in t.keys() {
        if !["title", "motto", "word", "logo", "logo_text", "logo_file"].contains(&k.as_str()) {
            w.push(format!("[brand] unknown key {k:?} (ignored)"));
        }
    }
    Some(ThemeBrand {
        title: brand_text(t, "title", w),
        motto: brand_text(t, "motto", w),
        word: brand_text(t, "word", w),
        logo,
        logo_text: brand_text(t, "logo_text", w),
        logo_file: brand_text(t, "logo_file", w),
    })
}

fn parse_section(k: &str, t: &toml::Table, w: &mut Vec<String>) -> Section {
    let mut s = Section::default();
    for (key, v) in t {
        match (key.as_str(), v) {
            ("palette", toml::Value::Table(p)) => for (n, c) in p {
                if !valid_name(n) { w.push(format!("[{k}.palette] {n:?}: names are a-z, 0-9, - and _ (ignored)")); continue; }
                match parse_color(c) {
                    Ok(c) => s.palette.push((n.clone(), c)),
                    Err(e) => w.push(format!("[{k}.palette] {n}: {e} (ignored)")),
                }
            },
            ("styles", toml::Value::Table(st)) => for (n, v) in st {
                let Some(e) = Element::by_name(n) else { w.push(format!("[{k}.styles] unknown element {n:?} (ignored)")); continue };
                let toml::Value::Table(fields) = v else { w.push(format!("[{k}.styles.{n}]: expected a table (ignored)")); continue };
                let mut spec = StyleSpec::default();
                for (f, v) in fields {
                    match (f.as_str(), v) {
                        ("fg" | "bg" | "border", c) => match parse_color(c) {
                            Ok(c) => spec.set_color(f, Some(c)),
                            Err(er) => w.push(format!("[{k}.styles.{n}] {f}: {er} (ignored)")),
                        },
                        ("bold" | "italic" | "underline" | "reverse" | "dim", toml::Value::Boolean(b)) => spec.set_flag(f, Some(*b)),
                        ("bold" | "italic" | "underline" | "reverse" | "dim", _) => w.push(format!("[{k}.styles.{n}] {f}: expected true or false (ignored)")),
                        _ => w.push(format!("[{k}.styles.{n}] unknown key {f:?} (ignored)")),
                    }
                }
                if spec.border.is_some() && !e.is_box() { w.push(format!("[{k}.styles.{n}] border: {n} has no border (ignored)")); spec.border = None; }
                s.set_style(e, spec);
            },
            ("palette" | "styles", v) => w.push(format!("[{k}.{key}]: expected a table, found {} (ignored)", v.type_str())),
            _ => w.push(format!("[{k}] unknown key {key:?} (ignored)")),
        }
    }
    s
}

/// The file text for `f` (canonical order: core palette keys first).
pub fn to_toml(f: &ThemeFile) -> String {
    let q = |s: &str| toml::Value::String(s.into()).to_string();
    let mut o = format!("# acp-red-star theme, format v{THEME_FORMAT_VERSION}. See docs/USAGE.md (Themes).\nversion = {}\nname = {}\ndescription = {}\n",
        f.version, q(&f.name), q(&f.description));
    if let Some(b) = &f.brand {
        o.push_str("\n[brand]\n");
        for (k, v) in [("title", &b.title), ("motto", &b.motto), ("word", &b.word), ("logo", &b.logo), ("logo_text", &b.logo_text), ("logo_file", &b.logo_file)] {
            o.push_str(&format!("{k} = {}\n", q(v)));
        }
    }
    let mut sec = |k: &str, s: &Section| {
        o.push_str(&format!("\n[{k}.palette]\n"));
        let mut pal: Vec<&(String, ColorSpec)> = s.palette.iter().collect();
        pal.sort_by_key(|(n, _)| PALETTE_KEYS.iter().position(|(p, _)| p == n).unwrap_or(usize::MAX));
        for (n, c) in pal { o.push_str(&format!("{n} = {}\n", color_toml(c))); }
        for (n, st) in &s.styles {
            o.push_str(&format!("\n[{k}.styles.{n}]\n"));
            for key in STYLE_KEYS {
                if let Some(c) = st.color(key) { o.push_str(&format!("{key} = {}\n", color_toml(c))); }
                if let Some(b) = st.flag(key) { o.push_str(&format!("{key} = {b}\n")); }
            }
        }
    };
    sec("dark", &f.dark);
    if let Some(l) = &f.light { sec("light", l); }
    o
}

/// WCAG contrast ratio of two colours (1–21); `None` when either is the terminal's own.
pub fn contrast(a: Color, b: Color) -> Option<f64> {
    let (la, lb) = (luminance(a)?, luminance(b)?);
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    Some((hi + 0.05) / (lo + 0.05))
}

/// sRGB of a colour (the xterm 256-colour palette for indexed ones).
pub fn rgb(c: Color) -> Option<(u8, u8, u8)> {
    const BASE: [(u8, u8, u8); 16] = [(0, 0, 0), (205, 0, 0), (0, 205, 0), (205, 205, 0), (0, 0, 238), (205, 0, 205), (0, 205, 205), (229, 229, 229),
        (127, 127, 127), (255, 0, 0), (0, 255, 0), (255, 255, 0), (92, 92, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255)];
    let idx = |n: u8| -> (u8, u8, u8) {
        match n {
            0..=15 => BASE[n as usize],
            16..=231 => { let i = n - 16; let s = |v: u8| if v == 0 { 0 } else { 55 + v * 40 }; (s(i / 36), s((i / 6) % 6), s(i % 6)) }
            _ => { let g = 8 + (n - 232) * 10; (g, g, g) }
        }
    };
    match c {
        Color::Reset => None,
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Indexed(n) => Some(idx(n)),
        other => ANSI.iter().position(|(_, a)| *a == other).map(|i| BASE[i]),
    }
}

fn luminance(c: Color) -> Option<f64> {
    let (r, g, b) = rgb(c)?;
    let ch = |v: u8| { let s = v as f64 / 255.0; if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) } };
    Some(0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b))
}
