//! The theme editor (Shift-F5): full screen, branding then the palette and every
//! element on the left (swatch, value, `*` when changed, `⚠` below 3:1 contrast),
//! a live preview on the right drawn by the real UI code over sample posts. The
//! preview follows branding (logo, title, motto) as you type, and colours as you
//! set them. `Tab` flips dark / light; colours come from a hex field or a grid
//! (16 colours, the 216 cube, 24 greys, `@` palette references). `^S` saves, `^R`
//! reverts, `^N` new, `^D` duplicate, `^X` delete, `[` `]` other themes, `Esc`
//! closes (asking first when there are unsaved changes).

use crate::prelude::*;
use crate::themes::{self, ColorSpec, Element, PALETTE_KEYS, STYLE_KEYS, ThemeBrand, ThemeFile, Variant, contrast};
use crate::tui::state::{App, Focus};
use crate::tui::theme::Theme;
use crate::tui::widgets::text::TextField;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

/// A row of the left list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// A section heading.
    Heading(&'static str),
    /// One branding field (`title`, `motto`, `word`, `logo`, `logo_text`, `logo_file`).
    Brand(&'static str),
    /// A palette colour.
    Palette(String),
    /// An element (Enter shows its keys).
    Element(Element),
    /// One key of the expanded element.
    Field(Element, &'static str),
}

/// Branding rows, in editor order.
pub const BRAND_ROWS: [&str; 6] = ["title", "motto", "word", "logo", "logo_text", "logo_file"];

/// A branding line being typed. Each key updates the preview.
#[derive(Clone, Debug, PartialEq)]
pub struct BrandEdit {
    /// Which [`BRAND_ROWS`] key.
    pub key: &'static str,
    /// The line so far.
    pub field: TextField,
}

/// What a colour pick changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A palette entry.
    Palette(String),
    /// `fg` / `bg` / `border` of an element.
    Field(Element, &'static str),
}

/// Grid tabs of the colour picker.
pub const PICKER_TABS: [&str; 4] = ["16 colours", "216 cube", "24 greys", "@ palette"];

/// The colour picker.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorPicker {
    /// What it sets.
    pub target: Target,
    /// Grid tab (see [`PICKER_TABS`]).
    pub tab: usize,
    /// Selected cell of the grid.
    pub sel: usize,
    /// Typed colour (hex, 0-255, a name, `@name`, `reset`).
    pub hex: TextField,
    /// Why the typed colour was refused.
    pub error: Option<String>,
}

/// What a question is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Unsaved changes, then close.
    UnsavedClose,
    /// Unsaved changes, then open this theme.
    UnsavedOpen(String),
    /// Name for a new theme.
    NewName,
    /// Name for a copy.
    DuplicateName,
    /// Name for a new palette colour.
    PaletteName,
    /// Delete this theme file.
    Delete(String),
    /// The file changed on disk since it was opened: overwrite it, reload it, or cancel
    /// (then continue with the unsaved-changes question it came from, if any).
    ChangedOnDisk(Option<Box<Ask>>),
    /// The file on disk is broken: replace it, or cancel.
    OverwriteBroken(Option<Box<Ask>>),
}

/// An open question at the bottom of the editor.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    /// About.
    pub ask: Ask,
    /// The name field (name questions).
    pub field: TextField,
    /// Why the answer was refused.
    pub error: Option<String>,
}

/// Editor state.
pub struct ThemeEditor {
    /// The theme as edited.
    pub file: ThemeFile,
    /// As on disk (`None`: not saved yet).
    pub saved: Option<ThemeFile>,
    /// The variant shown and edited.
    pub variant: Variant,
    /// Selected row.
    pub cursor: usize,
    /// First row shown.
    pub scroll: usize,
    /// Element whose keys are shown.
    pub expanded: Option<Element>,
    /// Open colour picker.
    pub picker: Option<ColorPicker>,
    /// Open question.
    pub prompt: Option<Prompt>,
    /// Last result (`true`: an error).
    pub message: Option<(bool, String)>,
    /// Mark the selected colour or element in the preview (reverse video; `h` toggles).
    pub highlight: bool,
    /// The file's text when it was opened or last saved (`None`: there was no file), to
    /// notice changes made on disk meanwhile.
    pub disk: Option<String>,
    /// The file on disk could not be read as a theme (saving asks before replacing it).
    pub broken: bool,
    /// The terminal shows 24-bit colour (`$COLORTERM` is `truecolor` or `24bit`).
    pub truecolor: bool,
    /// The preview: the real UI over sample posts.
    pub preview: Box<App>,
    /// A branding line open for typing. The preview updates on each key.
    pub editing: Option<BrandEdit>,
}

/// After a key: keep the editor open or close it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Stay.
    Stay,
    /// Close.
    Close,
}

