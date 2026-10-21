# Working rules for agents on ACP Red Star (acp-rs-scraper)

These are Michael's standing rules for anyone (human or agent) working in this repo.
Follow them on every change. When a rule conflicts with a task prompt, ask Michael.

## Releases and versioning
- **One minor bump per feature, one patch bump per fix.** Several features in one change means several minor bumps.
- **Every version bump edits `CHANGELOG.md`** in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format: a dated `## [x.y.z]` section with Added / Changed / Fixed / Removed subsections (one heading of each kind per release). Nothing ships from `## [Unreleased]`.
- **Every commit that changes the version gets an annotated tag `vX.Y.Z`.** The `.githooks/post-commit` hook creates it, so don't tag by hand. `.githooks/pre-commit` blocks a version bump without a matching changelog entry.
- Keep the version in sync everywhere: `Cargo.toml`/`Cargo.lock`, the README version line, `debian/changelog`, the Fedora spec and `PKGBUILD`.
- Never rewrite published history: no force-pushes, and never move or delete existing tags.

## Commits, pushes and PRs
- Commit when work is done **and tested**.
- **Push only when Michael explicitly asks for that push.** When he does, push to all remotes: `origin` (Forgejo) and `mirror` (GitHub).
- Commit messages describe what the change opens up for people, not just a list of edits.

## Security and credentials
- Never commit credentials. The pre-commit hook blocks secret-looking files everywhere and credential patterns everywhere except generated `docs/api/**`. `build.rs` sets `core.hooksPath=.githooks`, so don't bypass hooks (`--no-verify`).
- Never print, log or store credentials, session cookies, tokens or one-time codes. `--debug` output masks session secrets; keep it that way, and keep secrets out of test fixtures and captures (redact before committing).
- Forgejo push credentials are only ever used through a temporary git credential helper that's deleted afterwards.

## Testing
- **Smoke and manual runs always use a throwaway `HOME`, never the real `~/.acp`.** Auto-sync is on there, and a stray run can post or sync for real.
- End-to-end tests are mocked Gherkin scenarios (the `cucumber` crate with `.feature` files and a mock server). Tests must never touch the real acp.us site, X, YouTube or any other live service.
- Every write to a live site needs a mocked test, a preview, and a confirmation where Cancel is the default; `--dry-run` must show exactly what would be sent.
- `cargo build`, `cargo test` and `cargo clippy` (no warnings) pass before every commit. MSRV is 1.88 (check `rust-version` in `Cargo.toml` in case it has changed).

## Code and UI
- Rust 2024 edition, GPL-3.0-or-later.
- **All TUI styling goes through Michael's [`ratatui-lipgloss`](https://crates.io/crates/ratatui-lipgloss) crate** (the published crates.io version). If it's missing a feature, report the gap to Michael rather than quietly working around it.
- The header uses `tui-big-text`.
- Installs come from GitHub Releases binaries or building from source.
- Themes: TOML files in `~/.acp/themes/`. Keep the theme system in sync with grokbot-tui: when themes change here, note it so the change can be ported there (and vice versa).
- Decentralization uses libp2p.

## Docs
- README: the current version under the banner, install instructions near the top, then the full `--help` block. Keep the help block identical to the real `--help` output.
- `docs/USAGE.md` covers every flag with examples; update it alongside any CLI change.
- Keep `architecture/` plans current: when a phase ships, update its status and version numbers.
