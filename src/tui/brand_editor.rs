//! The brand editor: title, motto, word, and logo.
//!
//! The chords match the theme editor: Ctrl-S saves, Ctrl-R reverts, Ctrl-N saves
//! as new, Ctrl-D duplicates, Ctrl-X deletes, Ctrl-C closes. Esc closes too.

use crate::brands::{self, Brand, LogoKind};
use crate::prelude::*;
use crate::tui::state::{App, Hit};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

/// Stay in the editor, or close it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Keep editing.
    Stay,
    /// Close. The caller restores the saved brand when the draft was discarded.
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Ask {
    /// Edit the field at this row.
    Field,
    NewName,
    DupName,
    Delete,
    UnsavedClose,
}

#[derive(Clone, Debug)]
struct Prompt {
    ask: Ask,
    text: String,
    error: Option<String>,
}

/// Full-screen editor for one brand.
#[derive(Clone, Debug)]
pub struct BrandEditor {
    /// What is being edited.
    pub draft: Brand,
    /// The last saved copy (built-in or file), restored by Ctrl-R and by a discarded close.
    pub saved: Brand,
    /// Highlighted field.
    pub cursor: usize,
    prompt: Option<Prompt>,
    /// A line under the fields.
    pub message: Option<String>,
}

const ROWS: &[&str] = &["Title", "Motto", "Word", "Logo", "Logo text", "Logo file"];

impl BrandEditor {
    /// Open on the brand the app is using.
    pub fn open(app: &App) -> Self {
        let saved = app.brand.clone();
        BrandEditor { draft: saved.clone(), saved, cursor: 0, prompt: None, message: None }
    }

    fn dirty(&self) -> bool { self.draft != self.saved }

