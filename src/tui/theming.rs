//! Theme files inside the TUI: selecting, previewing, reloading and exporting
//! themes, and the picker behind F5 and the header's `‹theme› ▾` switch.

use crate::config::DEFAULT_THEME;
use crate::themes::{self, Variant};
use crate::tui::modals::*;
use crate::tui::state::{App, Effect};
use crate::tui::theme::{Theme, rgb_bytes};

impl App {
    /// The variant in use (`[ui] theme_mode`, and the terminal's background for `system`).
    pub fn variant(&self) -> Variant { self.theme_mode.variant(self.dark_bg) }

    /// Use `t` for drawing (and recolour the header logo).
    pub fn apply_theme(&mut self, t: Theme) {
        self.theme = t;
        if let Some(im) = self.images.as_mut() { im.set_logo_colors(rgb_bytes(t.on_accent), rgb_bytes(t.accent)); }
    }

    /// Select theme `name` from the themes folder; `persist` also stores it as `[ui] theme`.
    /// A missing or broken file selects the terminal's colours and explains how to fix it.
    pub fn use_theme(&mut self, name: &str, persist: bool) -> bool {
        let Some(dir) = self.themes_dir.clone() else { return false };
        self.theme_name = name.to_string();
        match themes::load(&dir, name) {
            Ok(l) => {
                if let Some(n) = themes::warning_notice(&l.path, &l.warnings) { self.push_status(n); }
                let t = Theme::from_file(&l.file, self.variant());
                let has_brand = l.file.brand.is_some();
                if !self.brand_pinned || has_brand {
                    let b = self.branding_of(&l.file);
                    self.install_brand(b);
                    if has_brand { self.brand_pinned = false; }
                }
                self.theme_file = Some(l.file);
                self.theme_error = None;
                self.theme_fault = None;
                self.apply_theme(t);
                if persist { self.effects.push(Effect::SaveUiTheme(name.to_string())); }
                self.note_no_color();
                true
            }
            Err(e) => {
                let n = e.notice();
                self.theme_fault = Some(if matches!(e, themes::LoadError::Missing(_)) { "missing" } else { "broken" });
                self.theme_file = None;
                self.theme_error = Some(n.clone());
                self.apply_theme(Theme::reset());
                self.push_status(format!("warning: {n}"));
                self.modal = Some(Modal::Message(MessageModal::error("Theme", &n)));
                self.note_no_color();
                false
            }
        }
    }

    /// Redraw with the loaded theme in the current variant (after a background change).
    pub fn refresh_theme(&mut self) {
        let t = self.theme_file.as_ref().map(|f| Theme::from_file(f, self.variant())).unwrap_or_else(Theme::reset);
        self.apply_theme(t);
    }

    /// Read the selected theme's file again.
    pub fn reload_theme(&mut self) {
        let n = self.theme_name.clone();
        if self.use_theme(&n, false) { self.push_status(format!("theme {n} reloaded")); self.note_no_color(); }
    }

    /// Every theme file, for the picker.
    pub fn theme_entries(&self) -> Vec<themes::Entry> { self.themes_dir.as_deref().map(themes::list).unwrap_or_default() }

    /// Open the theme picker (F5).
    /// Branding for `file`: its `[brand]`, else `~/.acp/brands/<name>.toml`, else a built-in.
    pub fn branding_of(&self, file: &themes::ThemeFile) -> crate::brands::Brand {
        if let Some(b) = &file.brand {
            return crate::brands::from_theme_brand(&file.name, b, self.brands_dir.as_deref());
        }
        if let Some(dir) = &self.brands_dir && let Ok(b) = crate::brands::resolve(dir, &file.name) {
            return b;
        }
        match file.name.as_str() {
            "plain" => crate::brands::builtin_plain(),
            _ => crate::brands::builtin_default(),
        }
    }

    /// Put `brand` on screen (header, menus, About). Does not write preferences.
    pub fn install_brand(&mut self, brand: crate::brands::Brand) {
        self.brand_name = brand.name.clone();
        self.menu.menu.apply_brand(&brand);
        crate::brands::install(&brand);
        self.brand_error = None;
        self.brand_fault = None;
        self.brand = brand;
    }

    /// Open the theme picker (F5).
    pub fn open_theme_picker(&mut self) {
        let entries = self.theme_entries();
        self.modal = Some(Modal::ThemePicker(Box::new(ThemePickerModal::new(entries, &self.theme_name, self.theme, self.brand.clone()))));
        self.note_no_color();
    }

    /// Show the highlighted picker entry (without selecting it).
    pub fn preview_picker(&mut self) {
        let Some(Modal::ThemePicker(p)) = &self.modal else { return };
        let restore = p.restore;
        let restore_brand = p.restore_brand.clone();
        let highlighted = p.highlighted().map(str::to_string);
        let dir = self.themes_dir.clone();
        let loaded = highlighted.zip(dir).and_then(|(name, dir)| themes::load(&dir, &name).ok());
        match loaded {
            Some(l) => {
                if !self.brand_pinned {
                    let b = self.branding_of(&l.file);
                    self.install_brand(b);
                }
                self.apply_theme(Theme::from_file(&l.file, self.variant()));
            }
            None => {
                if !self.brand_pinned { self.install_brand(restore_brand); }
                self.apply_theme(restore);
            }
        }
    }

