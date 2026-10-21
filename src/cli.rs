//! Command-line interface: arguments, nick lists, prompts, man page and completions.

use crate::prelude::*;

/// Print `msg` on stderr and read one trimmed line from stdin.
pub fn prompt(msg: &str) -> Result<String> {
    eprint!("{msg}");
    io::stderr().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

#[derive(clap::Parser, Clone)]
#[command(
    name = "acp-red-star",
    version,
    about = "ACP Red Star: a local client for the Red Star forum (acp.us/rs)",
    long_about = "ACP Red Star. Run with no flags (or --tui) for the terminal UI: browse every nick and post in the local database, open post details and the posts they link, search, and start scans, exports and imports from a menu bar. With flags it runs headless: log in to https://acp.us/rs as Firefox, scrape posts (and optionally mentions + profiles) for one or more nicks into a local SQLite database, download attachments, and export JSON/HTML — or work entirely offline with --search / --export / --import.",
    after_help = concat!(include_str!("crypto/recovery_guide.txt"), "
Examples:
  acp-red-star                      # terminal UI
  acp-red-star --tui --nicks @hazaldin -v
  acp-red-star --theme default      # terminal UI with this theme
  acp-red-star --list-themes
  acp-red-star --theme-init all     # download every theme in the repo into ~/.acp/themes
  acp-red-star --theme-starter mine # a minimal theme to edit (Shift-F5)
  acp-red-star --scrape             # headless scrape; asks for nicks
  acp-red-star --nicks @hazaldin,@m3talsmith --format html --compress --tar-gz
  acp-red-star --nick @hazaldin --format json,html --output hazaldin/hazaldin.json
  acp-red-star --nick alice --no-mentions --crawl-limit 200
  acp-red-star --nick alice --limit 100
  acp-red-star --nick alice --order oldest --limit 100
  acp-red-star --nick alice --profile-feed top --limit 100
  acp-red-star --search \"general strike\" --format json
  acp-red-star --export all.json
  acp-red-star --export strike.tar.gz --search strike --compress --tar-gz --name strike
  acp-red-star --export picked.html --format html --post-ids A1B2C3D4,E5F6G7H8
  acp-red-star --research-create strike --research-description \"1877 and after\"
  acp-red-star --research-add strike --post-ids A1B2C3D4 --note \"key source\"
  acp-red-star --research-add strike --url https://example.org/article
  acp-red-star --research-add strike --file ~/notes/strike.md
  acp-red-star --research-export strike --export strike.tar.gz --format json,html --compress --tar-gz
  acp-red-star --import strike.tar.gz --compress --tar-gz
  acp-red-star --import all.json
  ACP_DEBUG=1 acp-red-star --nick alice --debug
  acp-red-star --list-nicks
  acp-red-star --list-nicks --format json
  acp-red-star --nick alice --full
  acp-red-star --nick alice --said-by
  \
acp-red-star --nick alice -q
  \
acp-red-star --nick alice -v
  \
acp-red-star --nicks @hazaldin --latest
  \
acp-red-star --nicks @hazaldin --trending --crawl-limit 200
  \
acp-red-star --top week
  \
acp-red-star --top all --top-replies
  \
acp-red-star --eb
  \
acp-red-star --eb --eb-replies
  \
acp-red-star --explore
  \
acp-red-star --explore-latest
  \
acp-red-star --site-search strike
  \
acp-red-star --tag strike
  \
acp-red-star --backfill-tags
  \
acp-red-star --backfill-tags --nick alice
  \
acp-red-star --sort-thread THREAD --thread-sort oldest
  \
acp-red-star --sort-thread THREAD --thread-sort top
  \
acp-red-star --reposts POST
  \
acp-red-star --edits THREAD/POST
  \
acp-red-star --identity-init
  \
acp-red-star --show-phrase
  \
acp-red-star --acp-id
  \
acp-red-star --shares-print
  \
acp-red-star --shares-verify --shares-file shares.txt
  \
acp-red-star --backup --output backup.acpbackup
  \
acp-red-star --restore backup.acpbackup
  \
acp-red-star --backup-schedule weekly --keep 4 --dest ~/backups
"),
)]
#[command(group(clap::ArgGroup::new("compress_format").multiple(false)))]
#[command(group(clap::ArgGroup::new("mode").multiple(false)))]
#[command(group(clap::ArgGroup::new("maint").args(["remove_nick", "clear_all"]).multiple(false)))]
#[command(group(clap::ArgGroup::new("research").args(["research_list", "research_create", "research_add", "research_show", "research_export"]).multiple(false)))]
#[command(group(clap::ArgGroup::new("share_action").args(["shares_print", "shares_verify"]).multiple(false)))]
#[command(group(clap::ArgGroup::new("confirmable").args(["remove_nick", "clear_all", "new_post", "reply", "vote", "subscribe", "delete"]).multiple(true)))]
#[command(group(clap::ArgGroup::new("writing").args(["new_post", "reply", "capture_reply", "vote", "subscribe", "delete"]).multiple(false).conflicts_with_all([
    "remove_nick", "clear_all", "scrape", "nick", "nicks", "logout", "sync", "sync_bookmarks",
])))]
#[command(group(clap::ArgGroup::new("reading").args(["top", "eb", "explore", "explore_latest", "site_search", "tag", "sort_thread", "reposts", "edits"]).multiple(false).conflicts_with("writing").conflicts_with_all([
    "nick", "nicks", "scrape", "latest", "trending", "search", "export", "import", "sync", "sync_bookmarks",
    "list_nicks", "remove_nick", "clear_all", "bookmark", "unbookmark", "list_bookmarks", "refetch_truncated",
    "logout", "post_ids", "research_list", "research_create", "research_add", "research_show", "research_export",
])))]
/// Command-line arguments (see `--help`).
pub struct Args {
    /// Site feed: Latest tab (/rs/latest). Default — even though the site opens on Trending.
    #[arg(long, group = "mode", help_heading = "Mode")]
    pub latest: bool,
    /// Site feed: Trending tab (/rs). Opt-in; incremental early-stop disabled (not chronological).
    #[arg(long, group = "mode", help_heading = "Mode")]
    pub trending: bool,
    /// Site-wide Top feed (/rs/top). PERIOD is day, week, month, year or all (default: week). Stored locally; View › Site Top opens it.
    #[arg(long, value_name = "PERIOD", num_args = 0..=1, default_missing_value = "week", value_parser = parse_top_period, help_heading = "Reading")]
    pub top: Option<TopPeriod>,
    /// With --top: include replies (the feed's replies filter).
    #[arg(long, requires = "top", help_heading = "Reading")]
    pub top_replies: bool,
    /// Executive Board feed (/rs/eb). Stored locally; the EB row of the nick bar opens it.
    #[arg(long, help_heading = "Reading")]
    pub eb: bool,
    /// With --eb: include replies (the feed's replies filter).
    #[arg(long, requires = "eb", help_heading = "Reading")]
    pub eb_replies: bool,
    /// Explore feed (/rs/explore). Stored locally; the Explore row of the nick bar opens it.
    #[arg(long, help_heading = "Reading")]
    pub explore: bool,
    /// Explore's Latest tab (/rs/explore/latest).
    #[arg(long, help_heading = "Reading")]
    pub explore_latest: bool,
    /// Search the site (`/rs/explore?q=`). The Search modal's Where: Site does this too.
    #[arg(long, value_name = "TEXT", help_heading = "Reading")]
    pub site_search: Option<String>,
    /// Tag feed (`/rs/tag/TAG`). A `#tag` in a post opens the same view. Stored locally.
    #[arg(long, value_name = "TAG", help_heading = "Reading")]
    pub tag: Option<String>,
    /// Fill `post_tags` from stored bodies, then re-read threads whose ProseMirror JSON is still empty. `--nick` / `--nicks` limit the rows. Does not change stored text.
    #[arg(long, conflicts_with = "reading", help_heading = "Reading")]
    pub backfill_tags: bool,
    /// Re-read a thread after the `sort` hook (`oldest` or `top`) and store scores. `s` in a thread does this.
    #[arg(long, value_name = "ID", requires = "thread_sort", help_heading = "Reading")]
    pub sort_thread: Option<String>,
    /// With `--sort-thread`: `oldest` or `top`. Newest is the site default and sends no hook.
    #[arg(long, value_name = "ORDER", requires = "sort_thread", value_parser = ["oldest", "top"], help_heading = "Reading")]
    pub thread_sort: Option<String>,
    /// Reposts of a post (`/rs/post/ID/reposts`). The post menu (`p`) › View reposts does this.
    #[arg(long, value_name = "ID", help_heading = "Reading")]
    pub reposts: Option<String>,
    /// Edit history (`/rs/post/THREAD/POST/edits`). THREAD/POST, or a post id already stored. The post menu (`p`) › View edits does this.
    #[arg(long, value_name = "THREAD/POST", help_heading = "Reading")]
    pub edits: Option<String>,

