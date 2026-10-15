# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed
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