fn preview_app(theme: Theme, brand_name: &str, brand: &crate::brands::Brand) -> Box<App> {
    use clap::Parser;
    let args = Args::parse_from(["acp-red-star"]);
    let mut a = App::new(Box::new(crate::tui::sample::sample_store_for(brand)), &args, theme, true);
    a.brand_name = brand_name.to_string();
    a.brand = brand.clone();
    let keep = crate::tui::sample::SAMPLE_NICKS;
    a.all_nicks.retain(|n| keep.contains(&n.nick.as_str()));
    a.nicks.retain(|n| keep.contains(&n.nick.as_str()));
    let i = a.nicks.iter().position(|n| n.nick == "sample").unwrap_or(0);
    a.select_only(i);
    a.focus = Focus::Body;
    a.open_post(0);
    a.anim = None;
    a.push_status("theme preview: sample posts");
    Box::new(a)
}

impl ThemeEditor {
    /// Edit theme `name` (a new empty one when it can't be read; a broken file is
    /// opened as a new theme and only replaced if you save it).
    pub fn open(app: &App, name: &str) -> Self {
        let dir = app.themes_dir.clone().unwrap_or_default();
        let disk = themes::theme_path(&dir, name).ok().and_then(|p| std::fs::read_to_string(p).ok());
        let (mut file, mut saved, message, broken) = match themes::load(&dir, name) {
            Ok(l) => (l.file.clone(), Some(l.file), None, false),
            Err(e) => (ThemeFile::empty(name), None, Some((true, e.notice())), matches!(e, themes::LoadError::Broken(..))),
        };
        // A theme file with no [brand] shows the branding already on screen, so saving
        // writes it into the theme. Both copies match, so opening is not a change.
        if file.brand.is_none() {
            let seeded = crate::brands::theme_brand_of(&app.brand);
            file.brand = Some(seeded.clone());
            if let Some(s) = saved.as_mut() { s.brand = Some(seeded); }
        }
        let shown = file.brand.as_ref().map(|b| crate::brands::from_theme_brand(&file.name, b, app.brands_dir.as_deref()))
            .unwrap_or_else(|| app.brand.clone());
        let t = Theme::from_file(&file, app.variant());
        let mut ed = ThemeEditor { file, saved, variant: app.variant(), cursor: 1, scroll: 0, expanded: None, picker: None, prompt: None, message, highlight: true,
            disk, broken, truecolor: truecolor(std::env::var("COLORTERM").ok().as_deref()), preview: preview_app(t, &shown.name, &shown), editing: None };
        ed.sync_preview();
        ed
    }

    /// What the selected row is called in the preview title (`None` for a heading).
    pub fn row_name(&self) -> Option<String> {
        match self.current() {
            Row::Heading(_) | Row::Brand(_) => None,
            Row::Palette(n) => Some(format!("@{n}")),
            Row::Element(e) => Some(e.name().to_string()),
            Row::Field(e, k) => Some(format!("{}.{k}", e.name())),
        }
    }

    /// The edited theme with the selected palette colour, or the selected element's
    /// colours, replaced by sentinel colours the theme doesn't use. Drawing the preview
    /// with it and with [`ThemeEditor::theme`] shows which cells that row paints.
    pub fn marked_theme(&self) -> Option<Theme> {
        let row = self.current();
        if matches!(row, Row::Heading(_) | Row::Brand(_)) { return None; }
        let base = self.theme();
        let mut used: Vec<Color> = vec![base.accent, base.muted, base.border, base.on_accent, base.fg, base.bg];
        for e in Element::ALL {
            let st = ratatui::style::Style::from(&base.element(*e));
            used.extend(st.fg.into_iter().chain(st.bg));
        }
        let mut free = (1u8..=250).step_by(3).map(|n| Color::Rgb(n, 255 - n, n / 2 + 7)).filter(|c| !used.contains(c));
        let (a, b) = (free.next().unwrap_or(Color::Rgb(1, 2, 3)), free.next().unwrap_or(Color::Rgb(3, 2, 1)));
        let mut f = self.file.clone();
        let s = f.section_mut(self.variant);
        match row {
            Row::Palette(n) => s.set(&n, ColorSpec::Value(a)),
            Row::Element(e) | Row::Field(e, _) => {
                let mut st = s.style(e);
                st.set_color("fg", Some(ColorSpec::Value(a)));
                st.set_color("bg", Some(ColorSpec::Value(b)));
                if e.is_box() { st.set_color("border", Some(ColorSpec::Value(a))); }
                s.set_style(e, st);
            }
            Row::Heading(_) | Row::Brand(_) => {}
        }
        Some(Theme::from_file(&f, self.variant))
    }

    /// Unsaved changes.
    pub fn dirty(&self) -> bool { self.saved.as_ref() != Some(&self.file) }

    /// The edited theme in the shown variant.
    pub fn theme(&self) -> Theme { Theme::from_file(&self.file, self.variant) }

