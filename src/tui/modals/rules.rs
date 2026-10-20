//! Sync › Rules. A list, then one if-this-then-that form.
//!
//! The form edits one condition and one action. A system rule's body is read-only;
//! Save on that row changes priority only. Clone makes a user rule.

use super::*;
use crate::prelude::*;
use crate::rules::{Condition, NewRule, Origin, Rule, RuleAction, RuleEdit, Trigger};
use crate::tui::widgets::text::TextField;
use crossterm::event::{KeyCode, KeyEvent};

const COND: &[&str] = &["none", "author", "keyword", "regex", "source", "tag", "saved_search", "replies_to"];
const ACT: &[&str] = &["notify", "bookmark", "tag", "research", "export"];

/// Which control is focused in the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleFocus {
    /// The rule name. `?` opens the trigger topic.
    Name,
    /// Condition kind.
    Cond,
    /// Condition value.
    CondValue,
    /// Action kind.
    Act,
    /// Action value.
    ActValue,
    /// Priority. Lower runs first.
    Priority,
    /// The n-th button.
    Button(usize),
}

/// The rules window.
#[derive(Clone, Debug, PartialEq)]
pub struct RulesModal {
    /// Database the rows are stored in.
    pub db: PathBuf,
    /// Every rule, lower priority first.
    pub rules: Vec<Rule>,
    /// Highlighted list row.
    pub cursor: usize,
    /// The form is open.
    pub editing: bool,
    /// Row being edited. None is a new user rule.
    pub id: Option<i64>,
    /// System or user.
    pub origin: Origin,
    /// Shipped key, when this is a system row.
    pub system_key: Option<String>,
    /// Kept across a save. The form does not edit it.
    pub enabled: bool,
    /// Name.
    pub name: TextField,
    /// Index into the condition kinds.
    pub cond: usize,
    /// Condition value.
    pub cond_value: TextField,
    /// Index into the action kinds.
    pub act: usize,
    /// Action value.
    pub act_value: TextField,
    /// Priority text.
    pub priority: TextField,
    /// Focused control.
    pub focus: RuleFocus,
    /// Validation error.
    pub error: Option<String>,
    /// Status after a save, shown on the list.
    pub notice: Option<String>,
}

impl RulesModal {
    /// Open the list. Missing system rules are inserted.
    pub fn open(db: PathBuf) -> Result<Self> {
        let rules = crate::rules::list_rules(&crate::rules::open_rules(&db)?)?;
        Ok(Self {
            db, rules, cursor: 0, editing: false, id: None, origin: Origin::User, system_key: None, enabled: true,
            name: TextField::new(""), cond: 0, cond_value: TextField::new(""), act: 0, act_value: TextField::new(""),
            priority: TextField::new("100"), focus: RuleFocus::Name, error: None, notice: None,
        })
    }

