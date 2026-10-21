//! Keyboard and mouse → state transitions (every action has both).

use crate::tui::commands::{is_path_field, Action, Widget};
use crate::tui::layout::contains;
use crate::tui::modals::*;
use crate::tui::state::{App, Effect, Focus, Hit, ViewMode};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::Instant;

/// Handle one terminal event.
pub fn on_event(app: &mut App, ev: Event) {
    match ev {
        Event::Key(k) if k.kind != KeyEventKind::Release => on_key(app, k),
        Event::Mouse(m) => on_mouse(app, m),
        _ => {}
    }
}

fn after_modal(app: &mut App, r: Option<ModalResult>) {
    if let Some(Modal::Login(m)) = &mut app.modal
        && let Some(req) = m.request.take() {
            app.effects.push(match req {
                LoginRequest::SendEmail(e) => Effect::LoginEmail(e),
                LoginRequest::SubmitCode(c) => Effect::LoginCode(c),
                LoginRequest::Resend => Effect::LoginResend,
            });
        }
    if let Some(pid) = app.take_job_cancel() { app.cancel_tracked(pid); }
    if let Some(r) = r { app.close_modal(r); }
    else if matches!(app.modal, Some(Modal::ThemePicker(_))) { app.preview_picker(); }
}

fn editor_key(app: &mut App, k: KeyEvent) {
    let Some(mut ed) = app.theme_editor.take() else { return };
    if ed.on_key(app, k) == crate::tui::editor::Outcome::Stay { app.theme_editor = Some(ed); }
}

/// Handle a key.
pub fn on_key(app: &mut App, k: KeyEvent) {
    if app.egg.is_some() { app.dismiss_egg(); return; }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if k.code == KeyCode::Char('~') && !ctrl && !app.text_input_focused() { app.open_credits(); return; }
    if ctrl && k.code == KeyCode::Char('c') && app.modal.is_none() && app.theme_editor.is_none() { app.act(Action::Quit); return; }
    if !app.text_input_focused() {
        if app.konami.feed(k.code) { app.show_egg(crate::eggs::EggKind::Burst); return; }
    } else {
        app.konami.reset();
    }
    if matches!(app.modal, Some(Modal::Job(_))) { app.refresh_job_modal(); }
    if let Some(m) = &mut app.modal {
        let r = m.state().on_key(k);
        after_modal(app, r);
        return;
    }
    if app.theme_editor.is_some() { editor_key(app, k); return; }
    if app.notice_page.is_some() && notice_key(app, k) { return; }
    if app.filter_editing { filter_key(app, k); return; }
    if app.focus == Focus::Menu && app.menu.editing { menu_edit_key(app, k); return; }
    // Global keys.
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    match k.code {
        KeyCode::Char(' ') | KeyCode::Null if ctrl => { app.modal = Some(Modal::Palette(PaletteModal::new(app.palette_entries()))); return; }
        KeyCode::Char('p') if ctrl => { app.modal = Some(Modal::Palette(PaletteModal::new(app.palette_entries()))); return; }
        KeyCode::Char('s') | KeyCode::Char('S') if ctrl => { app.act(Action::ClearSearch); return; }
        KeyCode::Char('r') if ctrl => { app.act(Action::RefreshNicks); return; }
        KeyCode::Char('a') if ctrl && app.focus == Focus::Nicks => { app.select_all(); return; }
        KeyCode::Left if alt => { app.act(Action::StackPrev); return; }
        KeyCode::Right if alt => { app.act(Action::StackNext); return; }
        KeyCode::Char(c) if alt => {
            if let Some(i) = app.menu.menu.bar_by_hint(c) { app.open_menu(i); }
            return;
        }
        KeyCode::Char('/') if app.focus != Focus::Menu => { app.focus_nick_filter(); return; }
        KeyCode::Char('S') => { app.act(Action::OpenSearchModal); return; }
        KeyCode::Char('B') => { app.act(Action::ShowBookmarks); return; }
        KeyCode::Char('?') | KeyCode::F(1) => { let topic = app.menu_help_topic(); app.open_help(topic); return; }
        KeyCode::F(5) if k.modifiers.contains(KeyModifiers::SHIFT) => { app.act(Action::ThemeEditor); return; }
        KeyCode::F(17) => { app.act(Action::ThemeEditor); return; }
        KeyCode::F(5) => { app.act(Action::ThemePicker); return; }
        KeyCode::F(6) => { app.act(Action::StartScan); return; }
        KeyCode::F(10) => { app.focus = Focus::Menu; return; }
        KeyCode::Tab => { app.close_menu_dropdown(); app.focus = next_focus(app, 1); return; }
        KeyCode::BackTab => { app.close_menu_dropdown(); app.focus = next_focus(app, -1); return; }
        _ => {}
    }
    match app.focus {
        Focus::Menu => menu_key(app, k),
        Focus::Nicks => nicks_key(app, k),
        Focus::Body => body_key(app, k),
        Focus::Status => status_key(app, k),
        Focus::Actions => actions_key(app, k),
    }
}

