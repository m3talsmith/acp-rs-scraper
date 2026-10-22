# acp-red-star: usage guide

Run `acp-red-star` with no flags (or `--tui`) for the [terminal UI](#terminal-ui). With flags, `acp-red-star` logs in to the Red Star forum (https://acp.us/rs) as Firefox and scrapes one or more nicks' posts, the replies in their threads, the posts that mention them, and their profiles into a local SQLite database. It downloads attachments and exports to JSON or HTML, optionally compressed. Search, export, import, `--list-nicks`, `--remove-nick` and `--clear-all` run offline.

**v3 pre-launch write-up (historical draft, coming soon):** [redstar-v3-prelaunch-post.md](history/v3/redstar-v3-prelaunch-post.md). The launch post, [redstar-v3-launch-post.md](history/v3/redstar-v3-launch-post.md), stays unpublished until 3.x has shipped. At the 3.0 release, switch this link to the launch post.

- [Install and build](#install-and-build)
- [Terminal UI](#terminal-ui)
- [First run and login](#first-run-and-login)
- [Where data lives](#where-data-lives)
- [Flags](#flags)
- [Writing](#writing)
- [Workflows](#workflows)
- [Database schema](#database-schema)
- [Export format (version 2)](#export-format-version-2)
- [Troubleshooting and debugging](#troubleshooting-and-debugging)
- [Known limitations](#known-limitations)
- [Contributing](#contributing)
- [API docs](#api-docs)

## Install and build

For distro packages (Arch, Debian, Fedora and derivatives), `scripts/install.sh` and uninstalling, see [INSTALL.md](INSTALL.md). Published binaries and alphas, once a release includes them, are on the [GitHub Releases](https://github.com/m3talsmith/acp-rs-scraper/releases) page. Building by hand:

You need Rust 1.88 or newer (`rust-version` in Cargo.toml: edition 2024, plus let-chains in the `scraper` dependency) (install with [rustup](https://rustup.rs)). SQLite is compiled in (rusqlite `bundled`) and TLS uses rustls, so there are no system libraries to install.

    git clone https://github.com/m3talsmith/acp-rs-scraper
    cd acp-red-star
    cargo build --release
    ./target/release/acp-red-star --help

That build also writes `./target/release/acprs` and `./target/release/acp-rs-scraper`. `acprs` is the same program. `cargo install --path .` installs all three, because cargo cannot create a symlink. `acprs --version` prints `acp-red-star` and the version. `acprs --help` uses `acprs` in the usage line, the same way `acp-rs-scraper --help` uses that name. `acp-rs-scraper` prints one deprecation line and then runs the same program, through version 3. `acprs` does not.

You can also run it through cargo with `cargo run --release -- <flags>`, or `cargo run --release --bin acprs -- <flags>`. The first build sets git's `core.hooksPath` to `.githooks` (see [Contributing](#contributing)).

The workspace members are `acp-red-star` (repository root, the terminal client and binary) and `acp_red_star_node_core` (`crates/acp_red_star_node_core`), the library for account identity, canonical CBOR, and a secret-store interface. `acp_red_star_node_core` is `publish = false`. The binary depends on that library. The module list is in [INSTALL.md](INSTALL.md#workspace-crates). Command-line flags, `preferences.toml`, the encrypted database, and `.acpbackup` files are unchanged. The CBOR rules are in [PROTOCOL.md](PROTOCOL.md).

## Terminal UI

`acp-red-star` (no flags) or `acp-red-star --tui` opens it. It needs a terminal on stdin/stdout
(at least 60×19); piped or scripted runs get an error naming the headless flags instead.

- **Layout:** a 4-row header on the red bar (the red star logo in white over all four rows, a big
  "ACP" drawn with tui-big-text on rows 2–4 so it has a row of room above, then name, version,
  session and running job on the middle row of the "ACP") · menu bar (View, Search, Compose,
  Library, Bookmarks, Research, Sync, Discover, Account, Debug, Help, About, Quit; Scrape and
  Start Scanning pinned on the right) · nick bar on the left (`@nick` and post count) · Body · status
  line (`l` expands it to a log panel). Click that line, or the header's `⟳ kind running (pid)` mark, or press Tab until Status is focused and then Enter, to open job details: the name, what started it, the state, the start time and elapsed time, the stage and a spinner, and a scrollable log. Elapsed counts only while the job is running. After it finishes, fails, or is cancelled, that line stays at the total run time. If the end was not recorded, it says `unknown`. Several jobs are a list; the elapsed line is the selected job. Esc closes the window. `c` or Cancel stops a job that is still running. The password and the database key are not shown. The header mark and the status bar do not show a duration, and a later start of the terminal UI does not reload jobs from log files.
- **Notifications.** An error, a warning, a skipped sync, or a finished job raises a toast at the lower right. It dismisses itself, and a click dismisses it sooner. There is no cap. When the stack would be taller than a quarter of the screen, as many as fit stay up and View all opens the rest. View › Notifications is that same page. It lists current and past notices, newest first. Order switches to oldest, Kind filters by error, warning, sync, or job, and the search box matches the title and the text. Each row has clear. Clear all deletes every saved notice. The rows live in the encrypted database and are still there after a restart. Opening the page, View all, search, order, and clearing a row are not steps on the action bar. Do not disturb and a switch for each kind silence the toast and the desktop notification together. The list still keeps the row. The status line still shows job and list state. The same notice prints on stderr as `Error: title — body`, and that line stays plain when colours are off (`NO_COLOR` or `--color never`). Linux shows it with `notify-send`. This is the planned 3.31.0 minor. It is not part of 3.5.0, and it is not the site Notifications pane. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §7.14.13.
- **Side bars.** The nick bar on the left collapses to one column (`›` opens it, `‹` closes it). A right bar lists every action that led to the current view, with no cap, and collapses the same way. The list scrolls with the wheel and, when the bar is focused, with the arrows, PgUp, and PgDn. Clicking an earlier action jumps back. A new action after a jump keeps the later steps as another branch; the bar shows which branch, and `b` lists them. Branches are stored in the encrypted database. Clear history deletes the searches and the branches. An export from a restored search still includes the query and the filters. This is not the previous and next post walk. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §7.14.5.
- **Embedded assistant, not in 3.x.** A local model inside the client is a 4.0 roadmap item. It is not a heavy interest of 3.0. It answers questions about the posts already stored here, and it can run a search, a filter, or an export from plain language. An AI button at the bottom center opens a chat strip that pushes the body and the side bars up. It is not a modal. The strip keeps the prompts and replies, and the actions change the Body. The voice is a local persona: a system prompt and optional retrieval first, and a later fine-tune on local material if wanted. The model is not chosen. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §7.14.12.
- **Browsing** is offline and read-only (the database is opened `SQLITE_OPEN_READ_ONLY`). Select a
  nick to list their posts and replies, newest first; select several with `Space`, `Shift-↑/↓`,
  `Ctrl-A`, or Ctrl/Shift-click. The **filter** field on top of the nick bar (click, or `/` from the
  nick bar or Body) narrows the list as you type: case-insensitive, substring matches first, then
  fuzzy (in-order letters), display names included, with a `matched/total` count. Selection and
  `Ctrl-A` act on the shown nicks; selected nicks that are filtered out stay selected. `Esc` or
  `x` clears it; it survives view-mode switches and searches.
- **Details** open in place: the list slides up to a strip, the detail fills the Body, with the same
  fields as the HTML export. Links (reply-to, thread, post ids in the body, replies) cross-load: the
  linked post nests inside and the previous detail is pushed up into a header strip. `Esc` pops one,
  clicking a header pops back to it, at most 8 deep. Ids not in the database say so. Bodies render
  as full Markdown (CommonMark + GFM via `pulldown-cmark`): headings H1–H6 (H1 a banner over a
  `━` rule, H2 over a `─` rule), bold, italic, strikethrough, inline code, fenced/indented code
  blocks boxed with the language on the top edge, nested block quotes with a `┃` bar, bullet
  (`•` `◦` `▪` by depth), ordered (right-aligned numbers) and loose lists with hanging indents,
  task lists (`☐`/`☑`), tables (box-drawn, column alignment, cells wrap and the widest column
  shrinks to fit), horizontal rules, links and autolinks (bare URLs too; the URL follows the text
  in the link colour unless the text already shows it, and http/mailto links are OSC 8
  hyperlinks), images as `▣ alt <url>`, footnotes (`[1]`, notes at the end), soft and hard line
  breaks (both break the line, as on the site), entities, escapes and the usual inline HTML
  tags. Before parsing, lines that start with a typed `•` (also tab-indented) become list items
  and a single indented line is plain text, not code (that's how the site stores them). One
  column of padding inside the borders. Attachments are links: Enter / click opens the stored file with
  `xdg-open` (`ACP_OPENER` overrides), and they are also emitted as OSC 8 `file://` hyperlinks
  unless `TERM` is `linux`/`dumb` or `ACP_NO_OSC8` is set.
- **Pictures** (the logo and post images): the graphics protocol is detected at start by querying
  the terminal (kitty graphics reply, sixel in DA1, cell size via `CSI 16 t`; iTerm2/WezTerm from
  the environment; tmux always uses the fallback), falling back to Unicode half blocks (the logo
  is a two-colour sextant silhouette of the PNG there: 8×4 cells, 16×12 dots);
  `ACP_IMAGE_PROTOCOL=halfblocks|sixel|kitty|iterm2` forces one and `ACP_IMAGES=off` disables
  pictures. Each stored picture gets a 14-row slot under the body; it decodes on a background thread
  and is drawn when its slot is fully visible ("scroll to show it" otherwise). AVIF/HEIC are
  converted once to PNG under `~/.acp/cache/thumbs/` with `avifdec`, `magick` or `ffmpeg`
  (whichever is installed first); images are shrunk to at most 1024 px.
- **Search** (Search menu, or `Shift-S` for keyword + nicks + post id + date range + posts/replies + order + limit)
  replaces the Body in place; `<` / `< Back` returns, `Ctrl-S` clears. Order is newest or oldest.
  Limit keeps that many matches at the chosen end (blank keeps every match), the same window as
  `--order` and `--limit`. The status line shows it, for example `search "strike": 100 of 236 posts · oldest`.
  Search history is not on the bar yet. When it ships it is part of Search: each local or site search keeps the query and the filters (keyword, nicks, post id, dates, posts or replies or both, Where, order, and limit) in the encrypted database, and picking one runs that search again. Clear history removes those rows. The default keeps every search. A preference can instead keep the N most recent. Exporting that result includes the same query and filters with the posts. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §7.14.5.
- **Menu → flags:**

  | Menu | Flags it builds |
  |---|---|
  | Scrape (Start Scanning, `F6`) | `--nicks` (field, else the selected nicks) `--latest`/`--trending` `--no-mentions` `--said-by` `--profile-feed` `--order` `--limit` `--crawl-limit` `--no-attachments` `--full` `--refetch-truncated` `--output --format`; Whole site is `acprs --scrape-all`, with Time budget as `--scrape-budget` |
  | Search | in-app search (same matching as `--search`, plus the filters above, Order, and Limit) |
  | Export | `--export PATH` + `--nicks` (selection) or `--search`/`--nicks` (current view) or nothing (everything) or `--post-ids` (Basket, and the Export basket button), `--format`, `--name`, `--compress --tar-gz` |
  | Import | `--import PATH` `--compress --tar-gz` |
  | List | re-reads the nick bar (`--list-nicks` data) |
  | Maintain | `--remove-nick` (field, else selection) `--keep-replies` `--dry-run`; Clear all = `--clear-all`; both get `--yes` after the TUI's own confirmation |
  | Session | Log in (modal) / Log out (`--logout`, then the Login modal) |
  | Bookmarks | Show (`B`), Bookmark current post (`b`: `--bookmark` / `--unbookmark`), Sync with acp.us (`--sync-bookmarks`) |
  | View | Nested / Stack / Stack + content pane / Carousel (infinite) / Carousel (fixed) / Grid; Order; Nick order; Nick limit; Colour; ◂ Previous post / Next post ▸ (TUI only) |
  | Sync | `--sync` `--nicks` (field, else selection) `--no-mentions` `--no-bookmarks`; Whole site is `acprs --sync --scrape-all` (incremental, then rules); on a timer when Auto sync is ticked; Rules… (`--rules-list`) |
  | Research | `--research-add NAME` with `--post-ids` / `--search [--nicks]` / `--file` / `--url`, `--note`, `--research-description`; `--research-create`; Show = the project's items; Export project = `--research-export NAME --export PATH --format [--compress --tar-gz]` |
  | Debug | `-v` / `--debug` / `--enable-logging` added to every job |

  The bar is View, Search, Compose, Library, Bookmarks, Research, Sync, Discover, Account, and Debug, then Help, About, and Quit, with Scrape immediately left of Start Scanning. Library opens Export, Import, Basket, List, and Maintain. Bookmarks is its own menu: Show, Bookmark current, and Sync with acp.us. Account is Log in and Log out. Search keeps local and site search. Search › History re-runs a past query, Clear history deletes those rows and the action branches, and History kept is blank by default, which keeps every search. A number there is an optional cap. Sync keeps the local copy up to date (site sync, auto-sync, and later peer replication). Discover finds people, nodes, and rooms, and stays a separate menu. That layout is its own minor and is not part of 3.5.0. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §4.2.

  Hover a button, menu item, or icon (the star, the basket, the research ◈, and the rest) and a short explanation appears, including its key when it has one, as in `Bookmark (b)`. The next line is `? {topic}` and opens the Help topic for that action. Bookmark, the basket, and research do not open the same page. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §4.3.

  Help stays on `?` and F1. It is topics you can move between with ← and → (Getting started, Browse, Compose, Research, Sync, Discover, Account, Keys, and the rest). Typing filters those topics and the key lines. A `?` in the header corner opens Help. Every user-facing flag, menu command, menu option, and control has a description there, and a tooltip. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §4.4.

  About sits beside Help. The menu has three segments: Credits (who worked on the project), Why this is necessary, and ACP platform. Choosing any segment opens the same modal, focused on that section. ACP platform is the full American Communist Party Program (adopted October 12, 2024), a vendored copy with headings and scroll, and a link to <https://acp.us/program>. The same how-to-join link, <https://acp.us/join>, sits at the top and the bottom of that text only. Those links are chrome around the platform section, not a fourth section. It is not paraphrased. Each segment has a tooltip and a Help topic, and the tooltip deep-links to that segment. Text posts and replies are sent. Uploads and interactions stay gated. The walkthrough is [GETTING-STARTED.md](GETTING-STARTED.md): the command line, the terminal prompts outside the TUI, and the TUI. It is the Getting started topic. The user-facing name is ACP Red Star. End-of-3.0 communications (a pitch, a demo, stickers, backgrounds, a cutscene in video and ASCII from licensed archival footage, and a comrades' forum post) wait until the remaining launch docs exist. The forum post features the cutscene in both forms beside the pitch and the demo. A separate community-work video is also planned and is not the cutscene or the demo. The pre-launch / coming-soon draft is [docs/history/v3/redstar-v3-prelaunch-post.md](history/v3/redstar-v3-prelaunch-post.md). It is not the live forum post. At the 3.0 release, the README link switches to `docs/history/v3/redstar-v3-launch-post.md`. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §4.5.

  Every argv is validated by clap before it runs, then runs as the headless binary in its own
  process group, logging to `~/.acp/logs/tui-<kind>-<time>.log` (mode 0600). Cancel kills it;
  quitting asks whether to keep it running in the background (default) or cancel it.
- **Previous / next:** `◂ prev` / `next ▸` on the nav bar (`,` `.` or `Alt-←` `Alt-→`) walk back
  and forth through the posts opened in the detail stack, with an `n/m` position; opening a new
  post or link drops the forward part.
- **Order** (`o`, the nav bar's `[Order: newest ▸]` / `[Order: oldest ▸]`, or View › Order): newest or
  oldest for the list on screen. Each list remembers its own choice in `~/.acp/preferences.toml`
  under `[ui.list_order]`. Nick timelines, bookmarks, search results, research items, and the
  export basket sort by posted time (a research file or page uses the time it was added). Site
  lists (Top, EB, Explore, a tag, reposts) keep the site's order; oldest reverses that list, so
  Top stays a ranking. Nested view orders the roots, and a reply whose thread root is not in the
  list; replies stay under their root, oldest first. Flat layouts order every card. Edit history
  stays in version order. `s` inside an open thread still cycles that thread's replies and is
  separate from Order. While Nicks is focused, `o` toggles the nick list by each nick's latest
  post (View › Nick order). View › Nick limit keeps N nicks at that end after the `/` filter;
  blank keeps every nick. Highlight a menu option and press `?` for its Help topic.
- **View modes** (`v`/`V`, the nav bar's `[View: …]`, or the View menu): Nested (default), Stack
  (full posts in one column, flat details), Stack + pane (list above, content pane below that
  follows `↑`/`↓`), Carousel ∞ (wraps) and Carousel (stops at the ends) with `←`/`→`, and Grid
  (arrows move; the post opens in a modal with a Close button). View › Site Top opens `/rs/top`
  (period and replies); the **EB** row under Bookmarks opens `/rs/eb`. Both show the saved order
  at once and refresh it from the site. The **Explore** row opens `/rs/explore` (View › Explore
  latest selects `/rs/explore/latest`). Explore waits 12 seconds for each LiveView reply, prints
  `Explore: still waiting`, then stops and stores nothing. Shift-S search can look at the local database or the site.
  `p` opens a post menu; View reposts opens `/rs/post/{id}/reposts`, and View edits
  opens `/rs/post/{thread}/{id}/edits` as a diff.
- **Bookmarks:** a `☆`/`★` star on every card and detail (click or `b`) bookmarks locally at once
  (a background `--bookmark`/`--unbookmark` job stores it, offline, next to a running scan); the
  **★ Bookmarks (n)** row on top of the nick bar (or `B`) shows only bookmarked posts and comments;
  Bookmarks › Sync with acp.us runs `--sync-bookmarks`.
- **Export basket:** a `+` button left of the star on every card and detail (click, or `x`) adds
  that post or reply to the basket (`✓` = in it; click again to take it out). Export › Show basket
  lists it (`<` back), Export basket writes it with Export's Path / Format / Name / Compress
  (`--export PATH --post-ids …`), Clear basket empties it; Export › From also has a Basket choice.
  The basket lives for the TUI session. Export › Path, Import › Path, Research › Export to,
  Research › Local file, and Start Scanning › Output file complete a real path on Tab
  (`~` becomes `~/`). The job expands `~` before it runs, because it is not a shell.
- **Research:** a `◈` button left of the basket button on every card and detail (click, or `r`)
  opens the research picker: **Add** (‹ › chooses: this post or reply, the posts it links, its
  attachment files, the web pages in its body, or all posts of the view / the search results),
  **Note**, the **project list** (the last one used is preselected) and **…or a new project**
  name + description. `Enter` adds; it runs `--research-add NAME …` as a background job and the
  status line reports "N added". The Research menu: Project (name), Projects… (pick or create, then
  show it), Show project (its items as cards: posts from the database, files and pages with their
  note, metadata and text; Enter opens one, its stored file is an attachment link), Add current
  post…, Add current view / search…, Local file + Add local file…, Web page URL + Add web page…,
  Google Drive / Telegram (pinned for later), Export to / Format / Compress + Export project
  (`--research-export`). Making **Add current view / search…** easier to find is required before 3.x is finished. It already queues the current list or search. Whether that should also mean an open thread's root and every loaded reply is still open, and no new key is assigned. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §4.2.
- **Sync:** Sync menu: Auto sync, Interval (minutes, default 15), Nicks (blank: selected), Mentions
  (off), Bookmarks (on), Sync now, Save as default (`[sync]` in `preferences.toml`). An automatic
  sync runs `--sync` as a background job when it's due and nothing else runs; it checks the saved
  session first and skips quietly when offline or logged out.
- **Login** happens only when needed: Start Scanning checks `~/.acp/session.json` against the site
  and, when logged out, opens the Login modal (email, then the code or login link), then starts the
  scan.
- **Empty-Body animation:** text frames loop under the hint while the Body is empty, shrunk to fit
  (never enlarged) and centred; paused below 24×8; one redraw per frame. Source, first match wins:
  `[ui] empty_animation_path`, `~/.acp/animations/empty-body.acpanim.gz` (or `.acpanim`), the
  built-in clip (first 10 s of "Debating Is Man the Measure of All Things?", IGG | Qizilbash,
  used with permission of the ACP; `assets/empty-body.NOTICE.md`), then the procedural red star. A
  file that fails to parse is skipped with a warning. `[ui] empty_animation = false` / `"off"`
  turns it off, `"star"` / `"minimal"` plays only the star; `ACP_EMPTY_ANIMATION=off|star|on`
  overrides the setting; `[ui] empty_animation_color = "accent"` colours it with the accent.
  `tools/acp-anim SOURCE [--seconds 10] [--start 0] [--fps 12] [--cols 120] [--rows 34] [--invert]
  [--out PATH]` makes a frames file from a local video or a URL (yt-dlp downloads only the
  section needed into a temporary directory, which is deleted); the format is gzip'd text:
  `ACPANIM 1`, `fps N`, `size COLS ROWS`, then each frame after a `%%` line.
- **Themes:** files in `~/.acp/themes/`; `F5` or the header `‹name› ▾` switch picks one, `Shift-F5`
  edits them. See [Themes](#themes).
- **Keys:** `?`/`F1` lists them all next to the full `--help`.

## Themes

Every colour of the terminal UI comes from a theme file; nothing is compiled in.

- **Where.** `~/.acp/themes/<name>.toml`. Names are lowercase letters, digits, `-` and `_`, at
  most 32 characters; anything else (such as `../x`) is refused. This folder (under `$HOME`) is
  the only place themes are read from: no system folder, nothing next to the binary, nothing
  installed by the packages, nothing built in.
- **First run: downloading the default.** When `~/.acp/themes` is empty the TUI starts at once with
  the terminal's colours and downloads `default.toml` in the background from
  `https://raw.githubusercontent.com/m3talsmith/acp-rs-scraper/v<version>/themes/default.toml`
  (`ACP_THEME_URL` replaces everything before `default.toml`, for tests). The download uses the
  program's HTTP client with an empty cookie jar (no cookies), gives up after 3 seconds, refuses
  more than 64 KiB, must parse as a theme file, and is written atomically with mode 0600, never
  over an existing file. When it arrives it is applied live if `default` is still the theme you
  want (you haven't picked another). If it fails (offline, timeout, too big, not a theme) the
  terminal's colours stay and a notice names the problem and the fix. `--list-themes` and
  `--theme` do the same download (and wait for it) on a first run.
- **Turning it off.** `ACP_THEME_DOWNLOAD=off` (also `0`, `false`, `no`) or `[ui] theme_download =
  false` in `preferences.toml`. Downloads you ask for still work.
- **`--theme-init [NAME|all]`.** Download a theme (default `default`) or every theme in the repo's
  `themes/` into `~/.acp/themes`, keeping files that are already there; exits non-zero if a
  download fails (the error also suggests `--theme-starter`). *Theme: init* in the command
  palette, and **Download the default theme** in the `F5` list when there are no themes, download
  the default.
- **`--theme-starter NAME`.** Write the minimal starter theme as `~/.acp/themes/NAME.toml`: the six
  palette colours as plain terminal colours (`red`, `bright_black`, `white`, `reset`), no element
  overrides, one variant. Generated by the program (not a copy of a shipped theme), written
  atomically with mode 0600, never downloads, and fails if the file exists. *Theme: starter
  [name]* in the command palette does the same (type the name after the words, e.g. `theme
  starter mine`; without one the first free `starter`, `starter-2`, … is used), and **Create
  starter theme** in the `F5` list (offered when there are no themes) writes one and selects it.
- **Header.** While the first-run download runs the switch reads `‹default · downloading…› ▾`;
  a theme that can't be used reads `‹default: missing› ▾` (no file) or `‹mine: broken› ▾` (the
  file can't be read as a theme).
- **Themes in the repo.** `default` (ACP red on the terminal's colours), `soviet` (red and gold),
  `constructivist` (red, black and cream), `cuba` (olive green, red and black), `red-star` (ACP Red
  Star brand: ink black, vermilion and gold, cream text, from the banner), `chinese-poster` (vermilion and golden yellow), `plattenbau` (concrete greys, red
  accent), `rs` (Red Star site: near-black zinc with ACP red and vote gold), `acp` (American Communist Party: ACP red, black and white on blush pink); each has
  dark and light variants with text contrast of at least 3:1. Only `default`
  is downloaded automatically; get the rest with `--theme-init NAME` or `--theme-init all`.
- **Choosing.** `[ui] theme = "name"` (written by the picker), `[ui] theme_mode = "dark" | "light"
  | "system"`. `system` (default) uses `COLORFGBG` when set, else asks the terminal for its background colour
  (`OSC 11`), else dark. `--theme NAME` overrides for one run; `--list-themes`
  lists them.
- **Brands.** The logo, the title, and the motto are a brand. The theme is a different choice:
  any brand works with any theme. The built-in `default` brand is ACP Red Star (the red-star logo,
  the word ACP, the title, and "The future belongs to the working class"). The built-in `plain`
  brand is the title Red Star, a star, and no motto. `acp-red-star --brand plain` uses it for this
  run and does not write the preference. `[ui] brand = "plain"` in `~/.acp/preferences.toml` keeps
  it. `--list-brands` lists built-ins and files. A user brand is `~/.acp/brands/<name>.toml`
  (names: lowercase letters, digits, `-` and `_`, at most 32 characters). A file overrides a
  built-in of the same name. `version = 1` is required. `title` is the header, the window title,
  About, `--help`, `--version`, and the HTML export header. `motto` is the status bar and the
  About border; an empty motto is hidden. `word` is the big letters beside the logo; empty draws
  none. `logo` is `builtin` (the red star), `text` (`logo_text`, one line per row), or `file`
  (`logo_file`, a path inside `~/.acp/brands`). An absolute path, `..`, or a symlink that leaves
  that folder is refused. A broken file is reported and never overwritten. The header switch
  `«name» ▾` opens the picker (moving the highlight previews it; Esc puts the old brand back).
  Edit brands… and the command palette's Brand: edit… open the editor. Ctrl-S saves, Ctrl-R
  reverts, Ctrl-N saves as new, Ctrl-D duplicates, Ctrl-X deletes, Ctrl-C closes. Credits and the
  other surprises keep their own text.
- **Colour.** `--color auto|always|never`. `auto` (the default) follows `NO_COLOR`: a non-empty value
  turns colours off and leaves bold, dim, and underline. `always` forces colours on, which is what a
  recording needs when the shell exported `NO_COLOR=1`. `never` turns colours off. `[ui] color` in
  `preferences.toml` is used when the flag is omitted. View › Colour sets it and remembers it.
  When `NO_COLOR` is set and colours are off, the status line, the theme picker, and the theme editor
  say `NO_COLOR is set, so colours are off (use --color always)`. `?` on View › Colour opens that Help topic.
- **Format v1.**

  ```toml
  version = 1                 # files from a newer format are refused
  name = "default"
  description = "ACP red on your terminal's own colours"

  [dark.palette]
  accent = "#dc2626"          # focused borders, selection, links, buttons
  muted = "#9ca3af"           # dates, ids, hints
  border = "#4b5563"          # unfocused borders
  on_accent = "#ffffff"       # text on the accent colour
  fg = "reset"                # text; also fills the screen
  bg = "reset"                # background; also fills the screen

  [dark.styles.button]        # per-element overrides
  fg = "@on_accent"
  bg = "@accent"
  bold = true

  [light.palette]             # optional: missing keys come from [dark]
  ```

  Colours: `#rrggbb`, `#rgb`, `0`–`255` (xterm palette), `black` `red` `green` `yellow` `blue`
  `magenta` `cyan` `white` and their `bright_` forms (`light_…`, `gray`/`grey` and `dark_gray` work too), `reset`, or
  `@entry` for a palette entry (palette entries may refer to each other). Style keys: `fg`, `bg`,
  `border` (box elements), and the flags `bold`, `italic`, `underline`, `reverse`, `dim`. Unknown
  keys are warned about one by one and ignored; missing keys fall back to the built-in style
  (from the palette), then to the terminal's colour. Files over 64 KiB are refused.
- **Elements.** Text: `text`, `bold`, `accent`, `accent_bold`, `link`, `muted`, `gone` (deleted
  or missing posts), `code`, `selected`, `field`, `label`, `label_focused`, `bar` (the header and
  status bars), `button`, `post_tag`, `reply_tag`, `dimmed` (behind a modal), `border`,
  `border_focused`. Boxes (`border` sets the border colour): `pane`, `pane_focused`, `card`,
  `card_selected`, `card_focused`, `detail`, `detail_focused`, `dialog`. Markdown in post bodies:
  `h1` … `h6`, `quote` (text; `border` is its `┃` bar), `code_block` (box), `list_marker`
  (bullets and numbers), `task` (`☐`/`☑`), `table` (box), `table_header`, `rule` and `footnote`.
  Border shapes and padding are fixed in code; there is no strikethrough key (`~~text~~` is
  always struck through).
- **Editor** (`Shift-F5`, or *Edit themes…* in the picker). The left list holds the palette and
  every element (`→` expands an element into its keys, `←` hides them) with swatches, `*` for values that differ
  from the saved file and `⚠x.x:1` when an element's text/background contrast is under 3:1. The
  right side is the real UI drawn with sample posts, updated as you edit, plus a *Sample dialog*
  frame with the parts the post view doesn't show (dialog box, focused and plain labels, a text
  field, the Post/Reply tags, a link, a deleted post, inline code, dimmed text, a button and a
  selected row). The selected palette
  colour or element is highlighted there: the preview is drawn a second time with that row's
  colours swapped for unused sentinel colours, and every cell that changes is shown in reverse
  video. The preview title names it (`‹@accent› highlighted (h)`) or says `‹dialog› isn't on
  screen`; `h` turns the highlight off and on. When the theme uses `#rrggbb` colours and
  `$COLORTERM` isn't `truecolor`/`24bit`, the preview's bottom border notes that the terminal may
  show the nearest of 256. Keys: `Tab` dark/light,
  `Enter`/`Space` edit or toggle, `Del`/`Backspace` clear, `a`/`+` add a palette colour, `[`/`]`
  previous/next theme, `Ctrl-S` save, `Ctrl-R` revert everything, `r` revert the selected palette colour, element
  or key only, `Ctrl-N` new, `Ctrl-D` duplicate, `Ctrl-X`
  delete (asks; the theme in use can't be deleted), `Esc` close (asks Save / Discard / Cancel when
  there are unsaved changes). Saving asks first when the file changed on disk since the editor
  read it (*Overwrite* / *Load* it, dropping your changes / *Cancel*) and when the file on disk is
  broken (*Overwrite* / *Cancel*). The colour picker takes hex input or a grid with tabs for the 16
  ANSI colours, the 216-colour cube, the 24 greys and the palette `@refs`.
- **When something is wrong.** A missing, unreadable or broken file never stops the UI: the
  layout stays and uses the terminal's own colours, and a notice names the file, the problem and
  how to fix it (fix the file, pick another theme with `F5`, or move it away and run
  `--theme-init` to download the default again). Broken files are never rewritten automatically; opening one in the editor
  starts a fresh theme of that name, saved only when you press `Ctrl-S` and confirm. Unknown or
  bad keys are skipped; the warnings of one file are one notice (`warning: …/x.toml: 3 keys
  skipped: …`). All writes are atomic
  (temporary file, fsync, rename) with mode 0600.
- **Migration from `[theme]`.** Up to 2.11 colours lived in `[theme]` (`accent`, `muted`, `border`,
  `on_accent`). On the first 2.12 run they are converted into `~/.acp/themes/custom.toml`
  (`custom-2` and so on if taken), on top of the default theme file when there is one (otherwise
  only the old colours are set), and `[ui] theme = "custom"` is set; a
  notice says so. The `[theme]` keys are left untouched but are no longer read.
- **Export.** *Theme: export* in the command palette writes the current theme to
  `~/.acp/ai/data/exports/themes/<name>-<time>.toml`.

## First run and login

1. Run a scrape, for example `acp-red-star --nick alice` (or Start Scanning in the TUI, whose Login modal asks for the email and code instead of the prompts below).
2. **Email:** the tool reads `email` from `~/.acp/preferences.toml`. If the file doesn't exist, it creates `~/.acp/`, asks for your email, and saves it:

       email = "you@example.com"

   To use a different account, edit or delete that file.
3. The tool asks the site to email you login details (the same form as the "Create account or log in" button).
4. **Code:** enter the verification code from the email (e.g. `KWMRUV`), or paste the login link instead.
5. The tool submits the code, completes the sign-in form the site sends back, and checks that `/rs` no longer redirects to `/auth`.
6. With `--scrape` and no `--nick`/`--nicks`, it asks for a nick; a comma-separated list is accepted.

The session is saved to `~/.acp/session.json` (mode `0600`), so later runs skip the code until the site expires it; `--logout` (or Account › Log out) deletes it. Search, export, import, `--list-nicks`, `--remove-nick` and `--clear-all` never log in.

`ACP_HOME`, when set to a non-empty path, is used instead of `$HOME` as the root of that `.acp` directory (session, database, preferences, and the rest of the table below). Leave it unset for the normal home directory. A login try that must not touch the real `~/.acp` sets it to a fresh directory.

## Upgrading from 2.x

3.0.0 is a breaking change. The local database is encrypted, and opening it requires a password. 2.x had neither.

**What breaks**

- Starting 3.0.0 against `~/.acp/ai/data/rs.database` from 2.x stops immediately. The message names the file and tells you to run `--encrypt-db`. It does not try to read the plaintext file as if it were SQLCipher.
- A new database, and every later start of the program, asks for a password on a terminal, or reads `ACP_PASSWORD` when there is no terminal. After that, the same run keeps the unlocked key in memory: Scrape, Research, Explore, Executive Board, Sync, and the other jobs use it and do not ask again. The key is wiped when the process exits. The password is at least 8 characters and has to pass a strength check. There is no password reset.
- Files 2.x does not know: `~/.acp/ai/data/rs.database.bootstrap` (the wrapped database key), `~/.acp/recovery.seal` (the encrypted recovery phrase, after `--identity-init`), and `~/.acp/acp-id`. `session.json` is still the login session and is still outside the backup.

**Migrate**

1. If you may want to go back to 2.x, copy `~/.acp/ai/data/rs.database` somewhere outside `~/.acp`.
2. Run `acp-red-star --encrypt-db`.
3. The command writes a `*.acpbackup` under the exports directory first, then encrypts the database. It deletes the plaintext file only after the row counts and a content hash match. You choose the password during this command. After Enter, the terminal shows a spinner, the stage it is on, and how long it has been running. Keypresses are ignored until it finishes.

Stretching the password is the slow part on a small database. The settings are Argon2id with 64 MiB of memory, 3 iterations, and 1 lane, and one migration does that once (the backup, the database key, and the account key are labels of that single stretch). On a 4-core test machine that stretch took about 0.1 seconds in a release build and about 1.7 seconds in an unoptimized `cargo run` build; three separate stretches, which is what an earlier build did, took about 0.3 seconds and about 5 seconds. Copying the database was about 0.1 seconds at 20 MB and about 0.5 seconds at 100 MB in a release build (a few milliseconds per megabyte, including SQLCipher's per-page HMAC). A database of a few thousand posts is usually tens of megabytes, so a release build finishes in well under a second after the password is accepted, most of it the single key stretch. A debug build spends most of that wait on the stretch and shows "Deriving key" while it does.

When `--encrypt-db` finishes on a terminal, and no account is pinned yet, it creates the recovery phrase. The password you just chose is not the master key. The master key is a 24-word recovery phrase. You type the words a second time to confirm you wrote them down, and then each of the 5 shares is shown on its own screen. Each share is 33 words. Any 3 of 5 rebuild the same secret. The password is not in a share. After the phrase exists, `acp-red-star --show-recovery` shows the words and a fresh set of shares. It asks for the password and does not read it from the environment. The same explanation is in `--help` and in the terminal UI (press `?` or F1). Account › Security opens that screen. A new database created from the terminal UI shows the phrase and the shares before the full-screen client covers them.

`--change-password` (on a terminal) asks for the current password and a new one, then rewraps the database. The account id does not change. When a recovery phrase is saved on this computer, it moves to the new password too. `--backup` and `--restore` use the same progress screen. `ACP_PASSWORD` with no terminal skips the screen and does the work inline. A non-interactive `--encrypt-db` does not dump the recovery steps into the output; the next run on a terminal shows them.

**Roll back**

- Until `--encrypt-db` finishes successfully, a 2.x binary still reads the plaintext database. A failed migration leaves that file in place.
- After a successful migration, 2.x cannot open the database or the `*.acpbackup`. To return to 2.x, install a 2.x build, put your saved plaintext copy back at `~/.acp/ai/data/rs.database`, and delete `rs.database.bootstrap` if it is still there.
- To stay on 3.0.0 and recover the database, use `--restore` on the `*.acpbackup` that `--encrypt-db` wrote. That needs the same password.

## Identity

The password that opens the database is not the master key, and there is no password reset. The master key is a 24-word BIP-39 recovery phrase. Argon2id mixes that phrase with the password. A new computer needs both before it can open a `.acpbackup`. Losing the password locks the database even if the phrase is still on paper.

`--identity-init` (on a terminal) creates the phrase. It shows the 24 words, asks you to type them again to confirm the paper copy, and stores an encrypted copy at `~/.acp/recovery.seal` (mode `0600`). The words are not logged. That file lets this computer show them again. It does not open a backup on a new computer, and the password alone cannot rebuild the words. A phrase created before this file existed was shown once and cannot be recovered from the password.

The command then shows five SLIP-39 shares, one at a time. Each share is 33 words (any 3 of 5, one group, group threshold 1 of 1). One or two shares do nothing. The password is not part of a share. You type each share before the next one is shown. The words are not logged and they are not stored as text. Write the words and each share on paper, in different places, and not in the same note as the password. `--shares-print` prints a fresh split later, from the phrase or from three existing shares plus the password. A new split cannot reprint an older one. Destroy any older shares. An older share still rebuilds the secret until that paper is destroyed. `--shares-verify` checks three to five shares and writes nothing secret.

`--show-recovery` shows the 24 words again and then each 33-word share. `--show-phrase` opens the same screen. It always asks for the password on a terminal (it does not read `ACP_PASSWORD`). It warns that the words will be on the screen and may stay in scrollback, asks you to type the 24 words, then each share, and clears the terminal afterward. Clearing the screen does not erase a screenshot. Without `~/.acp/recovery.seal`, the password cannot rebuild the words.

`--backup` writes an encrypted `*.acpbackup` (use `--output` for the path). The archive includes the encrypted phrase file when one exists, so a restore on this computer keeps `--show-phrase` working. `--restore FILE` puts the archive back. A new machine whose backup names an `acp-id` also asks for the phrase or three shares. A wrong password or a damaged file is refused and nothing is written. `session.json` and the node secret are not in the archive, so a restored machine signs in again and is not a copy of the old node. `--backup-schedule daily|weekly --keep N --dest DIR` remembers a schedule; this build reminds you when it is due.

`--encrypt-db` encrypts an existing plaintext database after writing a backup, and removes the plaintext file only after the encrypted copy matches. On a terminal it then shows the phrase and the shares when no account is pinned yet. The terminal UI does the same after it creates a database, before the full-screen client opens. If the phrase was never sealed, Help opens and the password cannot rebuild the words. `--help` includes the steps too. `--change-password` changes the database password and reseals the phrase with the new password when the phrase used the same one. The slow steps show a spinner, the stage, and the elapsed time. That progress screen is separate from the job-details window in the terminal UI. Encrypting the database, backup and restore are not listed there. A database this program creates is encrypted. The password is asked on a terminal, or taken from `ACP_PASSWORD` for a non-interactive run. That variable is visible to the same user and is not logged.

`--acp-id` prints the pinned id from `~/.acp/acp-id`, which holds the id, the Argon2 parameters and the two public keys. It does not print the phrase. A wrong password still derives an id; it is shown and refused when it does not match the pin. There is no password reset. A lost password together with lost shares cannot be recovered.

`--node-init` creates this computer's node after `--identity-init`. The node key is generated here and is not derived from the master. The command unlocks the database, rebuilds the master from `~/.acp/recovery.seal` and the password (or asks you to paste the phrase when that file is missing), signs the certificates, and drops the master. It prints the node name, the account id, the peer id, a fingerprint, and six verification words. It does not print the phrase or the node secret. `--node-label NAME` sets the name; otherwise the name is this computer's hostname, letters and digits only. An authorizer certificate lasts 30 days and a node certificate lasts 365 days. Running `--node-init` again does not replace an existing keystore, and there is no separate re-certify command. A node created on 3.3.0 or 3.3.1 has those two lifetimes reversed. Remove `~/.acp/node` and run `--node-init` again to sign a new node key with the corrected lifetimes. The peer id changes. The account id does not. `--node-id` prints that public record and says whether the certificate still checks. The header of the terminal UI shows `◉` and the name. The secret is `~/.acp/node/keystore` (mode `0600`). A plain `identity.key` is not created. A backup does not include the keystore, so restoring onto another computer does not copy this node.

## Where data lives

`~` below is `$HOME`, or `$ACP_HOME` when that variable is set and not empty.

| What | Path |
|---|---|
| Preferences | `~/.acp/preferences.toml` |
| Saved login session (mode `0600`) | `~/.acp/session.json` |
| Logs of jobs started from the TUI (mode `0600`) | `~/.acp/logs/tui-<kind>-<time>.log` |
| Database (encrypted in 3.0.0) | `~/.acp/ai/data/rs.database` |
| Wrapped database key (mode `0600`) | `~/.acp/ai/data/rs.database.bootstrap` |
| Pinned `acp-id`, after `--identity-init` | `~/.acp/acp-id` |
| Encrypted recovery phrase (mode `0600`), after `--identity-init` | `~/.acp/recovery.seal` |
| Node keystore (mode `0600`), after `--node-init`. Not in a backup | `~/.acp/node/keystore` |
| Public node name, peer id, and certificates | `~/.acp/node/node.public` |
| Attachments (persistent copies) | `~/.acp/ai/data/attachments/<hash>.<ext>` |
| Default export / archive location | `~/.acp/ai/data/exports/` |
| Research files (copies) and web page snapshots | `~/.acp/research/<project>/files/`, `…/web/` |
| Picture cache (AVIF/HEIC converted to PNG) | `~/.acp/cache/thumbs/` |
| Your empty-Body animation (`tools/acp-anim`) | `~/.acp/animations/empty-body.acpanim.gz` |

Every scrape is saved to the database. Exports are written on top of that.

## Flags

These are grouped the same way as `--help`. `-h`/`--help` prints help and `-V`/`--version` prints the version.

### Interface

| Flag | Meaning |
|---|---|
| *(no flags)* | Open the terminal UI. Logging flags (`-q`, `-v`, `--debug`, `--enable-logging`, `--log`, `--error-log`) don't count as flags here. |
| `--tui` | Open the terminal UI even with other flags; `--nick`/`--nicks` pre-select those nicks, logging flags pre-tick the Debug menu. |
| `--scrape` | Headless scrape without other scraping flags; asks for nicks when none are given (what a bare run did before 2.0). |
| `--theme NAME` | Use the theme `~/.acp/themes/NAME.toml` for this run (not saved). Fails if it is missing or broken. |
| `--theme-init [NAME\|all]` | Download a theme (default `default`) or all the repo's themes into `~/.acp/themes` and exit; existing files are kept. |
| `--theme-starter NAME` | Write the minimal starter theme as `~/.acp/themes/NAME.toml` (no download) and exit; fails if the file exists. |
| `--list-themes` | List the theme files (`●` marks the one in use, broken ones say why) and exit. |
| `--brand NAME` | Use this brand for this run (a built-in, or `~/.acp/brands/NAME.toml`). Not saved. `plain` is a neutral title and a star with no motto. Fails if the name is unknown or the file is broken. |
| `--list-brands` | List built-in brands and the files in `~/.acp/brands` (`●` marks the one in use, broken ones say why) and exit. |
| `--doctor` | Print the rustc pin and the `acp_rs_plugin` version, then exit. Plugin authors read the pin only here. |
| `--plugins-reload` | Unload installed packages, then load every archive in `~/.acp/plugins` that passes the sha256 gate. An ABI or plugin-crate mismatch is refused and the archive stays. A bad archive is quarantined and is not opened. |

### Mode

| Flag | Meaning |
|---|---|
| `--latest` | Site-wide crawls use the Latest tab (`/rs/latest`). **Default.** It is chronological, so stopping early once pages are already known works. |
| `--trending` | Site-wide crawls use the Trending tab (`/rs`, the site's own default). It isn't chronological, so the early stop is disabled; paging runs until the end of the feed or `--crawl-limit`. |

The two can't be combined. Profiles always use their Latest feed (`/rs/@nick`) in both modes.

### Reading

These read a site view and store it. They don't scrape nicks, and they don't post. Each one is saved in `site_feeds` (the site's order) so the TUI can open it offline and then refresh it. The direct URL is tried first; if that page has no feed, the same fields are submitted with `configure-feed`.

| Flag | Meaning |
|---|---|
| `--top [PERIOD]` | Site-wide Top (`/rs/top`). PERIOD is `day`, `week`, `month`, `year` or `all` (default `week`). View › Site Top. |
| `--top-replies` | With `--top`: the feed's replies filter. |
| `--eb` | Executive Board feed (`/rs/eb`). The **EB** row of the nick bar, or View › Executive Board. |
| `--eb-replies` | With `--eb`: the feed's replies filter (View › EB includes replies). |
| `--explore` | Explore (`/rs/explore`). The **Explore** row of the nick bar, or View › Explore. One LiveView reply waits 12 seconds; a timeout stores nothing. |
| `--explore-latest` | Explore's Latest tab (`/rs/explore/latest`). View › Explore latest. |
| `--site-search TEXT` | Search the site (`/rs/explore?q=`, form `search`, field `q`). The Search modal's **Where: Site** does the same and uses only the keyword; nicks, dates, post id and kind stay local. |
| `--tag TAG` | Tag feed (`/rs/tag/TAG`). A `#tag` in a post body, or the Tags row on the detail, opens the same view. `#post-…` anchors, `##` headings and tags inside code are not links. |
| `--backfill-tags` | Rewrite `post_tags` from stored bodies, then re-read threads whose `body_json` is still empty (one page per thread, with a pause). `--nick` / `--nicks` limit the rows. Stored post text is not replaced. |
| `--sort-thread ID` | Re-read that thread after the `sort` hook and store scores. `s` in an open thread does this. |
| `--thread-sort ORDER` | With `--sort-thread`: `oldest` or `top`. Newest is the page default and sends no hook. |
| `--reposts ID` | Reposts of that post (`/rs/post/ID/reposts`). `p` › View reposts does the same. |
| `--edits THREAD/POST` | Edit history (`/rs/post/THREAD/POST/edits`). A post id already in the database works too (the thread is its parent, or itself when it is a root). `p` › View edits shows each version as a line diff against the previous one. |

### Scraping

| Flag | Meaning |
|---|---|
| `--nick <NICK>` | One nick; the `@` is optional. Can't be used with `--nicks`. |
| `--nicks <LIST>` | A comma-separated list, e.g. `@hazaldin,@m3talsmith`. Spaces are trimmed, `@` is optional, and repeats are removed (ignoring case). |
| `--no-mentions` | Skip the mentions crawl, which is on by default. |
| `--said-by` | Only what the nick said: their own top-level posts **and** their own replies/comments, from the Posts and "Posts and replies" feeds. Other people's replies in the nick's threads aren't fetched and the mentions crawl is skipped (implies `--no-mentions`). A thread page is read only when one of the nick's own items has a blank body in the feed. Exports list the posts first and the replies after, each marked `"kind": "post"`/`"reply"` (JSON) and with a Post/Reply badge (HTML). |
| `--refetch-truncated` | Complete bodies that were stored shortened. Feeds send long posts and comments truncated (`body_truncated: true`, the "show more" case); the scraper now reads the full text from the thread page during every scrape, and this flag backfills rows already in the DB: every row whose `body_state` isn't `full` (rows saved truncated, and every row stored before 1.26.0, `unknown`) for the given nicks — or all nicks when none is given, in which case nothing else is scraped. One thread page per thread, fetched one at a time with a short pause. Needs a login. |
| `--profile-feed <FEED>` | Which profile a nick scrape reads. `newest` (default) is `/rs/@nick` plus Posts and replies. `top` is the profile Top feed (`/rs/@nick/top?t=all`) only. Other Top periods were not captured for profiles, and this flag is not `--latest` / `--trending` (those still choose the mentions crawl). |
| `--order <ORDER>` | Which end of that profile feed `--limit` keeps. `newest` (default) keeps the start of the site's list. `oldest` keeps the end, so a limited scrape reads the whole feed first. On `newest` that is the oldest posts. On `top` that is the end of the Top ranking. The site has no oldest-first profile URL. |
| `--limit <N>` | How many profile items to keep from each nick. With `--order newest` this is the start of the feed (the newest posts, or the top of `--profile-feed top`). With `--order oldest` this is the end of that feed. Omit it to scrape the whole profile. Replies in those threads are still fetched, unless `--said-by`. This is not `--crawl-limit`. |
| `--crawl-limit <N>` | The most site-feed posts to scan when looking for mentions (default 500). |
| `--scrape-all` | Read every reachable page of the Red Star site (`acprs --scrape-all`): newest and top feeds, threads and replies, profiles, tags, spaces and communities when they are linked, reposts, and attachment URLs and types. The file bytes are not downloaded. The frontier is stored in the encrypted database, so a stopped run resumes. After a finished run the next one is incremental unless `--full`: newest feeds stop at a page of known posts, and recent threads are read again for new replies. Pages wait between requests, back off after a refusal, and honor `robots.txt`. `--limit` caps how many new posts this run stores. The status line counts pages, posts, and what is left. Finish and failure raise a notice. Nothing is sent to the site. |
| `--scrape-budget <SECS>` | With `--scrape-all`: stop after this many seconds and leave the frontier for the next run. At least 1. |
| `--no-attachments` | Don't download media, avatars or banners. Profile images are otherwise saved to `~/.acp/ai/data/attachments/profiles/<nick>.<ext>` and shown in HTML exports. |
| `--full` | Re-crawl everything: ignore known post ids when paging, refetch every thread, and process attachments for every post (files already on disk are still reused). |

**"Posts and replies":** every `--nick`/`--nicks` scrape reads the profile twice. First it reads the default **Posts** feed (`/rs/@nick`). Then it reads the **Posts and replies** view (`/rs/@nick?replies=1`, the profile Filter dialog's second option), which adds the nick's replies in other people's threads. Both are paged to the end, or to `--limit` when that is set, and merged by `post_id`, so nothing is stored twice; the database's unique `post_id` index enforces this too. With `--limit N` and `--order newest` (the default) the replies feed is the window: the newest N items that nick wrote. The Posts feed fills that window only when the replies feed is shorter. `--order oldest` reads both feeds to the end, with no incremental stop, and keeps the oldest N. `--profile-feed top` reads `/rs/@nick/top?t=all` instead and does not also read Posts and replies; `--limit` then keeps the start of that ranking, or the end of it with `--order oldest`. Replies are stored with `parent_post_id` set to their thread's root post. Incremental runs stop each feed once a page is entirely already-known posts; `--full` walks both completely. If the `?replies=1` URL has no feed, the scraper falls back to submitting the Filter form (`configure-feed`, `replies=1`) on the profile's LiveView. The per-nick summary shows how many items came only from the replies view.

### Search / Export (offline)

| Flag | Meaning |
|---|---|
| `--list-nicks` | List the nicks in the database: display name, post count, latest post, whether a profile was scraped and when. Prints a table, or JSON with `--format json`. |
| `--search <Q>` | Case-insensitive search of title and content. Exports each match plus its parent post and direct replies. Writes to stdout or `--output`. |
| `--export <FILE>` | Dump the database (everything, or filtered by `--nick`/`--nicks` and/or `--search`) to FILE. Takes the place of `--output`. |
| `--post-ids <IDS>` | Export exactly these posts and replies (comma-separated post ids, in that order) with their attachments, to `--export FILE` or stdout / `--output`. Ids not in the database are warned about; none found is an error. The TUI's export basket runs it. Can't be combined with `--search` / `--nick(s)`. |
| `--name <NAME>` | Sets the export's file name, archive name, folder inside the archive, and HTML title. |

`--nick`/`--nicks` also filter `--search` and `--export` (posts by any listed nick).

Default names: `<nick>-<YYYYmmdd-HHMMSS>`, `nick1+nick2-<ts>` for several nicks (`multi-<ts>` for more than 4 nicks or more than 40 characters), `search-<slug>-<ts>` for a search, `basket-<ts>` for `--post-ids`, and `db-<ts>` for a full dump.

A search export does not yet record the query. Before 3.x is finished, exporting a search result includes the query and the filters that produced it (keyword, nicks, post id, dates, posts or replies or both, Where, order, and limit), along with the posts. A full dump, a nick filter, or a basket does not add a search. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §7.14.5.

### Maintenance (offline)

| Flag | Meaning |
|---|---|
| `--remove-nick <LIST>` | Delete nicks from the database. Comma-separated, `@` optional, same parsing as `--nicks`; matching ignores case. |
| `--keep-replies` | With `--remove-nick`: keep other people's replies in the removed nicks' threads. |
| `--clear-all` | Delete the database file and the attachments directory. |
| `--dry-run` | Show what would be removed; change nothing. |
| `-y`, `--yes` | Skip the confirmation prompt. |

These can't be combined with each other's opposite (`--remove-nick` vs `--clear-all`) or with any scraping, search, export, import or output flag.

**What `--remove-nick` deletes**, all in one SQLite transaction:
- every post authored by the nicks;
- **other people's replies in those nicks' threads** (replies whose `parent_post_id` is one of the removed root posts), because they only exist as children of the thread. Pass `--keep-replies` to keep them; they then remain in the database as orphans. The nick's own replies in other people's threads are always removed, and the rest of those threads stays;
- `attachments` rows of every removed post;
- `mentions` rows where the nick is `mentioned_nick`, plus mention rows that pointed at a removed post;
- the nick's `profiles` row.

After the commit, attachment, avatar and banner files are deleted from disk **only if no remaining `attachments` row or profile still references them**, and only if they are inside `~/.acp/ai/data/attachments/`. Nicks with nothing stored are listed as "not in database". A summary of what was removed is printed.

**What `--clear-all` deletes:** `~/.acp/ai/data/rs.database` (and any `-wal`/`-shm`/`-journal` sidecar files) and the whole `~/.acp/ai/data/attachments/` directory. **It does not touch `~/.acp/preferences.toml` or `~/.acp/ai/data/exports/`**; your saved email and previous exports stay.

**Confirmation:**
- `--remove-nick` prints the counts first, then asks `Proceed? [y/N]`.
- `--clear-all` lists the paths and sizes, then requires typing `yes`.
- Prompts go to stderr. `--yes` skips them.
- If stdin is not a terminal (a pipe, cron, CI), the command refuses to run unless `--yes` is given.
- `--dry-run` prints the same preview (and, for `--remove-nick`, the files it would delete) and exits without changing anything.

### Bookmarks

| Flag | What it does |
|---|---|
| `--bookmark IDS` | Offline. Bookmark these posts (comma-separated post ids). |
| `--unbookmark IDS` | Offline. Remove these bookmarks; ones the site has are removed there at the next sync. |
| `--list-bookmarks` | Offline. List bookmarks (`★` = also on the site); `--format json` prints JSON. |
| `--sync-bookmarks` | Log in, read the site's bookmarks (storing their posts), push local additions and removals. With `--nicks` it then scrapes them too. |

Sync rules: a site bookmark missing locally is added; a local one the site lacks is pushed; a
local removal is pushed and the row dropped; a bookmark that vanished from a fully read site list
is dropped locally. The site page (`/rs/bookmarks`, override with `ACP_BOOKMARKS_PATH`) was
confirmed against the live site (2.5.0). Since 2.24.1 the push is the hook `bookmark`
with `state` `"1"` or `"0"`, which is what the current page sends.

### Research (offline, except `--url`)

| Flag | What it does |
|---|---|
| `--research-list` | List projects (name, items, created, description), newest first; `--format json` for JSON. |
| `--research-create NAME` | Create a project (1–80 printable characters, names are case-insensitive); an existing name is an error. |
| `--research-description TEXT` | Its description (also used when `--research-add` creates the project). |
| `--research-add NAME` | Add to NAME (created if missing) one of: `--post-ids IDS` (posts/replies in the database; unknown ids are warned about), `--search TEXT [--nicks a,b]` (every match, as search results), `--file PATH`, `--url URL`. `--note TEXT` attaches a note. Re-adding updates the note only. |
| `--file PATH` | A local file: size, MIME type, modified time and (text files up to 2 MB) its text are kept; copied to `~/.acp/research/<project>/files/` unless `--no-copy`. |
| `--no-copy` | With `--file`: reference the file in place. |
| `--url URL` | http(s) only, no acp.us cookies, 20 MB limit: keeps the title, readable text and an HTML snapshot (other content types are stored as a file). |
| `--note TEXT` | Note for the added items. |
| `--research-show NAME` | List a project's items (`--format json` for the export's `research` section). |
| `--research-export NAME` | With `--export FILE`: the project's posts (with attachments) plus a `research` section in JSON / HTML; `--compress --tar-gz` bundles the files and snapshots. |

### Sync

| Flag | What it does |
|---|---|
| `--sync` | Log in, sync bookmarks, then incrementally scrape `--nick`/`--nicks` if given. With `--scrape-all` (`acprs --sync --scrape-all`) it also runs the incremental whole-site update. No export unless `--output`; never prompts. After the new rows are saved, enabled rules run on those rows only. |
| `--no-bookmarks` | With `--sync`: skip the bookmark sync. |

### Rules (offline)

Sync › Rules is the same list as these flags. A system rule is read-only; clone it to edit, disable, or delete the copy. The shipped rule `synced-post` ("Synced post") starts off. Lower priority runs first. `--rules-preview` matches posts already stored and applies nothing. Import and export are TOML and JSON (`--rules-format toml|json`). Import always creates user rules. Notify has no cap. A post fires at most once per rule.

`--rules-watch NICK`, `--rules-saved TEXT`, and `--rules-replies NICK` insert a user rule: watch that nick, a keyword, or replies to that nick. The same presets are `w`, `s`, and `m` on the rule list. Each starts enabled at priority 40 and notifies. The line names the author, the post id, and the site URL. There is no separate alert list.

| Flag | What it does |
|---|---|
| `--rules-list` | List rules, lower priority first. `--rules-format json` prints JSON. |
| `--rules-show NAME` | One rule, as TOML, or JSON with `--rules-format json`. |
| `--rules-enable NAME` / `--rules-disable NAME` | A user rule only. |
| `--rules-clone KEY` | Clone a system rule by its key (`synced-post`). The clone starts enabled. |
| `--rules-delete NAME` | Delete a user rule. |
| `--rules-priority NAME --rules-rank N` | Set priority on a system or user rule. |
| `--rules-import PATH` / `--rules-export PATH` | TOML or JSON. The extension picks the format unless `--rules-format` is set. |
| `--rules-preview` | Run enabled rules on stored posts and change nothing. |
| `--rules-watch NICK` | Add a user rule that notifies when this nick posts. |
| `--rules-saved TEXT` | Add a user rule that notifies when a synced post matches this keyword. |
| `--rules-replies NICK` | Add a user rule that notifies when someone replies to this nick. |

### Import (offline)

| Flag | Meaning |
|---|---|
| `--import <FILE>` | Read an earlier export back into the database. Plain `.json` or `.html` by default; add `--compress --tar-gz` for an archive. |

Whether the file is unpacked depends on the flags, not the file name. If the file is gzip but no flags were given, or the other way round, you get a clear error. HTML exports are read through the JSON embedded in them. Can't be combined with the scrape, search and output flags.

### Output

| Flag | Meaning |
|---|---|
| `--format json\|html[,…]` | Export format(s), default `json`. Comma-separated or repeated (`--format json,html`) writes every format from one run; see [Several formats](#several-formats). |
| `--output <PATH>` | Output file. With `--compress` it's the archive path. Defaults to stdout. |

- **Stdout:** attachment paths are the absolute paths under `~/.acp/ai/data/attachments/`.
- **File:** an `attachments/` folder is written next to the file, and paths are relative.
- **Stdout stays clean:** progress and prompts go to stderr, so you can pipe it.

### Compression

| Flag | Meaning |
|---|---|
| `--compress` | Compress the export. Needs exactly one format flag. |
| `--tar-gz` | Format: `.tar.gz`. Needs `--compress`. |

### Several formats

Several formats (`--format json,html` or `--format json --format html`) all come from the same run; duplicates are ignored. Paths: with `--output <dir>` each goes to `<dir>/<name>.<ext>`; with `--output <file>` (or `--export <file>`) the stem is kept and each format gets `<stem>.<ext>` (`--output out/haz.html` writes `out/haz.json` and `out/haz.html`); with `--compress` both files go into the same archive folder (`<name>/<name>.json`, `<name>/<name>.html`, one shared `attachments/`). Several formats can't go to stdout, so pass `--output`, `--export` or `--compress`. The 0-post skip applies to every format.

```sh
acp-red-star --nick @hazaldin --format json,html --output hazaldin/hazaldin.html   # hazaldin.json + hazaldin.html
acp-red-star --export db --format html --format json                               # db.html + db.json
```

An archive contains `<name>/<name>.<ext>` (one per format) and `<name>/attachments/`. It's written to `--output` or `--export`. A path that already ends in `.tar.gz` or `.tgz` is that file. A directory, or a path ending in `/`, gets `<dir>/<export-name>.tar.gz` (the export name, as a file name: `Resonance Cosmos` becomes `resonance-cosmos.tar.gz`). Any other path gets `.tar.gz` appended, so a bare name such as `resonance-cosmos` becomes `resonance-cosmos.tar.gz` instead of failing with "Is a directory". The success log prints `wrote <final path>`. With no path, the archive is `~/.acp/ai/data/exports/<name>.tar.gz`. `--tar-gz` is the only format for now. Once the archive is written, the uncompressed `<name>/` folder is removed; if compression fails it is left in place. The terminal UI expands a leading `~` or `~/` on Export › Path before it starts that export.

### Debug

| Flag | Meaning |
|---|---|
| `-q`, `--quiet` | Hide progress and summaries. Prompts and errors still show. |
| `-v`, `--verbose` | Show detail per page and per thread, and counts of skipped items. |
| `--debug` | Print every LiveView websocket message (cut at 2000 characters). Turns on verbose too. `ACP_DEBUG=1` does the same. CSRF tokens, signed sessions, cookies and bearer tokens are replaced with `[REDACTED]` before anything reaches stderr or `success.log` (1.24.0). |

`-q` can't be combined with `-v` or `--debug`.

## Workflows

```bash
# One nick, JSON to stdout (prompts for login)
acp-red-star --nick alice > alice.json

# Several nicks in one session, HTML bundle with attachments
acp-red-star --nicks @hazaldin,@m3talsmith --format html --compress --tar-gz --name comrades
# -> ~/.acp/ai/data/exports/comrades.tar.gz

# Skip mentions; or scan fewer feed posts for them
acp-red-star --nick alice --no-mentions
acp-red-star --nick alice --crawl-limit 200

# Only the nick's own posts and replies (no thread crawl, no mentions)
acp-red-star --nick alice --said-by

# Newest 100, oldest 100, or the top of the profile
acp-red-star --nick alice --limit 100
acp-red-star --nick alice --order oldest --limit 100
acp-red-star --nick alice --profile-feed top --limit 100

# Latest (default) vs Trending for the site-wide mentions crawl
acp-red-star --nicks @hazaldin --latest
acp-red-star --nicks @hazaldin --trending --crawl-limit 300

# Complete posts/comments stored with truncated ("show more") text
acp-red-star --refetch-truncated                     # every nick in the DB, nothing else
acp-red-star --nick alice --said-by --refetch-truncated   # backfill alice, then scrape as usual

# Incremental re-run (default) vs full re-crawl
acp-red-star --nick alice          # stops paging at known posts, skips unchanged threads
acp-red-star --nick alice --full   # refetches everything

# What's in the DB?
acp-red-star --list-nicks
acp-red-star --list-nicks --format json

# Offline search and export
acp-red-star --search "general strike" --format html --output strike.html
acp-red-star --search strike --nicks alice,bob > strike.json
acp-red-star --export all.json
acp-red-star --export alice.html --format html --nick alice
acp-red-star --export strike.tar.gz --search strike --compress --tar-gz --name strike
acp-red-star --export picked.html --format html --post-ids A1B2C3D4,E5F6G7H8

# Round trip into another machine / fresh DB
acp-red-star --export backup.tar.gz --compress --tar-gz
acp-red-star --import backup.tar.gz --compress --tar-gz
acp-red-star --import all.json
acp-red-star --import alice.html

# Maintenance
acp-red-star --remove-nick @spam,@bot --dry-run
acp-red-star --remove-nick alice                  # prompts with counts
acp-red-star --remove-nick alice --keep-replies -y
acp-red-star --clear-all                          # type 'yes'; prefs and exports are kept

# Logging levels
acp-red-star --nick alice -q
acp-red-star --nick alice -v
acp-red-star --nick alice --debug 2> wire.log
```

When the mentions crawl is on, these steps happen in order:
1. Each nick's profile and feed are scraped.
2. The site feed (Latest or Trending) is crawled up to `--crawl-limit` posts.
3. Each likely post is checked by loading its whole thread.
4. Confirmed matches are stored with their threads, plus a row in `mentions`.

A summary for each nick is printed to stderr at the end.

## Database schema

```sql
-- The original spec plus in_reply_to_id (1.20.0). post_id is unique (index posts_post_id_uq);
-- posts_parent_idx and posts_in_reply_to_idx index the two relationship columns.
CREATE TABLE posts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id TEXT NOT NULL,          -- 22-char base64url site id
  parent_post_id TEXT NOT NULL,   -- thread root: '' for thread roots; thread id for replies
  in_reply_to_id TEXT NOT NULL DEFAULT '',  -- immediate parent: the comment a reply answers,
                                  -- else the thread root; '' for thread roots
  nick TEXT NOT NULL,             -- author username
  title TEXT NOT NULL,            -- '' for replies
  content TEXT NOT NULL,          -- ProseMirror body flattened to text; paragraphs
                                  -- separated by a blank line (1.26.0)
  posted_at TEXT NOT NULL,        -- inserted_at (ISO 8601)
  body_state TEXT NOT NULL DEFAULT 'unknown', -- 'full' | 'truncated' | 'unknown' (1.26.0)
  body_json TEXT NOT NULL DEFAULT '', -- ProseMirror body, compact JSON (2.21.0); '' until a feed or thread supplies it
  score INTEGER                  -- site score (2.22.0); NULL until a thread page sends one. A later save does not blank it
);

CREATE TABLE post_tags (             -- 2.21.0; #tag without the hash
  post_id TEXT NOT NULL,
  tag TEXT NOT NULL,
  PRIMARY KEY (post_id, tag)
);
CREATE INDEX post_tags_tag_idx ON post_tags(tag);

CREATE TABLE post_versions (        -- 2.24.0; edit history. source is site for /rs/post/…/edits
  post_id TEXT NOT NULL,
  version_hash TEXT NOT NULL,      -- FNV-1a 64 of body_json and edited_at
  body_json TEXT NOT NULL,         -- ProseMirror JSON, or the text when that is all the page sent
  edited_at TEXT NOT NULL,         -- edited_at, else inserted_at / updated_at; ~NNNN when the page sent none
  observed_at TEXT NOT NULL,       -- when this client stored the row
  source TEXT NOT NULL CHECK (source IN ('site', 'peer', 'local', 'draft')),
  PRIMARY KEY (post_id, version_hash)
);

-- body_state (1.26.0, index posts_body_state_idx): 'full' = complete body;
-- 'truncated' = only the feed's shortened preview could be read (the thread page
-- failed), re-read on the next scrape that sees it; 'unknown' = stored before 1.26.0.
-- --refetch-truncated re-reads the thread page of every row that isn't 'full'.

-- How nesting works on the site: threads are flat (every reply carries only
-- thread_id). A reply's body starts with a postRef node {postId, threadId}.
-- On a root reply both ids are the thread id. On a reply to a comment, postId
-- is that comment. That postId is stored as in_reply_to_id.
-- Databases from before 1.20.0 are upgraded in place on first use: the column is
-- added and existing replies get in_reply_to_id = parent_post_id until a re-scrape
-- (any run that sees the reply again, e.g. --full) records the real comment.
-- Example: the whole reply chain under a comment
--   WITH RECURSIVE t(id) AS (SELECT 'COMMENT_ID' UNION
--     SELECT p.post_id FROM posts p JOIN t ON p.in_reply_to_id = t.id) SELECT * FROM t;

CREATE TABLE attachments (        -- unique (post_id, url)
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id TEXT NOT NULL,
  url TEXT NOT NULL,
  local_path TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  on_site INTEGER NOT NULL DEFAULT 1  -- 1 when this file is on the Red Star copy (at most 4)
);

CREATE TABLE mentions (           -- unique (post_id, mentioned_nick, match_type)
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id TEXT NOT NULL,
  mentioned_nick TEXT NOT NULL,
  match_type TEXT NOT NULL        -- 'mention' | 'nick' | 'display_name'
);

CREATE TABLE bookmarks (          -- 2.1.0; created on first use
  post_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  remote INTEGER NOT NULL DEFAULT 0,   -- the site has it too
  deleted INTEGER NOT NULL DEFAULT 0   -- removed locally, still to be removed on the site
);
CREATE TABLE profiles (           -- upserted on every scrape
  nick TEXT PRIMARY KEY,
  user_id TEXT, display_name TEXT, bio TEXT,
  avatar_url TEXT, avatar_path TEXT, banner_url TEXT, banner_path TEXT,
  location TEXT, role TEXT, joined_at TEXT,
  followers INTEGER, following INTEGER, posts_count INTEGER,
  raw_json TEXT NOT NULL, scraped_at TEXT NOT NULL
);
```

How each mention type is decided:
- **`mention`:** an @mention in the post whose user id or label matches the nick.
- **`nick`:** the nick appears as a whole word, ignoring case, so `haz` does not match `hazard`.
- **`display_name`:** the profile display name appears as a whole word. Display names shorter than 3 characters are skipped.

A nick's own posts can also be recorded as mentions of that nick.

Research (2.9.0):

```sql
CREATE TABLE site_feeds (          -- 2.18.0; created on first site read
    key TEXT PRIMARY KEY,         -- top:week, eb, explore, explore:latest, search:…, tag:strike, reposts:…, …
    title TEXT NOT NULL,
    fetched_at TEXT NOT NULL,     -- RFC 3339
    post_ids TEXT NOT NULL        -- JSON array, in the site's order
);

CREATE TABLE research_projects (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  description TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);
CREATE TABLE research_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES research_projects(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('post','reply','search-result','local-file','web-url')),
  ref TEXT NOT NULL,                 -- post id, absolute file path or URL
  title TEXT NOT NULL DEFAULT '', note TEXT NOT NULL DEFAULT '',
  meta TEXT NOT NULL DEFAULT '{}',   -- JSON: size / mime_type / modified / copied, or status / content_type / final_url / fetched_at
  content TEXT NOT NULL DEFAULT '',  -- readable text (pages, text files)
  local_path TEXT NOT NULL DEFAULT '', -- the copy or snapshot
  added_at TEXT NOT NULL,
  UNIQUE (project_id, kind, ref)
);
```

## Export format (version 2)

```json
{
  "format": "acp-rs-export",
  "format_version": 2,
  "title": "<name>",
  "nick": "@alice, @bob",
  "exported_at": "2026-10-15T04:30:00-04:00",
  "counts": { "posts": 65, "replies": 158 },
  "profiles": [ { "nick": "alice", "user_id": "...", "display_name": "...", "bio": "...",
                  "avatar_url": "...", "avatar_path": "...", "banner_url": null, "banner_path": null,
                  "location": null, "role": null, "joined_at": null,
                  "followers": null, "following": null, "posts_count": null, "scraped_at": "..." } ],
  "mentions": [ { "post_id": "...", "mentioned_nick": "alice", "match_type": "mention" } ],
  "posts": [ {
      "post_id": "...", "kind": "post", "parent_post_id": "", "in_reply_to_id": "", "nick": "alice", "title": "...",
      "content": "...", "posted_at": "...", "truncated": false,
      "attachments": [ { "url": "...", "kind": "image", "mime_type": "image/png",
                         "local_path": "attachments/abc123.png" } ],
      "children": [ /* replies, same shape; nested under their thread root */ ]
  } ]
}
```

- **HTML exports** are a single page. They show a profile header per nick, threads with nested replies, embedded media and a Mentions section. Every post is an `<article id="post-{post_id}">`. A reply shows "Replying to {id}" linking to its immediate parent, plus "thread {id}" when the parent is a comment rather than the root. A link goes to `#post-{id}` when that post is in the export and to `https://acp.us/rs/post/{thread}/{id}` (or `/rs/post/{id}` for a thread root) when it isn't. The same JSON is embedded in `<script type="application/json" id="acp-data">`, which is how HTML exports can be imported again.
- **Posts vs replies:** every node has `"kind"`: `"post"` for a top-level thread post (`parent_post_id` is `""`), `"reply"` for a reply/comment. `counts` totals both over the whole tree. Top-level `posts` lists thread posts first (newest first), then replies whose parent isn't in the export (newest first). The HTML page has a "N posts · M replies" line, a **Posts** section (thread posts with their nested replies) and a **Replies in other threads** section, and every article carries a Post/Reply badge and a `kind-post`/`kind-reply` class.
- **Parent ids:** `parent_post_id` is the thread root and `in_reply_to_id` the immediate parent (equal to the root for a direct reply, `""` for roots). Import reads `in_reply_to_id` when present and falls back to `parent_post_id`.
- **Research exports** (`--research-export`) add `"research": { "name", "description", "created_at", "items": [ { "id", "kind", "ref", "title", "note", "meta", "content", "file", "added_at" } ] }`, where `file` is the bundle-relative path of the copy or snapshot (`attachments/…`) or the absolute one outside a bundle. The HTML page gets a Research section listing every item with its note, linking posts to their `#post-` article. Import ignores the section.
- **Versions:** import accepts versions 1 (no profiles or mentions) and 2, and refuses newer versions or unknown formats.
- **Search metadata, not in version 2 yet.** Before 3.x is finished, an export of a search result also carries the query and the filters that produced those posts. A dump, a nick filter, or a basket does not. The field names are not assigned here. See [architecture/PLAN-3.0-CLIENT.md](../architecture/PLAN-3.0-CLIENT.md) §7.14.5.

## Troubleshooting and debugging

- **Login errors:**
  - Run with `--debug` and look at the `submit-code` reply and the sign-in POST (its field names are printed with `-v`).
  - Remove `~/.acp/preferences.toml` to enter a different email.
  - Run `acp-red-star --logout` to forget the saved session (`~/.acp/session.json`) and log in again.
- **"feed paging stopped before at_end":**
  - Rerun with `--debug`.
  - Check the `page-next` replies: posts should be in `response.diff.r`, or in a rebuilt `forum/Feed` data-props.
- **"no profile @nick … redirected to /rs":** the nick doesn't exist. ACP redirects unknown profiles to Trending; since 1.23.0 the scraper stops that nick with this error (also written to `error.log` with `--enable-logging`) instead of scraping Trending. Check the spelling, e.g. Haz is `@hazaldin`.
- **"0 posts kept, not writing an export":** a run that kept nothing (for example `--said-by` filtered everything out) never writes or overwrites an export file; any existing export of that name is left as it was.
- **Re-run finds nothing new:** that's the incremental stop at work. Use `--full`.
- **Attachment warnings:** failed downloads are skipped. Rerun later; files already on disk are reused.
- **Import errors about gzip:** add `--compress --tar-gz`, or remove them if the file is plain JSON or HTML.
- **Inspect the database directly:**

      sqlite3 ~/.acp/ai/data/rs.database 'select nick,count(*) from posts group by nick'

- **Check the output:** stdout carries only the export, so `acp-red-star ... | jq .` works.

## Known limitations

- **Untested against the live site:**
  - Paging through a feed (`page-next {cursor}`).
  - Loading more thread replies through the InfiniteScroll `next-page` event (a guess).
  - Profile fields beyond author info, data-props and `og:` tags.
  - Whether media hosts require the login cookies.
- **Mentions are limited by the crawl:** the mentions pass still scans the chosen feed (there is no mentions search API). Only `--crawl-limit` posts of that feed are scanned, so older mentions, and replies inside threads whose root isn't in that window, can be missed. Searching posts on the site is `--site-search` (or Shift-S › Where: Site).
- **"Posts and replies":** this is the profile Filter dialog (`<dialog id="js-configureFeed">`, form `configure-feed`, radios `replies=0|1`). Checked on the live site: choosing it switches to `/rs/@nick?replies=1`, the URL the scraper loads. The fallback form submission is covered by the mocked end-to-end tests. `--profile-feed top` scrapes the profile Top feed (`/rs/@nick/top?t=all`). Only `t=all` was captured for profiles; day, week, month, and year stay on the site-wide `--top` feed.
- **Trending feeds:** these aren't chronological, so incremental stopping is disabled there.
- **Embeds:** YouTube, X and other embedded players aren't downloaded.
- **Logins:** a saved session is reused until the site expires it; then the next scrape asks for a fresh email code.
- **TUI:** bodies are stored as plain text (the site's bold/italic marks are not kept by the
  scraper), so Markdown styling shows only where the text itself is Markdown; no syntax
  highlighting in code blocks; pictures need
  `avifdec`, ImageMagick or ffmpeg for the site's AVIF files, and half-block pictures are coarse;
  video attachments are links only; research has no delete (use `sqlite3`), Google Drive and
  Telegram sources are pinned for later, and web pages that need JavaScript to render come out
  thin; Ctrl-Shift-S arrives as Ctrl-S in most terminals; cancelling a job kills it outright.
- **Truncated bodies:** feeds shorten long posts and comments (`body_truncated: true`, cut `body` plus `body_text_summary`). Checked on the live site: the thread page (`/rs/post/{thread}`) carries every item's full body with `body_truncated: false`, so the scraper reads it (once per thread) instead of pushing the "show more" action over LiveView.

## Writing

3.34.0 stores a draft on this machine before it is sent. `--draft-save --draft-id ID --body TEXT` writes it. `--draft-title` is the new-post title. `--draft-thread THREAD` makes it a reply. The same id writes another version in `post_versions`. `--keep-local` stores it as `local` and leaves it off the queue. `--draft-list` prints the drafts. `--outbox-queue ID` queues that local id. The outbox id is `Q-` plus the local id. `--outbox-list` prints the queue. `--outbox-cancel ID` marks a queued, failed, or draft row cancelled. A row left in `sending` is not cancelled until a read-back. `--outbox-send ID` asks `Send from the outbox? [y/N]`. `--yes` confirms. `--dry-run` prints the same validate and save frames as a direct text publish and opens no connection. An item older than 24 hours asks again. The queue sends only a text post or a reply. A vote or another gated write is refused. In the terminal UI, the compose box has Save draft (Cancel stays the default). Compose › Save draft uses the draft fields. Send from outbox starts on Cancel.

```bash
acp-red-star --draft-save --draft-id D1 --draft-title "Shop notes" --body "hello"
acp-red-star --draft-save --draft-id D1 --body "hello, revised"
acp-red-star --outbox-queue D1
acp-red-star --outbox-send Q-D1 --dry-run
```

3.4.0 composes a new post or a reply. Writes are on unless `ui.writes = false` in `~/.acp/preferences.toml` or `ACP_WRITES=off`. The environment wins.

In the terminal UI, `n` opens New post. On an open post, `r` replies (a comment pre-fills the reference) and `Q` quotes that post. A reply's body starts with a `postRef` node. On the thread root, `postId` and `threadId` are both that thread's id. On a comment, `postId` is the comment. `+` prepares an upvote on the open or highlighted post and `-` prepares a downvote. Neither sends. `--vote POST` with `--vote-state 1`, `0`, or `-1` does the same from the command line (`1` is the default). `--dry-run` prints the hook. A confirmed vote does not connect. The live frame has not been captured. Post menu (`p`) › Subscribe prepares `subscribe` with state `1` and does not send it. `--subscribe THREAD` with `--subscribe-state 1` or `0` does the same from the command line (`1` is the default). `--dry-run` prints the hook. A confirmed subscribe does not connect. The live frame has not been captured. Post menu (`p`) › Delete asks you to type yes. Cancel is the default. `--delete POST` asks the same on a terminal. `--yes` confirms. `--dry-run` prints the `delete-post` hook. A confirmed delete does not connect. The live frame has not been captured. Post menu (`p`) › Edit refuses a post older than one hour. On a reply, inside that hour, the compose box starts on Cancel and then asks you to type yes. A confirmed reply edit sends validate, then save, once. `--edit REPLY` needs `--body` or `--body-file`. `--posted-at UNIX` checks the same hour. Omit `--posted-at` and the hour is not checked. `--dry-run` prints the frames and does not connect. A top-level post edit is not sent. The site does not support changing attachments on an edit, so `--attachment` with `--edit` is refused before a connection. Post menu (`p`) › Repost names `/rs/post/new?repost=POST` and does not send. `--repost POST` does the same. `--reposts` still only reads the list. The repost form has not been captured, so it is not built. `--dry-run` prints the route. A confirmed repost does not connect. The button under the cursor starts on Cancel. A text post or reply sends validate, then save, once, and prints the new post id and URL. A rejection is shown and is not retried. `--dry-run` prints those frames and opens no connection. The 30 second gap starts when you confirm. A failed upload is not retried, and a save is not sent again. `--dry-run` does not start the gap.

3.5.0 adds local attachments on that same compose box. The attach line takes a path; Enter adds the file, and Backspace on an empty line removes the last one. Tab on a non-empty line completes the next real directory entry. `~` becomes `~/`. `~/…` is completed under the home directory and keeps the `~/` prefix on the line; the stored path is expanded. `~user` is not completed. A name starting with `.` is offered only when the prefix starts with `.`. One match fills the line (a directory ends with `/`). Several matches stay listed: Tab, Up, and Down move the highlight, Enter writes that path and does not attach it yet, and Esc clears the list and stays on the line. A Tab that changes nothing moves to the next field, including a second Tab on a file that is already complete, and a second Tab on a completed directory lists what is inside it. An empty attach line still moves focus. `--attachment PATH` repeats for `--new-post` and `--reply`. The shell completes that flag; Tab completion is the compose box. Any file can be attached locally, including audio and documents. Images and videos are `media_attachments`. A PDF is `file_attachments` and counts toward the same 4. `--dry-run` prints the `allow_upload` plan (entry refs, MIME types, and chunk counts) and does not connect. A confirmed publish uploads that site copy, then sends validate and save once. The command line prints `uploading name N%` as each file moves. The form's accept list and the `allow_upload` limits (chunk size, timeout, max entries, max size) are what a live send obeys. A file the site rejects stops the publish before save and names the file. A failed chunk is not retried. The app, the archive, and the network keep every file. Red Star takes up to 4 images, videos, or PDFs on the site copy. Audio and other documents stay on the local copy and are named in the result, not dropped. More than 4 of those, with no choice, keeps the first 4 and skips the rest; a notice names the files left off. `--site-attachments 1,3,5,6` (1-based, at most 4, no duplicates) chooses which images, videos, or PDFs are that site copy. An index that names an audio file or another ineligible type fails and names that file. In the compose box the same choice is a picker: Space toggles an image, a video, or a PDF, audio and other documents are greyed out with the reason, a fifth check is refused, Enter confirms, Esc returns to Publish without sending. The status line shows upload progress. `?` opens the `site-attachments` Help topic. Each attachment row stores `on_site`; a row from before that column counts as on the site. Export JSON includes `"on_site"`.

```bash
acp-red-star --new-post "Title" --body "hello **world**" --attachment photo.jpg --dry-run
acp-red-star --reply THREAD --in-reply-to COMMENT --body "agreed" --attachment notes.pdf --dry-run
acp-red-star --reply THREAD --quote "selected lines" --quote-post POST --body "yes" --yes
```

`--yes` confirms a text post or reply and sends it once. Without it, a terminal asks `Publish? [y/N]`. The third command above sends one text reply. Run it only when you mean to post. A dry-run check comes first: [LIVE-PUBLISH-TEST.md](LIVE-PUBLISH-TEST.md). A second confirmed attempt within 30 seconds waits. That gap is for a new post or reply, not for an edit. Vote, subscribe, delete, repost, and a top-level post edit are not sent. A reply edit is sent. The site does not support changing attachments on an edit. The clicks still to record are [../architecture/INTERACTIONS-CAPTURE.md](../architecture/INTERACTIONS-CAPTURE.md). The command line asks `Vote? [y/N]`, `Subscribe? [y/N]`, and `Repost? [y/N]`. Delete and edit ask that, then `Type yes`. A confirm of vote, subscribe, delete, repost, or a top-level post edit does not send. The hidden `--capture-reply` flag does not copy the session and does not send; it is described in `architecture/PLAN-3.0-CLIENT.md`.

## Contributing

The repo ships `.githooks/pre-commit`, a credential scanner, and `build.rs` sets `git config core.hooksPath .githooks` on the first `cargo build`.

### Releases

A GitHub Release for a version that is not yet on `main` is a draft. `main` includes 3.4.0. 3.5.0 stays a draft until it is merged. The maintainer publishes that draft after the merge. Versions already on `main` (2.16.0 and earlier) are left as published. `.githooks/post-commit` creates the annotated git tag `vX.Y.Z` when `Cargo.toml` changes and does not create a GitHub Release. There is no CI job that publishes a release. When a release is created it uses `gh release create --draft`, the matching `CHANGELOG.md` section, and no attached files. See [CONTRIBUTING.md](../CONTRIBUTING.md).

The hook blocks commits that contain:
- **Secret files:** `preferences.toml`, `*.database`, `*.db`, `.env`, keys, cookies and similar.
- **Credential patterns:** private keys, cloud and GitHub tokens, JWTs, `_acp_key=` cookies and `password=`-style assignments.

It also runs `gitleaks` if installed. Generated API docs under `docs/api/` are scanned like everything else and currently pass cleanly. Never commit your database or preferences. Bypass with `git commit --no-verify` only if you're sure.

Licensed GPL-3.0-or-later (see `LICENSE`).

### Code layout and tests

The crate is a library with a thin binary (`src/main.rs` → `app::run`). Modules are split by domain: `cli`, `config` (all constants and site URLs), `logging`, `http`, `auth`, `liveview`, `scrape/{feed,thread,profile,mentions,reads,refetch,edits}`, `site` (New post, Reply, and local attachments), `media`, `storage/{db,schema}`, `export/{json,html,archive}`, `import`, `maintenance`, `research/{mod,cli}`, `themes/{format,download,mod}`, `tui/{state,input,layout,commands,data,jobs,theme,theming,editor,sample,markdown,images,widgets/,modals/}` and `app`. Unit tests live next to their module in `tests.rs` files (e.g. `src/cli/tests.rs`).

TUI styling goes through `tui::theme` only. `themes` (`themes/format.rs`: the TOML v1 parser,
colours, elements, contrast; `themes/mod.rs`: files, migration, background detection;
`themes/download.rs`: fetching theme files from the repo)
loads a theme file into `tui::theme::Theme`: the palette colours plus one optional override per
element. `Theme` returns `ratatui_lipgloss::Style` values for text (`accent()`, `muted()`,
`selected()`, `bar()`, …) and boxes (`pane()`, `card()`, `detail()`, `dialog()`: border kind,
border colour and padding), each built from the palette and then the element's override.
`theme::frame(&style)` turns a box style into a ratatui `Block` (via the crate's `to_block`) so a
title can be added. Widgets don't build `ratatui::style::Style`, `Modifier` or `BorderType`
values themselves. The picker is `tui/modals/theme_picker.rs`, the editor `tui/editor.rs` (state)
and `tui/widgets/editor.rs` (drawing; the preview runs the real `widgets::draw` on sample data
from `tui/sample.rs` in an off-screen terminal).

The end-to-end suite is Gherkin (`cucumber`). The scenarios are in `tests/features/*.feature` and the steps in `tests/e2e.rs`. It runs the real binary against a **mock acp.us** (`tests/support/mock.rs`: HTTP pages, the LiveView websocket, OTP sign-in, threads and media) with a temporary `HOME`. It covers:
- login (right and wrong code);
- profile paging, threads and replies, attachments;
- incremental runs and `--full`;
- mentions;
- the "Posts and replies" URL and its form fallback;
- truncated feed bodies completed from the thread page, and `--refetch-truncated`;
- search, export/import round trip, `--remove-nick` and `--clear-all`;
- the TUI entry (bare / `--tui` / `--scrape`) and the argvs its Scrape, Export and Maintain menus build;
- the export basket (`--post-ids`) and research projects (create, list, add posts / search results / a local file / a web page from the mock site, show, export, and the TUI's add argv).

TUI state, menu → argv mapping, layout geometry and headless renders (ratatui `TestBackend`) are unit-tested in `src/tui/tests/`. `cargo test smoke_real_db -- --ignored --nocapture` renders your own database read-only.

It never contacts the real site.

```bash
cargo test                 # unit tests + the Gherkin suite
cargo test --test e2e      # just the scenarios
```

The site origin comes from `ACP_BASE_URL` (default `https://acp.us`; websocket URLs follow the scheme). That is how the tests point the binary at the mock; you normally never set it.

## API docs

The rendered rustdoc lives in [`docs/api/`](api/) (entry: [`docs/index.html`](index.html)). Regenerate with:

    scripts/gen-docs.sh