    /// Help topic for the focused control.
    pub fn help_topic(&self) -> &'static str {
        if !self.editing { return "rules"; }
        match self.focus {
            RuleFocus::Name => "rules-trigger",
            RuleFocus::Cond | RuleFocus::CondValue => "rules-condition",
            RuleFocus::Act | RuleFocus::ActValue => "rules-action",
            RuleFocus::Priority => "rules-priority",
            RuleFocus::Button(i) if self.buttons().get(i) == Some(&"Clone") => "rules-clone",
            RuleFocus::Button(_) => "rules",
        }
    }

    /// A text control has the cursor.
    pub fn text_focused(&self) -> bool {
        self.editing && matches!(self.focus, RuleFocus::Name | RuleFocus::CondValue | RuleFocus::ActValue | RuleFocus::Priority)
    }

    /// Button labels for the form.
    pub fn buttons(&self) -> Vec<&'static str> {
        if self.origin == Origin::System && self.id.is_some() { vec!["Save", "Clone", "Cancel"] }
        else if self.id.is_some() { vec!["Save", "Delete", "Cancel"] }
        else { vec!["Save", "Cancel"] }
    }

    fn order(&self) -> Vec<RuleFocus> {
        let mut v = vec![RuleFocus::Name, RuleFocus::Cond, RuleFocus::CondValue, RuleFocus::Act, RuleFocus::ActValue, RuleFocus::Priority];
        for i in 0..self.buttons().len() { v.push(RuleFocus::Button(i)); }
        v
    }

    fn body_locked(&self) -> bool { self.origin == Origin::System && self.id.is_some() }

    fn reload(&mut self) -> Result<()> {
        self.rules = crate::rules::list_rules(&crate::rules::open_rules(&self.db)?)?;
        if self.cursor >= self.rules.len() { self.cursor = self.rules.len().saturating_sub(1); }
        Ok(())
    }

    fn edit_rule(&mut self, rule: &Rule) {
        self.editing = true;
        self.id = Some(rule.id);
        self.origin = rule.origin;
        self.system_key = rule.system_key.clone();
        self.enabled = rule.enabled;
        self.name.set(&rule.name);
        self.priority.set(&rule.priority.to_string());
        match rule.conditions.first() {
            None => { self.cond = 0; self.cond_value.set(""); }
            Some(c) => {
                let (kind, value) = cond_parts(c);
                self.cond = COND.iter().position(|k| *k == kind).unwrap_or(0);
                self.cond_value.set(&value);
            }
        }
        match rule.actions.first() {
            None => { self.act = 0; self.act_value.set(""); }
            Some(a) => {
                let (kind, value) = act_parts(a);
                self.act = ACT.iter().position(|k| *k == kind).unwrap_or(0);
                self.act_value.set(&value);
            }
        }
        self.focus = RuleFocus::Name;
        self.error = None;
    }

    fn edit_new(&mut self) {
        self.editing = true;
        self.id = None;
        self.origin = Origin::User;
        self.system_key = None;
        self.enabled = true;
        self.name.set("");
        self.cond = 0;
        self.cond_value.set("");
        self.act = 0;
        self.act_value.set("");
        self.priority.set("100");
        self.focus = RuleFocus::Name;
        self.error = None;
    }

    fn move_focus(&mut self, d: i32) {
        let order = self.order();
        let i = order.iter().position(|f| *f == self.focus).unwrap_or(0);
        self.focus = order[cycle(i, order.len(), d)];
    }

    fn press(&mut self, i: usize) -> Option<ModalResult> {
        match self.buttons().get(i).copied() {
            Some("Save") => self.save_and_list(),
            Some("Delete") => self.delete_and_list(),
            Some("Clone") => self.clone_and_list(),
            Some("Cancel") => { self.editing = false; self.error = None; None }
            _ => None,
        }
    }

    fn save_and_list(&mut self) -> Option<ModalResult> {
        match self.save() {
            Ok(()) => { self.editing = false; self.notice = Some("saved".into()); None }
            Err(e) => { self.error = Some(e.to_string()); None }
        }
    }

    fn save(&mut self) -> Result<()> {
        let c = crate::rules::open_rules(&self.db)?;
        let priority = self.priority_value()?;
        if self.body_locked() {
            crate::rules::set_priority(&c, &self.name.value, priority)?;
            self.reload()?;
            return Ok(());
        }
        let conditions = self.conditions()?;
        let actions = self.actions()?;
        if let Some(id) = self.id {
            crate::rules::update_user(&c, id, &RuleEdit {
                name: self.name.value.clone(), enabled: self.enabled, priority, trigger: Trigger::PostSynced, conditions, actions,
            })?;
        } else {
            crate::rules::insert_user(&c, &NewRule {
                name: self.name.value.clone(), enabled: true, priority, cloned_from: None, template_key: None,
                trigger: Trigger::PostSynced, conditions, actions,
            })?;
        }
        self.reload()?;
        Ok(())
    }

    fn delete_and_list(&mut self) -> Option<ModalResult> {
        let c = match crate::rules::open_rules(&self.db) { Ok(c) => c, Err(e) => { self.error = Some(e.to_string()); return None } };
        match crate::rules::delete_user(&c, &self.name.value) {
            Ok(()) => { let _ = self.reload(); self.editing = false; self.notice = Some("deleted".into()); }
            Err(e) => self.error = Some(e.to_string()),
        }
        None
    }

    fn clone_and_list(&mut self) -> Option<ModalResult> {
        let Some(key) = self.system_key.clone() else { self.error = Some("this row has no system key".into()); return None };
        let c = match crate::rules::open_rules(&self.db) { Ok(c) => c, Err(e) => { self.error = Some(e.to_string()); return None } };
        match crate::rules::clone_system(&c, &key) {
            Ok(rule) => { let _ = self.reload(); self.editing = false; self.notice = Some(format!("cloned as {}", rule.name)); }
            Err(e) => self.error = Some(e.to_string()),
        }
        None
    }

    fn priority_value(&self) -> Result<i64> {
        let t = self.priority.value.trim();
        if t.is_empty() { return Ok(100); }
        t.parse::<i64>().map_err(|_| anyhow!("priority needs a whole number"))
    }

    fn conditions(&self) -> Result<Vec<Condition>> {
        let v = self.cond_value.value.trim();
        let need = |label: &str| if v.is_empty() { bail!("{label} needs a value") } else { Ok(v.to_string()) };
        Ok(match COND[self.cond] {
            "none" => Vec::new(),
            "author" => vec![Condition::Author { nick: strip_at(&need("author")?) }],
            "keyword" => vec![Condition::Keyword { text: need("keyword")? }],
            "regex" => vec![Condition::Regex { pattern: need("regex")? }],
            "source" => vec![Condition::Source { source: need("source")? }],
            "tag" => vec![Condition::Tag { tag: need("tag")?.trim_start_matches('#').to_string() }],
            "saved_search" => vec![Condition::SavedSearch { keyword: need("saved search")?, nicks: Vec::new() }],
            "replies_to" => vec![Condition::RepliesTo { nick: strip_at(&need("replies to")?) }],
            _ => Vec::new(),
        })
    }

    fn actions(&self) -> Result<Vec<RuleAction>> {
        let v = self.act_value.value.trim();
        Ok(vec![match ACT[self.act] {
            "bookmark" => RuleAction::Bookmark,
            "tag" => { if v.is_empty() { bail!("tag needs a value") } RuleAction::Tag { tag: v.trim_start_matches('#').to_string() } }
            "research" => { if v.is_empty() { bail!("research needs a project name") } RuleAction::Research { name: v.to_string() } }
            "export" => RuleAction::Export,
            _ => RuleAction::Notify,
        }])
    }

    fn list_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc => Some(ModalResult::ClosedCancelled),
            KeyCode::Char('?') | KeyCode::F(1) => Some(ModalResult::Help(self.help_topic())),
            KeyCode::Char('j') | KeyCode::Down => { if !self.rules.is_empty() { self.cursor = (self.cursor + 1) % self.rules.len(); } None }
            KeyCode::Char('k') | KeyCode::Up => { if !self.rules.is_empty() { self.cursor = (self.cursor + self.rules.len() - 1) % self.rules.len(); } None }
            KeyCode::Enter => {
                if let Some(rule) = self.rules.get(self.cursor).cloned() { self.edit_rule(&rule); }
                None
            }
            KeyCode::Char('n') => { self.edit_new(); None }
            KeyCode::Char(' ') => { self.toggle(); None }
            _ => None,
        }
    }

    fn toggle(&mut self) {
        let Some(rule) = self.rules.get(self.cursor).cloned() else { return };
        if rule.origin == Origin::System {
            self.notice = Some("a system rule is read-only; clone it".into());
            return;
        }
        match crate::rules::open_rules(&self.db).and_then(|c| crate::rules::set_enabled(&c, &rule.name, !rule.enabled)) {
            Ok(()) => { let _ = self.reload(); }
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    fn edit_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Char('?') | KeyCode::F(1) => return Some(ModalResult::Help(self.help_topic())),
            KeyCode::Esc => { self.editing = false; self.error = None; return None; }
            KeyCode::Tab => { self.move_focus(1); return None; }
            KeyCode::BackTab => { self.move_focus(-1); return None; }
            KeyCode::Enter => {
                if let RuleFocus::Button(i) = self.focus { return self.press(i); }
                self.move_focus(1);
                return None;
            }
            _ => {}
        }
        let locked = self.body_locked();
        match self.focus {
            RuleFocus::Cond if !locked => if matches!(k.code, KeyCode::Left | KeyCode::Char('h')) {
                self.cond = cycle(self.cond, COND.len(), -1);
            } else if matches!(k.code, KeyCode::Right | KeyCode::Char('l')) {
                self.cond = cycle(self.cond, COND.len(), 1);
            },
            RuleFocus::Act if !locked => if matches!(k.code, KeyCode::Left | KeyCode::Char('h')) {
                self.act = cycle(self.act, ACT.len(), -1);
            } else if matches!(k.code, KeyCode::Right | KeyCode::Char('l')) {
                self.act = cycle(self.act, ACT.len(), 1);
            },
            RuleFocus::Name | RuleFocus::CondValue | RuleFocus::ActValue if !locked => { let _ = self.field_mut().on_key(k); }
            RuleFocus::Priority => { let _ = self.priority.on_key(k); }
            RuleFocus::Button(i) => if matches!(k.code, KeyCode::Left | KeyCode::Char('h')) {
                self.focus = RuleFocus::Button(i.saturating_sub(1));
            } else if matches!(k.code, KeyCode::Right | KeyCode::Char('l')) {
                self.focus = RuleFocus::Button((i + 1).min(self.buttons().len().saturating_sub(1)));
            },
            _ => {}
        }
        None
    }

    fn field_mut(&mut self) -> &mut TextField {
        match self.focus {
            RuleFocus::CondValue => &mut self.cond_value,
            RuleFocus::ActValue => &mut self.act_value,
            _ => &mut self.name,
        }
    }

    /// Condition kind label.
    pub fn cond_label(&self) -> &'static str { COND[self.cond] }
    /// Action kind label.
    pub fn act_label(&self) -> &'static str { ACT[self.act] }
}