    /// The picker closed.
    pub fn theme_picked(&mut self, p: ThemePickerModal, r: ModalResult) {
        let choice = p.chosen();
        let restore = p.restore;
        let restore_brand = p.restore_brand;
        match (r, choice) {
            (ModalResult::ClosedSubmitted, PickerChoice::Theme(name)) => {
                self.brand_pinned = false;
                self.use_theme(&name, true);
            }
            (ModalResult::ClosedSubmitted, PickerChoice::Edit) => {
                self.install_brand(restore_brand);
                self.apply_theme(restore);
                self.open_theme_editor();
            }
            (ModalResult::ClosedSubmitted, PickerChoice::Download) => {
                self.install_brand(restore_brand);
                self.apply_theme(restore);
                self.init_theme();
            }
            (ModalResult::ClosedSubmitted, PickerChoice::Starter) => {
                self.install_brand(restore_brand);
                self.apply_theme(restore);
                self.write_starter_theme(None, true);
            }
            _ => {
                self.install_brand(restore_brand);
                self.apply_theme(restore);
            }
        }
    }

    /// Open the full-screen theme editor (Shift-F5) on the selected theme.
    pub fn open_theme_editor(&mut self) {
        if self.themes_dir.is_none() { self.error("Theme editor", "no themes folder (~/.acp/themes)"); return; }
        let name = self.theme_name.clone();
        self.theme_editor = Some(Box::new(crate::tui::editor::ThemeEditor::open(self, &name)));
        self.note_no_color();
    }

    /// Show every theme file in a message.
    pub fn list_themes(&mut self) {
        let dir = self.themes_dir.clone().unwrap_or_default();
        let mut lines = vec![format!("Themes in {}:", dir.display()), String::new()];
        let entries = self.theme_entries();
        if entries.is_empty() { lines.push("  (none)".into()); }
        for e in entries {
            let mark = if e.name == self.theme_name { "●" } else { " " };
            lines.push(match e.error {
                Some(err) => format!("{mark} {}  ⚠ can't be used: {err}", e.name),
                None => {
                    let title = if e.title.is_empty() { String::new() } else { format!("  · {}", e.title) };
                    format!("{mark} {}  {}{title}", e.name, e.description)
                }
            });
        }
        lines.push(String::new());
        lines.push(format!("Mode: {} ([ui] theme_mode). F5 switches, Shift-F5 edits.", self.variant().name()));
        self.modal = Some(Modal::Message(MessageModal::info("Themes", &lines.join("\n"))));
    }

    /// Export the selected theme (a copy in the exports folder).
    pub fn export_theme(&mut self) {
        match self.theme_file.clone() {
            Some(f) => self.effects.push(Effect::ExportTheme(Box::new(f))),
            None => self.error("Export theme", self.theme_error.clone().as_deref().unwrap_or("no theme loaded")),
        }
    }

    /// Theme: init: download the default theme (an existing file is kept).
    pub fn init_theme(&mut self) {
        if self.themes_dir.is_none() { return self.error("Theme: init", "no themes folder"); }
        if self.theme_downloading.is_some() { return self.push_status("a theme download is already running"); }
        self.start_theme_download(DEFAULT_THEME, true);
    }

    /// Download `name` in the background; the result arrives in [`App::theme_downloaded`].
    pub fn start_theme_download(&mut self, name: &str, manual: bool) {
        self.theme_downloading = Some(name.to_string());
        self.push_status(format!("downloading the {name} theme from {}…", crate::config::theme_download_url(name)));
        self.effects.push(Effect::DownloadTheme(name.to_string(), manual));
    }

    /// A background download finished. On success the theme is applied when it is still
    /// the selected one and could not be loaded (the user hasn't picked another);
    /// on failure the terminal's colours stay and a notice says how to fix it.
    pub fn theme_downloaded(&mut self, name: &str, manual: bool, r: Result<themes::download::Fetched, String>) {
        self.theme_downloading = None;
        let Some(dir) = self.themes_dir.clone() else { return };
        let waiting = self.theme_name == name && self.theme_error.is_some();
        match r {
            Ok(f) => {
                let mut n = themes::download::done_notice(name, &f);
                if waiting && self.theme_editor.is_none() && self.use_theme(name, false) { n.push_str(" and applied it"); }
                self.push_status(n.clone());
                if manual && self.modal.is_none() { self.modal = Some(Modal::Message(MessageModal::info("Theme", &n))); }
            }
            Err(e) => {
                let n = themes::download::failure_notice(&dir, name, &e);
                if waiting { self.theme_error = Some(n.clone()); }
                self.push_status(format!("warning: {n}"));
                if (manual || waiting) && self.modal.is_none() && self.theme_editor.is_none() {
                    self.modal = Some(Modal::Message(MessageModal::error("Theme", &n)));
                }
            }
        }
    }

    /// `Theme: starter [name]`: write the minimal starter theme (no download, never over a
    /// file; without a name the first free `starter`, `starter-2`…). It is used at once
    /// when `select` is set (the F5 list's "Create starter theme") or when the selected
    /// theme can't be loaded.
    pub fn theme_starter(&mut self, name: Option<String>) { self.write_starter_theme(name, false) }

    /// See [`App::theme_starter`].
    pub fn write_starter_theme(&mut self, name: Option<String>, select: bool) {
        let Some(dir) = self.themes_dir.clone() else { return self.error("Theme: starter", "no themes folder (~/.acp/themes)") };
        let name = name.unwrap_or_else(|| themes::download::free_name(&dir, "starter"));
        match themes::download::write_starter(&dir, &name) {
            Ok(f) => {
                let n = themes::download::starter_notice(&name, &f);
                self.push_status(n.clone());
                let written = matches!(f, themes::download::Fetched::Written(_));
                if written && (select || self.theme_error.is_some()) { self.use_theme(&name, select); }
                if self.modal.is_none() {
                    self.modal = Some(if written { Modal::Message(MessageModal::info("Theme", &n)) } else { Modal::Message(MessageModal::error("Theme", &n)) });
                }
            }
            Err(e) => self.error("Theme: starter", &format!("{e:#}")),
        }
    }
}