    /// New post. The body is `--body` or `--body-file`. Publishing asks first (Cancel is the default) and does not send until a guided capture confirms the payload. `--dry-run` prints the frames and does not connect.
    #[arg(long, value_name = "TITLE", help_heading = "Writing")]
    pub new_post: Option<String>,
    /// Reply in a thread (`/rs/post/THREAD`). `--in-reply-to` pre-fills a comment reference. Same confirm and dry-run rules as `--new-post`.
    #[arg(long, value_name = "THREAD", help_heading = "Writing")]
    pub reply: Option<String>,
    /// Delete a post. The terminal asks you to type yes. `--yes` confirms. `--dry-run` prints the documented hook and does not connect. A confirmed delete does not send: the live frame has not been captured.
    #[arg(long, value_name = "POST", help_heading = "Writing")]
    pub delete: Option<String>,
    /// Subscribe to a thread. STATE is `1` (subscribe) or `0` (clear). The default is `1`. `--dry-run` prints the documented hook and does not connect. A confirmed subscribe does not send: the live frame has not been captured.
    #[arg(long, value_name = "THREAD", help_heading = "Writing")]
    pub subscribe: Option<String>,
    /// With `--subscribe`. `1` or `0`.
    #[arg(long, value_name = "STATE", requires = "subscribe", value_parser = ["1", "0"], help_heading = "Writing")]
    pub subscribe_state: Option<String>,
    /// Vote on a post. STATE is `1` (upvote), `0` (clear), or `-1` (downvote). The default is `1`. `--dry-run` prints the documented hook and does not connect. A confirmed vote does not send: the live frame has not been captured.
    #[arg(long, value_name = "POST", help_heading = "Writing")]
    pub vote: Option<String>,
    /// With `--vote`. `1`, `0`, or `-1`.
    #[arg(long, value_name = "STATE", requires = "vote", value_parser = ["1", "0", "-1"], help_heading = "Writing")]
    pub vote_state: Option<String>,
    /// Markdown body for `--new-post` or `--reply`.
    #[arg(long, value_name = "MARKDOWN", help_heading = "Writing")]
    pub body: Option<String>,
    /// Read the Markdown body from a file.
    #[arg(long, value_name = "PATH", help_heading = "Writing")]
    pub body_file: Option<PathBuf>,
    /// With `--reply`: the comment this answers. A reply to the thread root omits this.
    #[arg(long, value_name = "POST", requires = "reply", help_heading = "Writing")]
    pub in_reply_to: Option<String>,
    /// Selected text, inserted as a blockquote. With `--reply`.
    #[arg(long, value_name = "TEXT", help_heading = "Writing")]
    pub quote: Option<String>,
    /// Post the quote refers to. With `--reply` this also adds a post reference.
    #[arg(long, value_name = "POST", requires = "reply", help_heading = "Writing")]
    pub quote_post: Option<String>,
    /// Allow writes after the site's script URL changes from the one stored in `site_state`.
    #[arg(long, help_heading = "Writing")]
    pub accept_site_update: bool,
    /// Local file for `--new-post` or `--reply`. Repeat for more than one. Every file stays on the local copy, including audio and documents. Red Star takes up to 4 images or videos (image/* and video/*) on the site copy; other types stay local. `--dry-run` lists them and does not upload. A confirmed publish still does not send. See --site-attachments
    #[arg(long, value_name = "PATH", help_heading = "Writing")]
    pub attachment: Vec<PathBuf>,
    /// Which `--attachment` files go to Red Star. LIST is 1-based indexes, comma-separated, for example 1,3,5,6. At most 4, and each index has to be an image or a video. Naming an audio file, a pdf, or another type is an error that names that file. Omit it and the first 4 images or videos are the site copy; other types are skipped. A notice names files left off when there are more than 4 images or videos.
    #[arg(long, value_name = "LIST", value_parser = parse_site_attachments, help_heading = "Writing")]
    pub site_attachments: Option<SiteAttachmentList>,
    /// Guided capture of one reply. Documented in architecture/PLAN-3.0-CLIENT.md. Does not publish.
    #[arg(long, value_name = "THREAD", hide = true)]
    pub capture_reply: Option<String>,
    /// Hidden.
    #[arg(long, hide = true)]
    pub roar: bool,

