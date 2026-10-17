# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [2.11.0] - 2026-10-17

### Added
- **Empty-Body animation** in the TUI: while the Body is empty a text animation loops under the
  hint, shrunk to fit and centred in the muted colour (`[ui] empty_animation_color = "accent"` for
  the accent). It pauses when the Body is smaller than 24×8 and redraws only when the frame
  changes. The built-in clip is the first 10 seconds of "Debating Is Man the Measure of All
  Things?" (IGG | Qizilbash) as 120×34 text art at 12 fps, used with permission of the ACP
  (`assets/empty-body.NOTICE.md`, 35 KB, embedded in the binary); a procedural turning red star
  with an ACP wordmark is the fallback. `[ui] empty_animation = false|"off"|"star"`,
  `[ui] empty_animation_path`, and `ACP_EMPTY_ANIMATION=off|star|on`.
- `tools/acp-anim`: turns the first N seconds of a local video or a URL (via yt-dlp) into a frames
  file, by default `~/.acp/animations/empty-body.acpanim.gz`, which the TUI plays instead of the
  built-in clip. Packages install it as `/usr/share/acp-rs-scraper/tools/acp-anim`.

## [2.10.0] - 2026-10-17

### Changed
- All TUI styling now goes through [ratatui-lipgloss](https://crates.io/crates/ratatui-lipgloss)
  1.1 (a Lip Gloss port for ratatui): `tui::theme` builds every text style and every box (border
  kind, border colour, padding) as a lipgloss `Style`, and widgets only pick one. The look is
  unchanged (checked cell for cell against 2.9.0), the accent is still `#dc2626`, and `[theme]`
  overrides work as before.

## [2.9.0] - 2026-10-17

### Added
- **Research projects**: named collections of posts, replies, search results, local files and web
  pages, each with an optional note, stored in the new `research_projects` / `research_items`
  tables. Flags: `--research-list`, `--research-create NAME [--research-description TEXT]`,
  `--research-add NAME` with `--post-ids IDS`, `--search TEXT [--nicks]`, `--file PATH [--no-copy]`
  or `--url URL` (plus `--note`), `--research-show NAME` and `--research-export NAME --export FILE`.
  Local files keep their metadata (and text) and are copied to `~/.acp/research/<project>/files/`;
  web pages are fetched once without the acp.us cookies and keep their title, readable text and an
  HTML snapshot. Exports add a `research` section to the JSON and HTML; bundles carry the files.
- TUI: a `◈` button on every card and detail (or `r`) opens a research picker — add this post or
  reply, the posts it links, its attachment files, the web pages in its body, or the whole view /
  search results to an existing or new project, with a note. A Research menu (`Alt-r`): Projects…
  (pick or create), Show project (its items as cards), add a local file or a web page URL, Google
  Drive and Telegram (pinned for later), and Export project.
### Changed
- The menu bar's scan button shortens to `▶ Scan` when the window is too narrow for every command.
### Fixed
- Readable web text no longer puts a space before punctuation after inline tags.

## [2.8.0] - 2026-10-17

### Added
- **Export basket** in the TUI: a `+` button next to the star on every card and detail (or `x`)
  adds that post or reply (`✓` when in; click again to remove). Export › Show basket, Export basket
  (uses Export's Path / Format / Name / Compress) and Clear basket; Export › From has a Basket
  choice. The basket lasts for the session.
- `--post-ids IDS`: offline export of exactly these posts and replies (in the order given) with
  their attachments, to `--export FILE` or stdout / `--output`; default name `basket-<ts>`. Unknown
  ids are warned about; none found is an error.

## [2.7.0] - 2026-10-17

### Added
- **Pictures in the TUI**: stored post images show inline in a detail (a 14-row
  slot under the body, drawn once fully on screen), decoded on a background thread and shrunk to
  1024 px. The graphics protocol is detected by querying the terminal (kitty, sixel, iTerm2) with a
  Unicode half-block fallback (tmux always falls back); `ACP_IMAGE_PROTOCOL` forces one,
  `ACP_IMAGES=off` disables pictures. The encoders (kitty, sixel, iTerm2, half blocks) are built
  in: ratatui-image was not used because its sixel quantiser needs Rust 1.90 (MSRV is 1.88).
  AVIF/HEIC are converted once to PNG in `~/.acp/cache/thumbs/` with `avifdec`, `magick` or `ffmpeg`.
- Packaging recommends an AVIF converter (libavif / ImageMagick / ffmpeg) and xdg-utils.

### Changed
- The TUI header is three rows tall: the red star logo from `assets/red-star.png` (on a light badge
  so it shows on the red bar), a big "ACP" drawn with tui-big-text (third-height pixels), and the
  name, version, session and running job vertically centred. Minimum terminal size is now 60×18.

## [2.6.0] - 2026-10-17

### Added
- TUI post bodies render as **Markdown** (pulldown-cmark → styled, word-wrapped lines): headings,
  bold / italic / strikethrough, inline and block code, bullet and numbered lists with hanging
  indents, block quotes, links (the URL is shown when it differs from the text). Line breaks and
  paragraph breaks are kept.
- **Attachments are links** in a post detail: click, or `←`/`→` then `Enter`, opens the stored file
  with `xdg-open` (`open` on macOS; `ACP_OPENER` overrides), detached from the TUI. Missing files say
  so. Where supported they are also OSC 8 `file://` hyperlinks (off for `TERM=linux`/`dumb` or with
  `ACP_NO_OSC8`).

### Changed
- One column of padding between the borders of cards and details and their text.

## [2.5.0] - 2026-10-17

### Added
- TUI **nick filter**: a field at the top of the nick bar (click it, or `/` from the nick bar or
  Body) narrows the list as you type. Case-insensitive; substring matches on the nick or display
  name come first, then fuzzy in-order matches (`hzd` → `hazaldin`). Shows `matched/total` in the
  field and the bar title; `Esc` or the `x` clears it. Selection, Ctrl/Shift-click, `Shift-↑/↓`
  and `Ctrl-A` act on the shown nicks; selected nicks that are filtered out stay selected. The
  ★ Bookmarks row stays visible, and the filter survives view-mode switches, searches and
  refreshes.

### Changed
- `Ctrl-A` in the nick bar adds the shown nicks to the selection (with no filter: every nick, as
  before).

### Notes
- Bookmark sync is confirmed against the live site: the `/rs/bookmarks` page and the
  `toggle-bookmark` LiveView event are no longer marked as guesses.

## [2.4.0] - 2026-10-16

### Added
- TUI **previous / next** through the detail stack: `◂ prev` and `next ▸` on the Body's navigation
  bar with an `n/m` position, `,` / `.`, `Alt-←` / `Alt-→`, the palette, and View › Previous post /
  Next post. Prev steps back down to the list; next reopens what you closed (also after `Esc`, a
  header click or the Grid's Close). Opening a different post or link starts a new trail.

### Changed
- The flat view modes (Stack, Pane, carousels, Grid) allow up to 256 open posts; Nested keeps its
  limit of 8.

## [2.3.0] - 2026-10-16

### Added
- TUI **view modes**, switched with the `[View: …]` button on the Body's navigation bar, `v` / `V`,
  or the new View menu:
  - **Nested** (the 2.0 layout, still the default).
  - **Stack**: one endless column of full-height posts; details open flat, without nesting.
  - **Stack + pane**: the stack above a content pane showing the clicked post; `↑`/`↓` move the
    list and the pane follows.
  - **Carousel ∞** (wraps around) and **Carousel** (stops at the ends): one post at a time with
    its neighbours peeking at the sides; `←`/`→` or a click on a side flips.
  - **Grid**: cards in as many columns as fit; arrows move by cell and row; a post opens in a
    modal with a **Close** button.
- The read-only real-database smoke render now draws every view mode and the bookmarks view.

## [2.2.0] - 2026-10-16

### Added
- **Sync.** `--sync` logs in, syncs bookmarks both ways (skip with `--no-bookmarks`) and
  incrementally scrapes `--nick`/`--nicks` if given; it writes no export unless `--output` is set
  and never prompts, so it suits cron.
- TUI **Sync menu**: Auto sync on a timer (Interval, default 15 minutes), Nicks (blank: the
  selection), Mentions (off), Bookmarks (on), Sync now, and Save as default (`[sync]` in
  `~/.acp/preferences.toml`, loaded at start). Automatic syncs run as background jobs only when
  nothing else runs, check the session first, and skip quietly when offline or logged out; their
  failures go to the status line instead of a modal.

## [2.1.0] - 2026-10-16

### Added
- **Bookmarks**, local first and synced with the site:
  - `--bookmark IDS` / `--unbookmark IDS` / `--list-bookmarks` (offline) keep them in a new
    `bookmarks` table (created on first use; removed by `--clear-all`).
  - `--sync-bookmarks` logs in, reads the site's bookmarks (`/rs/bookmarks`, storing their posts),
    and pushes local additions and removals (`toggle-bookmark`). Local changes win; bookmarks
    removed on the site are dropped. The page path and event are inferred (override the path with
    `ACP_BOOKMARKS_PATH`).
  - TUI: a clickable `☆`/`★` star on every card and detail (or `b`), stored by a background side
    job that runs offline and next to a scan; a **★ Bookmarks (n)** row on the nick bar (or `B`)
    shows only bookmarked posts and comments; a Bookmarks menu (Show / Bookmark current post /
    Sync with acp.us).

### Changed
- The menu bar drops its separators (then its padding) when the terminal is too narrow.
- Database writers wait up to 10 s for a lock instead of failing, so TUI bookmark writes and a
  running scan don't collide.

## [2.0.0] - 2026-10-16

Major release: a bare `acp-rs-scraper` now opens a terminal UI instead of prompting for nicks and
scraping. Every flag still works headless; the old bare behaviour is `--scrape`.

### Added
- **Terminal UI** (ratatui + crossterm, `src/tui/`), opened with no flags or `--tui`:
  - top bar, menu bar, nick bar with post counts, Body, status/log line; mouse and keyboard for
    every action; command palette (`Ctrl-Space` / `Ctrl-P`); Help modal generated from `--help`
    plus the key bindings.
  - offline, read-only browsing of the database; **multi-select nicks** (`Space`, `Shift-↑/↓`,
    `Ctrl-A`, Ctrl/Shift-click).
  - post details at parity with the HTML export; the list slides up when a detail opens, linked
    posts (reply-to, thread, ids in the body, replies) nest inside and push the previous detail up
    (animated; at most 8 deep; ids not in the database are flagged).
  - search replaces the Body in place with a `<` back button; `Shift-S` modal filters by keyword,
    nicks, post id (post + thread), date range and posts/replies; `Ctrl-S` clears.
  - menu bar built from the CLI flags (Scrape, Search, Export, Import, List, Maintain, Session,
    Debug, Help, Quit) with checkbox / dropdown / text / button options and `*` for required ones.
  - **Start Scanning** (and Export / Import / Maintain) build the CLI argv, validate it with clap and
    run the headless binary as a child process in its own process group, logging to
    `~/.acp/logs/tui-<kind>-<time>.log` (0600). **Cancel** kills it; **quitting during a scan
    detaches it** to keep running in the background (or cancels it, if you choose).
  - **bulk export from the selection** (selected nicks, current search, or everything).
  - Start Scanning while logged out opens the **Login modal** (email, then the code or login link)
    and starts the scan once logged in; Session › Log out deletes the saved session and returns to
    the Login modal. Sessions persist in `~/.acp/session.json` (0600) as since 1.21.
  - confirm modals for Remove nicks and Clear all (type `yes`).
  - **theme**: ACP red `#dc2626` accent by default; `[theme]` in `~/.acp/preferences.toml`.
- `--tui` (force the TUI; `--nick(s)` pre-select, logging flags pre-tick Debug) and `--scrape`
  (headless scrape that asks for nicks).
- `login_flow` (caller-supplied email/code), `session_valid`, `set_log_sink`, `list_nicks_conn`.
- Tests: TUI unit tests for the detail stack, search back, Args-from-menu mapping, layout
  geometry, modals, the read-only store, jobs and headless renders; Gherkin `tui.feature` for the
  entry modes and the argvs the Scrape / Export / Maintain menus build, against the mock site.

### Changed
- **Breaking:** no flags (or only logging flags) opens the TUI. Scripts that relied on the bare
  prompt-and-scrape should pass `--scrape` (or `--nick`/`--nicks`).
- Saving the login email keeps other settings in `preferences.toml`.

## [1.26.1] - 2026-10-16

### Fixed
- **Duplicate attachments from re-signed media links.** The site serves media through presigned URLs
  whose `X-Amz-Date`/`X-Amz-Signature` change on every page load. Files were named by a hash of the
  whole URL, so every scrape downloaded each image again under a new name and added another
  `attachments` row (hazaldin's 17 images had become 68 rows and files). Files are now keyed by the URL
  without its query string, a fresh link replaces the stored row for the same file, and DB exports
  (`--export`) list each file once even for rows stored before this fix. Scrape exports were already
  correct; DB exports and the attachments directory were the bloated ones.
- Tests: unit test for URL keys; e2e scenario re-scraping with re-signed links.

## [1.26.0] - 2026-10-16

One feature/fix since 1.25.1 (minor 25 → 26).

### Fixed
1. **Truncated post and comment text.** Feeds send long posts and replies shortened
   (`body_truncated: true`, a cut `body` doc and `body_text_summary`: the site's "show more"), and the
   scraper stored that preview. Every truncated feed item is now completed from its thread page
   (`/rs/post/{thread}`), which has the full body of the root and every reply; one fetch per thread,
   shared with the reply crawl, with a 250 ms pause after each fill-in fetch.
2. **Paragraph breaks.** Bodies keep a blank line between paragraphs (ProseMirror blocks and HTML
   `<p>`/`<div>`; `<br>`/hard breaks stay single newlines). HTML exports render each paragraph as `<p>`.

### Added
- `--refetch-truncated`: backfill stored rows whose body isn't known to be complete, for the given nicks
  or (with no nick) every nick and nothing else. Prints rows checked / completed / already full / not found.
- `posts.body_state` column (`full` / `truncated` / `unknown`), added in place to older databases
  (existing rows become `unknown`). A row stored `truncated` is re-read on the next scrape that sees it,
  and saving a full body over a non-`full` row replaces its text.
- JSON exports carry `"truncated"` per post; scrape summaries report truncated feed bodies and how many
  were completed.
- Tests: unit tests for paragraphs, truncation flags, the migration and backfill selection; e2e
  `truncated.feature` (mock feed with truncated post + reply, backfill, re-scrape repair).

## [1.25.1] - 2026-10-16

Bug fix since 1.25.0 (patch).

### Fixed
1. **`--said-by` exports looked like "comments only"**: the nick's own top-level posts were exported, but
   interleaved by date with their replies, with nothing marking which was which; replies in other people's
   threads (newest) filled the top of the page. Exports now separate and label them:
   - JSON: every node has `"kind": "post"` or `"reply"`; the document has `"counts": {"posts", "replies"}`;
     top-level `posts` lists thread posts first, then replies whose parent isn't in the export.
   - HTML: a "N posts · M replies" line, a **Posts** section, a **Replies in other threads** section, and a
     Post/Reply badge (`kind-post`/`kind-reply` class) on every article.
   Applies to every export (not only `--said-by`); `format_version` stays 2 (fields are additive, import ignores them).
2. `--said-by` help/docs now say explicitly that it keeps the nick's top-level posts **and** their replies.
3. Tests: unit tests for JSON kinds/counts/order and HTML sections/badges; e2e `--said-by --format json,html` scenario.

## [1.25.0] - 2026-10-15

One feature since 1.24.0 (minor 24 → 25).

### Added
1. **Several export formats per run**: `--format` takes a comma-separated list or can be repeated
   (`--format json,html`, `--format json --format html`); duplicates are dropped. With `--output <file>`
   / `--export <file>` each format is written as `<stem>.<ext>`; with a directory as `<dir>/<name>.<ext>`;
   with `--compress --tar-gz` all formats share one archive folder and `attachments/`. Several formats to
   stdout are rejected with a clear error. The 0-post empty-export skip applies to every format.
   `--list-nicks --format json` still selects JSON when `json` is among the formats.
2. Tests: CLI parsing of comma/repeated formats, output-path rule, JSON+HTML written side by side, both
   formats in one archive, empty-run skip for both, stdout rejection; two new e2e scenarios.

### Documentation
1. README shows the current version under the banner and a refreshed `--help` block; examples use the real
   `@hazaldin` profile; USAGE/README describe unknown-nick errors, empty-export skipping and `--debug`
   redaction; packaging metadata (PKGBUILD, RPM spec, debian/changelog, INSTALL.md) synced to 1.24.0.
2. New unit test: an empty run leaves an existing export file untouched.

## [1.24.0] - 2026-10-15

One security hardening since 1.23.0 (minor 23 → 24).

### Security
1. **Debug logs no longer leak session secrets**: `--debug` / `ACP_DEBUG=1` wire frames
   (`dbg_log`) now run through `redact_secrets` before stderr or `success.log`. JSON values
   for `_csrf_token`, `session`, `static`, cookies, and bearer-style tokens are replaced with
   `[REDACTED]`, as are `_csrf_token=` query params, Phoenix signed blobs (`SFMyNTY.…`), and
   long hidden-input `value="…"` attributes that carry CSRF tokens in LiveView HTML diffs.
   Event names, topics, and feed structure stay visible.

## [1.23.0] - 2026-10-15

Two fixes since 1.22.0, released together (minor 22 → 23).

### Fixed
1. **Unknown nick no longer scrapes Trending**: ACP answers `/rs/@{nick}` for a nick that does not
   exist with a 302 to `/rs` and the flash "No profile exists with the username @{nick}". The HTTP
   client follows redirects, so the Trending feed was scraped as if it were the profile (and
   `--said-by` then kept nothing). `fetch_feed` now checks that a profile URL landed on that same
   profile (and that the page has no "No profile exists" flash) and fails the nick with a clear
   error, which also goes to `error.log` when logging is on.
2. **No empty exports**: when a run kept 0 posts (and 0 mention hits), no export file is written,
   so an earlier good export of the same name is no longer overwritten with an empty one. A warning
   explains why nothing was written.

## [1.22.0] - 2026-10-15

One new feature since 1.21.0 (minor 21 → 22).

### Added
1. **File logging**: `--enable-logging` appends every message from the logging macros, one
   timestamped line each, to `~/.acp/logs/success.log` (progress, verbose detail, debug wire
   frames) and `~/.acp/logs/error.log` (errors and warnings). `--log <PATH>` and
   `--error-log <PATH>` override either file and turn logging on by themselves. `--debug`
   (or `ACP_DEBUG=1`) turns logging on too. The log directory is created as needed and files are
   opened in append mode. Stderr still follows `-q` / `-v` / `--debug` as before; files always get
   the verbose record. Paths live in `config.rs` (`LOGS_REL_DIR`, `SUCCESS_LOG_NAME`, `ERROR_LOG_NAME`).

## [1.21.0] - 2026-10-15

One new feature since 1.20.0 (minor 20 → 21).

### Added
1. **Remembered login**: after a successful login the session cookies are saved to
   `~/.acp/session.json` (owner-only, mode `0600`). The next run loads them, and if `/rs` no
   longer redirects to `/auth` it prints "Already logged in." and skips the one-time code.
   **`--logout`** (offline) clears the session and deletes the file; the next scrape asks for a
   code again. Cookie values are never logged. `logout`, `save_session` and `load_session` are
   exported from `auth::session` for the coming TUI.

## [1.20.0] - 2026-10-15

Three changes since 1.17.0 (minor 17 → 20).

### Added
1. **Immediate-parent relationship in the database**: new `posts.in_reply_to_id` column holds
   the post or comment a reply answers. The site keeps threads flat (`thread_id` only) and marks
   a reply to a comment with a `postRef` node in its body; that id is stored. `parent_post_id`
   keeps meaning the thread root. Both columns are indexed. Existing databases upgrade in place
   on first use (column added, replies backfilled with their thread root); a later scrape that
   sees the reply again records the real comment.
2. **Parent links in HTML exports**: each post is an `<article id="post-{id}">`, and every reply
   shows "Replying to {id}" (plus "thread {id}" when it answers a comment). Links jump to the
   anchor when the parent is in the export, otherwise to the post on acp.us.
3. **`in_reply_to_id` in JSON exports and imports**, next to `parent_post_id`. Imports of older
   exports fall back to the thread root. Searches now include each match's immediate parent too.

### Changed
- HTML anchors are now `#post-{id}` (were `#p-{id}`); the Mentions list links to them.

## [1.17.0] - 2026-10-15

One change since 1.16.0 (minor 16 → 17).

### Added
1. **`--said-by`**: limit a scrape to what the nick said — their own posts and their own replies
   from the profile's Posts and "Posts and replies" feeds. Other members' replies in the nick's
   threads are not crawled and the mentions crawl is skipped; a thread page is read only to
   recover a blank body of the nick's own item. Default scraping is unchanged.

## [1.16.0] - 2026-10-15

One change since 1.15.0 (minor 15 → 16).

### Fixed
1. **Profile photos for reply-only members** (e.g. `@haz`): the photo was only read from a
   top-level `author` on feed items, so replies that wrap the post (`{"reply": {...}}`,
   `{"post": {...}}`) or nest the author elsewhere left the profile with no photo, falling back to
   the site's `banner.jpg`. Feed items are now searched at any depth for the member's own author
   object (matched by `username`), plus flat `author_*` fields; site share images are skipped
   wherever they appear, so a real `media.acp.us` photo later in the data still wins. With
   `--debug`, a profile with no photo logs the first feed item so its shape can be checked.

## [1.15.0] - 2026-10-15

Three changes since 1.12.0 (minor 12 → 15).

### Fixed
1. **Blank posts**: replies (and some roots) were stored with empty `title`/`content`. Post bodies
   were only read as a ProseMirror doc in `body` (object or JSON string); a `body` that was HTML or
   plain text, or missing from the feed item, was silently dropped. Bodies are now read from
   `body` in any shape (ProseMirror, HTML, plain text, `{html|text|markdown}`), then `body_html`,
   `body_text`, `body_text_summary`, `text`, `content`, and wrapped `post`/`reply` objects. A post
   that still has no text and no media is read from its thread page. Blank rows already in the
   database are no longer treated as "known", so the next scrape re-fetches them and fills in
   their title/content in place (no duplicates, nothing deleted).
2. **Profile images**: the profile's photo is downloaded to
   `~/.acp/ai/data/attachments/profiles/{nick}.{ext}`, recorded as `avatar_path`, copied into
   exports as `attachments/profiles/…`, and shown in the HTML export's profile header. Author data
   is also read from the "Posts and replies" feed, so reply-only nicks get theirs. The site's
   generic `og:image` banner and `og:title` (e.g. "Trending") are no longer mistaken for a
   profile photo or display name.

### Changed
3. **Compressed exports clean up after themselves**: after the `.tar.gz` is written, the
   uncompressed `<name>/` folder beside it is deleted. If compression fails, the folder is kept.
- Project URLs (Cargo `repository`/`homepage`, Arch `url`, Fedora `URL`, Debian `Homepage`/`Source`,
  and the clone commands in README, INSTALL and USAGE) now point at the public GitHub mirror,
  https://github.com/m3talsmith/acp-rs-scraper, so anyone can clone and package it.

## [1.12.0] - 2026-10-15

Eight changes since 1.4.0 (minor 4 → 12): a cleanup release, plus the real "Posts and replies" mechanism and one bug fix.

### Changed
1. **Domain modules**: the crate is now a library (`src/lib.rs`) with a thin binary, split into
   `cli`, `config`, `logging`, `http`, `auth`, `liveview`, `scrape/{feed,thread,profile,mentions}`,
   `media`, `storage/{db,schema}`, `export/{json,html,archive}`, `import`, `maintenance`, `app`.
2. **Constants in one place**: `src/config.rs` (paths, site URLs, LiveView events, selectors, export
   format) grouped by domain; SQL schemas in `src/storage/schema.rs`. The site origin is configurable
   with `ACP_BASE_URL` (default `https://acp.us`).
3. **Tests moved out of source files** into per-module `tests.rs` files.
4. **Doc comments on every module, type, field and function** (`#![warn(missing_docs)]`; rustdoc clean).
5. **"Posts and replies" uses the verified site mechanism**: the profile Filter dialog
   (`configure-feed`, `replies=1`) → `/rs/@nick?replies=1`, loaded directly and paged with `page-next`,
   falling back to submitting the form. Replaces the 1.4.0 runtime tab heuristic. Still merged by
   `post_id`; `/rs/@nick/top?t=all` documented as the profile Top feed.

### Added
6. **Gherkin end-to-end suite** (`cucumber`): `tests/features/*.feature` + `tests/e2e.rs`, running the
   real binary against a mock acp.us (HTTP + LiveView websocket, OTP sign-in) with a temporary HOME:
   login, paging, threads/replies, attachments, incremental/`--full`, mentions, the replies filter and
   its fallback, search, export/import round trip, `--remove-nick`, `--clear-all`. Never touches the real site.

### Fixed
- Pre-commit credential hook matches token formats (AWS `AKIA…`, Google `AIza…`, GitHub, Slack, …)
  case-sensitively, so generated rustdoc search indexes no longer trip false positives; the
  password/secret assignment check still ignores case.
7. **First scrape on a fresh HOME**: saving the profile before any post created the data directory
   failed with "unable to open database file"; every write now creates `~/.acp/ai/data` first
   (found by the new end-to-end suite).

### Documentation
8. USAGE: "Posts and replies" filter, code layout and tests, `ACP_BASE_URL`; API docs regenerated.
   README opens with a Red Star banner, then an **Install** section (clone + `scripts/install.sh`)
   followed by the full `--help` output, so new users can install and start scraping without
   leaving the front page.

## [1.4.0] - 2026-10-15

### Added
- **"Posts & replies" profile tab**: nick scrapes also walk the profile's Posts & replies tab
  (the nick's replies in other threads), paged to the end and merged by `post_id` (never stored
  twice). Replies keep their thread root as `parent_post_id`; incremental/`--full` rules match
  the Posts feed. The tab is discovered at runtime from the logged-in profile HTML (link,
  `phx-click` event + `phx-value-*`, or JS push/patch); mocked tests only, not yet verified live.
- **Maintenance** (offline, under a new "Maintenance" heading in `--help`)
  - `--remove-nick @a,@b`: deletes the nicks' posts, other people's replies in their
    threads (`--keep-replies` keeps those), their attachment rows, mention rows and
    profile, in one SQLite transaction; files are deleted only when nothing else
    references them. Prints a summary.
  - `--clear-all`: deletes `~/.acp/ai/data/rs.database` and `~/.acp/ai/data/attachments/`;
    `preferences.toml` and exports are kept.
  - `--dry-run` previews; `-y`/`--yes` skips the prompt. Without `--yes`, a
    non-interactive stdin is refused; `--clear-all` requires typing `yes`.
- **Packaging, always built from source** (see `docs/INSTALL.md`)
  - Arch: `packaging/arch/PKGBUILD` (`makepkg -si`; remove with `pacman -R`).
  - Debian and derivatives: `debian/` source package (`dpkg-buildpackage`; `apt remove`).
  - Fedora/RHEL and derivatives: `packaging/fedora/acp-rs-scraper.spec` (`rpmbuild -ba`, SRPM; `dnf remove`).
  - `scripts/package.sh` builds `.deb`/`.rpm`/`.pkg.tar.zst` into `dist/`, using host tools or a
    debian/fedora/archlinux container (podman/docker).
  - `scripts/install.sh` detects the distro (`ID`/`ID_LIKE`), installs build dependencies,
    checks for Rust >= 1.88 (preferring rustup over old distro Rust), compiles locally and installs the
    native package; `--user` installs into `~/.local` with a manifest.
  - `scripts/uninstall.sh` removes the package **and purges `~/.acp` data by default** after one
    confirmation listing everything (`--yes` to skip it, `--no-purge` to keep data). Package managers alone
    never touch home directories.
  - Packages ship a man page and bash/zsh/fish completions (hidden `--generate` flag, clap_mangen /
    clap_complete) plus docs and license.

### Changed
- `rust-version = "1.88"` declared in `Cargo.toml` (edition 2024 needs 1.85, but the `scraper`
  dependency uses let-chains, stable since 1.88); `dist/` is git-ignored.

## [1.0.0] - 2026-10-15

First release of the ACP Red Star (acp.us/rs) scraper.

### Added
- **Login**: email + one-time-code sign-in over Phoenix LiveView, finishing with the
  `/auth/user/otp/sign_in` form POST; the email is remembered in `~/.acp/preferences.toml`.
- **Browser identity**: requests use Firefox headers and a persistent cookie jar.
- **Scraping**
  - `--nick` / `--nicks @a,@b`: scrape one or many members in one session.
  - Full profile feed paging via the site's `page-next` event (LiveView 1.2 replies).
  - Thread replies stored as children (`parent_post_id`) of their post.
  - Member profiles (display name, bio, avatar/banner, counts, join date).
  - Mentions (on by default, `--no-mentions` to skip): posts mentioning a nick by
    @mention, bare nick or display name, verified against the full thread;
    `--crawl-limit` bounds the site-wide crawl.
  - `--latest` (default) and `--trending` feed modes.
  - Incremental scrapes that stop at already-stored posts; `--full` forces a re-crawl.
  - Attachments (images, GIFs, video, audio) downloaded with posts; `--no-attachments`.
- **Storage**: SQLite at `~/.acp/ai/data/rs.database` with `posts`, `attachments`,
  `mentions` and `profiles` tables; re-runs never duplicate rows.
- **Search and sharing**
  - `--search` queries the local database offline, with `--nick`/`--nicks` filters.
  - `--format json|html`: JSON (default) or a self-contained HTML page.
  - `--output`, `--name`, and `--compress --tar-gz` for shareable archives.
  - `--export` / `--import` round-trip data between databases (export format v2,
    v1 still importable); `--import` uses `--compress --tar-gz` to unpack archives.
  - `--list-nicks` shows the members already in the database.
- **CLI**: grouped `--help` with examples, `--version`, `--quiet`, `--verbose`, `--debug`.
- **Docs**: `docs/USAGE.md`, generated API docs in `docs/api/` (`scripts/gen-docs.sh`).
- **Safety**: `.githooks/pre-commit` blocks credentials and secret files from commits;
  `build.rs` enables it automatically via `core.hooksPath`.
- **License**: GPL-3.0-or-later.

### Known limitations
- Paging through very long threads and some profile fields are not yet verified live.
- Mentions are only found within the crawled window (`--crawl-limit`).