    /// Handle a key. Ctrl chords match the theme editor.
    pub fn on_key(&mut self, app: &mut App, k: KeyEvent) -> Outcome {
        if self.prompt.is_some() { return self.prompt_key(app, k); }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        self.message = None;
        match k.code {
            KeyCode::Char('s') if ctrl => { self.save(app); }
            KeyCode::Char('r') if ctrl => self.revert(app),
            KeyCode::Char('n') if ctrl => self.prompt = Some(Prompt { ask: Ask::NewName, text: String::new(), error: None }),
            KeyCode::Char('d') if ctrl => self.prompt = Some(Prompt { ask: Ask::DupName, text: String::new(), error: None }),
            KeyCode::Char('x') if ctrl => self.prompt = Some(Prompt { ask: Ask::Delete, text: String::new(), error: None }),
            KeyCode::Char('c') if ctrl => return self.close_or_ask(),
            KeyCode::Esc => return self.close_or_ask(),
            KeyCode::Up | KeyCode::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.cursor = (self.cursor + 1).min(ROWS.len() - 1),
            KeyCode::Enter | KeyCode::Char(' ') => self.activate(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = ROWS.len() - 1,
            _ => {}
        }
        self.preview(app);
        Outcome::Stay
    }

    fn close_or_ask(&mut self) -> Outcome {
        if self.dirty() {
            self.prompt = Some(Prompt { ask: Ask::UnsavedClose, text: String::new(), error: None });
            Outcome::Stay
        } else {
            Outcome::Close
        }
    }

    fn activate(&mut self) {
        if self.cursor == 3 {
            self.draft.logo = match self.draft.logo {
                LogoKind::Builtin => LogoKind::Text,
                LogoKind::Text => LogoKind::File,
                LogoKind::File => LogoKind::Builtin,
            };
            if self.draft.logo == LogoKind::Text && self.draft.logo_text.is_empty() {
                self.draft.logo_text = "★".into();
            }
            return;
        }
        let text = match self.cursor {
            0 => self.draft.title.clone(),
            1 => self.draft.motto.clone(),
            2 => self.draft.word.clone(),
            4 => self.draft.logo_text.clone(),
            5 => self.draft.logo_file.clone(),
            _ => return,
        };
        self.prompt = Some(Prompt { ask: Ask::Field, text, error: None });
    }

    fn preview(&mut self, app: &mut App) {
        app.brand = self.draft.clone();
        brands::install(&app.brand);
    }

    fn revert(&mut self, app: &mut App) {
        self.draft = self.saved.clone();
        self.message = Some("reverted".into());
        self.preview(app);
    }

    fn save(&mut self, app: &mut App) -> bool {
        let Some(dir) = app.brands_dir.clone() else {
            self.message = Some("no brands folder (~/.acp/brands)".into());
            return false;
        };
        match brands::finalize(&self.draft, &dir) {
            Ok(b) => self.draft = b,
            Err(e) => { self.message = Some(e); return false; }
        }
        match brands::save(&dir, &self.draft) {
            Ok(p) => {
                self.draft.from_file = true;
                self.saved = self.draft.clone();
                app.brand_name = self.draft.name.clone();
                app.brand_error = None;
                app.brand_fault = None;
                self.preview(app);
                let note = format!("saved {}", p.display());
                self.message = Some(note.clone());
                app.push_status(note);
                app.effects.push(crate::tui::state::Effect::SaveUiBrand(self.draft.name.clone()));
                true
            }
            Err(e) => { self.message = Some(format!("{e:#}")); false }
        }
    }

    fn prompt_key(&mut self, app: &mut App, k: KeyEvent) -> Outcome {
        let Some(mut p) = self.prompt.take() else { return Outcome::Stay };
        match p.ask {
            Ask::Field => match k.code {
                KeyCode::Esc => return Outcome::Stay,
                KeyCode::Enter => {
                    self.set_field(&p.text);
                    self.preview(app);
                    return Outcome::Stay;
                }
                KeyCode::Backspace => { p.text.pop(); }
                KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) && c != '\n' && c != '\r' => p.text.push(c),
                _ => {}
            },
            Ask::NewName | Ask::DupName => match k.code {
                KeyCode::Esc => return Outcome::Stay,
                KeyCode::Enter => {
                    let name = p.text.trim().to_string();
                    if !crate::themes::valid_name(&name) {
                        p.error = Some("use 1–32 of a-z, 0-9, - and _".into());
                        self.prompt = Some(p);
                        return Outcome::Stay;
                    }
                    if let Some(dir) = app.brands_dir.clone()
                        && brands::brand_path(&dir, &name).ok().is_some_and(|path| path.exists())
                    {
                        p.error = Some(format!("{name}.toml already exists"));
                        self.prompt = Some(p);
                        return Outcome::Stay;
                    }
                    if p.ask == Ask::NewName {
                        let mut b = brands::builtin_plain();
                        b.name = name.clone();
                        b.title = name.clone();
                        b.description = "A new brand".into();
                        b.from_file = false;
                        self.draft = b;
                    } else {
                        self.draft.name = name.clone();
                        self.draft.description = format!("A copy of {}", self.saved.name);
                        self.draft.from_file = false;
                    }
                    self.message = Some(format!("{name}: not saved yet (Ctrl-S saves)"));
                    self.preview(app);
                    return Outcome::Stay;
                }
                KeyCode::Backspace => { p.text.pop(); }
                KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => p.text.push(c),
                _ => {}
            },
            Ask::Delete => match k.code {
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('c') => return Outcome::Stay,
                KeyCode::Char('y') => {
                    self.delete(app);
                    return Outcome::Stay;
                }
                _ => { self.prompt = Some(p); return Outcome::Stay; }
            },
            Ask::UnsavedClose => match k.code {
                KeyCode::Esc | KeyCode::Char('c') => return Outcome::Stay,
                KeyCode::Char('s') | KeyCode::Enter => {
                    if !self.save(app) { self.prompt = Some(p); return Outcome::Stay; }
                    return Outcome::Close;
                }
                KeyCode::Char('d') => {
                    app.brand = self.saved.clone();
                    brands::install(&app.brand);
                    return Outcome::Close;
                }
                _ => { self.prompt = Some(p); return Outcome::Stay; }
            },
        }
        self.prompt = Some(p);
        Outcome::Stay
    }

    fn set_field(&mut self, text: &str) {
        let text = text.to_string();
        match self.cursor {
            0 => self.draft.title = text,
            1 => self.draft.motto = text,
            2 => self.draft.word = text,
            4 => self.draft.logo_text = text,
            5 => self.draft.logo_file = text,
            _ => {}
        }
    }