    /// Open the terminal UI (also the default when no other flag is given). Other flags pre-fill it: --nick/--nicks select those nicks, -v/--debug/--enable-logging apply to scans started from it.
    #[arg(long, help_heading = "Interface")]
    pub tui: bool,
    /// Use this theme (a file in ~/.acp/themes, without .toml) in the terminal UI for this run; ui.theme in preferences.toml sets it for good.
    #[arg(long, value_name = "NAME", help_heading = "Interface")]
    pub theme: Option<String>,
    /// List the themes in ~/.acp/themes (● marks the one in use; broken files say why) and exit.
    #[arg(long, help_heading = "Interface")]
    pub list_themes: bool,
    /// Download a theme from the acp-red-star repo into ~/.acp/themes and exit: NAME (default: default), one of the shipped themes (default, soviet, constructivist, cuba, red-star, chinese-poster, plattenbau, rs, acp), or all. Never replaces an existing file.
    #[arg(long, value_name = "NAME", num_args = 0..=1, default_missing_value = "default", help_heading = "Interface")]
    pub theme_init: Option<String>,
    /// Write the minimal starter theme (plain terminal colours, no download) as ~/.acp/themes/NAME.toml and exit. Never replaces an existing file.
    #[arg(long, value_name = "NAME", help_heading = "Interface")]
    pub theme_starter: Option<String>,
    /// Colour output. auto follows NO_COLOR (a non-empty value turns colours off). always forces colours on, even when NO_COLOR is set. never turns colours off. Omitted: [ui] color in preferences.toml, or auto.
    #[arg(long, value_name = "WHEN", value_parser = parse_color_mode, help_heading = "Interface")]
    pub color: Option<ColorMode>,
    /// Scrape headlessly, without the terminal UI (what a bare invocation did before 2.0). Asks for nicks when no --nick/--nicks is given. Implied by any other scraping flag.
    #[arg(long, help_heading = "Scraping")]
    pub scrape: bool,

