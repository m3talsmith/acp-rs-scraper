//! Local list order. These pick which end of a stored list to keep.
//! Nothing here opens a connection.

use crate::stored::Post;

/// Which stored profile list a limit applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProfileFeed {
    /// Newest first.
    #[default]
    Newest,
    /// A ranked list. Not chronological.
    Top,
}

impl ProfileFeed {
    /// `newest` or `top`, ignoring case.
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "newest" => Ok(Self::Newest),
            "top" => Ok(Self::Top),
            other => Err(format!("--profile-feed must be newest or top, not {other:?}")),
        }
    }
}

impl std::fmt::Display for ProfileFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Newest => "newest",
            Self::Top => "top",
        })
    }
}

/// Which end of a newest-first list a limit keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScrapeOrder {
    /// Keep the start of the list.
    #[default]
    Newest,
    /// Keep the end of the list.
    Oldest,
}

impl ScrapeOrder {
    /// `newest` or `oldest`, ignoring case.
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "newest" => Ok(Self::Newest),
            "oldest" => Ok(Self::Oldest),
            other => Err(format!("--order must be newest or oldest, not {other:?}")),
        }
    }
}

impl std::fmt::Display for ScrapeOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Newest => "newest",
            Self::Oldest => "oldest",
        })
    }
}

/// Keep the head (`newest`) or the tail (`oldest`) of a newest-first list.
///
/// `newest` keeps the start. `oldest` keeps the end and reverses it.
/// `limit` is how many to keep. `None` keeps the whole list.
pub fn apply_order_limit<T>(mut items: Vec<T>, limit: Option<usize>, order: ScrapeOrder) -> Vec<T> {
    if let Some(n) = limit {
        match order {
            ScrapeOrder::Newest => items.truncate(n),
            ScrapeOrder::Oldest => {
                if items.len() > n {
                    items = items.split_off(items.len() - n);
                }
                items.reverse();
            }
        }
    } else if order == ScrapeOrder::Oldest {
        items.reverse();
    }
    items
}

/// How an open thread's stored replies are ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThreadSort {
    /// Newest first.
    #[default]
    Newest,
    /// Oldest first.
    Oldest,
    /// Highest score first. A missing score sorts last.
    Top,
}

impl ThreadSort {
    /// Next order in the cycle: newest, oldest, top.
    pub fn cycle(self) -> Self {
        match self {
            ThreadSort::Newest => ThreadSort::Oldest,
            ThreadSort::Oldest => ThreadSort::Top,
            ThreadSort::Top => ThreadSort::Newest,
        }
    }
    /// Status-line name.
    pub fn label(self) -> &'static str {
        match self {
            ThreadSort::Newest => "newest",
            ThreadSort::Oldest => "oldest",
            ThreadSort::Top => "top",
        }
    }
    /// The sort name, when this order is not the default.
    pub fn hook(self) -> Option<&'static str> {
        match self {
            ThreadSort::Newest => None,
            ThreadSort::Oldest => Some("oldest"),
            ThreadSort::Top => Some("top"),
        }
    }
    /// Parse `oldest` or `top`.
    pub fn parse_hook(s: &str) -> std::result::Result<ThreadSort, String> {
        match s.trim() {
            "oldest" => Ok(ThreadSort::Oldest),
            "top" => Ok(ThreadSort::Top),
            other => Err(format!("thread sort must be oldest or top, not {other:?}")),
        }
    }
}

/// Reorder `replies` in place. Top puts a missing score last.
pub fn sort_replies(replies: &mut [Post], sort: ThreadSort) {
    match sort {
        ThreadSort::Newest => {
            replies.sort_by(|a, b| b.posted_at.cmp(&a.posted_at).then(a.post_id.cmp(&b.post_id)))
        }
        ThreadSort::Oldest => {
            replies.sort_by(|a, b| a.posted_at.cmp(&b.posted_at).then(a.post_id.cmp(&b.post_id)))
        }
        ThreadSort::Top => replies.sort_by(|a, b| {
            b.score.cmp(&a.score).then(b.posted_at.cmp(&a.posted_at)).then(a.post_id.cmp(&b.post_id))
        }),
    }
}

/// A window a ranked list can be limited to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopPeriod {
    /// The last day.
    Day,
    /// The last week (the menu's default).
    Week,
    /// The last month.
    Month,
    /// The last year.
    Year,
    /// All time.
    All,
}

impl TopPeriod {
    /// Every period, week first.
    pub const ALL: [TopPeriod; 5] = [
        TopPeriod::Week,
        TopPeriod::Day,
        TopPeriod::Month,
        TopPeriod::Year,
        TopPeriod::All,
    ];
    /// The stored name.
    pub fn as_str(self) -> &'static str {
        match self {
            TopPeriod::Day => "day",
            TopPeriod::Week => "week",
            TopPeriod::Month => "month",
            TopPeriod::Year => "year",
            TopPeriod::All => "all",
        }
    }
    /// Parse `day`, `week`, `month`, `year`, or `all`, ignoring case.
    pub fn parse(s: &str) -> std::result::Result<TopPeriod, String> {
        Self::ALL
            .iter()
            .copied()
            .find(|p| p.as_str().eq_ignore_ascii_case(s.trim()))
            .ok_or_else(|| format!("period must be day, week, month, year or all, not {s:?}"))
    }
}

/// Value parser for [`TopPeriod`].
pub fn parse_top_period(s: &str) -> std::result::Result<TopPeriod, String> {
    TopPeriod::parse(s)
}

/// Stored name of a tag, without a leading `#`.
pub fn tag_name(tag: &str) -> String {
    tag.trim().trim_start_matches('#').trim().to_string()
}

/// Database key for a ranked list (`top:week`, `top:all:replies`).
pub fn top_key(period: TopPeriod, replies: bool) -> String {
    if replies { format!("top:{}:replies", period.as_str()) } else { format!("top:{}", period.as_str()) }
}

/// Database key for the executive-board list.
pub fn eb_key(replies: bool) -> String {
    if replies { "eb:replies".into() } else { "eb".into() }
}

/// Database key for an explore list.
pub fn explore_key(latest: bool) -> String {
    if latest { "explore:latest".into() } else { "explore".into() }
}

/// Database key for a stored search (`search:{query}`).
pub fn search_key(query: &str) -> String {
    format!("search:{}", query.trim())
}

/// Database key for a reposts list.
pub fn reposts_key(id: &str) -> String {
    format!("reposts:{}", id.trim())
}

/// Database key for a tag list. Case is folded.
pub fn tag_key(tag: &str) -> String {
    format!("tag:{}", tag_name(tag).to_lowercase())
}

/// Database key for stored edit versions (`edits:{thread}/{post}`).
pub fn edits_key(thread: &str, post: &str) -> String {
    format!("edits:{}/{}", thread.trim(), post.trim())
}

/// Split [`edits_key`] back into `(thread, post)`.
pub fn parse_edits_key(key: &str) -> Option<(&str, &str)> {
    let rest = key.strip_prefix("edits:")?;
    let (thread, post) = rest.split_once('/')?;
    if thread.is_empty() || post.is_empty() || post.contains('/') {
        return None;
    }
    Some((thread, post))
}