fn strip_at(s: &str) -> String { s.trim().trim_start_matches('@').to_string() }

fn cond_parts(c: &Condition) -> (&'static str, String) {
    match c {
        Condition::Author { nick } => ("author", nick.clone()),
        Condition::Keyword { text } => ("keyword", text.clone()),
        Condition::Regex { pattern } => ("regex", pattern.clone()),
        Condition::Source { source } => ("source", source.clone()),
        Condition::Tag { tag } => ("tag", tag.clone()),
        Condition::SavedSearch { keyword, .. } => ("saved_search", keyword.clone()),
        Condition::RepliesTo { nick } => ("replies_to", nick.clone()),
    }
}

fn act_parts(a: &RuleAction) -> (&'static str, String) {
    match a {
        RuleAction::Notify => ("notify", String::new()),
        RuleAction::Bookmark => ("bookmark", String::new()),
        RuleAction::Tag { tag } => ("tag", tag.clone()),
        RuleAction::Research { name } => ("research", name.clone()),
        RuleAction::Export => ("export", String::new()),
    }
}

impl ModalState for RulesModal {
    fn title(&self) -> String { if self.editing { "Rule".into() } else { "Rules".into() } }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        if self.editing { self.edit_key(k) } else { self.list_key(k) }
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Item(i) if !self.editing && i < self.rules.len() => {
                self.cursor = i;
                if let Some(rule) = self.rules.get(i).cloned() { self.edit_rule(&rule); }
                None
            }
            ModalHit::Button(i) if self.editing => self.press(i),
            ModalHit::Field(i) if self.editing => {
                self.focus = self.order().get(i).copied().unwrap_or(RuleFocus::Name);
                None
            }
            _ => None,
        }
    }
}