    fn sync_preview(&mut self) {
        self.preview.theme = self.theme();
        if let Some(b) = &self.file.brand {
            self.preview.brand = crate::brands::from_theme_brand(&self.file.name, b, None);
        }
    }

    /// Palette names in the shown variant: the six used ones, then any others.
    pub fn palette_names(&self) -> Vec<String> {
        let s = self.file.section(self.variant);
        let mut v: Vec<String> = PALETTE_KEYS.iter().map(|(k, _)| k.to_string()).collect();
        for (n, _) in &s.palette { if !v.contains(n) { v.push(n.clone()); } }
        v
    }

    /// The list rows.
    pub fn rows(&self) -> Vec<Row> {
        let mut v = vec![Row::Heading("Branding")];
        v.extend(BRAND_ROWS.map(Row::Brand));
        v.push(Row::Heading("Palette"));
        v.extend(self.palette_names().into_iter().map(Row::Palette));
        v.push(Row::Heading("Elements"));
        for e in Element::ALL {
            v.push(Row::Element(*e));
            if self.expanded == Some(*e) {
                v.extend(STYLE_KEYS.iter().filter(|k| **k != "border" || e.is_box()).map(|k| Row::Field(*e, k)));
            }
        }
        v
    }

    /// The colour a row shows (resolved), if it has one.
    pub fn row_color(&self, r: &Row) -> Option<Color> {
        let s = self.file.section(self.variant);
        match r {
            Row::Palette(n) => Some(s.color(n)),
            Row::Field(e, k) => s.style(*e).color(k).map(|c| s.resolve(c)),
            _ => None,
        }
    }

    /// The value a row shows, as written.
    pub fn row_value(&self, r: &Row) -> String {
        let s = self.file.section(self.variant);
        match r {
            Row::Brand(k) => self.file.brand.as_ref().map(|b| brand_field(b, k)).filter(|s| !s.is_empty()).unwrap_or_else(|| "–".into()),
            Row::Palette(n) => s.get(n).map(themes::color_text).unwrap_or_else(|| "(unset)".into()),
            Row::Element(e) => {
                let st = s.style(*e);
                let mut parts: Vec<String> = Vec::new();
                for k in STYLE_KEYS {
                    if let Some(c) = st.color(k) { parts.push(format!("{k}={}", themes::color_text(c))); }
                    if let Some(b) = st.flag(k) { parts.push(if b { k.to_string() } else { format!("no {k}") }); }
                }
                if parts.is_empty() { "built-in".into() } else { parts.join(" ") }
            }
            Row::Field(e, k) => {
                let st = s.style(*e);
                st.color(k).map(themes::color_text).or_else(|| st.flag(k).map(|b| b.to_string())).unwrap_or_else(|| "–".into())
            }
            Row::Heading(_) => String::new(),
        }
    }

    /// Changed since the last save.
    pub fn row_changed(&self, r: &Row) -> bool {
        let Some(saved) = &self.saved else { return !matches!(r, Row::Heading(_)) };
        let (a, b) = (self.file.section(self.variant), saved.section(self.variant));
        match r {
            Row::Brand(k) => self.file.brand.as_ref().map(|x| brand_field(x, k)) != saved.brand.as_ref().map(|x| brand_field(x, k)),
            Row::Palette(n) => a.get(n) != b.get(n),
            Row::Element(e) => a.style(*e) != b.style(*e),
            Row::Field(e, k) => { let (x, y) = (a.style(*e), b.style(*e)); x.color(k) != y.color(k) || x.flag(k) != y.flag(k) }
            Row::Heading(_) => false,
        }
    }

    /// Text contrast of a row when below [`THEME_MIN_CONTRAST`].
    pub fn row_warning(&self, r: &Row) -> Option<f64> { warning(&self.file, self.variant, r) }

    /// Keep the cursor off headings.
    fn settle(&mut self, d: i32) {
        let rows = self.rows();
        let n = rows.len();
        self.cursor = self.cursor.min(n - 1);
        let mut guard = 0;
        while matches!(rows.get(self.cursor), Some(Row::Heading(_))) && guard < n {
            self.cursor = if d < 0 { if self.cursor == 0 { 1 } else { self.cursor - 1 } } else { (self.cursor + 1).min(n - 1) };
            guard += 1;
        }
    }

    fn move_cursor(&mut self, d: i32) {
        let n = self.rows().len() as i32;
        self.cursor = (self.cursor as i32 + d).clamp(0, n - 1) as usize;
        self.settle(d);
    }

    /// The selected row.
    pub fn current(&self) -> Row { self.rows().get(self.cursor).cloned().unwrap_or(Row::Heading("")) }

    fn set_target(&mut self, t: &Target, c: Option<ColorSpec>) {
        set_target(&mut self.file, self.variant, t, c);
        self.sync_preview();
    }

