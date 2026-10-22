# Working rules for agents on ACP Red Star (acp-rs-scraper)

These are Michael's standing rules for anyone (human or agent) working in this repo.
Follow them on every change. When a rule conflicts with a task prompt, ask Michael.
All rule changes and additions go in this file.

## Releases and versioning
- **One minor bump per feature, one patch bump per fix.** Several features in one change means several minor bumps.
- **A stub is not a feature.** A stub is code that has merged but is gated, disabled, or prepared and not sent, so a person cannot use it yet. It gets no version of its own. Record it in the next real release, under Added, as `prepared, gated: …`. It earns its minor when it is enabled and usable, and that release says so. A fix to a stub gets a patch only after that stub has shipped.
- **Every version bump edits `CHANGELOG.md`** in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format: a dated `## [x.y.z]` section with Added / Changed / Fixed / Removed subsections (one heading of each kind per release). Nothing ships from `## [Unreleased]`.
- **Tags are created and pushed only from main; never push a tag from a feature branch.** On any other branch `.githooks/post-commit` does not tag: it prints that tagging happens when the bump lands on main. A commit on local `main` is tagged by that hook. A merge on GitHub is tagged by `.github/workflows/tag-on-main.yml`, which creates `vX.Y.Z` only when `Cargo.toml`'s version changed and the tag does not exist. Don't tag by hand. `.githooks/pre-commit` blocks a version bump without a matching changelog entry.
- Keep the version in sync everywhere: `Cargo.toml`/`Cargo.lock`, the README version line, `debian/changelog`, the Fedora spec and `PKGBUILD`.
- Never rewrite published history: no force-pushes, and never move or delete existing tags.

## Commits, pushes and PRs
- Commit when work is done **and tested**.
- **Push only when Michael explicitly asks for that push.** When he does, push to all remotes: `origin` (GitHub, `git@github.com:m3talsmith/acp-rs-scraper.git`, the primary) and `mirror` (Forgejo, `https://forgejo.acp.us/m3talsmith/acp-rs-scraper.git`).
- Commit messages describe what the change opens up for people, not just a list of edits.

## Security and credentials
- Never commit credentials. The pre-commit hook blocks secret-looking files everywhere and credential patterns everywhere except generated `docs/api/**`. `build.rs` sets `core.hooksPath=.githooks`, so don't bypass hooks (`--no-verify`).
- Never print, log or store credentials, session cookies, tokens or one-time codes. `--debug` output masks session secrets; keep it that way, and keep secrets out of test fixtures and captures (redact before committing).
- Push credentials for the Forgejo mirror are only ever used through a temporary git credential helper that's deleted afterwards.

## Plugins and quarantine
- An external plugin is a gzipped tar named `<name>-<os>-<architecture>-<version>.tar.gz` that contains `package.toml`. `version` is a quoted semver string (`version = "1.2.3"`). The app never downloads a plugin. Built-ins are linked into the binary and are not packages.
- Before `dlopen`, unpack the archive, check `package.toml`, and require the `sha256` field on each `binaries` and `libraries` entry to match that file. There is no sidecar hash. The archive name's name and version match the manifest. `os` and `architecture` match the host. Every path is relative and stays inside the package.
- Any failure of that check means the plugin is not loaded. The app logs it as a potential security threat and moves the archive to `~/.acp/quarantined/plugins/`. A rustc or core-version mismatch refuses registration and does not quarantine the package. `--doctor` prints that rustc and core-version pin. Authors read it only there.
- Quarantine is `~/.acp/quarantined/<kind>/`. Plugins are the first kind. The user can read the reason, restore, or delete from Preferences and from the CLI. Keys come from the keymap.
- Reload from the app runs that same gate and picks up newly placed plugins. A loaded plugin whose package changed is re-verified. A failure unloads it and quarantines it.
- A sha256 shipped inside the same package catches corruption and an accidental mismatch. It does not catch a package that was rewritten and rehashed. A user-pinned hash or signatures are possible later hardening.

## Testing
- **Smoke and manual runs always use a throwaway `HOME`, never the real `~/.acp`.** Auto-sync is on there, and a stray run can post or sync for real.
- End-to-end tests are mocked Gherkin scenarios (the `cucumber` crate with `.feature` files and a mock server). Tests must never touch the real acp.us site, X, YouTube or any other live service.
- Recovery stays a 24-word BIP-39 phrase and SLIP-39 shares of that same 256-bit secret. Each share this program issues is 33 words. Any 3 of 5 restore the secret. The password is not in a share. A 20-word share is accepted only so the official 128-bit SLIP-39 vectors still parse. Do not invent a 24-word share encoding. The published `slip39` crate does not build here because it depends on a yanked `bitvec`. The implementation in `crates/acp_red_star_node_core/src/slip39.rs` is the SLIP-39 construction, checked against the official vectors. `recovery.seal` stays the sealed 256-bit entropy. An existing paper phrase still restores.
- Every write to a live site needs a mocked test, a preview, and a confirmation where Cancel is the default; `--dry-run` must show exactly what would be sent.
- `cargo build`, `cargo test` and `cargo clippy` (no warnings) pass before every commit. MSRV is 1.88 (check `rust-version` in `Cargo.toml` in case it has changed).

## Code and UI
- Rust 2024 edition, GPL-3.0-or-later.
- **All TUI styling goes through Michael's [`ratatui-lipgloss`](https://crates.io/crates/ratatui-lipgloss) crate** (the published crates.io version). If it's missing a feature, report the gap to Michael rather than quietly working around it.
- The header uses `tui-big-text`.
- Installs come from GitHub Releases binaries or building from source.
- Themes: TOML files in `~/.acp/themes/`. Keep the theme system in sync with grokbot-tui: when themes change here, note it so the change can be ported there (and vice versa).
- Decentralization uses libp2p.

## Services, input and UX
- Every service is its own crate behind the Service and capability traits, Red Star included. The UI and the CLI offer an action only when the focused or selected service implements that capability. A gated or not-ready action shows the reason.
- Red Star is on by default. Every other service is off until the user turns it on in Services preferences.
- Credentials, keys, tokens, and cookies are sealed in the encrypted database and are never stored in a file. Other per-service settings live in `~/.acp/services/<service>.toml`.
- A service starts on the user's logged-in session. The user can make that service's API the default. A metered API has a monthly cap and a per-run cap, an estimate, and a confirm before spending.
- A write across services asks once and sends to every service where that write is ready. The others are skipped. The preview and `--dry-run` name each skipped service and the reason. Cancel is the default.
- Every action that notifies implements the Notification trait.
- Plugins use the registrar pattern in `architecture/PLAN-SCRAPERS.md`, including the version and ABI check. A plugin is enabled explicitly and is never downloaded by the app.
- Input is modal. Focus sets the mode. Global keys are Ctrl-<key>. Every other key is a mode hotkey. Help, tooltips, the action bar, and docs show the active keymap, not a hardcoded key. Bindings live in `~/.acp/keymap.toml`, reload in the app, and edit in Preferences. The design is `architecture/PLAN-MODES-KEYMAP.md`.

## Docs
- README: the current version under the banner, install instructions near the top, then the full `--help` block. Keep the help block identical to the real `--help` output.
- `docs/USAGE.md` covers every flag with examples; update it alongside any CLI change.
- Keep `architecture/` plans current: when a phase ships, update its status and version numbers.
