//! Arrange a TUI list with the same window the CLI uses ([`apply_order_limit`]).
//!
//! A chronological list is sorted newest-first by `posted_at`, then [`ScrapeOrder`]
//! and an optional limit pick the head or the tail. A site list (Top, EB, Explore)
//! is not re-sorted by time: `newest` keeps the stored order and `oldest` reverses it.
//!
//! Nested layout orders roots, and a reply whose thread root is not in the list.
//! Replies stay directly under that root, oldest first. A limit counts those roots,
//! not the replies. Flat layouts order every card. A limit counts cards.
//! Edit history is left in version order.

use crate::prelude::*;
use crate::tui::data::NickEntry;

/// Which list a Body view is, and the preferences key it remembers its order under.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListKind {
    /// The selected nicks' posts.
    #[default]
    Timeline,
    /// Bookmarked posts.
    Bookmarks,
    /// Search results.
    Search,
    /// One research project's items.
    Research,
    /// A site read (Top, EB, Explore, a tag, reposts).
    Site,
    /// The export basket.
    Basket,
    /// Stored edit versions. Order does not apply.
    Edits,
}

impl ListKind {
    /// Key under `[ui.list_order]`, when this list remembers an order.
    pub fn pref_key(self) -> Option<&'static str> {
        match self {
            Self::Timeline => Some("timeline"),
            Self::Bookmarks => Some("bookmarks"),
            Self::Search => Some("search"),
            Self::Research => Some("research"),
            Self::Site => Some("site"),
            Self::Basket => Some("basket"),
            Self::Edits => None,
        }
    }
}

/// Newest or oldest for each list that remembers one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListOrders {
    /// Nick timeline.
    pub timeline: ScrapeOrder,
    /// Bookmarks.
    pub bookmarks: ScrapeOrder,
    /// Search results.
    pub search: ScrapeOrder,
    /// Research items.
    pub research: ScrapeOrder,
    /// Site lists.
    pub site: ScrapeOrder,
    /// Export basket.
    pub basket: ScrapeOrder,
}

impl Default for ListOrders {
    fn default() -> Self {
        Self {
            timeline: ScrapeOrder::Newest,
            bookmarks: ScrapeOrder::Newest,
            search: ScrapeOrder::Newest,
            research: ScrapeOrder::Newest,
            site: ScrapeOrder::Newest,
            basket: ScrapeOrder::Newest,
        }
    }
}

impl ListOrders {
    /// The order stored for `kind` (newest for edit history).
    pub fn get(self, kind: ListKind) -> ScrapeOrder {
        match kind {
            ListKind::Timeline => self.timeline,
            ListKind::Bookmarks => self.bookmarks,
            ListKind::Search => self.search,
            ListKind::Research => self.research,
            ListKind::Site => self.site,
            ListKind::Basket => self.basket,
            ListKind::Edits => ScrapeOrder::Newest,
        }
    }
    /// Remember `order` for `kind`. Edit history is skipped.
    pub fn set(&mut self, kind: ListKind, order: ScrapeOrder) {
        match kind {
            ListKind::Timeline => self.timeline = order,
            ListKind::Bookmarks => self.bookmarks = order,
            ListKind::Search => self.search = order,
            ListKind::Research => self.research = order,
            ListKind::Site => self.site = order,
            ListKind::Basket => self.basket = order,
            ListKind::Edits => {}
        }
    }
    /// Fill from `[ui.list_order]`. Unknown or missing values stay newest.
    pub fn apply_prefs(&mut self, p: &crate::prefs::ListOrderPrefs) {
        let one = |s: &Option<String>| {
            s.as_deref()
                .and_then(|s| ScrapeOrder::parse(s).ok())
                .unwrap_or_default()
        };
        self.timeline = one(&p.timeline);
        self.bookmarks = one(&p.bookmarks);
        self.search = one(&p.search);
        self.research = one(&p.research);
        self.site = one(&p.site);
        self.basket = one(&p.basket);
    }
    /// Preferences to write. Newest is omitted so the file only records a change.
    pub fn to_prefs(
        self,
        nick_order: ScrapeOrder,
        nick_limit: Option<u64>,
    ) -> crate::prefs::ListOrderPrefs {
        let s = |o: ScrapeOrder| (o == ScrapeOrder::Oldest).then(|| o.to_string());
        crate::prefs::ListOrderPrefs {
            timeline: s(self.timeline),
            bookmarks: s(self.bookmarks),
            search: s(self.search),
            research: s(self.research),
            site: s(self.site),
            basket: s(self.basket),
            nicks: s(nick_order),
            nick_limit,
        }
    }
}