    /// Single nick (alias for --nicks). With --scrape and neither given, you are asked.
    #[arg(long, help_heading = "Scraping", conflicts_with = "nicks")]
    pub nick: Option<String>,
    /// Comma-separated nicks, e.g. "@hazaldin,@m3talsmith" (@ optional, trimmed, deduped).
    #[arg(long, value_parser = parse_nicks, value_name = "LIST", help_heading = "Scraping")]
    pub nicks: Option<NickList>,
    /// Find posts that mention each nick (default: on). Crawls the site feed (--latest/--trending); no server search in the JS.
    #[arg(long, default_value_t = true, action = clap::ArgAction::SetTrue, help_heading = "Scraping")]
    #[arg(long = "no-mentions", action = clap::ArgAction::SetFalse, help_heading = "Scraping",
          help = "Skip the mentions crawl")]
    pub mentions: bool,
    /// Only what the nick said: their own top-level posts AND their replies/comments (Posts + "Posts and replies" feeds). Exports list the posts first, then replies, each marked as Post/Reply. Skips other people's replies in the nick's threads and the mentions crawl; a thread page is read only to recover a blank body of the nick's own post.
    #[arg(long, help_heading = "Scraping")]
    pub said_by: bool,
    /// Complete bodies stored shortened: re-read the thread page (the full text "show more" opens) of every stored row not marked complete — rows saved from a truncated feed item, and rows stored before 1.26.0 — for the given nicks (or every nick when none is given; then nothing else is scraped). New scrapes complete truncated items on their own.
    #[arg(long, help_heading = "Scraping",
          conflicts_with_all = ["import", "export", "search", "list_nicks", "remove_nick", "clear_all", "logout"])]
    pub refetch_truncated: bool,
    /// How many profile items to keep from each nick. With --order newest (the default) this is the start of the feed: the newest posts, or the top of --profile-feed top. With --order oldest this is the end of that feed. Omit it to scrape the whole profile. Replies in those threads are still fetched unless --said-by. --crawl-limit still only bounds the mentions scan.
    #[arg(long, value_name = "N", value_parser = parse_post_limit, help_heading = "Scraping")]
    pub limit: Option<usize>,
    /// Which profile feed a nick scrape reads. newest (default) is /rs/@nick plus Posts and replies (?replies=1). top is the profile Top feed (/rs/@nick/top?t=all) only. Other Top periods were not captured for profiles. This is not --latest / --trending, which still choose the mentions crawl.
    #[arg(long, value_name = "FEED", default_value = "newest", value_parser = parse_profile_feed, help_heading = "Scraping")]
    pub profile_feed: ProfileFeed,
    /// Which end of that profile feed --limit keeps. newest (default) keeps the start of the site's list. oldest keeps the end, so a limited scrape reads the whole feed first. On newest that is the oldest posts. On top that is the end of the Top ranking. The site has no oldest-first profile URL. The terminal UI uses this same window for each list and for a search.
    #[arg(long, value_name = "ORDER", default_value = "newest", value_parser = parse_scrape_order, help_heading = "Scraping")]
    pub order: ScrapeOrder,
    /// Max site-feed posts to scan when hunting mentions (default 500).
    #[arg(long, default_value_t = DEFAULT_CRAWL_LIMIT, help_heading = "Scraping")]
    pub crawl_limit: usize,
    /// Skip downloading media attachments.
    #[arg(long, help_heading = "Scraping")]
    pub no_attachments: bool,
    /// Force a full re-crawl (ignore already-known post_ids when paging / refetching threads).
    #[arg(long, help_heading = "Scraping")]
    pub full: bool,
    /// Offline: list known nicks from the DB (posts + profiles).
    #[arg(long, help_heading = "Search / Export")]
    pub list_nicks: bool,

    /// Search the local DB (title + content); exports matches + parent/children.
    #[arg(long, help_heading = "Search / Export")]
    pub search: Option<String>,
    /// Query and filters for an export of a search, as JSON. Not a snapshot of the results.
    /// A full dump, a nick filter, and a basket do not use it.
    #[arg(long, value_name = "JSON", help_heading = "Search / Export", conflicts_with_all = ["post_ids", "import"])]
    pub search_json: Option<String>,
    /// Offline: dump the local DB (all / --nick(s) / --search) to FILE. Replaces --output for DB dumps.
    #[arg(long, value_name = "FILE", help_heading = "Search / Export", conflicts_with_all = ["output", "import"])]
    pub export: Option<PathBuf>,
    /// Offline: export exactly these posts and replies (comma-separated post ids), with their attachments — to --export FILE, or stdout / --output. The TUI's export basket uses it.
    #[arg(long, value_name = "IDS", value_parser = parse_post_id_arg, help_heading = "Search / Export",
          conflicts_with_all = ["search", "nick", "nicks", "import", "bookmark", "unbookmark", "list_bookmarks", "sync", "sync_bookmarks", "list_nicks"])]
    pub post_ids: Option<PostIds>,
    /// Export name: basename, archive name, folder inside archive, HTML title.
    #[arg(long, help_heading = "Search / Export")]
    pub name: Option<String>,

    /// Offline: import a previous export into the DB. Add --compress --tar-gz for archives.
    #[arg(long, value_name = "FILE", help_heading = "Import",
          conflicts_with_all = ["export", "search", "nick", "nicks", "output", "format", "name", "no_attachments", "mentions", "crawl_limit", "limit", "profile_feed", "order", "full", "list_nicks", "said_by"])]
    pub import: Option<PathBuf>,

    /// Export format(s): comma-separated or repeated (e.g. json,html). Several formats need --output, --export or --compress.
    #[arg(long, value_enum, value_delimiter = ',', default_values_t = [Format::Json], help_heading = "Output")]
    pub format: Vec<Format>,
    /// Output file (with --compress: the archive path). Several formats: <stem>.<ext> each, or <dir>/<name>.<ext>. Default: stdout.
    #[arg(long, help_heading = "Output")]
    pub output: Option<PathBuf>,

    /// Compress the export; requires a format flag (e.g. --tar-gz).
    #[arg(long, requires = "compress_format", help_heading = "Compression")]
    pub compress: bool,
    /// Compression format: gzip-compressed tar (.tar.gz). Requires --compress.
    #[arg(long, group = "compress_format", requires = "compress", help_heading = "Compression")]
    pub tar_gz: bool,

    /// Offline: delete these nicks (comma-separated, @ optional) — their posts, replies in their threads, attachments, mentions and profiles.
    #[arg(long, value_parser = parse_nicks, value_name = "LIST", help_heading = "Maintenance",
          conflicts_with_all = ["latest", "trending", "nick", "nicks", "mentions", "crawl_limit", "limit", "profile_feed", "order", "no_attachments", "full", "said_by", "list_nicks", "search", "export", "name", "import", "format", "output", "compress", "tar_gz"])]
    pub remove_nick: Option<NickList>,
    /// With --remove-nick: keep other people's replies in the removed nicks' threads (they become orphans).
    #[arg(long, requires = "remove_nick", conflicts_with = "clear_all", help_heading = "Maintenance")]
    pub keep_replies: bool,
    /// Offline: delete the whole database and the attachments directory (preferences.toml and exports are kept).
    #[arg(long, help_heading = "Maintenance", conflicts_with_all = ["latest", "trending", "nick", "nicks", "mentions", "crawl_limit", "limit", "profile_feed", "order", "no_attachments", "full", "said_by", "list_nicks", "search", "export", "name", "import", "format", "output", "compress", "tar_gz"])]
    pub clear_all: bool,
    /// Offline: forget the saved login (delete ~/.acp/session.json); the next scrape asks for a code again.
    #[arg(long, help_heading = "Session", conflicts_with_all = ["latest", "trending", "nick", "nicks", "mentions", "crawl_limit", "limit", "profile_feed", "order", "no_attachments", "full", "said_by", "list_nicks", "search", "export", "name", "import", "format", "output", "compress", "tar_gz", "remove_nick", "clear_all"])]
    pub logout: bool,
    /// Offline: bookmark these posts locally (comma-separated post ids). --sync-bookmarks pushes them to the site.
    #[arg(long, value_name = "IDS", value_parser = parse_post_id_arg, help_heading = "Bookmarks",
          conflicts_with_all = ["unbookmark", "nick", "nicks", "search", "export", "import", "list_nicks", "remove_nick", "clear_all", "logout", "refetch_truncated"])]
    pub bookmark: Option<PostIds>,
    /// Offline: remove these bookmarks (comma-separated post ids); ones the site has are removed there by the next --sync-bookmarks.
    #[arg(long, value_name = "IDS", value_parser = parse_post_id_arg, help_heading = "Bookmarks",
          conflicts_with_all = ["nick", "nicks", "search", "export", "import", "list_nicks", "remove_nick", "clear_all", "logout", "refetch_truncated"])]
    pub unbookmark: Option<PostIds>,
    /// Offline: list bookmarks (add --format json for JSON).
    #[arg(long, help_heading = "Bookmarks", conflicts_with_all = ["nick", "nicks", "search", "export", "import", "list_nicks", "remove_nick", "clear_all"])]
    pub list_bookmarks: bool,
    /// Log in, read the site's bookmarks (storing their posts) and push local bookmark changes to the site.
    #[arg(long, help_heading = "Bookmarks", conflicts_with_all = ["search", "export", "import", "list_nicks", "remove_nick", "clear_all", "logout"])]
    pub sync_bookmarks: bool,
    /// Log in and bring the database up to date with the site: sync bookmarks (unless --no-bookmarks), then an incremental scrape of --nick/--nicks if given. Writes no export unless --output is set; never prompts for nicks.
    #[arg(long, help_heading = "Sync", conflicts_with_all = ["search", "export", "import", "list_nicks", "remove_nick", "clear_all", "logout", "list_bookmarks", "bookmark", "unbookmark"])]
    pub sync: bool,
    /// With --sync: skip the bookmark sync.
    #[arg(long, help_heading = "Sync", requires = "sync")]
    pub no_bookmarks: bool,
    /// Offline: list research projects (name, items, created, description); --format json for JSON.
    #[arg(long, help_heading = "Research", conflicts_with_all = ["nick", "nicks", "search", "export", "import", "post_ids"])]
    pub research_list: bool,
    /// Offline: create a research project NAME (with --research-description).
    #[arg(long, value_name = "NAME", help_heading = "Research", conflicts_with_all = ["nick", "nicks", "search", "export", "import", "post_ids"])]
    pub research_create: Option<String>,
    /// Description for --research-create (or to set when --research-add creates the project).
    #[arg(long, value_name = "TEXT", help_heading = "Research")]
    pub research_description: Option<String>,
    /// Add to research project NAME (created if missing): --post-ids IDS, --search TEXT [--nicks] (the matches), --file PATH or --url URL; with --note.
    #[arg(long, value_name = "NAME", help_heading = "Research", conflicts_with_all = ["export", "import"])]
    pub research_add: Option<String>,
    /// With --research-add: a local file to add (copied to ~/.acp/research/<project>/files/ unless --no-copy).
    #[arg(long, value_name = "PATH", help_heading = "Research", requires = "research_add")]
    pub file: Option<PathBuf>,
    /// With --file: reference the file where it is instead of copying it.
    #[arg(long, help_heading = "Research", requires = "file")]
    pub no_copy: bool,
    /// With --research-add: fetch this web page (no acp.us cookies) and keep its title, readable text and an HTML snapshot.
    #[arg(long, value_name = "URL", help_heading = "Research", requires = "research_add")]
    pub url: Option<String>,
    /// With --research-add: a note for the added items.
    #[arg(long, value_name = "TEXT", help_heading = "Research", requires = "research_add")]
    pub note: Option<String>,
    /// Offline: show a research project's items (--format json for JSON).
    #[arg(long, value_name = "NAME", help_heading = "Research", conflicts_with_all = ["nick", "nicks", "search", "export", "import", "post_ids"])]
    pub research_show: Option<String>,
    /// Offline: export research project NAME (its posts, files, pages and notes) to --export FILE (--format json,html; --compress --tar-gz bundles the files).
    #[arg(long, value_name = "NAME", help_heading = "Research", requires = "export", conflicts_with_all = ["nick", "nicks", "search", "import", "post_ids"])]
    pub research_export: Option<String>,
    /// With a write: print the documented frames and open no connection. With --remove-nick / --clear-all: show what would be removed and change nothing.
    #[arg(long, requires = "confirmable", help_heading = "Writing")]
    pub dry_run: bool,
    /// Confirm. With a write, this is Publish (the send stays gated). With --remove-nick / --clear-all, skip the prompt. Required when stdin is not a terminal for those maintenance commands.
    #[arg(short = 'y', long, requires = "confirmable", help_heading = "Writing")]
    pub yes: bool,

    /// Packaging helper: print a man page or shell completion script to stdout.
    #[arg(long, value_enum, value_name = "WHAT", hide = true, exclusive = true)]
    pub generate: Option<Generate>,

    /// Suppress progress/summary on stderr (prompts and errors still show).
    #[arg(short = 'q', long, help_heading = "Debug", conflicts_with_all = ["verbose", "debug"])]
    pub quiet: bool,
    /// Extra progress detail (per-page / per-thread / skipped items).
    #[arg(short = 'v', long, help_heading = "Debug", conflicts_with = "quiet")]
    pub verbose: bool,
    /// Raw LiveView wire log (implies --verbose). Also ACP_DEBUG=1.
    #[arg(long, help_heading = "Debug", conflicts_with = "quiet")]
    pub debug: bool,
    /// Also append progress and errors, timestamped, to ~/.acp/logs/success.log and error.log (on with --debug).
    #[arg(long, help_heading = "Debug")]
    pub enable_logging: bool,
    /// Success/progress log file (default ~/.acp/logs/success.log); implies --enable-logging.
    #[arg(long, value_name = "PATH", help_heading = "Debug")]
    pub log: Option<PathBuf>,
    /// Error log file (default ~/.acp/logs/error.log); implies --enable-logging.
    #[arg(long, value_name = "PATH", help_heading = "Debug")]
    pub error_log: Option<PathBuf>,

    /// Create the master secret: a 24-word phrase, a mandatory password, and a pinned acp-id.
    /// The phrase is shown on this terminal. An encrypted copy is kept so --show-phrase can show it again. Needs a terminal.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "acp_id", "shares_print", "shares_verify",
        "backup", "restore", "backup_schedule", "encrypt_db", "change_password", "show_phrase",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub identity_init: bool,
    /// Print the pinned acp-id.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "shares_print", "shares_verify",
        "backup", "restore", "backup_schedule", "encrypt_db", "change_password", "show_phrase",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub acp_id: bool,
    /// Print five SLIP-39 recovery sheets (3 of 5) for the pinned account. Needs a terminal.
    /// Reissuing shares of the same seed does not invalidate older shares.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id", "shares_verify",
        "backup", "restore", "backup_schedule", "encrypt_db", "change_password", "show_phrase",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub shares_print: bool,
    /// Check three to five recovery shares against the pinned acp-id. Writes nothing secret.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id",
        "backup", "restore", "backup_schedule", "encrypt_db", "change_password", "show_phrase",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub shares_verify: bool,
    /// Sheet or paste to read instead of typing shares. Used with --shares-print or --shares-verify.
    #[arg(long, value_name = "FILE", help_heading = "Identity", requires = "share_action")]
    pub shares_file: Option<PathBuf>,

    /// Write an encrypted *.acpbackup. --output FILE chooses the path; otherwise the schedule directory or the exports folder.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id", "shares_print",
        "shares_verify", "restore", "backup_schedule", "change_password", "show_phrase",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub backup: bool,
    /// Restore an *.acpbackup into this home. A new machine also asks for the phrase or 3 shares.
    #[arg(long, value_name = "FILE", help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id", "shares_print",
        "shares_verify", "backup", "backup_schedule", "change_password", "show_phrase",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub restore: Option<PathBuf>,
    /// Remember a daily or weekly backup. The headless service that runs it is a later release.
    #[arg(long, value_name = "WHEN", value_parser = ["daily", "weekly"], requires_all = ["backup_keep", "backup_dest"], help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id", "shares_print",
        "shares_verify", "backup", "restore", "change_password", "show_phrase",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub backup_schedule: Option<String>,
    /// How many *.acpbackup files to keep. Used with --backup-schedule.
    #[arg(long = "keep", value_name = "N", requires = "backup_schedule", help_heading = "Identity")]
    pub backup_keep: Option<u32>,
    /// Directory for scheduled backups. Used with --backup-schedule.
    #[arg(long = "dest", value_name = "DIR", requires = "backup_schedule", help_heading = "Identity")]
    pub backup_dest: Option<PathBuf>,
    /// Encrypt an existing plaintext database. Writes a backup first, then checks counts and text.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id", "shares_print",
        "shares_verify", "backup", "restore", "backup_schedule", "change_password", "show_phrase",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub encrypt_db: bool,
    /// Change the password on an encrypted database. Rewraps the local key and the
    /// account key. The account id stays the same. Needs a terminal.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id", "shares_print",
        "shares_verify", "backup", "restore", "backup_schedule", "encrypt_db", "show_phrase",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub change_password: bool,
    /// Show the 24-word recovery phrase again. Asks for the password on a terminal.
    /// Warns that the words will be on screen and can clear the terminal afterward.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id", "shares_print",
        "shares_verify", "backup", "restore", "backup_schedule", "encrypt_db", "change_password",
        "node_init", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub show_phrase: bool,

    /// Create this machine's node key and certificates. Needs a pinned acp-id.
    /// The node key is generated here. It is not derived from the master.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id", "shares_print",
        "shares_verify", "backup", "restore", "backup_schedule", "encrypt_db", "change_password",
        "show_phrase", "node_id", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub node_init: bool,
    /// Name for --node-init. The default is this computer's hostname.
    #[arg(long, value_name = "NAME", help_heading = "Identity", requires = "node_init")]
    pub node_label: Option<String>,
    /// Print this machine's node name, account id, peer id, and fingerprint.
    #[arg(long, help_heading = "Identity", conflicts_with_all = [
        "latest", "trending", "nick", "nicks", "scrape", "search", "export", "import", "list_nicks",
        "remove_nick", "clear_all", "logout", "bookmark", "unbookmark", "list_bookmarks", "sync",
        "sync_bookmarks", "tui", "top", "eb", "explore", "explore_latest", "site_search", "tag",
        "reposts", "edits", "sort_thread", "backfill_tags", "refetch_truncated", "post_ids",
        "theme_init", "theme_starter", "list_themes", "identity_init", "acp_id", "shares_print",
        "shares_verify", "backup", "restore", "backup_schedule", "encrypt_db", "change_password",
        "show_phrase", "node_init", "new_post", "reply", "capture_reply", "vote", "subscribe", "delete",
    ])]
    pub node_id: bool,
}

