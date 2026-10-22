//! One helper draws every hover tooltip.
//!
//! A control does not draw its own bubble. [`tip_for`] pairs the text with the
//! Help topic for that action, and [`paint`] draws both. The second line is the
//! deep link: choosing it opens Help on that topic.

use crate::tui::commands::command_topic;
use crate::tui::modals::ModalHit;
use crate::tui::state::{App, Hit};
use crate::tui::theme::frame as boxed;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::{Clear, Paragraph};

/// What hover shows: the explanation, and the Help topic it opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    /// Short explanation. A key is included when the control has one.
    pub text: String,
    /// Help topic id (`[[topic]]` in Help).
    pub topic: &'static str,
}

fn tip(text: impl Into<String>, topic: &'static str) -> Tip {
    Tip { text: text.into(), topic }
}

/// The tooltip for a clickable control, when it has one.
pub fn tip_for(app: &App, hit: Hit) -> Option<Tip> {
    match hit {
        Hit::MenuItem(i) => menu_item_tip(app, i),
        Hit::MenuOpt(k) => menu_opt_tip(app, k),
        Hit::StartScan => Some(tip(
            if app.job.is_some() { "Cancel the running job (F6)" } else { "Start Scanning (F6)" },
            "scrape",
        )),
        Hit::Back => Some(tip("Back to the previous list (<)", "search")),
        Hit::ViewMode => Some(tip("Next view mode (v)", "view")),
        Hit::ListOrder => Some(tip("Order (o)", "order")),
        Hit::StackPrev => Some(tip("Previous post (,)", "view")),
        Hit::StackNext => Some(tip("Next post (.)", "view")),
        Hit::CloseDetail => Some(tip("Close this detail (Esc)", "browse")),
        Hit::Bookmarks => Some(tip("Show bookmarks (B)", "bookmark")),
        Hit::Eb => Some(tip("Executive Board", "view")),
        Hit::Explore => Some(tip("Explore", "view")),
        Hit::NickFilter => Some(tip("Filter nicks (/)", "browse")),
        Hit::NickFilterClear => Some(tip("Clear the nick filter (Esc)", "browse")),
        Hit::Star(_) | Hit::StarTop => Some(tip("Bookmark (b)", "bookmark")),
        Hit::Basket(_) | Hit::BasketTop => Some(tip("Export basket (x)", "basket")),
        Hit::Research(_) | Hit::ResearchTop => Some(tip("Add to a research project (r)", "research")),
        Hit::ThemeSwitch => Some(tip("Theme (F5)", "debug")),
        Hit::Job | Hit::Status => Some(tip("Job details (Enter)", "keys")),
        Hit::Logo => Some(tip("ACP Red Star", "about-credits")),
        Hit::HelpMark => Some(tip("Help (? / F1)", "getting-started")),
        Hit::Toast(_) => Some(tip("Dismiss this notice", "notifications")),
        Hit::ViewAllNotices => Some(tip("View all notifications", "notifications")),
        Hit::NoticeSearch => Some(tip("Search notifications", "notifications")),
        Hit::NoticeOrder => Some(tip("Newest or oldest", "notifications")),
        Hit::NoticeKind => Some(tip("Filter by kind", "notifications")),
        Hit::NoticeSwitch(0) => Some(tip("Do not disturb", "notifications")),
        Hit::NoticeSwitch(_) => Some(tip("Silence this kind of notice", "notifications")),
        Hit::NoticeRow(_) => Some(tip("Notification", "notifications")),
        Hit::NoticeClear(_) => Some(tip("Clear this notification", "notifications")),
        Hit::NoticeClearAll => Some(tip("Clear all notifications", "notifications")),
        Hit::ToggleNicks => Some(tip("Collapse or open the nick bar", "browse")),
        Hit::ToggleActions => Some(tip("Collapse or open the action history", "actions")),
        Hit::Action(_) => Some(tip("Jump back to this action", "actions")),
        Hit::ActionBranches => Some(tip("Action branches (b)", "actions")),
        Hit::ActionPane => Some(tip("Scroll the action history", "actions")),
        Hit::Nick(_) => Some(tip("Show this nick's posts (Enter)", "browse")),
        Hit::Post(_) => Some(tip("Open this post (Enter)", "browse")),
        Hit::Link(_) => Some(tip("Open this link (Enter)", "browse")),
        Hit::Frame(_) => Some(tip("Back to this post", "browse")),
        Hit::Modal(h) => modal_tip(app, h),
        Hit::Tip | Hit::NickPane | Hit::ListPane | Hit::DetailPane | Hit::Backdrop | Hit::EditorRow(_) => None,
    }
}

