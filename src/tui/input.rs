//! Keyboard and mouse → state transitions (every action has both).

use crate::tui::commands::{Action, Widget};
use crate::tui::layout::contains;
use crate::tui::modals::*;
use crate::tui::state::{App, Effect, Focus, Hit, ViewMode};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

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
            app.effects.push(match req { LoginRequest::SendEmail(e) => Effect::LoginEmail(e), LoginRequest::SubmitCode(c) => Effect::LoginCode(c) });
        }
    if let Some(r) = r { app.close_modal(r); }
    else if matches!(app.modal, Some(Modal::ThemePicker(_))) { app.preview_picker(); }
}

fn editor_key(app: &mut App, k: KeyEvent) {
    let Some(mut ed) = app.theme_editor.take() else { return };
    if ed.on_key(app, k) == crate::tui::editor::Outcome::Stay { app.theme_editor = Some(ed); }
}

/// Handle a key.
pub fn on_key(app: &mut App, k: KeyEvent) {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && k.code == KeyCode::Char('c') && app.modal.is_none() && app.theme_editor.is_none() { app.act(Action::Quit); return; }
    if let Some(m) = &mut app.modal {
        let r = m.state().on_key(k);
        after_modal(app, r);
        return;
    }
    if app.theme_editor.is_some() { editor_key(app, k); return; }
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
            if let Some(i) = app.menu.menu.items.iter().position(|it| it.hint == c.to_ascii_lowercase()) { app.open_menu(i); }
            return;
        }
        KeyCode::Char('/') if app.focus != Focus::Menu => { app.focus_nick_filter(); return; }
        KeyCode::Char('S') => { app.act(Action::OpenSearchModal); return; }
        KeyCode::Char('B') => { app.act(Action::ShowBookmarks); return; }
        KeyCode::Char('?') | KeyCode::F(1) => { app.act(Action::Help); return; }
        KeyCode::F(5) if k.modifiers.contains(KeyModifiers::SHIFT) => { app.act(Action::ThemeEditor); return; }
        KeyCode::F(17) => { app.act(Action::ThemeEditor); return; }
        KeyCode::F(5) => { app.act(Action::ThemePicker); return; }
        KeyCode::F(6) => { app.act(Action::StartScan); return; }
        KeyCode::F(10) => { app.focus = Focus::Menu; return; }
        KeyCode::Tab => { app.close_menu_dropdown(); app.focus = next_focus(app.focus, 1); return; }
        KeyCode::BackTab => { app.close_menu_dropdown(); app.focus = next_focus(app.focus, -1); return; }
        _ => {}
    }
    match app.focus {
        Focus::Menu => menu_key(app, k),
        Focus::Nicks => nicks_key(app, k),
        Focus::Body => body_key(app, k),
    }
}

fn next_focus(f: Focus, d: i32) -> Focus {
    let all = [Focus::Nicks, Focus::Body, Focus::Menu];
    let i = all.iter().position(|x| *x == f).unwrap_or(0);
    all[cycle(i, 3, d)]
}

fn common_key(app: &mut App, k: KeyEvent) -> bool {
    match k.code {
        KeyCode::Char('q') => app.act(Action::Quit),
        KeyCode::Char('l') => app.act(Action::ToggleLog),
        KeyCode::Char('m') => app.focus = Focus::Menu,
        KeyCode::Char('<') => { app.search_back(); }
        KeyCode::Char('b') => app.act(Action::ToggleBookmark),
        KeyCode::Char('p') => app.open_post_menu(),
        KeyCode::Char('x') => app.act(Action::ToggleBasket),
        KeyCode::Char('r') => app.act(Action::AddToResearch),
        KeyCode::Char('v') => app.act(Action::CycleView),
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
    pub fn close_menu_dropdown(&mut self) { self.menu.open = None; self.menu.editing = false; self.needs_redraw = true; }
}

fn menu_key(app: &mut App, k: KeyEvent) {
    let n = app.menu.menu.items.len();
    match app.menu.open {
        None => match k.code {
            KeyCode::Left => app.menu.item = cycle(app.menu.item, n, -1),
            KeyCode::Right => app.menu.item = cycle(app.menu.item, n, 1),
            KeyCode::Enter | KeyCode::Down => { let i = app.menu.item; app.open_menu(i); }
            KeyCode::Esc => app.close_menu(),
            KeyCode::Char(c) => {
                if let Some(i) = app.menu.menu.items.iter().position(|it| it.hint == c.to_ascii_lowercase()) { app.open_menu(i); }
            }
            _ => {}
        },
        Some(item) => {
            let no = app.menu.menu.items[item].opts.len();
            match k.code {
                KeyCode::Esc => app.close_menu_dropdown(),
                KeyCode::Up => app.menu.opt = cycle(app.menu.opt, no, -1),
                KeyCode::Down => app.menu.opt = cycle(app.menu.opt, no, 1),
                KeyCode::Left | KeyCode::Right => {
                    let d = if k.code == KeyCode::Left { -1 } else { 1 };
                    let o = &mut app.menu.menu.items[item].opts[app.menu.opt];
                    if let Widget::Choice { choices, idx } = &mut o.widget { *idx = cycle(*idx, choices.len(), d); }
                    else { app.menu.item = cycle(item, n, d); let i = app.menu.item; app.menu.open = None; app.open_menu(i); }
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

fn menu_edit_key(app: &mut App, k: KeyEvent) {
    let Some(item) = app.menu.open else { app.menu.editing = false; return };
    match k.code {
        KeyCode::Enter | KeyCode::Esc | KeyCode::Tab => app.menu.editing = false,
        _ => {
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
    let hit = hit_at(app, m.column, m.row);
    let wheel = match m.kind { MouseEventKind::ScrollDown => 1, MouseEventKind::ScrollUp => -1, _ => 0 };
    if app.modal.is_some() {
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
        _ => {}
    }
}