    /// The colour cells of a picker tab.
    pub fn grid(&self, tab: usize) -> Vec<(String, ColorSpec)> {
        match tab {
            0 => themes::ansi_colors().iter().map(|(n, c)| (n.to_string(), ColorSpec::Value(*c))).collect(),
            1 => (16u8..=231).map(|i| (i.to_string(), ColorSpec::Value(Color::Indexed(i)))).collect(),
            2 => (232u8..=255).map(|i| (i.to_string(), ColorSpec::Value(Color::Indexed(i)))).collect(),
            _ => {
                let own = match self.picker.as_ref().map(|p| &p.target) { Some(Target::Palette(n)) => Some(n.clone()), _ => None };
                let mut v: Vec<(String, ColorSpec)> = self.palette_names().into_iter().filter(|n| Some(n) != own.as_ref())
                    .map(|n| (format!("@{n}"), ColorSpec::Ref(n))).collect();
                v.push(("reset".into(), ColorSpec::Value(Color::Reset)));
                v
            }
        }
    }

    /// Cells per grid row of a picker tab.
    pub fn grid_cols(tab: usize) -> usize { match tab { 0 => 8, 1 => 18, 2 => 12, _ => 1 } }

    fn open_picker(&mut self, target: Target) {
        let cur = match &target { Target::Palette(n) => self.file.section(self.variant).get(n).cloned(), Target::Field(e, k) => self.file.section(self.variant).style(*e).color(k).cloned() };
        let mut p = ColorPicker { target, tab: 0, sel: 0, hex: TextField::default(), error: None };
        if let Some(c) = &cur { p.hex = TextField::new(&themes::color_text(c)); }
        self.picker = Some(p);
        if let Some(c) = cur {
            for tab in 0..PICKER_TABS.len() {
                if let Some(i) = self.grid(tab).iter().position(|(_, g)| *g == c) {
                    if let Some(p) = self.picker.as_mut() { p.tab = tab; p.sel = i; }
                    break;
                }
            }
        }
    }

    /// Contrast of colour `c` for the picker's target, when below [`THEME_MIN_CONTRAST`].
    pub fn picker_warning(&self, c: &ColorSpec) -> Option<f64> {
        let p = self.picker.as_ref()?;
        let mut f = self.file.clone();
        set_target(&mut f, self.variant, &p.target, Some(c.clone()));
        let row = match &p.target { Target::Palette(n) => Row::Palette(n.clone()), Target::Field(e, k) => Row::Field(*e, k) };
        warning(&f, self.variant, &row)
    }

    fn picker_key(&mut self, k: KeyEvent) {
        let Some(mut p) = self.picker.take() else { return };
        let cells = self.grid(p.tab).len();
        let cols = Self::grid_cols(p.tab);
        match k.code {
            KeyCode::Esc => return,
            KeyCode::Tab => { p.tab = (p.tab + 1) % PICKER_TABS.len(); p.sel = 0; }
            KeyCode::BackTab => { p.tab = (p.tab + PICKER_TABS.len() - 1) % PICKER_TABS.len(); p.sel = 0; }
            KeyCode::Left => p.sel = p.sel.saturating_sub(1),
            KeyCode::Right => p.sel = (p.sel + 1).min(cells.saturating_sub(1)),
            KeyCode::Up => p.sel = p.sel.saturating_sub(cols),
            KeyCode::Down => p.sel = (p.sel + cols).min(cells.saturating_sub(1)),
            KeyCode::Enter => {
                let typed = p.hex.value.trim().to_string();
                let pick = if !typed.is_empty() && Some(typed.as_str()) != self.value_text(&p.target).as_deref() {
                    match themes::parse_color_str(&typed) {
                        Ok(c) => c,
                        Err(e) => { p.error = Some(e); self.picker = Some(p); return; }
                    }
                } else {
                    let tab = p.tab;
                    self.picker = Some(p.clone());
                    let g = self.grid(tab);
                    self.picker = None;
                    match g.get(p.sel) { Some((_, c)) => c.clone(), None => return }
                };
                if let ColorSpec::Ref(r) = &pick && !self.palette_names().contains(r) {
                    p.error = Some(format!("@{r} is not in the palette"));
                    self.picker = Some(p);
                    return;
                }
                self.set_target(&p.target, Some(pick));
                return;
            }
            _ => { if p.hex.on_key(k) { p.error = None; } }
        }
        self.picker = Some(p);
    }

    /// The target's value as written (what the hex field starts with).
    fn value_text(&self, t: &Target) -> Option<String> {
        let s = self.file.section(self.variant);
        match t {
            Target::Palette(n) => s.get(n).map(themes::color_text),
            Target::Field(e, k) => s.style(*e).color(k).map(themes::color_text),
        }
    }