/// `newest` or `oldest` for a menu choice labelled Newest or Oldest.
pub fn order_from_choice(choice: &str) -> ScrapeOrder {
    if choice.eq_ignore_ascii_case("oldest") {
        ScrapeOrder::Oldest
    } else {
        ScrapeOrder::Newest
    }
}

/// Menu label for `order`.
pub fn order_choice(order: ScrapeOrder) -> &'static str {
    match order {
        ScrapeOrder::Newest => "Newest",
        ScrapeOrder::Oldest => "Oldest",
    }
}

/// Newest `posted_at` first, then `post_id`, so a tie stays put.
pub fn sort_chrono(posts: &mut [Post]) {
    posts.sort_by(|a, b| {
        b.posted_at
            .cmp(&a.posted_at)
            .then_with(|| b.post_id.cmp(&a.post_id))
    });
}

/// Lay `source` out for the current view.
///
/// `nested` groups replies under their thread root. `site_order` keeps `source`'s
/// order instead of sorting by time. `edits` returns `source` unchanged.
pub fn arrange_posts(
    source: &[Post],
    order: ScrapeOrder,
    limit: Option<usize>,
    nested: bool,
    site_order: bool,
    edits: bool,
) -> Vec<Post> {
    if edits {
        return source.to_vec();
    }
    if nested {
        arrange_nested(source, order, limit, site_order)
    } else {
        let mut items = source.to_vec();
        if !site_order {
            sort_chrono(&mut items);
        }
        apply_order_limit(items, limit, order)
    }
}

fn arrange_nested(
    source: &[Post],
    order: ScrapeOrder,
    limit: Option<usize>,
    site_order: bool,
) -> Vec<Post> {
    let ids: HashSet<String> = source.iter().map(|p| p.post_id.clone()).collect();
    let mut replies: HashMap<String, Vec<Post>> = HashMap::new();
    let mut tops = Vec::new();
    for p in source {
        let root = p.parent_post_id.as_str();
        if !root.is_empty() && ids.contains(root) {
            replies.entry(root.to_string()).or_default().push(p.clone());
        } else {
            tops.push(p.clone());
        }
    }
    if !site_order {
        sort_chrono(&mut tops);
    }
    let tops = apply_order_limit(tops, limit, order);
    let mut out = Vec::with_capacity(source.len());
    for root in tops {
        let id = root.post_id.clone();
        out.push(root);
        if let Some(mut kids) = replies.remove(&id) {
            kids.sort_by(|a, b| {
                a.posted_at
                    .cmp(&b.posted_at)
                    .then_with(|| a.post_id.cmp(&b.post_id))
            });
            out.extend(kids);
        }
    }
    out
}

/// Nicks by their latest post, then the same window. A nick with no date sorts last
/// when the list is newest-first (and therefore first after `oldest` reverses it).
pub fn arrange_nicks(
    mut nicks: Vec<NickEntry>,
    order: ScrapeOrder,
    limit: Option<usize>,
) -> Vec<NickEntry> {
    nicks.sort_by(cmp_latest);
    apply_order_limit(nicks, limit, order)
}

fn cmp_latest(a: &NickEntry, b: &NickEntry) -> std::cmp::Ordering {
    match (&a.latest, &b.latest) {
        (Some(x), Some(y)) => y
            .cmp(x)
            .then_with(|| a.nick.to_lowercase().cmp(&b.nick.to_lowercase())),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.nick.to_lowercase().cmp(&b.nick.to_lowercase()),
    }
}

/// Status line for a filtered list. `label` is `search "strike"` or `nicks`.
pub fn result_status(
    label: &str,
    shown: usize,
    total: usize,
    order: ScrapeOrder,
    limited: bool,
) -> String {
    let what = if label.starts_with("nicks") {
        ""
    } else {
        " posts"
    };
    if limited || shown != total {
        format!("{label}: {shown} of {total}{what} · {order}")
    } else {
        format!("{label}: {shown}{what} · {order}")
    }
}

/// What `order` means for the list on screen, for the status tooltip.
pub fn order_meaning(nested: bool, site_order: bool, edits: bool) -> &'static str {
    if edits {
        "edit history stays in version order"
    } else if site_order {
        "reverses the site's list; Top stays a ranking"
    } else if nested {
        "roots; replies stay in thread order"
    } else {
        "every card, by posted time"
    }
}