/// Targets for the hidden `--generate` packaging flag.
///
/// The `acprs-*` values write the same page or script under the short command
/// name. The running program's own help stays `acp-red-star`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Generate {
    /// roff man page (`acp-red-star.1`).
    Man,
    /// bash completion script.
    Bash,
    /// zsh completion function (`_acp-red-star`).
    Zsh,
    /// fish completion script.
    Fish,
    /// roff man page for the `acprs` command (`acprs.1`).
    #[value(name = "acprs-man")]
    AcprsMan,
    /// bash completion script for `acprs`.
    #[value(name = "acprs-bash")]
    AcprsBash,
    /// zsh completion function for `acprs` (`_acprs`).
    #[value(name = "acprs-zsh")]
    AcprsZsh,
    /// fish completion script for `acprs`.
    #[value(name = "acprs-fish")]
    AcprsFish,
}

impl Generate {
    /// Command name written into the man page or completion script.
    fn bin_name(self) -> &'static str {
        match self {
            Self::AcprsMan | Self::AcprsBash | Self::AcprsZsh | Self::AcprsFish => "acprs",
            Self::Man | Self::Bash | Self::Zsh | Self::Fish => "acp-red-star",
        }
    }

    fn kind(self) -> Self {
        match self {
            Self::AcprsMan => Self::Man,
            Self::AcprsBash => Self::Bash,
            Self::AcprsZsh => Self::Zsh,
            Self::AcprsFish => Self::Fish,
            other => other,
        }
    }
}