    fn activate(&mut self) {
        match self.current() {
            Row::Brand("logo") => {
                let b = self.file.brand.get_or_insert_with(ThemeBrand::red_star);
                b.logo = cycle_logo(&b.logo).into();
                self.sync_preview();
            }
            Row::Brand(k) => {
                let v = self.file.brand.as_ref().map(|b| brand_field(b, k)).unwrap_or_default();
                self.editing = Some(BrandEdit { key: k, field: TextField::new(&v) });
            }
            Row::Palette(n) => self.open_picker(Target::Palette(n)),
            Row::Element(e) => { self.expanded = if self.expanded == Some(e) { None } else { Some(e) }; }
            Row::Field(e, k) if ["fg", "bg", "border"].contains(&k) => self.open_picker(Target::Field(e, k)),
            Row::Field(e, k) => {
                let v = self.variant;
                let mut st = self.file.section(v).style(e);
                st.set_flag(k, match st.flag(k) { None => Some(true), Some(true) => Some(false), Some(false) => None });
                self.file.section_mut(v).set_style(e, st);
                self.sync_preview();
            }
            Row::Heading(_) => {}
        }
    }

    fn clear(&mut self) {
        let v = self.variant;
        match self.current() {
            Row::Brand("logo") => {
                if let Some(b) = self.file.brand.as_mut() { b.logo = "builtin".into(); }
            }
            Row::Brand(k) => {
                if let Some(b) = self.file.brand.as_mut() { set_brand_field(b, k, String::new()); }
            }
            Row::Palette(n) => {
                let core = PALETTE_KEYS.iter().any(|(k, _)| *k == n);
                let sec = self.file.section_mut(v);
                if core { sec.set(&n, ColorSpec::Value(Color::Reset)); } else { sec.palette.retain(|(x, _)| *x != n); }
            }
            Row::Element(e) => self.file.section_mut(v).set_style(e, Default::default()),
            Row::Field(e, k) => {
                let mut st = self.file.section(v).style(e);
                st.set_color(k, None);
                st.set_flag(k, None);
                self.file.section_mut(v).set_style(e, st);
            }
            Row::Heading(_) => {}
        }
        self.settle(1);
        self.sync_preview();
    }

    fn ask(&mut self, ask: Ask) { self.prompt = Some(Prompt { ask, field: TextField::default(), error: None }); }

    /// Save to the themes folder (and use it at once when it is the selected theme).
    pub fn save(&mut self, app: &mut App) -> bool { self.save_with(app, false, None) }

    /// Save; unless `force`, ask first when the file on disk is broken or changed since it
    /// was opened (`then`: the question to continue with after that one).
    fn save_with(&mut self, app: &mut App, force: bool, then: Option<Ask>) -> bool {
        let Some(dir) = app.themes_dir.clone() else { self.message = Some((true, "no themes folder".into())); return false };
        let path = match themes::theme_path(&dir, &self.file.name) { Ok(p) => p, Err(e) => { self.message = Some((true, e)); return false } };
        let now = std::fs::read_to_string(&path).ok();
        if !force && self.broken && now.is_some() { self.ask(Ask::OverwriteBroken(then.map(Box::new))); return false; }
        if !force && now != self.disk { self.ask(Ask::ChangedOnDisk(then.map(Box::new))); return false; }
        match themes::save(&dir, &self.file) {
            Ok(p) => {
                self.disk = std::fs::read_to_string(&p).ok();
                self.broken = false;
                self.saved = Some(self.file.clone());
                self.message = Some((false, format!("saved {}", p.display())));
                app.push_status(format!("theme saved: {}", p.display()));
                if app.theme_name == self.file.name { let n = self.file.name.clone(); app.use_theme(&n, false); }
                true
            }
            Err(e) => { self.message = Some((true, format!("{e:#}"))); false }
        }
    }

    /// Put the selected row back as saved (in the shown variant): a palette colour, an
    /// element's overrides, or one key of an element.
    pub fn revert_row(&mut self) {
        let row = self.current();
        if let Row::Brand(k) = &row {
            let old = self.saved.as_ref().and_then(|s| s.brand.as_ref()).map(|b| brand_field(b, k)).unwrap_or_default();
            let b = self.file.brand.get_or_insert_with(ThemeBrand::red_star);
            set_brand_field(b, k, old);
        } else {
        let saved = self.saved.as_ref().map(|s| s.section(self.variant)).unwrap_or_default();
        let s = self.file.section_mut(self.variant);
        match &row {
            Row::Heading(_) => return,
            Row::Brand(_) => {}
            Row::Palette(n) => match saved.get(n) {
                Some(c) => s.set(n, c.clone()),
                None => s.palette.retain(|(k, _)| k != n),
            },
            Row::Element(e) => s.set_style(*e, saved.style(*e)),
            Row::Field(e, k) => {
                let (mut st, old) = (s.style(*e), saved.style(*e));
                st.set_color(k, old.color(k).cloned());
                st.set_flag(k, old.flag(k));
                s.set_style(*e, st);
            }
        }
        }
        // Nothing left that differs: the file is the saved one again (e.g. no `[light]` copy).
        if let Some(sv) = &self.saved
            && [Variant::Dark, Variant::Light].iter().all(|v| self.file.section(*v) == sv.section(*v))
            && self.file.name == sv.name && self.file.description == sv.description && self.file.brand == sv.brand {
            self.file = sv.clone();
        }
        self.message = Some((false, format!("{} reverted", self.row_name().unwrap_or_default())));
        self.settle(1);
        self.sync_preview();
    }

