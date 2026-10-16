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
    name = "acp-rs-scraper",
    version,
    about = "Scrape Red Star (acp.us/rs) posts into SQLite and export them",
    long_about = "Run with no flags (or --tui) for the terminal UI: browse every nick and post in the local database, open post details and the posts they link, search, and start scans, exports and imports from a menu bar. With flags it runs headless: log in to https://acp.us/rs as Firefox, scrape posts (and optionally mentions + profiles) for one or more nicks into a local SQLite database, download attachments, and export JSON/HTML — or work entirely offline with --search / --export / --import.",
    after_help = "Examples:
  acp-rs-scraper                      # terminal UI
  acp-rs-scraper --tui --nicks @hazaldin -v
  acp-rs-scraper --scrape             # headless scrape; asks for nicks
  acp-rs-scraper --nicks @hazaldin,@m3talsmith --format html --compress --tar-gz
  acp-rs-scraper --nick @hazaldin --format json,html --output hazaldin/hazaldin.json
  acp-rs-scraper --nick alice --no-mentions --crawl-limit 200
  acp-rs-scraper --search \"general strike\" --format json
  acp-rs-scraper --export all.json
  acp-rs-scraper --export strike.tar.gz --search strike --compress --tar-gz --name strike
  acp-rs-scraper --import strike.tar.gz --compress --tar-gz
  acp-rs-scraper --import all.json
  ACP_DEBUG=1 acp-rs-scraper --nick alice --debug
  acp-rs-scraper --list-nicks
  acp-rs-scraper --list-nicks --format json
  acp-rs-scraper --nick alice --full
  acp-rs-scraper --nick alice --said-by
  \
acp-rs-scraper --nick alice -q
  \
acp-rs-scraper --nick alice -v
  \
acp-rs-scraper --nicks @hazaldin --latest
  \
acp-rs-scraper --nicks @hazaldin --trending --crawl-limit 200
",
)]
#[command(group(clap::ArgGroup::new("compress_format").multiple(false)))]
#[command(group(clap::ArgGroup::new("mode").multiple(false)))]
#[command(group(clap::ArgGroup::new("maint").args(["remove_nick", "clear_all"]).multiple(false)))]
/// Command-line arguments (see `--help`).
pub struct Args {
    /// Site feed: Latest tab (/rs/latest). Default — even though the site opens on Trending.
    #[arg(long, group = "mode", help_heading = "Mode")]
    pub latest: bool,
    /// Site feed: Trending tab (/rs). Opt-in; incremental early-stop disabled (not chronological).
    #[arg(long, group = "mode", help_heading = "Mode")]
    pub trending: bool,

    /// Open the terminal UI (also the default when no other flag is given). Other flags pre-fill it: --nick/--nicks select those nicks, -v/--debug/--enable-logging apply to scans started from it.
    #[arg(long, help_heading = "Interface")]
    pub tui: bool,
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
    /// Offline: dump the local DB (all / --nick(s) / --search) to FILE. Replaces --output for DB dumps.
    #[arg(long, value_name = "FILE", help_heading = "Search / Export", conflicts_with_all = ["output", "import"])]
    pub export: Option<PathBuf>,
    /// Export name: basename, archive name, folder inside archive, HTML title.
    #[arg(long, help_heading = "Search / Export")]
    pub name: Option<String>,

    /// Offline: import a previous export into the DB. Add --compress --tar-gz for archives.
    #[arg(long, value_name = "FILE", help_heading = "Import",
          conflicts_with_all = ["export", "search", "nick", "nicks", "output", "format", "name", "no_attachments", "mentions", "crawl_limit", "full", "list_nicks", "said_by"])]
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
          conflicts_with_all = ["latest", "trending", "nick", "nicks", "mentions", "crawl_limit", "no_attachments", "full", "said_by", "list_nicks", "search", "export", "name", "import", "format", "output", "compress", "tar_gz"])]
    pub remove_nick: Option<NickList>,
    /// With --remove-nick: keep other people's replies in the removed nicks' threads (they become orphans).
    #[arg(long, requires = "remove_nick", conflicts_with = "clear_all", help_heading = "Maintenance")]
    pub keep_replies: bool,
    /// Offline: delete the whole database and the attachments directory (preferences.toml and exports are kept).
    #[arg(long, help_heading = "Maintenance", conflicts_with_all = ["latest", "trending", "nick", "nicks", "mentions", "crawl_limit", "no_attachments", "full", "said_by", "list_nicks", "search", "export", "name", "import", "format", "output", "compress", "tar_gz"])]
    pub clear_all: bool,
    /// Offline: forget the saved login (delete ~/.acp/session.json); the next scrape asks for a code again.
    #[arg(long, help_heading = "Session", conflicts_with_all = ["latest", "trending", "nick", "nicks", "mentions", "crawl_limit", "no_attachments", "full", "said_by", "list_nicks", "search", "export", "name", "import", "format", "output", "compress", "tar_gz", "remove_nick", "clear_all"])]
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
    /// With --remove-nick / --clear-all: show what would be removed and change nothing.
    #[arg(long, requires = "maint", help_heading = "Maintenance")]
    pub dry_run: bool,
    /// With --remove-nick / --clear-all: skip the confirmation prompt (required when stdin is not a terminal).
    #[arg(short = 'y', long, requires = "maint", help_heading = "Maintenance")]
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
}

/// Targets for the hidden `--generate` packaging flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Generate {
    /// roff man page (`acp-rs-scraper.1`).
    Man,
    /// bash completion script.
    Bash,
    /// zsh completion function (`_acp-rs-scraper`).
    Zsh,
    /// fish completion script.
    Fish,
}

/// Write the man page or a completion script for [`Args`] to `out`.
pub fn generate(what: Generate, out: &mut dyn Write) -> Result<()> {
    use clap::CommandFactory;
    let mut cmd = Args::command();
    let name = cmd.get_name().to_string();
    match what {
        Generate::Man => clap_mangen::Man::new(cmd).render(out)?,
        Generate::Bash => clap_complete::generate(clap_complete::Shell::Bash, &mut cmd, name, out),
        Generate::Zsh => clap_complete::generate(clap_complete::Shell::Zsh, &mut cmd, name, out),
        Generate::Fish => clap_complete::generate(clap_complete::Shell::Fish, &mut cmd, name, out),
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
