//! Help modal: the real `--help` text (generated from clap, so it cannot drift)
//! plus the TUI key bindings.

use super::*;
use crossterm::event::KeyCode;

/// Scrollable help text.
#[derive(Debug, Clone, PartialEq)]
pub struct HelpModal {
    /// Every line, unfiltered.
    pub lines: Vec<String>,
    /// First visible line of the current view.
    pub scroll: usize,
    /// Search. Typing filters topics and key bindings.
    pub query: String,
}

/// Help for the ACP brand. Tests and the default screen use this.
pub fn help_text() -> Vec<String> {
    help_for(&crate::brands::builtin_default())
}

/// Help text for `brand`: key bindings, topics, then that brand's `--help`.
///
/// `?` starts on the keys. The recovery phrase is the `recovery` topic, and
/// it is also still in `--help`.
pub fn help_for(brand: &crate::brands::Brand) -> Vec<String> {
    use clap::CommandFactory;
    let guide = crate::crypto::recovery::GUIDE.trim();
    let mut v = vec!["[[keys]]".into(), "Terminal UI keys".into(), String::new()];
    v.push("  Letters typed here search, including j and k. ↑ ↓ PgUp PgDn Home End scroll.".into());
    for (k, d) in crate::config::TUI_KEYBINDINGS { v.push(format!("  {k:<22} {d}")); }
    v.push(String::new());
    v.push("[[recovery]]".into());
    v.push("Recovery phrase".into());
    v.push(String::new());
    v.extend(guide.lines().map(|l| if l.is_empty() { String::new() } else { format!("  {l}") }));
    v.extend(["".into(), "Menu commands map to these flags; Start Scanning runs the same scrape the CLI does.".into()]);
    v.extend(help_topics());
    v.extend(area_topics(brand));
    v.extend(menu_topics(brand));
    v.push(String::new());
    let mut cmd = if brand.shows_acp() {
        crate::cli::Args::command()
    } else {
        crate::brands::style_command(crate::cli::Args::command(), brand)
    };
    let help = cmd.render_long_help().to_string();
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
        "  s inside an open thread still cycles that thread's stored replies: newest, then oldest,",
        "  then top. That sort is separate from Order. It does not connect, and it does not open login.",
        "",
        "[[layout]]",
        "Layout",
        "  v cycles the layout: Nested, Stack, Stack + content pane, Carousel (infinite),",
        "  Carousel (fixed), and Grid. V goes the other way. View sets the same layout.",
        "  Nested keeps replies under their root. Stack, Stack + pane, Carousel, and Grid",
        "  show every card. The nav bar shows the layout that is on screen.",
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
        "  Red Star takes up to 4 images, videos, or PDFs on a post. This app does not",
        "  have that limit. Local compose, the archive, and the network keep every file.",
        "  Audio and other documents stay local. They show in the picker greyed out, with",
        "  the reason, and they are not uploaded. A PDF counts toward the 4 and is sent",
        "  on the file input. The live form's accept list and size limits still apply.",
        "  When a post or reply has more than 4 of those files and Publish is chosen, the",
        "  compose box asks which 4 go to Red Star. The first four start",
        "  checked. The line shows n/4 selected for Red Star. Space checks or unchecks the",
        "  highlighted image, video, or PDF. Space on a greyed row does not check it. More than 4",
        "  cannot be checked. Fewer than 4 is allowed, including none. Enter confirms.",
        "  Esc leaves the picker and does not publish. With 4 or fewer images, videos, or PDFs",
        "  the picker is skipped. Audio and other documents still stay local.",
        "  On the command line, --site-attachments takes 1-based indexes, for example",
        "  --site-attachments 1,3,5,6. At most 4 indexes, and each one has to be an image,",
        "  a video, or a PDF. Naming an audio file or another type fails and names that",
        "  file. Omit the flag and the first 4 images, videos, or PDFs are the site copy;",
        "  other types are skipped. The notice names the files left off the site.",
        "  The post detail shows On Red Star: 4 of 6. ? on the picker opens this topic.",
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

fn push_topic(v: &mut Vec<String>, id: &str, title: &str, body: &[&str]) {
    v.push(format!("[[{id}]]"));
    v.push(title.into());
    for line in body { v.push((*line).into()); }
    v.push(String::new());
}

fn area_topics(brand: &crate::brands::Brand) -> Vec<String> {
    let mut v = Vec::new();
    let mut started = include_str!("../../../docs/GETTING-STARTED.md").to_string();
    if !brand.shows_acp() {
        started = started.replacen(
            "ACP Red Star is a local client",
            &format!("{} is a local client", brand.title),
            1,
        );
        started = started.replace(
            "About, beside Help, opens one window on Credits, Why this is necessary, or ACP platform.",
            "About, beside Help, opens one window on Credits or Why this is necessary.",
        );
        started = started.replace(
            "About has three sections and no others. ACP platform is the party program adopted October 12, 2024, with headings and a scroll. The link to join is at the top and the bottom of that section only.",
            "About has two sections and no others.",
        );
    }
    v.push("[[getting-started]]".into());
    v.push("Getting started".into());
    v.extend(started.lines().map(str::to_string));
    v.push(String::new());
    if brand.shows_acp() {
        push_topic(&mut v, "brands", "Brands", &[
            "  A brand is the logo, the title, and the motto. It is separate from the theme.",
            "  The built-in default brand is ACP Red Star. The built-in plain brand is the title",
            "  Red Star, a star, and no motto: acp-red-star --brand plain.",
            "  --brand NAME is this run. [ui] brand in ~/.acp/preferences.toml keeps the choice.",
            "  The header switch «name» ▾ opens the picker. Moving the highlight previews it.",
            "  Esc puts the previous brand back. Edit brands… opens the editor.",
            "  The command palette has Brand: use…, Brand: list, Brand: edit…, and Brand: reload.",
            "  In the editor, Ctrl-S saves, Ctrl-R reverts, Ctrl-N saves as new, Ctrl-D duplicates,",
            "  Ctrl-X deletes, and Ctrl-C closes. Those are the theme editor's chords.",
            "  A user brand is ~/.acp/brands/<name>.toml. The logo is builtin, inline text, or a",
            "  file inside that folder. A path that leaves the folder is refused.",
            "  --help, --version, the terminal title, and HTML export headers use the title.",
            "  Credits and the other surprises keep their own text.",
        ]);
    } else {
        push_topic(&mut v, "brands", "Brands", &[
            "  A brand is the logo, the title, and the motto. It is separate from the theme.",
            "  This brand is a neutral title and a star. It has no party name and no slogan.",
            "  acp-red-star --brand plain is the title Red Star, a star, and no motto.",
            "  --brand NAME is this run. [ui] brand in ~/.acp/preferences.toml keeps the choice.",
            "  The header switch «name» ▾ opens the picker. Each row is that brand's own title.",
            "  Esc puts the previous brand back. Edit brands… opens the editor.",
            "  The command palette has Brand: use…, Brand: list, Brand: edit…, and Brand: reload.",
            "  In the editor, Ctrl-S saves, Ctrl-R reverts, Ctrl-N saves as new, Ctrl-D duplicates,",
            "  Ctrl-X deletes, and Ctrl-C closes. Those are the theme editor's chords.",
            "  A user brand is ~/.acp/brands/<name>.toml. The logo is builtin, inline text, or a",
            "  file inside that folder. A path that leaves the folder is refused.",
            "  --help, --version, the terminal title, and HTML export headers use the title.",
            "  Credits and the other surprises keep their own text.",
        ]);
    }
    push_topic(&mut v, "browse", "Browse", &[
        "  The nick bar lists who is in the database. Enter opens that nick's posts.",
        "  ‹ collapses it. So does a click on the cells beside ‹, or on the bar's right edge.",
        "  Enter on a post opens it. Esc closes the top detail. / filters nicks.",
        "  Explore gives up after 12 seconds with no reply and stores nothing. It says it is still waiting.",
        "  b bookmarks the open or highlighted post. The star on a card does the same.",
    ]);
    push_topic(&mut v, "compose", "Compose", &[
        "  n is a new post. On an open post, r replies and Q quotes.",
        "  Cancel is the default. Save draft stores the text on this machine and sends nothing.",
        "  A text post or reply sends validate, then save, once.",
        "  --dry-run prints the frames and does not connect. The reply names the new post.",
        "  Attach lists local files. Images, videos, and PDFs upload, then the text is sent.",
        "  + and - ask before a vote. Post menu subscribe, delete, edit, and repost ask too.",
        "  Cancel is the default. Delete and edit also ask you to type yes. None of them send.",
    ]);
    push_topic(&mut v, "outbox", "Drafts and outbox", &[
        "  Save draft, in the compose box or Compose › Save draft, writes the post on this machine.",
        "  The same Draft id writes another version. Keep local leaves it off the queue.",
        "  Queue draft puts that local id in the outbox. The outbox id is Q- plus the local id.",
        "  Send from outbox asks first. Cancel is the default. It sends only a text post or a reply,",
        "  with the same frames as Publish. --dry-run prints those frames and does not connect.",
        "  An item older than 24 hours asks again. A row left in sending is read back before another try.",
        "  Cancel outbox item marks a queued row cancelled. A sending row waits for the read-back.",
    ]);
    push_topic(&mut v, "post-actions", "Post actions", &[
        "  Vote, subscribe, delete, edit, and repost stay gated. No live frame is in the repo.",
        "  + asks before an upvote. - asks before a downvote. Cancel is the default.",
        "  Post menu Subscribe and Repost ask the same way. Delete and Edit also ask you to type yes.",
        "  A confirm still sends nothing. --dry-run prints the stub and does not connect.",
        "  The clicks to record are in architecture/INTERACTIONS-CAPTURE.md.",
    ]);
    push_topic(&mut v, "research", "Research", &[
        "  r, or the ◈ on a card, adds the current post to a research project.",
        "  Add current view / search queues the list on screen. Export project writes that project.",
    ]);
    push_topic(&mut v, "sync", "Sync", &[
        "  Sync now reads the site into the local database. Auto sync repeats it while the TUI is open.",
        "  Whole site runs the incremental whole-site update. Rules run on the posts it stored.",
        "  Sync › Rules opens the rule list. A sync runs the enabled rules on the rows it just wrote.",
    ]);
    push_topic(&mut v, "whole-site", "Whole site", &[
        "  Scrape › Whole site, then Start Scanning, runs acprs --scrape-all.",
        "  It reads every reachable page: feeds, threads and replies, profiles, tags,",
        "  spaces and communities when the site has them, reposts, and attachment URLs.",
        "  The file bytes stay for a nick scrape. This crawl stores the URL and the type.",
        "  The frontier is in the encrypted database. A stopped run resumes.",
        "  --limit caps new posts. Time budget (--scrape-budget) caps seconds.",
        "  Between pages it waits, and it backs off after a refusal. robots.txt is honored.",
        "  The next finished run is incremental: newest feeds stop at known posts,",
        "  and recent threads are read again for new replies. --full walks everything.",
        "  Sync › Whole site is that incremental update. Rules and alerts run on the new rows.",
        "  The status line shows pages, posts, and how many are left.",
        "  Finish and failure raise a notice. This crawl does not publish.",
    ]);
    push_topic(&mut v, "rules", "Rules", &[
        "  Sync › Rules lists system rules and user rules. Lower priority runs first.",
        "  j and k move. Enter edits. n starts a user rule. Space enables or disables a user rule.",
        "  w watches a user, s is a saved search, and m is replies to me. Each is a user rule.",
        "  A system rule is read-only. Clone makes a user rule you can edit, disable, or delete.",
        "  The shipped Synced post rule starts off, so a sync does not notify on every post.",
        "  --rules-list, --rules-show, --rules-enable, --rules-disable, --rules-clone,",
        "  --rules-delete, --rules-priority with --rules-rank, --rules-import, --rules-export,",
        "  --rules-watch, --rules-saved, --rules-replies, and --rules-preview.",
        "  Import and export are TOML and JSON. --rules-preview applies nothing.",
    ]);
    push_topic(&mut v, "rules-template", "Alert templates", &[
        "  w, s, and m on the rule list start a preset. There is no separate alert screen.",
        "  Watch a user notifies when that nick posts. The line names the author and the post URL.",
        "  A saved search notifies when a synced post matches the keyword.",
        "  Replies to me notifies when someone replies to that nick.",
        "  --rules-watch NICK, --rules-saved TEXT, and --rules-replies NICK insert the same rules.",
        "  The preset is a user rule. Open it and edit the condition, the action, or the priority.",
    ]);
    push_topic(&mut v, "rules-trigger", "Rule trigger", &[
        "  The trigger is a post that just synced. Conditions are the filters.",
        "  A preview (--rules-preview) runs the same rules on posts already stored and applies nothing.",
        "  ? on the rule name opens this topic.",
    ]);
    push_topic(&mut v, "rules-condition", "Rule condition", &[
        "  none matches every synced post. author, keyword, regex, source, tag, saved_search,",
        "  and replies_to narrow it. A keyword is case-insensitive. An invalid regex is an error.",
        "  source site and redstar match Red Star. node does not, until peer sync exists.",
        "  saved_search keeps the keyword on the rule. It does not read search history.",
        "  replies_to matches a reply whose parent was written by that nick.",
        "  The editor keeps one condition. An imported rule may have several; all of them must match.",
    ]);
    push_topic(&mut v, "rules-action", "Rule action", &[
        "  notify prints a line for the job log. bookmark, tag, research, and export change local data.",
        "  Notify has no cap. A post fires at most once per rule.",
        "  A rule does not publish to the site.",
    ]);
    push_topic(&mut v, "rules-priority", "Rule priority", &[
        "  Lower priority runs first, system rules and user rules together.",
        "  Changing priority is allowed on a system rule. That is ordering, not a body edit.",
    ]);
    push_topic(&mut v, "rules-clone", "Clone a system rule", &[
        "  Clone copies a system rule into a user rule and leaves the original in place.",
        "  The clone starts enabled. Edit, disable, and delete apply to that copy.",
        "  --rules-clone takes the system key, for example synced-post.",
    ]);
    push_topic(&mut v, "discover", "Discover", &[
        "  People, nodes, and rooms arrive with the network node. Discover does not pretend they are here.",
    ]);
    push_topic(&mut v, "account", "Account", &[
        "  Log in uses the email code. Log out deletes ~/.acp/session.json.",
        "  Browsing the local database does not need a session. Start Scanning does.",
        "  Account › Services lists every registered service. Red Star is on when its file is missing.",
    ]);
    push_topic(&mut v, "services", "Services", &[
        "  Red Star is enabled when ~/.acp/services/redstar.toml is missing. Other services start off.",
        "  YouTube is registered and off until enabled. --youtube-read is prepared, gated, and fetches nothing.",
        "  Non-secret settings live in that toml file. Secrets stay in the encrypted database and are never printed.",
        "  Space toggles a service. Reload plugins runs the same check as plugins.reload.",
        "  The default chord for plugins.reload is ctrl-p in keymap/default.toml. The keymap can change it.",
        "  A failed package moves to ~/.acp/quarantined/plugins. u restores it and checks it again. d deletes that copy.",
        "  A rustc or core-version mismatch is refused and is not quarantined. --doctor prints that pin.",
    ]);
    push_topic(&mut v, "combine", "Combine", &[
        "  Mixed is the default: one list, the same order this view already uses.",
        "  Grouped adds a section per service. With only Red Star that section is Red Star.",
        "  The choice is remembered in [ui] combine.",
    ]);
    push_topic(&mut v, "source", "Source", &[
        "  View › Source cycles All and Red Star. The choice is remembered in [ui] source.",
        "  The default chord is view.source, ctrl-] in keymap/default.toml.",
        "  Bookmarks, site search, and edit history stay available on every source.",
    ]);
    if brand.shows_acp() {
        push_topic(&mut v, "about-credits", "About · Credits", &[
            "  About is one modal with three sections. Credits names who built the client.",
            "  Choose Credits, Why this is necessary, or ACP platform. Each opens this same window.",
        ]);
    } else {
        push_topic(&mut v, "about-credits", "About · Credits", &[
            "  About is one modal with two sections. Credits names who built the client.",
            "  Choose Credits or Why this is necessary. Each opens this same window.",
        ]);
    }
    push_topic(&mut v, "about-why", "About · Why this is necessary", &[
        "  The local copy is why this client exists: read, search, and export without the site.",
        "  The database is encrypted. This topic is not the party program.",
    ]);
    if brand.shows_acp() {
        push_topic(&mut v, "about-platform", "About · ACP platform", &[
            "  ACP platform is the American Communist Party Program adopted October 12, 2024.",
            "  The text is the vendored copy, with headings and a scroll. It is not paraphrased.",
            "  https://acp.us/program",
            "  https://acp.us/docs/2024-program-a51b2b90421960a720072cc1731dfac0.pdf",
            "  https://acp.us/join is at the top and the bottom of that section only.",
        ]);
    }
    push_topic(&mut v, "notifications", "Notifications", &[
        "  View › Notifications opens the saved list. It is not the site Notifications pane.",
        "  Toasts stack with no cap. When they would use more than a quarter of the screen,",
        "  as many as fit stay up and View all opens this same page.",
        "  Newest first. o switches to oldest. f filters by kind. / searches the title and the text.",
        "  x clears the highlighted row. Clear all deletes every saved notice.",
        "  They are stored in the encrypted database and are still there after a restart.",
        "  Opening this page, View all, search, order, and clearing a notice are not action-bar steps.",
        "  Do not disturb and each kind silence the toast and the desktop notification together.",
        "  The list still keeps those rows. Space toggles do not disturb.",
    ]);
    push_topic(&mut v, "actions", "Action history", &[
        "  The right bar lists every step that led to this view. It does not drop old steps.",
        "  The wheel scrolls the list. With the bar focused, the arrows, PgUp, and PgDn scroll it too.",
        "  Click a step, or Enter on the highlighted step, to jump back. The bar collapses to one column.",
        "  A new step after a jump keeps the later steps as another branch. The bar shows which path.",
        "  b, or a click on that line, lists the branches. Choosing one restores that path.",
        "  The branches are stored in the encrypted database with search history. Clear history deletes them.",
        "  Search › History kept is blank by default, which keeps every search. A number keeps that many.",
    ]);
    if brand.shows_acp() {
        push_topic(&mut v, "bookmark", "Bookmarks", &[
            "  Bookmark (b) marks the open or highlighted post. B shows the bookmark list.",
            "  Sync with acp.us is Bookmarks › Sync with acp.us.",
        ]);
    } else {
        push_topic(&mut v, "bookmark", "Bookmarks", &[
            "  Bookmark (b) marks the open or highlighted post. B shows the bookmark list.",
            "  Bookmarks stay on this machine.",
        ]);
    }
    push_topic(&mut v, "basket", "Basket", &[
        "  Export basket (x) adds or removes the open post. Library › Basket exports it.",
    ]);
    push_topic(&mut v, "quit", "Quit", &[
        "  q leaves the TUI. A running job can keep going in the background.",
    ]);
    v
}

fn menu_topics(brand: &crate::brands::Brand) -> Vec<String> {
    use clap::Parser;
    let Ok(args) = crate::cli::Args::try_parse_from(["acp-red-star"]) else { return Vec::new() };
    let mut menu = crate::tui::commands::Menu::new(&args);
    menu.apply_brand(brand);
    let mut v = Vec::new();
    for id in [
        crate::tui::commands::CommandId::View,
        crate::tui::commands::CommandId::Search,
        crate::tui::commands::CommandId::Library,
        crate::tui::commands::CommandId::Scrape,
        crate::tui::commands::CommandId::Export,
        crate::tui::commands::CommandId::Import,
        crate::tui::commands::CommandId::List,
        crate::tui::commands::CommandId::Maintain,
        crate::tui::commands::CommandId::Debug,
    ] {
        let c = menu.get(id);
        let topic = crate::tui::commands::command_topic(id);
        v.push(format!("[[{topic}]]"));
        v.push(c.label.into());
        for o in &c.opts {
            let flag = if o.flag.is_empty() { String::new() } else { format!(" {}", o.flag) };
            let label = o.label;
            let help = o.help;
            v.push(format!("  {label}{flag}. ? {help}"));
        }
        v.push(String::new());
    }
    v
}

impl HelpModal {
    /// Help for the ACP brand.
    pub fn new() -> Self { Self::from_brand(&crate::brands::builtin_default()) }
    /// Help for `brand`. The ACP brand keeps the party text. A neutral brand does not.
    pub fn from_brand(brand: &crate::brands::Brand) -> Self {
        HelpModal { lines: help_for(brand), scroll: 0, query: String::new() }
    }
    /// Help scrolled to `[[topic]]`.
    pub fn topic(topic: &str) -> Self { Self::topic_in(&crate::brands::builtin_default(), topic) }
    /// Help for `brand`, scrolled to `[[topic]]`.
    pub fn topic_in(brand: &crate::brands::Brand, topic: &str) -> Self {
        let mut h = Self::from_brand(brand);
        let mark = format!("[[{topic}]]");
        if let Some(i) = h.lines.iter().position(|l| l == &mark) { h.scroll = i; }
        h
    }
    /// Lines after the search filter. An empty query is the full text.
    pub fn view_lines(&self) -> Vec<String> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() { return self.lines.clone(); }
        let query = self.query.as_str();
        let mut out = vec![format!("Search: {query}")];
        let mut block = Vec::new();
        let mut take = false;
        let flush = |out: &mut Vec<String>, block: &mut Vec<String>, take: bool| {
            if take { out.append(block); } else { block.clear(); }
        };
        for line in &self.lines {
            if line.starts_with("[[") {
                flush(&mut out, &mut block, take);
                take = false;
            }
            block.push(line.clone());
            if line.to_lowercase().contains(&q) { take = true; }
        }
        flush(&mut out, &mut block, take);
        if out.len() == 1 { out.push("No matching topic.".into()); }
        out
    }
    fn scroll_by(&mut self, d: i32) {
        let max = self.view_lines().len().saturating_sub(1) as i64;
        self.scroll = (self.scroll as i64 + d as i64).clamp(0, max) as usize;
    }
    fn jump_topic(&mut self, d: i32) {
        let lines = self.view_lines();
        let marks: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.starts_with("[[")).map(|(i, _)| i).collect();
        if marks.is_empty() { return; }
        let cur = marks.iter().rposition(|&i| i <= self.scroll).unwrap_or(0);
        self.scroll = marks[crate::tui::modals::cycle(cur, marks.len(), d)];
    }
}

impl Default for HelpModal { fn default() -> Self { Self::new() } }

impl ModalState for HelpModal {
    fn title(&self) -> String { "Help".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1) => return Some(ModalResult::ClosedCancelled),
            KeyCode::Char('q') if self.query.is_empty() => return Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => return Some(ModalResult::ClosedSubmitted),
            KeyCode::Down => self.scroll_by(1),
            KeyCode::Up => self.scroll_by(-1),
            KeyCode::PageDown => self.scroll_by(15),
            KeyCode::PageUp => self.scroll_by(-15),
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll_by(i32::MAX / 2),
            KeyCode::Left => self.jump_topic(-1),
            KeyCode::Right => self.jump_topic(1),
            KeyCode::Backspace => { self.query.pop(); self.scroll = 0; }
            KeyCode::Char(c) => { self.query.push(c); self.scroll = 0; }
            _ => {}
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        matches!(h, ModalHit::Button(_)).then_some(ModalResult::ClosedSubmitted)
    }
    fn on_scroll(&mut self, d: i32) { self.scroll_by(d * 3); }
}