/// Write the man page or a completion script for [`Args`] to `out`.
pub fn generate(what: Generate, out: &mut dyn Write) -> Result<()> {
    use clap::CommandFactory;
    let bin = what.bin_name();
    let mut cmd = Args::command();
    cmd.set_bin_name(bin);
    if bin != "acp-red-star" && matches!(what.kind(), Generate::Man) {
        // The man page title follows the command name. Help text at runtime does not.
        cmd = cmd.name(bin);
    }
    match what.kind() {
        Generate::Man => clap_mangen::Man::new(cmd).render(out)?,
        Generate::Bash => clap_complete::generate(clap_complete::Shell::Bash, &mut cmd, bin, out),
        Generate::Zsh => clap_complete::generate(clap_complete::Shell::Zsh, &mut cmd, bin, out),
        Generate::Fish => clap_complete::generate(clap_complete::Shell::Fish, &mut cmd, bin, out),
        Generate::AcprsMan | Generate::AcprsBash | Generate::AcprsZsh | Generate::AcprsFish => {
            unreachable!("kind() maps the acprs values onto the primary ones")
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Which site-wide feed the mentions crawl walks (`--latest` / `--trending`).
pub enum FeedMode {
    /// `/rs/latest`: chronological, so incremental early-stop works (default).
    Latest,
    /// `/rs`: the site's Trending tab; not chronological, no early stop.
    Trending,
}

impl FeedMode {
    /// Site-wide feed URL crawled for mentions in this mode.
    pub fn site_feed_url(self) -> String {
        match self {
            FeedMode::Latest => feed_latest_url(),
            FeedMode::Trending => feed_trending_url(),
        }
    }
    /// Whether paging may stop early at an all-known page (only for chronological feeds).
    pub fn allow_incremental_stop(self) -> bool {
        matches!(self, FeedMode::Latest)
    }
}

impl std::fmt::Display for FeedMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedMode::Latest => write!(f, "latest"),
            FeedMode::Trending => write!(f, "trending"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// A parsed `--nicks` list: `@` stripped, trimmed, deduplicated case-insensitively.
pub struct NickList(pub Vec<String>);

/// Whether coloured output is forced on, forced off, or left to NO_COLOR.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    /// Follow NO_COLOR. A non-empty value turns colours off.
    #[default]
    Auto,
    /// Colours on, even when NO_COLOR is set.
    Always,
    /// Colours off.
    Never,
}

impl ColorMode {
    /// `auto`, `always`, or `never`. Case does not matter.
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(format!("--color must be auto, always, or never, not {s:?}")),
        }
    }
    /// The preferences and menu spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

impl std::fmt::Display for ColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
}

/// Menu label for `mode`.
pub fn color_choice(mode: ColorMode) -> &'static str {
    match mode {
        ColorMode::Auto => "Auto",
        ColorMode::Always => "Always",
        ColorMode::Never => "Never",
    }
}