    /// The truecolor note: shown when the theme uses `#rrggbb` colours and the terminal
    /// doesn't say it shows 24-bit colour.
    pub fn truecolor_note(&self) -> Option<&'static str> {
        if self.truecolor { return None; }
        let s = self.file.section(self.variant);
        let rgb = |c: &ColorSpec| matches!(c, ColorSpec::Value(Color::Rgb(..)));
        let uses = s.palette.iter().any(|(_, c)| rgb(c))
            || s.styles.iter().any(|(_, st)| [&st.fg, &st.bg, &st.border].into_iter().flatten().any(rgb));
        uses.then_some("#rrggbb colours need a true-colour terminal ($COLORTERM isn't truecolor): you may see the nearest of 256")
    }

    fn revert(&mut self) {
        self.file = self.saved.clone().unwrap_or_else(|| ThemeFile::empty(&self.file.name));
        self.message = Some((false, "reverted to the saved file".into()));
        self.settle(1);
        self.sync_preview();
    }

    /// Theme names in the folder.
    fn names(app: &App) -> Vec<String> { app.theme_entries().into_iter().map(|e| e.name).collect() }

    fn open_other(&mut self, app: &App, name: &str) {
        let v = self.variant;
        *self = ThemeEditor::open(app, name);
        self.variant = v;
        self.sync_preview();
    }

    fn step_theme(&mut self, app: &App, d: i32) {
        let names = Self::names(app);
        if names.is_empty() { return; }
        let i = names.iter().position(|n| *n == self.file.name).map(|i| (i as i32 + d).rem_euclid(names.len() as i32) as usize).unwrap_or(0);
        let next = names[i].clone();
        if self.dirty() { self.ask(Ask::UnsavedOpen(next)); } else { self.open_other(app, &next); }
    }

    fn prompt_key(&mut self, app: &mut App, k: KeyEvent) -> Outcome {
        let Some(mut p) = self.prompt.take() else { return Outcome::Stay };
        let named = matches!(p.ask, Ask::NewName | Ask::DuplicateName | Ask::PaletteName);
        if named {
            match k.code {
                KeyCode::Esc => return Outcome::Stay,
                KeyCode::Enter => {
                    let name = p.field.value.trim().to_string();
                    if !themes::valid_name(&name) { p.error = Some("use 1–32 of a-z, 0-9, - and _".into()); self.prompt = Some(p); return Outcome::Stay; }
                    match p.ask {
                        Ask::PaletteName => {
                            if self.palette_names().contains(&name) { p.error = Some(format!("{name} is already in the palette")); self.prompt = Some(p); return Outcome::Stay; }
                            self.file.section_mut(self.variant).set(&name, ColorSpec::Value(Color::Reset));
                            if let Some(i) = self.rows().iter().position(|r| *r == Row::Palette(name.clone())) { self.cursor = i; }
                            self.open_picker(Target::Palette(name));
                        }
                        _ => {
                            if Self::names(app).contains(&name) { p.error = Some(format!("{name}.toml already exists")); self.prompt = Some(p); return Outcome::Stay; }
                            let mut f = if p.ask == Ask::NewName {
                                let dir = app.themes_dir.clone().unwrap_or_default();
                                themes::load(&dir, DEFAULT_THEME).map(|l| l.file).unwrap_or_else(|_| ThemeFile::empty(&name))
                            } else { self.file.clone() };
                            f.name = name.clone();
                            f.description = if p.ask == Ask::NewName { "A new theme".into() } else { format!("A copy of {}", self.file.name) };
                            self.file = f;
                            self.saved = None;
                            self.disk = None;
                            self.broken = false;
                            self.message = Some((false, format!("{name}: not saved yet (^S saves)")));
                            self.sync_preview();
                        }
                    }
                    return Outcome::Stay;
                }
                _ => { p.field.on_key(k); p.error = None; }
            }
            self.prompt = Some(p);
            return Outcome::Stay;
        }
        match (&p.ask, k.code) {
            (_, KeyCode::Esc) | (_, KeyCode::Char('c')) | (Ask::Delete(_), KeyCode::Char('n')) => Outcome::Stay,
            (Ask::Delete(name), KeyCode::Char('y')) => {
                let name = name.clone();
                if name == app.theme_name { self.message = Some((true, format!("{name} is the theme in use; switch to another (F5) first"))); return Outcome::Stay; }
                let dir = app.themes_dir.clone().unwrap_or_default();
                match themes::delete(&dir, &name) {
                    Ok(()) => {
                        app.push_status(format!("theme {name} deleted"));
                        let cur = app.theme_name.clone();
                        self.open_other(app, &cur);
                        self.message = Some((false, format!("deleted {name}.toml")));
                    }
                    Err(e) => self.message = Some((true, format!("{e:#}"))),
                }
                Outcome::Stay
            }
            (Ask::UnsavedClose | Ask::UnsavedOpen(_), KeyCode::Char('s') | KeyCode::Enter) => {
                if !self.save_with(app, false, Some(p.ask.clone())) { return Outcome::Stay; }
                self.after_unsaved(app, p.ask)
            }
            (Ask::ChangedOnDisk(then) | Ask::OverwriteBroken(then), KeyCode::Char('o') | KeyCode::Char('y')) => {
                let then = then.clone().map(|b| *b);
                if !self.save_with(app, true, None) { return Outcome::Stay; }
                match then { Some(a) => self.after_unsaved(app, a), None => Outcome::Stay }
            }
            (Ask::ChangedOnDisk(_), KeyCode::Char('l') | KeyCode::Char('r')) => {
                let n = self.file.name.clone();
                self.open_other(app, &n);
                self.message = Some((false, format!("reloaded {n}.toml from disk (your changes were dropped)")));
                Outcome::Stay
            }
            (Ask::UnsavedClose | Ask::UnsavedOpen(_), KeyCode::Char('d')) => self.after_unsaved(app, p.ask),
            _ => { self.prompt = Some(p); Outcome::Stay }
        }
    }

    fn after_unsaved(&mut self, app: &App, ask: Ask) -> Outcome {
        match ask {
            Ask::UnsavedOpen(n) => { self.open_other(app, &n); Outcome::Stay }
            _ => Outcome::Close,
        }
    }

    /// Handle a key.
    fn edit_key(&mut self, k: KeyEvent) {
        let Some(mut edit) = self.editing.take() else { return };
        match k.code {
            KeyCode::Enter | KeyCode::Esc => (),
            _ => {
                edit.field.on_key(k);
                let b = self.file.brand.get_or_insert_with(ThemeBrand::red_star);
                set_brand_field(b, edit.key, edit.field.value.clone());
                self.editing = Some(edit);
                self.sync_preview();
            }
        }
    }

    /// Handle a key.
    pub fn on_key(&mut self, app: &mut App, k: KeyEvent) -> Outcome {
        if self.prompt.is_some() { return self.prompt_key(app, k); }
        if self.picker.is_some() { self.picker_key(k); return Outcome::Stay; }
        if self.editing.is_some() { self.edit_key(k); return Outcome::Stay; }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        self.message = None;
        match k.code {
            KeyCode::Char('s') if ctrl => { self.save(app); }
            KeyCode::Char('r') if ctrl => self.revert(),
            KeyCode::Char('n') if ctrl => self.ask(Ask::NewName),
            KeyCode::Char('d') if ctrl => self.ask(Ask::DuplicateName),
            KeyCode::Char('x') if ctrl => { let n = self.file.name.clone(); self.ask(Ask::Delete(n)); }
            KeyCode::Char('c') if ctrl => return self.close_or_ask(),
            KeyCode::Esc | KeyCode::F(17) => return self.close_or_ask(),
            KeyCode::F(5) if k.modifiers.contains(KeyModifiers::SHIFT) => return self.close_or_ask(),
            KeyCode::Tab | KeyCode::BackTab => { self.variant = self.variant.flip(); self.settle(1); self.sync_preview(); }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::PageUp => self.move_cursor(-10),
            KeyCode::PageDown => self.move_cursor(10),
            KeyCode::Home => { self.cursor = 0; self.settle(1); }
            KeyCode::End => { self.cursor = self.rows().len() - 1; self.settle(-1); }
            KeyCode::Enter | KeyCode::Char(' ') => self.activate(),
            KeyCode::Right | KeyCode::Char('l') => if let Row::Element(e) = self.current() { self.expanded = Some(e); },
            KeyCode::Char('h') => self.highlight = !self.highlight,
            KeyCode::Char('r') => self.revert_row(),
            KeyCode::Left => {
                if let Row::Field(e, _) = self.current() && let Some(i) = self.rows().iter().position(|r| *r == Row::Element(e)) { self.cursor = i; }
                self.expanded = None;
            }
            KeyCode::Delete | KeyCode::Backspace => self.clear(),
            KeyCode::Char('a') | KeyCode::Char('+') => self.ask(Ask::PaletteName),
            KeyCode::Char('[') => self.step_theme(app, -1),
            KeyCode::Char(']') => self.step_theme(app, 1),
            _ => {}
        }
        Outcome::Stay
    }

    fn close_or_ask(&mut self) -> Outcome {
        if self.dirty() { self.ask(Ask::UnsavedClose); Outcome::Stay } else { Outcome::Close }
    }

    /// A click on list row `i` selects it (a second click activates it).
    pub fn click_row(&mut self, i: usize) {
        if self.prompt.is_some() || self.picker.is_some() || self.editing.is_some() { self.editing = None; return; }
        if matches!(self.rows().get(i), Some(Row::Heading(_)) | None) { return; }
        if self.cursor == i { self.activate(); } else { self.cursor = i; }
    }
}

