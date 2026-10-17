# acp-rs-scraper: usage guide

Run `acp-rs-scraper` with no flags (or `--tui`) for the [terminal UI](#terminal-ui). With flags, `acp-rs-scraper` logs in to the Red Star forum (https://acp.us/rs) as Firefox and scrapes one or more nicks' posts, the replies in their threads, the posts that mention them, and their profiles into a local SQLite database. It downloads attachments and exports to JSON or HTML, optionally compressed. Search, export, import, `--list-nicks`, `--remove-nick` and `--clear-all` run offline.

- [Install and build](#install-and-build)
- [Terminal UI](#terminal-ui)
- [First run and login](#first-run-and-login)
- [Where data lives](#where-data-lives)
- [Flags](#flags)
- [Workflows](#workflows)
- [Database schema](#database-schema)
- [Export format (version 2)](#export-format-version-2)
- [Troubleshooting and debugging](#troubleshooting-and-debugging)
- [Known limitations](#known-limitations)
- [Contributing](#contributing)
- [API docs](#api-docs)

## Install and build

For distro packages (Arch, Debian, Fedora and derivatives), `scripts/install.sh` and uninstalling, see [INSTALL.md](INSTALL.md). Building by hand:

You need Rust 1.88 or newer (`rust-version` in Cargo.toml: edition 2024, plus let-chains in the `scraper` dependency) (install with [rustup](https://rustup.rs)). SQLite is compiled in (rusqlite `bundled`) and TLS uses rustls, so there are no system libraries to install.

    git clone https://github.com/m3talsmith/acp-rs-scraper
    cd acp-rs-scraper
    cargo build --release
    ./target/release/acp-rs-scraper --help

You can also run it through cargo with `cargo run --release -- <flags>`. The first build sets git's `core.hooksPath` to `.githooks` (see [Contributing](#contributing)).

## Terminal UI

`acp-rs-scraper` (no flags) or `acp-rs-scraper --tui` opens it. It needs a terminal on stdin/stdout
(at least 60×16); piped or scripted runs get an error naming the headless flags instead.

- **Layout:** top bar (name, version, session, running job) · menu bar (one item per group of CLI
  flags, Start Scanning pinned right) · nick bar on the left (`@nick` and post count) · Body · status
  line (`l` expands it to a log panel).
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
  clicking a header pops back to it, at most 8 deep. Ids not in the database say so.
- **Search** (Search menu, or `Shift-S` for keyword + nicks + post id + date range + posts/replies)
  replaces the Body in place; `<` / `< Back` returns, `Ctrl-S` clears.
- **Menu → flags:**

  | Menu | Flags it builds |
  |---|---|
  | Scrape (Start Scanning, `F5`) | `--nicks` (field, else the selected nicks) `--latest`/`--trending` `--no-mentions` `--said-by` `--crawl-limit` `--no-attachments` `--full` `--refetch-truncated` `--output --format` |
  | Search | in-app search (same matching as `--search`, plus the filters above) |
  | Export | `--export PATH` + `--nicks` (selection) or `--search`/`--nicks` (current view) or nothing (everything), `--format`, `--name`, `--compress --tar-gz` |
  | Import | `--import PATH` `--compress --tar-gz` |
  | List | re-reads the nick bar (`--list-nicks` data) |
  | Maintain | `--remove-nick` (field, else selection) `--keep-replies` `--dry-run`; Clear all = `--clear-all`; both get `--yes` after the TUI's own confirmation |
  | Session | Log in (modal) / Log out (`--logout`, then the Login modal) |
  | Bookmarks | Show (`B`), Bookmark current post (`b`: `--bookmark` / `--unbookmark`), Sync with acp.us (`--sync-bookmarks`) |
  | View | Nested / Stack / Stack + content pane / Carousel (infinite) / Carousel (fixed) / Grid; ◂ Previous post / Next post ▸ (TUI only) |
  | Sync | `--sync` `--nicks` (field, else selection) `--no-mentions` `--no-bookmarks`; on a timer when Auto sync is ticked |
  | Debug | `-v` / `--debug` / `--enable-logging` added to every job |

  Every argv is validated by clap before it runs, then runs as the headless binary in its own
  process group, logging to `~/.acp/logs/tui-<kind>-<time>.log` (mode 0600). Cancel kills it;
  quitting asks whether to keep it running in the background (default) or cancel it.
- **Previous / next:** `◂ prev` / `next ▸` on the nav bar (`,` `.` or `Alt-←` `Alt-→`) walk back
  and forth through the posts opened in the detail stack, with an `n/m` position; opening a new
  post or link drops the forward part.
- **View modes** (`v`/`V`, the nav bar's `[View: …]`, or the View menu): Nested (default), Stack
  (full posts in one column, flat details), Stack + pane (list above, content pane below that
  follows `↑`/`↓`), Carousel ∞ (wraps) and Carousel (stops at the ends) with `←`/`→`, and Grid
  (arrows move; the post opens in a modal with a Close button).
- **Bookmarks:** a `☆`/`★` star on every card and detail (click or `b`) bookmarks locally at once
  (a background `--bookmark`/`--unbookmark` job stores it, offline, next to a running scan); the
  **★ Bookmarks (n)** row on top of the nick bar (or `B`) shows only bookmarked posts and comments;
  Bookmarks › Sync with acp.us runs `--sync-bookmarks`.
- **Sync:** Sync menu: Auto sync, Interval (minutes, default 15), Nicks (blank: selected), Mentions
  (off), Bookmarks (on), Sync now, Save as default (`[sync]` in `preferences.toml`). An automatic
  sync runs `--sync` as a background job when it's due and nothing else runs; it checks the saved
  session first and skips quietly when offline or logged out.
- **Login** happens only when needed: Start Scanning checks `~/.acp/session.json` against the site
  and, when logged out, opens the Login modal (email, then the code or login link), then starts the
  scan.
- **Theme:** `[theme]` in `~/.acp/preferences.toml` (`accent`, `muted`, `border`, `on_accent`,
  `#rrggbb`); the default accent is ACP red `#dc2626`.
- **Keys:** `?`/`F1` lists them all next to the full `--help`.

## First run and login

1. Run a scrape, for example `acp-rs-scraper --nick alice` (or Start Scanning in the TUI, whose Login modal asks for the email and code instead of the prompts below).
2. **Email:** the tool reads `email` from `~/.acp/preferences.toml`. If the file doesn't exist, it creates `~/.acp/`, asks for your email, and saves it:

       email = "you@example.com"

   To use a different account, edit or delete that file.
3. The tool asks the site to email you login details (the same form as the "Create account or log in" button).
4. **Code:** enter the verification code from the email (e.g. `KWMRUV`), or paste the login link instead.
5. The tool submits the code, completes the sign-in form the site sends back, and checks that `/rs` no longer redirects to `/auth`.
6. With `--scrape` and no `--nick`/`--nicks`, it asks for a nick; a comma-separated list is accepted.

The session is saved to `~/.acp/session.json` (mode `0600`), so later runs skip the code until the site expires it; `--logout` (or Session › Log out) deletes it. Search, export, import, `--list-nicks`, `--remove-nick` and `--clear-all` never log in.

## Where data lives

| What | Path |
|---|---|
| Preferences | `~/.acp/preferences.toml` |
| Saved login session (mode `0600`) | `~/.acp/session.json` |
| Logs of jobs started from the TUI (mode `0600`) | `~/.acp/logs/tui-<kind>-<time>.log` |
| Database | `~/.acp/ai/data/rs.database` |
| Attachments (persistent copies) | `~/.acp/ai/data/attachments/<hash>.<ext>` |
| Default export / archive location | `~/.acp/ai/data/exports/` |

Every scrape is saved to the database. Exports are written on top of that.

## Flags

These are grouped the same way as `--help`. `-h`/`--help` prints help and `-V`/`--version` prints the version.

### Interface

| Flag | Meaning |
|---|---|
| *(no flags)* | Open the terminal UI. Logging flags (`-q`, `-v`, `--debug`, `--enable-logging`, `--log`, `--error-log`) don't count as flags here. |
| `--tui` | Open the terminal UI even with other flags; `--nick`/`--nicks` pre-select those nicks, logging flags pre-tick the Debug menu. |
| `--scrape` | Headless scrape without other scraping flags; asks for nicks when none are given (what a bare run did before 2.0). |

### Mode

| Flag | Meaning |
|---|---|
| `--latest` | Site-wide crawls use the Latest tab (`/rs/latest`). **Default.** It is chronological, so stopping early once pages are already known works. |
| `--trending` | Site-wide crawls use the Trending tab (`/rs`, the site's own default). It isn't chronological, so the early stop is disabled; paging runs until the end of the feed or `--crawl-limit`. |

The two can't be combined. Profiles always use their Latest feed (`/rs/@nick`) in both modes.

### Scraping

| Flag | Meaning |
|---|---|
| `--nick <NICK>` | One nick; the `@` is optional. Can't be used with `--nicks`. |
| `--nicks <LIST>` | A comma-separated list, e.g. `@hazaldin,@m3talsmith`. Spaces are trimmed, `@` is optional, and repeats are removed (ignoring case). |
| `--no-mentions` | Skip the mentions crawl, which is on by default. |
| `--said-by` | Only what the nick said: their own top-level posts **and** their own replies/comments, from the Posts and "Posts and replies" feeds. Other people's replies in the nick's threads aren't fetched and the mentions crawl is skipped (implies `--no-mentions`). A thread page is read only when one of the nick's own items has a blank body in the feed. Exports list the posts first and the replies after, each marked `"kind": "post"`/`"reply"` (JSON) and with a Post/Reply badge (HTML). |
| `--refetch-truncated` | Complete bodies that were stored shortened. Feeds send long posts and comments truncated (`body_truncated: true`, the "show more" case); the scraper now reads the full text from the thread page during every scrape, and this flag backfills rows already in the DB: every row whose `body_state` isn't `full` (rows saved truncated, and every row stored before 1.26.0, `unknown`) for the given nicks — or all nicks when none is given, in which case nothing else is scraped. One thread page per thread, fetched one at a time with a short pause. Needs a login. |
| `--crawl-limit <N>` | The most site-feed posts to scan when looking for mentions (default 500). |
| `--no-attachments` | Don't download media, avatars or banners. Profile images are otherwise saved to `~/.acp/ai/data/attachments/profiles/<nick>.<ext>` and shown in HTML exports. |
| `--full` | Re-crawl everything: ignore known post ids when paging, refetch every thread, and process attachments for every post (files already on disk are still reused). |

**"Posts and replies":** every `--nick`/`--nicks` scrape reads the profile twice. First it reads the default **Posts** feed (`/rs/@nick`). Then it reads the **Posts and replies** view (`/rs/@nick?replies=1`, the profile Filter dialog's second option), which adds the nick's replies in other people's threads. Both are paged to the end and merged by `post_id`, so nothing is stored twice; the database's unique `post_id` index enforces this too. Replies are stored with `parent_post_id` set to their thread's root post. Incremental runs stop each feed once a page is entirely already-known posts; `--full` walks both completely. If the `?replies=1` URL has no feed, the scraper falls back to submitting the Filter form (`configure-feed`, `replies=1`) on the profile's LiveView. The per-nick summary shows how many items came only from the replies view.

### Search / Export (offline)

| Flag | Meaning |
|---|---|
| `--list-nicks` | List the nicks in the database: display name, post count, latest post, whether a profile was scraped and when. Prints a table, or JSON with `--format json`. |
| `--search <Q>` | Case-insensitive search of title and content. Exports each match plus its parent post and direct replies. Writes to stdout or `--output`. |
| `--export <FILE>` | Dump the database (everything, or filtered by `--nick`/`--nicks` and/or `--search`) to FILE. Takes the place of `--output`. |
| `--name <NAME>` | Sets the export's file name, archive name, folder inside the archive, and HTML title. |

`--nick`/`--nicks` also filter `--search` and `--export` (posts by any listed nick).

Default names: `<nick>-<YYYYmmdd-HHMMSS>`, `nick1+nick2-<ts>` for several nicks (`multi-<ts>` for more than 4 nicks or more than 40 characters), `search-<slug>-<ts>` for a search, and `db-<ts>` for a full dump.

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
is dropped locally. The site page (`/rs/bookmarks`, override with `ACP_BOOKMARKS_PATH`) and the
LiveView event (`toggle-bookmark`) are confirmed against the live site (2.5.0).

### Sync

| Flag | What it does |
|---|---|
| `--sync` | Log in, sync bookmarks, then incrementally scrape `--nick`/`--nicks` if given. No export unless `--output`; never prompts. |
| `--no-bookmarks` | With `--sync`: skip the bookmark sync. |

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
acp-rs-scraper --nick @hazaldin --format json,html --output hazaldin/hazaldin.html   # hazaldin.json + hazaldin.html
acp-rs-scraper --export db --format html --format json                               # db.html + db.json
```

An archive contains `<name>/<name>.<ext>` (one per format) and `<name>/attachments/`. It's written to `--output`, or to `~/.acp/ai/data/exports/<name>.tar.gz` by default. `--tar-gz` is the only format for now. Once the archive is written, the uncompressed `<name>/` folder is removed; if compression fails it is left in place.

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
acp-rs-scraper --nick alice > alice.json

# Several nicks in one session, HTML bundle with attachments
acp-rs-scraper --nicks @hazaldin,@m3talsmith --format html --compress --tar-gz --name comrades
# -> ~/.acp/ai/data/exports/comrades.tar.gz

# Skip mentions; or scan fewer feed posts for them
acp-rs-scraper --nick alice --no-mentions
acp-rs-scraper --nick alice --crawl-limit 200

# Only the nick's own posts and replies (no thread crawl, no mentions)
acp-rs-scraper --nick alice --said-by

# Latest (default) vs Trending for the site-wide mentions crawl
acp-rs-scraper --nicks @hazaldin --latest
acp-rs-scraper --nicks @hazaldin --trending --crawl-limit 300

# Complete posts/comments stored with truncated ("show more") text
acp-rs-scraper --refetch-truncated                     # every nick in the DB, nothing else
acp-rs-scraper --nick alice --said-by --refetch-truncated   # backfill alice, then scrape as usual

# Incremental re-run (default) vs full re-crawl
acp-rs-scraper --nick alice          # stops paging at known posts, skips unchanged threads
acp-rs-scraper --nick alice --full   # refetches everything

# What's in the DB?
acp-rs-scraper --list-nicks
acp-rs-scraper --list-nicks --format json

# Offline search and export
acp-rs-scraper --search "general strike" --format html --output strike.html
acp-rs-scraper --search strike --nicks alice,bob > strike.json
acp-rs-scraper --export all.json
acp-rs-scraper --export alice.html --format html --nick alice
acp-rs-scraper --export strike.tar.gz --search strike --compress --tar-gz --name strike

# Round trip into another machine / fresh DB
acp-rs-scraper --export backup.tar.gz --compress --tar-gz
acp-rs-scraper --import backup.tar.gz --compress --tar-gz
acp-rs-scraper --import all.json
acp-rs-scraper --import alice.html

# Maintenance
acp-rs-scraper --remove-nick @spam,@bot --dry-run
acp-rs-scraper --remove-nick alice                  # prompts with counts
acp-rs-scraper --remove-nick alice --keep-replies -y
acp-rs-scraper --clear-all                          # type 'yes'; prefs and exports are kept

# Logging levels
acp-rs-scraper --nick alice -q
acp-rs-scraper --nick alice -v
acp-rs-scraper --nick alice --debug 2> wire.log
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
  body_state TEXT NOT NULL DEFAULT 'unknown'  -- 'full' | 'truncated' | 'unknown' (1.26.0)
);

-- body_state (1.26.0, index posts_body_state_idx): 'full' = complete body;
-- 'truncated' = only the feed's shortened preview could be read (the thread page
-- failed), re-read on the next scrape that sees it; 'unknown' = stored before 1.26.0.
-- --refetch-truncated re-reads the thread page of every row that isn't 'full'.

-- How nesting works on the site: threads are flat (every reply carries only
-- thread_id). Replying to a comment puts a postRef node {postId, threadId} at the
-- top of the reply's body; that postId is stored as in_reply_to_id.
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
  mime_type TEXT NOT NULL
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
- **Versions:** import accepts versions 1 (no profiles or mentions) and 2, and refuses newer versions or unknown formats.

## Troubleshooting and debugging

- **Login errors:**
  - Run with `--debug` and look at the `submit-code` reply and the sign-in POST (its field names are printed with `-v`).
  - Remove `~/.acp/preferences.toml` to enter a different email.
  - Run `acp-rs-scraper --logout` to forget the saved session (`~/.acp/session.json`) and log in again.
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

- **Check the output:** stdout carries only the export, so `acp-rs-scraper ... | jq .` works.

## Known limitations

- **Untested against the live site:**
  - Paging through a feed (`page-next {cursor}`).
  - Loading more thread replies through the InfiniteScroll `next-page` event (a guess).
  - Profile fields beyond author info, data-props and `og:` tags.
  - Whether media hosts require the login cookies.
- **Mentions are limited by the crawl:** there is no site-wide search in the site's JavaScript. Only `--crawl-limit` posts of the chosen feed are scanned, so older mentions, and replies inside threads whose root isn't in that window, can be missed.
- **"Posts and replies":** this is the profile Filter dialog (`<dialog id="js-configureFeed">`, form `configure-feed`, radios `replies=0|1`). Checked on the live site: choosing it switches to `/rs/@nick?replies=1`, the URL the scraper loads. The fallback form submission is covered by the mocked end-to-end tests. Profiles also have a Top feed (`/rs/@nick/top?t=all`), which isn't scraped.
- **Trending feeds:** these aren't chronological, so incremental stopping is disabled there.
- **Embeds:** YouTube, X and other embedded players aren't downloaded.
- **Logins:** a saved session is reused until the site expires it; then the next scrape asks for a fresh email code.
- **TUI:** images are listed as paths (with an `xdg-open` hint), not drawn in the terminal; Ctrl-Shift-S arrives as Ctrl-S in most terminals; cancelling a job kills it outright.
- **Truncated bodies:** feeds shorten long posts and comments (`body_truncated: true`, cut `body` plus `body_text_summary`). Checked on the live site: the thread page (`/rs/post/{thread}`) carries every item's full body with `body_truncated: false`, so the scraper reads it (once per thread) instead of pushing the "show more" action over LiveView.

## Contributing

The repo ships `.githooks/pre-commit`, a credential scanner, and `build.rs` sets `git config core.hooksPath .githooks` on the first `cargo build`.

The hook blocks commits that contain:
- **Secret files:** `preferences.toml`, `*.database`, `*.db`, `.env`, keys, cookies and similar.
- **Credential patterns:** private keys, cloud and GitHub tokens, JWTs, `_acp_key=` cookies and `password=`-style assignments.

It also runs `gitleaks` if installed. Generated API docs under `docs/api/` are scanned like everything else and currently pass cleanly. Never commit your database or preferences. Bypass with `git commit --no-verify` only if you're sure.

Licensed GPL-3.0-or-later (see `LICENSE`).

### Code layout and tests

The crate is a library with a thin binary (`src/main.rs` → `app::run`). Modules are split by domain: `cli`, `config` (all constants and site URLs), `logging`, `http`, `auth`, `liveview`, `scrape/{feed,thread,profile,mentions,refetch}`, `media`, `storage/{db,schema}`, `export/{json,html,archive}`, `import`, `maintenance`, `tui/{state,input,layout,commands,data,jobs,theme,widgets/,modals/}` and `app`. Unit tests live next to their module in `tests.rs` files (e.g. `src/cli/tests.rs`).

The end-to-end suite is Gherkin (`cucumber`). The scenarios are in `tests/features/*.feature` and the steps in `tests/e2e.rs`. It runs the real binary against a **mock acp.us** (`tests/support/mock.rs`: HTTP pages, the LiveView websocket, OTP sign-in, threads and media) with a temporary `HOME`. It covers:
- login (right and wrong code);
- profile paging, threads and replies, attachments;
- incremental runs and `--full`;
- mentions;
- the "Posts and replies" URL and its form fallback;
- truncated feed bodies completed from the thread page, and `--refetch-truncated`;
- search, export/import round trip, `--remove-nick` and `--clear-all`;
- the TUI entry (bare / `--tui` / `--scrape`) and the argvs its Scrape, Export and Maintain menus build.

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