fn menu_item_tip(app: &App, i: usize) -> Option<Tip> {
    let c = app.menu.menu.items.get(i)?;
    let topic = command_topic(c.id);
    let text = if c.on_bar {
        format!("{} (Alt-{})", c.label, c.hint)
    } else {
        c.label.to_string()
    };
    Some(tip(text, topic))
}

fn menu_opt_tip(app: &App, k: usize) -> Option<Tip> {
    let item = app.menu.open?;
    let c = app.menu.menu.items.get(item)?;
    let o = c.opts.get(k)?;
    let topic = if o.help.is_empty() { command_topic(c.id) } else { o.help };
    let text = if o.flag.is_empty() { o.label.to_string() } else { format!("{} — {}", o.label, o.flag) };
    Some(tip(text, topic))
}

fn modal_tip(app: &App, h: ModalHit) -> Option<Tip> {
    if let (Some(crate::tui::modals::Modal::Rules(_)), ModalHit::Field(i)) = (&app.modal, h) {
        let topic = match i {
            0 => "rules-trigger",
            1 | 2 => "rules-condition",
            3 | 4 => "rules-action",
            _ => "rules-priority",
        };
        return Some(tip("Rule", topic));
    }
    let ModalHit::Button(i) = h else { return None };
    let (text, topic) = match &app.modal {
        Some(crate::tui::modals::Modal::Compose(m)) => {
            let edit = matches!(m.kind, crate::tui::modals::ComposeKind::EditReply { .. });
            if i == 0 {
                ("Cancel", "compose")
            } else if !edit && i == 1 {
                ("Save draft. Stored on this machine. Nothing is sent.", "outbox")
            } else {
                ("Publish. The site copy uploads, then the text is sent.", "compose")
            }
        }
        Some(crate::tui::modals::Modal::About(_)) => ("Close About (Esc)", "about-credits"),
        Some(crate::tui::modals::Modal::Help(_)) => ("Close Help (Esc)", "keys"),
        Some(crate::tui::modals::Modal::Login(_)) => ("Log in", "login"),
        Some(crate::tui::modals::Modal::Search(_)) => ("Search", "search"),
        Some(crate::tui::modals::Modal::Rules(m)) => {
            let label = m.buttons().get(i).copied().unwrap_or("Rules");
            let topic = if label == "Clone" { "rules-clone" } else { "rules" };
            (label, topic)
        }
        Some(crate::tui::modals::Modal::Branches(_)) => ("Close", "actions"),
        _ => ("Close", "keys"),
    };
    Some(tip(text, topic))
}

fn clip_cols(s: &str, width: usize) -> String {
    s.chars().take(width).collect()
}

/// Draw the hover tooltip. The underlined `?` line is [`Hit::Tip`].
pub fn paint(f: &mut Frame, app: &mut App, screen: Rect) {
    let Some(tip) = app.tip.clone() else { return };
    let t = app.theme;
    let link = format!("? {}", tip.topic);
    let width = tip.text.chars().count().max(link.chars().count()).saturating_add(2);
    let w = (width as u16).clamp(8, screen.width.max(1));
    let h = 4u16.min(screen.height);
    if h < 3 || w < 4 { return; }
    let anchor = app.tip_at;
    let x = anchor.x.min(screen.x.saturating_add(screen.width.saturating_sub(w)));
    let mut y = anchor.y.saturating_add(anchor.height.max(1));
    if y.saturating_add(h) > screen.y.saturating_add(screen.height) {
        y = anchor.y.saturating_sub(h);
    }
    if y < screen.y { y = screen.y; }
    let r = Rect { x, y, width: w, height: h };
    f.render_widget(Clear, r);
    let b = boxed(&t.dialog());
    let inner = b.inner(r);
    f.render_widget(b, r);
    if inner.height == 0 || inner.width == 0 { return; }
    let cols = inner.width as usize;
    f.render_widget(
        Paragraph::new(Span::styled(clip_cols(&tip.text, cols), t.plain())),
        Rect { height: 1, ..inner },
    );
    if inner.height >= 2 {
        let link_row = Rect { y: inner.y + 1, height: 1, ..inner };
        f.render_widget(
            Paragraph::new(Span::styled(clip_cols(&link, cols), t.accent().underline(true))),
            link_row,
        );
        app.hits.push((link_row, Hit::Tip));
    }
}