fn set_target(f: &mut ThemeFile, v: Variant, t: &Target, c: Option<ColorSpec>) {
    match t {
        Target::Palette(n) => f.section_mut(v).set(n, c.unwrap_or(ColorSpec::Value(Color::Reset))),
        Target::Field(e, k) => {
            let mut st = f.section(v).style(*e);
            st.set_color(k, c);
            f.section_mut(v).set_style(*e, st);
        }
    }
}

/// The background text sits on when a theme leaves it to the terminal (assumed
/// black for dark, white for light), and the matching text colour.
fn assumed(t: &Theme, v: Variant) -> (Color, Color) {
    let dark = v == Variant::Dark;
    let bg = if t.bg != Color::Reset { t.bg } else if dark { Color::Black } else { Color::White };
    let fg = if t.fg != Color::Reset { t.fg } else if dark { Color::White } else { Color::Black };
    (fg, bg)
}

/// Every palette and element row of `f`'s `v` variant (nothing expanded), e.g. to check all
/// of a theme's contrast warnings at once.
pub fn all_rows(f: &ThemeFile, v: Variant) -> Vec<Row> {
    let s = f.section(v);
    let mut names: Vec<String> = PALETTE_KEYS.iter().map(|(k, _)| k.to_string()).collect();
    for (n, _) in &s.palette { if !names.contains(n) { names.push(n.clone()); } }
    names.into_iter().map(Row::Palette).chain(Element::ALL.iter().map(|e| Row::Element(*e))).collect()
}