    fn delete(&mut self, app: &mut App) {
        let Some(dir) = app.brands_dir.clone() else { return };
        let name = self.draft.name.clone();
        let builtin = brands::builtin(&name).is_some();
        if name == app.brand_name && !builtin {
            self.message = Some(format!("{name} is the brand in use; switch to another first"));
            return;
        }
        match brands::delete(&dir, &name) {
            Ok(()) => {
                app.push_status(format!("brand {name} file deleted"));
                if let Ok(b) = brands::resolve(&dir, &name) {
                    self.draft = b.clone();
                    self.saved = b;
                    self.preview(app);
                    self.message = Some(format!("deleted {name}.toml; the built-in is back"));
                } else if builtin {
                    self.message = Some(format!("deleted {name}.toml"));
                } else {
                    let b = brands::builtin_default();
                    self.draft = b.clone();
                    self.saved = b;
                    app.brand_name = self.draft.name.clone();
                    self.preview(app);
                    self.message = Some(format!("deleted {name}.toml"));
                }
            }
            Err(e) => self.message = Some(format!("{e:#}")),
        }
    }

    /// A click on field row `i` (the header is not a field).
    pub fn click_row(&mut self, i: usize) {
        if self.prompt.is_some() || i >= ROWS.len() { return; }
        if self.cursor == i { self.activate(); } else { self.cursor = i; }
    }

    /// Draw the header with the draft, then the fields.
    pub fn draw(&mut self, f: &mut Frame, app: &mut App) {
        app.brand = self.draft.clone();
        brands::install(&app.brand);
        let area = f.area();
        let t = app.theme;
        f.buffer_mut().set_style(area, ratatui::style::Style::from(&t.base()));
        let header = Rect { x: area.x, y: area.y, width: area.width, height: TUI_HEADER_ROWS.min(area.height) };
        crate::tui::widgets::topbar(f, app, header);
        let y0 = header.y + header.height;
        let help = "Ctrl-S save · Ctrl-R revert · Ctrl-N new · Ctrl-D duplicate · Ctrl-X delete · Ctrl-C close";
        let mut line = |y, text: String, st| {
            let r = Rect { x: area.x, y, width: area.width, height: 1 };
            f.render_widget(Paragraph::new(Span::styled(text, st)), r);
            r
        };
        if y0 < area.y + area.height {
            line(y0, format!(" Brand editor · {help}"), t.muted());
        }
        for (i, label) in ROWS.iter().enumerate() {
            let y = y0 + 2 + i as u16;
            if y >= area.y + area.height { break; }
            let value = match i {
                0 => self.draft.title.clone(),
                1 => if self.draft.motto.is_empty() { "(none)".into() } else { self.draft.motto.clone() },
                2 => if self.draft.word.is_empty() { "(none)".into() } else { self.draft.word.clone() },
                3 => self.draft.logo.as_str().into(),
                4 => one_line_preview(&self.draft.logo_text),
                _ => if self.draft.logo_file.is_empty() { "(none)".into() } else { self.draft.logo_file.clone() },
            };
            let mark = if i == self.cursor { "▸" } else { " " };
            let st = if i == self.cursor { t.selected() } else { t.plain() };
            let r = line(y, format!(" {mark} {label:<12} {value}"), st);
            app.hits.push((r, Hit::EditorRow(i)));
        }
        let mut y = y0 + 2 + ROWS.len() as u16 + 1;
        if let Some(p) = &self.prompt {
            if y < area.y + area.height {
                let prompt = match p.ask {
                    Ask::Field => format!(" Edit {} : {}", ROWS[self.cursor], p.text),
                    Ask::NewName => format!(" New brand name: {}", p.text),
                    Ask::DupName => format!(" Duplicate as: {}", p.text),
                    Ask::Delete => " Delete this brand file? y / n".into(),
                    Ask::UnsavedClose => " Save changes? s save · d discard · Esc cancel".into(),
                };
                line(y, prompt, t.accent_bold());
                y += 1;
            }
            if let Some(e) = &p.error && y < area.y + area.height {
                line(y, format!(" {e}"), t.accent());
                y += 1;
            }
        }
        if let Some(m) = &self.message && y < area.y + area.height {
            line(y, format!(" {m}"), t.muted());
        }
    }
}