/// `Auto`, `Always`, or `Never` from a menu choice. Anything else is auto.
pub fn color_from_choice(choice: &str) -> ColorMode {
    ColorMode::parse(choice).unwrap_or_default()
}

/// Flag wins. Otherwise `[ui] color`. Otherwise auto.
pub fn resolve_color_mode(flag: Option<ColorMode>, pref: Option<&str>) -> ColorMode {
    if let Some(mode) = flag { return mode; }
    pref.and_then(|s| ColorMode::parse(s).ok()).unwrap_or_default()
}

/// NO_COLOR is set to a non-empty value (<https://no-color.org/>).
pub fn no_color_set() -> bool {
    std::env::var("NO_COLOR").ok().is_some_and(|v| !v.is_empty())
}

/// Colours are off for this mode when `no_color` is the NO_COLOR check.
pub fn colors_suppressed(mode: ColorMode, no_color: bool) -> bool {
    match mode {
        ColorMode::Always => false,
        ColorMode::Never => true,
        ColorMode::Auto => no_color,
    }
}

/// Shown when NO_COLOR is set and colours are off.
pub const NO_COLOR_HINT: &str = "NO_COLOR is set, so colours are off (use --color always). Help: color";

/// The hint, when NO_COLOR is set and this mode leaves colours off.
pub fn no_color_hint(mode: ColorMode, no_color: bool) -> Option<&'static str> {
    (no_color && colors_suppressed(mode, no_color)).then_some(NO_COLOR_HINT)
}

static COLOR_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn color_mode_code(mode: ColorMode) -> u8 {
    match mode { ColorMode::Auto => 0, ColorMode::Always => 1, ColorMode::Never => 2 }
}

/// The mode last passed to [`apply_color_mode`]. Auto until then.
pub fn current_color_mode() -> ColorMode {
    match COLOR_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ColorMode::Always,
        2 => ColorMode::Never,
        _ => ColorMode::Auto,
    }
}

/// Apply `mode` for this process. `always` forces colour on. `never` forces it off.
/// `auto` follows NO_COLOR. Call this before `enable_raw_mode`.
pub fn apply_color_mode(mode: ColorMode) {
    COLOR_MODE.store(color_mode_code(mode), std::sync::atomic::Ordering::Relaxed);
    let enabled = match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => !no_color_set(),
    };
    crossterm::style::force_color_output(enabled);
}