/// A row's text contrast in `f` when below [`THEME_MIN_CONTRAST`].
pub fn warning(f: &ThemeFile, v: Variant, r: &Row) -> Option<f64> {
    let t = Theme::from_file(f, v);
    let (fg, bg) = assumed(&t, v);
    let ratio = match r {
        Row::Palette(n) => match n.as_str() {
            "on_accent" => contrast(t.on_accent, t.accent),
            "accent" => contrast(t.on_accent, t.accent).into_iter().chain(contrast(t.accent, bg)).reduce(f64::min),
            "muted" => contrast(t.muted, bg),
            "fg" | "bg" => contrast(fg, bg),
            _ => None,
        },
        Row::Element(e) | Row::Field(e, _) => {
            if matches!(e, Element::Dimmed | Element::Border | Element::BorderFocused) || e.is_box() { return None; }
            let st = ratatui::style::Style::from(&t.element(*e));
            let (mut a, mut b) = (st.fg.filter(|c| *c != Color::Reset).unwrap_or(fg), st.bg.filter(|c| *c != Color::Reset).unwrap_or(bg));
            if st.add_modifier.contains(ratatui::style::Modifier::REVERSED) { std::mem::swap(&mut a, &mut b); }
            contrast(a, b)
        }
        Row::Heading(_) | Row::Brand(_) => None,
    };
    ratio.filter(|x| *x < THEME_MIN_CONTRAST)
}

fn brand_field(b: &ThemeBrand, k: &str) -> String {
    match k {
        "title" => b.title.clone(),
        "motto" => b.motto.clone(),
        "word" => b.word.clone(),
        "logo" => b.logo.clone(),
        "logo_text" => b.logo_text.clone(),
        "logo_file" => b.logo_file.clone(),
        _ => String::new(),
    }
}

fn set_brand_field(b: &mut ThemeBrand, k: &str, v: String) {
    match k {
        "title" => b.title = v,
        "motto" => b.motto = v,
        "word" => b.word = v,
        "logo" => b.logo = v,
        "logo_text" => b.logo_text = v,
        "logo_file" => b.logo_file = v,
        _ => {}
    }
}

fn cycle_logo(cur: &str) -> &'static str {
    crate::brands::LogoKind::parse(cur).unwrap_or(crate::brands::LogoKind::Builtin).next().as_str()
}

/// Whether `$COLORTERM` says the terminal shows 24-bit colour.
pub fn truecolor(colorterm: Option<&str>) -> bool {
    colorterm.is_some_and(|v| { let v = v.to_ascii_lowercase(); v.contains("truecolor") || v.contains("24bit") })
}