impl App {
    /// Use brand `name`. `persist` stores it as `[ui] brand`.
    ///
    /// A missing or broken brand keeps the default look, names the file, and does not overwrite it.
    pub fn use_brand(&mut self, name: &str, persist: bool) -> bool {
        let Some(dir) = self.brands_dir.clone() else { return false };
        self.brand_name = name.to_string();
        match brands::resolve(&dir, name) {
            Ok(b) => {
                self.brand_error = None;
                self.brand_fault = None;
                self.brand = b.clone();
                brands::install(&b);
                if persist { self.effects.push(crate::tui::state::Effect::SaveUiBrand(name.to_string())); }
                true
            }
            Err(e) => {
                let n = e.notice();
                self.brand_fault = Some(if matches!(e, brands::LoadError::Missing(_)) { "missing" } else { "broken" });
                self.brand_error = Some(n.clone());
                let fallback = brands::builtin_default();
                brands::install(&fallback);
                self.brand = fallback;
                self.push_status(format!("warning: {n}"));
                self.modal = Some(crate::tui::modals::Modal::Message(crate::tui::modals::MessageModal::error("Brand", &n)));
                false
            }
        }
    }

    /// Open the brand picker.
    pub fn open_brand_picker(&mut self) {
        let entries = self.brands_dir.as_deref().map(brands::list).unwrap_or_else(|| {
            brands::builtin_names().iter().filter_map(|n| brands::builtin(n)).map(|b| brands::Entry {
                name: b.name.clone(), description: b.description.clone(), title: b.title.clone(), motto: b.motto.clone(), error: None,
            }).collect()
        });
        self.modal = Some(crate::tui::modals::Modal::BrandPicker(Box::new(
            crate::tui::modals::BrandPickerModal::new(entries, &self.brand_name, self.brand.clone()),
        )));
    }

    /// Show the highlighted picker entry without selecting it.
    pub fn preview_brand_picker(&mut self) {
        let Some(crate::tui::modals::Modal::BrandPicker(p)) = &self.modal else { return };
        let restore = p.restore.clone();
        let highlighted = p.highlighted().map(str::to_string);
        let brand = match (highlighted, self.brands_dir.clone()) {
            (Some(name), Some(dir)) => brands::resolve(&dir, &name).unwrap_or(restore),
            _ => restore,
        };
        brands::install(&brand);
        self.brand = brand;
    }

    /// The picker closed.
    pub fn brand_picked(&mut self, p: crate::tui::modals::BrandPickerModal, r: crate::tui::modals::ModalResult) {
        let restore = p.restore.clone();
        match (r, p.chosen()) {
            (crate::tui::modals::ModalResult::ClosedSubmitted, crate::tui::modals::BrandChoice::Brand(name)) => {
                self.use_brand(&name, true);
            }
            (crate::tui::modals::ModalResult::ClosedSubmitted, crate::tui::modals::BrandChoice::Edit) => {
                self.brand = restore.clone();
                brands::install(&self.brand);
                self.open_brand_editor();
            }
            _ => {
                self.brand = restore;
                brands::install(&self.brand);
            }
        }
    }

    /// Open the brand editor on the brand in use.
    pub fn open_brand_editor(&mut self) {
        if self.brands_dir.is_none() {
            self.error("Brand editor", "no brands folder (~/.acp/brands)");
            return;
        }
        self.theme_editor = None;
        self.brand_editor = Some(Box::new(BrandEditor::open(self)));
    }

    /// Read the selected brand again.
    pub fn reload_brand(&mut self) {
        let n = self.brand_name.clone();
        if self.use_brand(&n, false) { self.push_status(format!("brand {n} reloaded")); }
    }

    /// Show every brand in a message.
    pub fn list_brands_modal(&mut self) {
        let dir = self.brands_dir.clone().unwrap_or_default();
        let mut lines = vec![format!("Brands (built-in, and files in {}):", dir.display()), String::new()];
        let entries = brands::list(&dir);
        for e in entries {
            let mark = if e.name == self.brand_name { "●" } else { " " };
            lines.push(match e.error {
                Some(err) => format!("{mark} {}  ⚠ can't be used: {err}", e.name),
                None => {
                    let motto = if e.motto.is_empty() { "(no motto)".to_string() } else { e.motto };
                    format!("{mark} {}  {}  — {motto}", e.name, e.title)
                }
            });
        }
        lines.push(String::new());
        lines.push("The header's «name» ▾ switch opens the picker. The theme is separate.".into());
        self.modal = Some(crate::tui::modals::Modal::Message(crate::tui::modals::MessageModal::info("Brands", &lines.join("\n"))));
    }
}

fn one_line_preview(s: &str) -> String {
    let first = s.lines().next().unwrap_or("");
    if s.lines().nth(1).is_some() { format!("{first} …") } else if first.is_empty() { "(none)".into() } else { first.into() }
}