/// `--profile-feed` is `newest` or `top`.
pub fn parse_profile_feed(s: &str) -> std::result::Result<ProfileFeed, String> {
    ProfileFeed::parse(s)
}

/// `--order` is `newest` or `oldest`.
pub fn parse_scrape_order(s: &str) -> std::result::Result<ScrapeOrder, String> {
    ScrapeOrder::parse(s)
}

/// 0-based attachment indexes chosen for the Red Star copy (`--site-attachments`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteAttachmentList(pub Vec<usize>);

impl std::ops::Deref for SiteAttachmentList {
    type Target = [usize];
    fn deref(&self) -> &Self::Target { &self.0 }
}

/// `--site-attachments`: 1-based indexes, at most [`SITE_MAX_ATTACHMENTS`], no duplicates. Stored 0-based.
pub fn parse_site_attachments(s: &str) -> std::result::Result<SiteAttachmentList, String> {
    let max = SITE_MAX_ATTACHMENTS;
    if s.trim().is_empty() {
        return Err(format!("--site-attachments needs a list such as 1,3,5,{max}"));
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for tok in s.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            return Err("--site-attachments needs numbers separated by commas".into());
        }
        let n: usize = tok.parse().map_err(|_| format!("--site-attachments needs numbers, not {tok:?}"))?;
        if n == 0 {
            return Err("attachment indexes start at 1".into());
        }
        if !seen.insert(n) {
            return Err(format!("attachment {n} is listed twice"));
        }
        out.push(n - 1);
    }
    if out.len() > max {
        return Err(format!("Red Star allows {max} attachments on the site copy; {} were listed", out.len()));
    }
    Ok(SiteAttachmentList(out))
}

/// `--color` is `auto`, `always`, or `never`.
pub fn parse_color_mode(s: &str) -> std::result::Result<ColorMode, String> {
    ColorMode::parse(s)
}

/// `--limit` must be a whole number of at least 1.
pub fn parse_post_limit(s: &str) -> std::result::Result<usize, String> {
    let n: usize = s.parse().map_err(|_| format!("--limit must be a positive number, not {s:?}"))?;
    if n == 0 { return Err("--limit must be at least 1".into()); }
    Ok(n)
}

/// "@a, b,,@A" -> ["a", "b"]: strip @, trim, drop empties, dedupe (case-insensitive, first wins).
pub fn parse_nicks(s: &str) -> std::result::Result<NickList, String> {
    let mut seen = HashSet::new();
    let v: Vec<String> = s.split(',')
        .map(|n| n.trim().trim_start_matches('@').trim().to_string())
        .filter(|n| !n.is_empty() && seen.insert(n.to_lowercase()))
        .collect();
    if v.is_empty() { return Err("expected at least one nick".into()); }
    if let Some(bad) = v.iter().find(|n| n.contains(char::is_whitespace) || n.contains('/')) {
        return Err(format!("invalid nick {bad:?}"));
    }
    Ok(NickList(v))
}

/// Default export name for a set of nicks.
pub fn nicks_name(nicks: &[String], stamp: &str) -> String {
    let j = nicks.join("+");
    if nicks.len() > 4 || j.len() > 40 { format!("multi-{stamp}") } else { format!("{j}-{stamp}") }
}

/// Whether these parsed arguments open the terminal UI: `--tui`, or no flag at all
/// other than the logging ones in [`TUI_NEUTRAL_ARGS`].
pub fn wants_tui(m: &clap::ArgMatches) -> bool {
    use clap::CommandFactory;
    if m.get_flag("tui") { return true; }
    let cmd = Args::command();
    !cmd.get_arguments().map(|a| a.get_id().as_str())
        .filter(|id| !TUI_NEUTRAL_ARGS.contains(id))
        .any(|id| m.value_source(id) == Some(clap::parser::ValueSource::CommandLine))
}

/// Parse `argv` into [`Args`] plus whether it opens the terminal UI ([`wants_tui`]).
pub fn parse_with_mode<I, T>(argv: I) -> std::result::Result<(Args, bool), clap::Error>
where I: IntoIterator<Item = T>, T: Into<std::ffi::OsString> + Clone {
    use clap::{CommandFactory, FromArgMatches};
    let m = Args::command().try_get_matches_from(argv)?;
    let tui = wants_tui(&m);
    Ok((Args::from_arg_matches(&m)?, tui))
}

impl Args {
    /// --nick / --nicks merged (empty = none given).
    pub fn nick_list(&self) -> Result<Vec<String>> {
        if let Some(n) = &self.nicks { return Ok(n.0.clone()); }
        match &self.nick { Some(n) => Ok(parse_nicks(n).map_err(|e| anyhow!(e))?.0), None => Ok(Vec::new()) }
    }

    /// Selected archive format (clap guarantees --compress <=> exactly one format flag).
    pub fn compression(&self) -> Option<Compress> {
        if !self.compress { return None; }
        if self.tar_gz { Some(Compress::TarGz) } else { None }
    }
    /// Selected export formats, deduplicated in command-line order (never empty).
    pub fn formats(&self) -> Vec<Format> {
        let mut v: Vec<Format> = Vec::new();
        for f in &self.format { if !v.contains(f) { v.push(*f); } }
        if v.is_empty() { v.push(Format::Json); }
        v
    }

    /// Whether the mentions crawl should run (`--no-mentions` turns it off).
    pub fn want_mentions(&self) -> bool { self.mentions && !self.said_by }
    /// Default is Latest (our default), even though the site lands on Trending.
    pub fn feed_mode(&self) -> FeedMode {
        if self.trending { FeedMode::Trending } else { FeedMode::Latest }
    }
}

#[cfg(test)]
mod tests;
