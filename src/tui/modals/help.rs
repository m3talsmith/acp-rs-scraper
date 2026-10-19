//! Help modal: the real `--help` text (generated from clap, so it cannot drift)
//! plus the TUI key bindings.

use super::*;
use crossterm::event::KeyCode;

/// Scrollable help text.
#[derive(Debug, Clone, PartialEq)]
pub struct HelpModal {
    /// Lines.
    pub lines: Vec<String>,
    /// First visible line.
    pub scroll: usize,
}

/// Help text: key bindings, the order and limit topics, then `acp-red-star --help`.
pub fn help_text() -> Vec<String> {
    use clap::CommandFactory;
    let guide = crate::crypto::recovery::GUIDE.trim();
    let mut v: Vec<String> = guide.lines().map(str::to_string).collect();
    v.extend(["".into(), "Terminal UI keys".into(), String::new()]);
    for (k, d) in crate::config::TUI_KEYBINDINGS { v.push(format!("  {k:<22} {d}")); }
    v.extend(["".into(), "Menu commands map to these flags; Start Scanning runs the same scrape the CLI does.".into()]);
    v.extend(help_topics());
    v.push(String::new());
    let help = crate::cli::Args::command().render_long_help().to_string();
    let help = help.replacen(guide, "", 1);
    v.extend(help.lines().map(str::to_string));
    v
}

/// One section per control. `[[topic]]` is the marker `?` scrolls to.
fn help_topics() -> Vec<String> {
    [
        "",
        "Topics. On a menu option, ? opens that option's topic.",
        "",
        "[[order]]",
        "Order",
        "  o toggles newest and oldest for the list on screen. The nav bar shows [Order: newest ▸]",
        "  or [Order: oldest ▸]; click it to toggle. View › Order sets the same choice. Each list",
        "  remembers its own order in ~/.acp/preferences.toml under [ui.list_order].",
        "  Nick timelines, bookmarks, search results, research items, and the export basket order",
        "  by posted time. A research file or page uses the time it was added.",
        "  Site lists (Top, EB, Explore, a tag, reposts) keep the site's order. oldest reverses that",
        "  list. Top is a ranking, not a time sort, so oldest is the end of the ranking.",
        "  Edit history stays in version order. o there does not reorder it.",
        "",
        "[[thread-order]]",
        "Thread order",
        "  Nested view orders the roots, and a reply whose thread root is not in the list. Replies",
        "  stay under their root, oldest first, which is the thread's reading order. A limit on a",
        "  nested search counts roots, not replies.",
        "  Stack, Stack + pane, Carousel, and Grid order every card, including replies. A limit",
        "  counts cards.",
        "  s inside an open thread still cycles that thread's replies: newest, then oldest, then",
        "  top. That sort is separate from Order.",
        "",
        "[[limit]]",
        "Limit",
        "  A limit keeps N items at the chosen end of a newest-first list. newest keeps the first N.",
        "  oldest keeps the last N and shows them oldest first. A blank limit keeps the whole list.",
        "  This is the same window as --limit with --order. 0 is rejected.",
        "",
        "[[nick-order]]",
        "Nick order",
        "  While Nicks is focused, o toggles the nick list between newest and oldest by each nick's",
        "  latest post. A nick with no dated post stays at the end of the newest list. View › Nick",
        "  order sets the same choice. It is remembered on its own, apart from the post lists.",
        "",
        "[[nick-limit]]",
        "Nick limit",
        "  View › Nick limit keeps that many nicks at the chosen end of the nick list. Blank keeps",
        "  every nick. The / filter runs first; the limit then keeps N of the matches. The nick pane",
        "  title shows the order and the limit while they are not the defaults.",
        "",
        "[[search-order]]",
        "Search order",
        "  Search › Order and the Shift-S Order field set newest or oldest for the matches. o on a",
        "  search result toggles it and updates the menu. The choice is part of the search filter,",
        "  with the keyword and the other fields, so running that filter again repeats the order.",
        "",
        "[[search-limit]]",
        "Search limit",
        "  Search › Limit and the Shift-S Limit field keep N matches at the chosen end. Blank keeps",
        "  every match. The status line shows the window, for example:",
        "  search \"strike\": 100 of 236 posts · oldest.",
        "",
        "[[login]]",
        "Log in",
        "  The terminal UI and the command line use the same email code. The terminal UI keeps",
        "  the connection alive while the email is read, and reconnects if it drops, so the same",
        "  code can still be entered. A wrong code stays on that step.",
        "  F2, or Resend code, asks for another code without starting over.",
        "  ? or F1 on the login window opens this topic and leaves the attempt running.",
        "  A finished login is saved in ~/.acp/session.json.",
        "",
        "[[site-attachments]]",
        "Site attachments",
        "  Red Star accepts 4 attachments on a post. This app does not. Local compose, the",
        "  archive, and the network keep every file. Only the copy published to the site is capped.",
        "  When a post or reply has more than 4 files and Publish is chosen, the compose box",
        "  asks which 4 go to Red Star. The first four start checked. The line shows",
        "  n/4 selected for Red Star. Space checks or unchecks the highlighted file. More than",
        "  4 cannot be checked. Fewer than 4 is allowed, including none. Enter confirms.",
        "  Esc leaves the picker and does not publish. With 4 or fewer files the picker is skipped.",
        "  On the command line, --site-attachments takes 1-based indexes, for example",
        "  --site-attachments 1,3,5,6. At most 4 indexes, and each one has to be in the",
        "  attachment list. Omit the flag and the first 4 are the site copy. The notice names",
        "  the files left off the site. The post detail shows On Red Star: 4 of 6.",
        "  ? on the picker opens this topic.",
        "",
        "[[color]]",
        "Colour",
        "  --color auto|always|never, and View › Colour. auto (the default) follows NO_COLOR: a",
        "  non-empty NO_COLOR turns colours off and bold, dim, and underline stay. always forces",
        "  colours on, which is what a demo needs when the shell exported NO_COLOR. never turns",
        "  colours off. [ui] color in ~/.acp/preferences.toml is used when the flag is omitted.",
        "  When NO_COLOR is set and colours are off, the status line, the theme picker, and the",
        "  theme editor say: NO_COLOR is set, so colours are off (use --color always).",
    ].into_iter().map(str::to_string).collect()
}

impl HelpModal {
    /// Help with the generated text.
    pub fn new() -> Self { HelpModal { lines: help_text(), scroll: 0 } }
    /// Help scrolled to `[[topic]]`.
    pub fn topic(topic: &str) -> Self {
        let mut h = Self::new();
        let mark = format!("[[{topic}]]");
        if let Some(i) = h.lines.iter().position(|l| l == &mark) { h.scroll = i; }
        h
    }
    fn scroll_by(&mut self, d: i32) {
        let max = self.lines.len().saturating_sub(1) as i64;
        self.scroll = (self.scroll as i64 + d as i64).clamp(0, max) as usize;
    }
}

impl Default for HelpModal { fn default() -> Self { Self::new() } }

impl ModalState for HelpModal {
    fn title(&self) -> String { "Help".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::F(1) => return Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => return Some(ModalResult::ClosedSubmitted),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_by(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_by(-1),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_by(15),
            KeyCode::PageUp => self.scroll_by(-15),
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll_by(i32::MAX / 2),
            _ => {}
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        matches!(h, ModalHit::Button(_)).then_some(ModalResult::ClosedSubmitted)
    }
    fn on_scroll(&mut self, d: i32) { self.scroll_by(d * 3); }
}
