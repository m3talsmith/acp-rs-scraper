# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