/// Keys for the notifications page. `false` lets the key through. Nothing here records an action step.
fn notice_key(app: &mut App, k: KeyEvent) -> bool {
    let editing = app.notice_page.as_ref().is_some_and(|p| p.editing);
    if editing {
        match k.code {
            KeyCode::Esc | KeyCode::Enter => { if let Some(p) = &mut app.notice_page { p.editing = false; } }
            KeyCode::Backspace => {
                if let Some(p) = &mut app.notice_page { p.query.pop(); p.cursor = 0; p.scroll = 0; }
            }
            KeyCode::Char(c) => {
                if let Some(p) = &mut app.notice_page { p.query.push(c); p.cursor = 0; p.scroll = 0; }
            }
            _ => return false,
        }
        return true;
    }
    match k.code {
        KeyCode::Esc => { app.notice_page = None; }
        KeyCode::Char('?') | KeyCode::F(1) => app.open_help(Some("notifications")),
        KeyCode::Char('/') => { if let Some(p) = &mut app.notice_page { p.editing = true; } }
        KeyCode::Char('o') => { if let Some(p) = &mut app.notice_page { p.toggle_order(); } }
        KeyCode::Char('f') => { if let Some(p) = &mut app.notice_page { p.cycle_kind(); } }
        KeyCode::Char(' ') => app.toggle_notice_row(0),
        KeyCode::Char('x') => {
            let i = app.notice_page.as_ref().map(|p| p.cursor).unwrap_or(0);
            app.delete_visible_notice(i);
        }
        KeyCode::Char('X') => app.clear_all_notices(),
        KeyCode::Up | KeyCode::Char('k') => app.move_notice_cursor(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_notice_cursor(1),
        KeyCode::PageUp => app.move_notice_cursor(-10),
        KeyCode::PageDown => app.move_notice_cursor(10),
        KeyCode::Home => app.move_notice_cursor(-1_000_000),
        KeyCode::End => app.move_notice_cursor(1_000_000),
        KeyCode::Enter => {}
        _ => return false,
    }
    true
}

fn next_focus(app: &App, d: i32) -> Focus {
    let mut all = vec![Focus::Nicks, Focus::Body, Focus::Menu, Focus::Status];
    if app.actions_open { all.insert(2, Focus::Actions); }
    let i = all.iter().position(|x| *x == app.focus).unwrap_or(0);
    all[cycle(i, all.len(), d)]
}

fn actions_key(app: &mut App, k: KeyEvent) {
    let page = app.action_page.max(1) as i32;
    match k.code {
        KeyCode::Up | KeyCode::Char('k') => app.move_action_cursor(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_action_cursor(1),
        KeyCode::PageUp => app.scroll_actions(-page),
        KeyCode::PageDown => app.scroll_actions(page),
        KeyCode::Home => app.move_action_cursor(-1_000_000),
        KeyCode::End => app.move_action_cursor(1_000_000),
        KeyCode::Enter => app.jump_action(app.action_cursor),
        KeyCode::Char('b') => app.open_branches(),
        KeyCode::Left | KeyCode::Char('h') => app.switch_branch(app.branch_at.saturating_sub(1)),
        KeyCode::Right | KeyCode::Char('l') => app.switch_branch(app.branch_at.saturating_add(1)),
        KeyCode::Esc => app.focus = Focus::Body,
        _ => { common_key(app, k); }
    }
}

fn status_key(app: &mut App, k: KeyEvent) {
    match k.code {
        KeyCode::Enter => app.open_jobs(),
        _ => { common_key(app, k); }
    }
}

fn common_key(app: &mut App, k: KeyEvent) -> bool {
    match k.code {
        KeyCode::Char('q') => app.act(Action::Quit),
        KeyCode::Char('l') => app.act(Action::ToggleLog),
        KeyCode::Char('m') => app.focus = Focus::Menu,
        KeyCode::Char('<') => { app.search_back(); }
        KeyCode::Char('b') => app.act(Action::ToggleBookmark),
        KeyCode::Char('+') => app.vote_current("1"),
        KeyCode::Char('-') => app.vote_current("-1"),
        KeyCode::Char('p') => app.open_post_menu(),
        KeyCode::Char('x') => app.act(Action::ToggleBasket),
        KeyCode::Char('n') => app.open_new_post(),
        KeyCode::Char('r') => app.act(Action::AddToResearch),
        KeyCode::Char('v') => app.act(Action::CycleView),
        KeyCode::Char('o') => { if app.focus == Focus::Nicks { app.toggle_nick_order(); } else { app.toggle_list_order(); } }
        KeyCode::Char(',') => app.act(Action::StackPrev),
        KeyCode::Char('.') => app.act(Action::StackNext),
        KeyCode::Char('V') => { let m = app.view_mode.cycle(-1); app.set_view_mode(m); }
        _ => return false,
    }
    true
}

fn nicks_key(app: &mut App, k: KeyEvent) {
    let shift = k.modifiers.contains(KeyModifiers::SHIFT);
    match k.code {
        KeyCode::Up | KeyCode::Char('k') if shift => app.extend_selection(-1),
        KeyCode::Down | KeyCode::Char('j') if shift => app.extend_selection(1),
        KeyCode::Up | KeyCode::Char('k') => app.move_nick_cursor(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_nick_cursor(1),
        KeyCode::PageUp => app.move_nick_cursor(-10),
        KeyCode::PageDown => app.move_nick_cursor(10),
        KeyCode::Home => app.nick_cursor = 0,
        KeyCode::End => app.nick_cursor = app.nicks.len().saturating_sub(1),
        KeyCode::Enter | KeyCode::Right => { let i = app.nick_cursor; app.select_only(i); if !app.view().posts.is_empty() { app.focus = Focus::Body; } }
        KeyCode::Char(' ') => { let i = app.nick_cursor; app.toggle_nick(i); }
        KeyCode::Esc if app.nick_filter_active() => app.clear_nick_filter(),
        KeyCode::Esc => { let i = app.nick_cursor; if app.selected.len() > 1 { app.select_only(i); } }
        _ => { common_key(app, k); }
    }
}

fn body_key(app: &mut App, k: KeyEvent) {
    let detail = !app.view().stack.is_empty();
    let mode = app.view_mode;
    if detail && mode == ViewMode::Pane {
        // The list stays live above the pane: ↑/↓ pick the post the pane shows.
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => { app.move_post_cursor(-1); app.preview_cursor(); return; }
            KeyCode::Down | KeyCode::Char('j') => { app.move_post_cursor(1); app.preview_cursor(); return; }
            _ => {}
        }
    }
    if detail {
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => app.scroll_detail(-1),
            KeyCode::Down | KeyCode::Char('j') => app.scroll_detail(1),
            KeyCode::PageUp => app.scroll_detail(-10),
            KeyCode::PageDown | KeyCode::Char(' ') => app.scroll_detail(10),
            KeyCode::Home => app.scroll_detail(-100_000),
            KeyCode::Left | KeyCode::Char('[') => app.move_link(-1),
            KeyCode::Right | KeyCode::Char(']') => app.move_link(1),
            KeyCode::Enter => { if let Some(i) = app.top().map(|f| f.link_cursor) { app.open_link(i); } }
            KeyCode::Char('s') => app.cycle_thread_sort(),
            KeyCode::Char('r') => { app.open_reply(); return; }
            KeyCode::Char('Q') => { app.open_quote(); return; }
            KeyCode::Esc | KeyCode::Backspace => { app.pop_detail(); }
            _ => { common_key(app, k); }
        }
        return;
    }
    if mode.horizontal() {
        let row = if mode == ViewMode::Grid { app.grid_cols.max(1) as i32 } else { 1 };
        match k.code {
            KeyCode::Left => { app.step_post(-1); return; }
            KeyCode::Right => { app.step_post(1); return; }
            KeyCode::Up | KeyCode::Char('k') => { app.step_post(-row); return; }
            KeyCode::Down | KeyCode::Char('j') => { app.step_post(row); return; }
            _ => {}
        }
    }
    match k.code {
        KeyCode::Up | KeyCode::Char('k') => app.move_post_cursor(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_post_cursor(1),
        KeyCode::PageUp => app.move_post_cursor(-(app.page_rows as i32)),
        KeyCode::PageDown => app.move_post_cursor(app.page_rows as i32),
        KeyCode::Home => app.move_post_cursor(i32::MIN / 2),
        KeyCode::End => app.move_post_cursor(i32::MAX / 2),
        KeyCode::Enter => { let i = app.view().cursor; app.open_post(i); }
        KeyCode::Esc | KeyCode::Backspace => { if !app.search_back() { app.focus = Focus::Nicks; } }
        KeyCode::Left => app.focus = Focus::Nicks,
        _ => { common_key(app, k); }
    }
}

impl App {
    /// Close the dropdown but keep focus where it is.
    pub fn close_menu_dropdown(&mut self) {
        self.menu.open = None;
        self.menu.anchor = None;
        self.menu.editing = false;
        self.needs_redraw = true;
    }
}

fn step_bar(app: &App, from: usize, delta: i32) -> usize {
    let bars = app.menu.menu.bar_indices();
    if bars.is_empty() { return from; }
    let pos = bars.iter().position(|&i| i == from).unwrap_or(0);
    bars[cycle(pos, bars.len(), delta)]
}

fn menu_key(app: &mut App, k: KeyEvent) {
    match app.menu.open {
        None => match k.code {
            KeyCode::Left => app.menu.item = step_bar(app, app.menu.item, -1),
            KeyCode::Right => app.menu.item = step_bar(app, app.menu.item, 1),
            KeyCode::Enter | KeyCode::Down => { let i = app.menu.item; app.open_menu(i); }
            KeyCode::Esc => app.close_menu(),
            KeyCode::Char(c) => {
                if let Some(i) = app.menu.menu.bar_by_hint(c) { app.open_menu(i); }
            }
            _ => {}
        },
        Some(item) => {
            let no = app.menu.menu.items[item].opts.len();
            match k.code {
                KeyCode::Esc => {
                    if let Some(parent) = app.menu.anchor.take() {
                        app.menu.open = Some(parent);
                        app.menu.item = parent;
                        app.menu.opt = 0;
                        app.menu.editing = false;
                        app.needs_redraw = true;
                    } else {
                        app.close_menu_dropdown();
                    }
                }
                KeyCode::Up => app.menu.opt = cycle(app.menu.opt, no, -1),
                KeyCode::Down => app.menu.opt = cycle(app.menu.opt, no, 1),
                KeyCode::Left | KeyCode::Right => {
                    let d = if k.code == KeyCode::Left { -1 } else { 1 };
                    let o = &mut app.menu.menu.items[item].opts[app.menu.opt];
                    if let Widget::Choice { choices, idx } = &mut o.widget {
                        *idx = cycle(*idx, choices.len(), d);
                        app.apply_open_choice();
                    } else {
                        let from = app.menu.anchor.unwrap_or(item);
                        let next = step_bar(app, from, d);
                        app.menu.anchor = None;
                        app.menu.open = None;
                        app.open_menu(next);
                    }
                }
                KeyCode::Char(' ') => {
                    if let Widget::Check(v) = &mut app.menu.menu.items[item].opts[app.menu.opt].widget { *v = !*v; }
                }
                KeyCode::Enter => { let i = app.menu.opt; app.activate_opt(i); }
                _ => {}
            }
        }
    }
}

/// Typing into the nick filter: it narrows the list on every key.
fn filter_key(app: &mut App, k: KeyEvent) {
    match k.code {
        KeyCode::Esc => app.clear_nick_filter(),
        KeyCode::Enter | KeyCode::Tab | KeyCode::Down => { app.filter_editing = false; app.focus = Focus::Nicks; }
        _ => {
            let before = app.nick_filter.value.clone();
            app.nick_filter.on_key(k);
            if app.nick_filter.value != before { app.apply_nick_filter(); }
        }
    }
}

fn menu_path_field(app: &App) -> bool {
    let Some(item) = app.menu.open else { return false };
    let id = app.menu.menu.items[item].id;
    let key = app.menu.menu.items[item].opts.get(app.menu.opt).map(|o| o.key).unwrap_or("");
    is_path_field(id, key)
}

fn open_text(app: &App) -> String {
    let Some(item) = app.menu.open else { return String::new() };
    match &app.menu.menu.items[item].opts.get(app.menu.opt).map(|o| &o.widget) {
        Some(Widget::Text(t)) => t.value.clone(),
        _ => String::new(),
    }
}

fn set_open_text(app: &mut App, text: &str) {
    let Some(item) = app.menu.open else { return };
    if let Some(Widget::Text(t)) = app.menu.menu.items[item].opts.get_mut(app.menu.opt).map(|o| &mut o.widget) {
        t.set(text);
    }
}

/// Tab on a path field completes one real segment, the same way Compose does.
/// An empty field, or a Tab that changes nothing, leaves the field.
fn complete_open_path(app: &mut App) {
    if !app.menu.path_matches.is_empty() {
        let n = app.menu.path_matches.len();
        app.menu.path_at = cycle(app.menu.path_at, n, 1);
        let text = app.menu.path_matches[app.menu.path_at].clone();
        set_open_text(app, &text);
        app.push_status(format!("{} of {n}: {text}", app.menu.path_at + 1));
        return;
    }
    let current = open_text(app);
    if current.trim().is_empty() {
        app.menu.editing = false;
        app.finish_menu_text();
        return;
    }
    let home = crate::config::home().ok();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let done = crate::site::complete_path(&current, home.as_deref(), &cwd);
    if done.matches.is_empty() && done.text == current {
        app.menu.editing = false;
        app.finish_menu_text();
        return;
    }
    set_open_text(app, &done.text);
    app.menu.path_matches = done.matches;
    app.menu.path_at = 0;
    if let Some(first) = app.menu.path_matches.first() {
        let n = app.menu.path_matches.len();
        app.push_status(format!("1 of {n}: {first}"));
    }
}

fn menu_edit_key(app: &mut App, k: KeyEvent) {
    let Some(item) = app.menu.open else { app.menu.editing = false; return };
    let path = menu_path_field(app);
    match k.code {
        KeyCode::Esc if path && !app.menu.path_matches.is_empty() => {
            app.menu.path_matches.clear();
            app.menu.path_at = 0;
        }
        KeyCode::Up | KeyCode::Down if path && !app.menu.path_matches.is_empty() => {
            let n = app.menu.path_matches.len();
            let d = if k.code == KeyCode::Up { -1 } else { 1 };
            app.menu.path_at = cycle(app.menu.path_at, n, d);
            let text = app.menu.path_matches[app.menu.path_at].clone();
            set_open_text(app, &text);
        }
        KeyCode::Tab | KeyCode::BackTab if path => complete_open_path(app),
        KeyCode::Enter | KeyCode::Esc | KeyCode::Tab => {
            app.menu.path_matches.clear();
            app.menu.editing = false;
            app.finish_menu_text();
        }
        _ => {
            app.menu.path_matches.clear();
            if let Widget::Text(t) = &mut app.menu.menu.items[item].opts[app.menu.opt].widget { t.on_key(k); }
        }
    }
}

/// The topmost recorded region under `(x, y)`.
pub fn hit_at(app: &App, x: u16, y: u16) -> Option<Hit> {
    app.hits.iter().rev().find(|(r, _)| contains(*r, x, y)).map(|(_, h)| *h)
}

/// Handle a mouse event.
pub fn on_mouse(app: &mut App, m: MouseEvent) {
    if app.egg.is_some() {
        if matches!(m.kind, MouseEventKind::Down(_) | MouseEventKind::ScrollDown | MouseEventKind::ScrollUp) {
            app.dismiss_egg();
        }
        return;
    }
    let hit = hit_at(app, m.column, m.row);
    if matches!(m.kind, MouseEventKind::Moved) {
        app.note_hover(hit);
        return;
    }
    if let MouseEventKind::Down(MouseButton::Left) = m.kind {
        match hit {
            Some(Hit::Tip) => {
                let topic = app.tip.as_ref().map(|t| t.topic);
                app.tip = None;
                app.open_help(topic);
                return;
            }
            Some(Hit::HelpMark) => {
                let topic = app.tip.as_ref().map(|t| t.topic);
                app.open_help(topic);
                return;
            }
            _ => {}
        }
    }
    let wheel = match m.kind { MouseEventKind::ScrollDown => 1, MouseEventKind::ScrollUp => -1, _ => 0 };
    if app.modal.is_some() {
        if matches!(app.modal, Some(Modal::Job(_))) { app.refresh_job_modal(); }
        if wheel != 0 { if let Some(md) = &mut app.modal { md.state().on_scroll(wheel); } return; }
        if let (MouseEventKind::Down(MouseButton::Left), Some(Hit::Modal(h))) = (m.kind, hit) {
            let r = app.modal.as_mut().and_then(|md| md.state().on_click(h));
            after_modal(app, r);
        }
        return;
    }
    if let Some(ed) = app.theme_editor.as_mut() {
        if wheel != 0 { ed.scroll = (ed.scroll as i64 + wheel as i64 * 3).max(0) as usize; return; }
        if let (MouseEventKind::Down(MouseButton::Left), Some(Hit::EditorRow(i))) = (m.kind, hit) { ed.click_row(i); }
        return;
    }
    if wheel != 0 {
        match hit {
            Some(Hit::Nick(_) | Hit::NickPane | Hit::Bookmarks | Hit::Eb | Hit::Explore | Hit::NickFilter | Hit::NickFilterClear) => { app.nick_offset = (app.nick_offset as i64 + wheel as i64 * 3).clamp(0, app.nicks.len().saturating_sub(1) as i64) as usize; app.move_nick_cursor(wheel * 3); }
            Some(Hit::Action(_) | Hit::ActionPane | Hit::ActionBranches) => app.scroll_actions(wheel * 3),
            Some(Hit::DetailPane | Hit::Link(_) | Hit::StarTop | Hit::BasketTop | Hit::ResearchTop) => app.scroll_detail(wheel * 3),
            Some(Hit::Post(_) | Hit::Star(_) | Hit::Basket(_) | Hit::Research(_) | Hit::ListPane) => { if app.view().stack.is_empty() { app.scroll_posts(wheel) } else { app.move_post_cursor(wheel) } }
            _ => {}
        }
        return;
    }
    let MouseEventKind::Down(MouseButton::Left) = m.kind else { return };
    let multi = m.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    if app.filter_editing && !matches!(hit, Some(Hit::NickFilter)) { app.filter_editing = false; }
    // A click outside the open dropdown closes it.
    if app.menu.open.is_some() && !matches!(hit, Some(Hit::MenuOpt(_) | Hit::MenuItem(_))) { app.close_menu(); }
    match hit {
        Some(Hit::Nick(i)) => { app.focus = Focus::Nicks; if multi { app.toggle_nick(i) } else { app.select_only(i) } }
        Some(Hit::Post(i)) => app.open_post(i),
        Some(Hit::Bookmarks) => app.open_bookmarks(),
        Some(Hit::Eb) => app.open_eb(),
        Some(Hit::Explore) => app.open_explore(),
        Some(Hit::NickFilter) => app.focus_nick_filter(),
        Some(Hit::NickFilterClear) => app.clear_nick_filter(),
        Some(Hit::ViewMode) => app.act(Action::CycleView),
        Some(Hit::ListOrder) => { app.focus = Focus::Body; app.toggle_list_order(); }
        Some(Hit::StackPrev) => { app.focus = Focus::Body; app.act(Action::StackPrev); }
        Some(Hit::StackNext) => { app.focus = Focus::Body; app.act(Action::StackNext); }
        Some(Hit::CloseDetail) => app.close_details(),
        Some(Hit::Star(i)) => { if let Some(id) = app.view().posts.get(i).map(|p| p.post_id.clone()) { app.focus = Focus::Body; app.toggle_mark(&id); } }
        Some(Hit::StarTop) => { if let Some(id) = app.top().map(|f| f.post.post_id.clone()) { app.toggle_mark(&id); } }
        Some(Hit::Basket(i)) => { if let Some(id) = app.view().posts.get(i).map(|p| p.post_id.clone()) { app.focus = Focus::Body; app.toggle_basket(&id); } }
        Some(Hit::Research(i)) => {
            if i < app.view().posts.len() { app.focus = Focus::Body; app.view_mut().cursor = i; app.act(Action::AddToResearch); }
        }
        Some(Hit::ResearchTop) => app.act(Action::AddToResearch),
        Some(Hit::BasketTop) => { if let Some(id) = app.top().map(|f| f.post.post_id.clone()) { app.toggle_basket(&id); } }
        Some(Hit::MenuItem(i)) => app.open_menu(i),
        Some(Hit::MenuOpt(i)) => { app.menu.editing = false; app.activate_opt(i); }
        Some(Hit::StartScan) => app.act(Action::StartScan),
        Some(Hit::ThemeSwitch) => app.act(Action::ThemePicker),
        Some(Hit::Back) => { app.search_back(); }
        Some(Hit::Link(i)) => { app.focus = Focus::Body; app.open_link(i); }
        Some(Hit::Frame(d)) => { app.focus = Focus::Body; app.pop_to(d); }
        Some(Hit::NickPane) => app.focus = Focus::Nicks,
        Some(Hit::ListPane | Hit::DetailPane) => app.focus = Focus::Body,
        Some(Hit::Status | Hit::Job) => { app.focus = Focus::Status; app.open_jobs(); }
        Some(Hit::Logo) => app.tap_logo(Instant::now()),
        Some(Hit::Toast(i)) => app.notices.dismiss(i),
        Some(Hit::ViewAllNotices) => app.open_notifications(),
        Some(Hit::NoticeSearch) => { if let Some(p) = &mut app.notice_page { p.editing = true; } }
        Some(Hit::NoticeOrder) => { if let Some(p) = &mut app.notice_page { p.toggle_order(); } }
        Some(Hit::NoticeKind) => { if let Some(p) = &mut app.notice_page { p.cycle_kind(); } }
        Some(Hit::NoticeSwitch(i)) => app.toggle_notice_row(i),
        Some(Hit::NoticeRow(i)) => { if let Some(p) = &mut app.notice_page { p.cursor = i; } }
        Some(Hit::NoticeClear(i)) => app.delete_visible_notice(i),
        Some(Hit::NoticeClearAll) => app.clear_all_notices(),
        Some(Hit::ToggleNicks) => app.nicks_open = !app.nicks_open,
        Some(Hit::ToggleActions) => app.toggle_actions(),
        Some(Hit::ActionBranches) => { app.focus = Focus::Actions; app.open_branches(); }
        Some(Hit::ActionPane) => app.focus = Focus::Actions,
        Some(Hit::Action(i)) => { app.focus = Focus::Actions; app.jump_action(i); },
        _ => {}
    }
}
