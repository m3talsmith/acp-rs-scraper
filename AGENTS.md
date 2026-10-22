# Working rules for agents on ACP Red Star

These are Michael's standing rules for anyone (human or agent) working in this repo.
Follow them on every change. When a rule conflicts with a task prompt, ask Michael.
All rule changes and additions go in this file.

## Releases and versioning
- **One minor bump per feature, one patch bump per fix.** Several features in one change means several minor bumps.
- **A stub is not a feature.** A stub is code that has merged but is gated, disabled, or prepared and not sent, so a person cannot use it yet. It gets no version of its own. Record it in the next real release, under Added, as `prepared, gated: …`. It earns its minor when it is enabled and usable, and that release says so. A fix to a stub gets a patch only after that stub has shipped.
- **Every version bump edits `CHANGELOG.md`** in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format: a dated `## [x.y.z]` section with Added / Changed / Fixed / Removed subsections (one heading of each kind per release). Nothing ships from `## [Unreleased]`.
- **Tags are created and pushed only from main; never push a tag from a feature branch.** On any other branch `.githooks/post-commit` does not tag: it prints that tagging happens when the bump lands on main. A commit on local `main` is tagged by that hook. A merge on GitHub is tagged by `.github/workflows/tag-on-main.yml`, which creates `vX.Y.Z` only when `Cargo.toml`'s version changed and the tag does not exist. Don't tag by hand. `.githooks/pre-commit` blocks a version bump without a matching changelog entry.
- Keep the version in sync everywhere: `Cargo.toml`/`Cargo.lock`, the README version line, `debian/changelog`, the Fedora spec and `PKGBUILD`.
- Never rewrite published history: no force-pushes, and never move or delete existing tags. One exception is approved and not yet run: a history scrub of site internals on this public repo, after step (c) has merged and the service tree is on `acp--red-star-service`. The spec is in `architecture/PLAN-SERVICE-SPLIT.md`. Do not run it before that. Do not contact GitHub Support.

## Commits, pushes and PRs
- Commit when work is done **and tested**.
- When `main` moves, merge `origin/main` into each open feature branch (no rebase, no force-push) and renumber that branch so its versions follow main and any PR already merged. Keep both sides of a changelog, rules, or docs conflict. Each open PR stays independently mergeable against current main.
- **Push only when Michael explicitly asks for that push.** When he does, push to `origin` and `mirror` as they are already configured. Do not retarget either remote because the GitHub repository is being renamed. GitHub redirects the old URL. After that rename, one patch updates repository URLs and badges. The placeholder those fields use until then is `<core-repo>` in the Cargo `repository` lines. The app stays Red Star.
- Commit messages describe what the change opens up for people, not just a list of edits.

## Security and credentials
- Never commit credentials. The pre-commit hook blocks secret-looking files everywhere and credential patterns everywhere except generated `docs/api/**`. `build.rs` sets `core.hooksPath=.githooks`, so don't bypass hooks (`--no-verify`).
- Never print, log or store credentials, session cookies, tokens or one-time codes. `--debug` output masks session secrets; keep it that way, and keep secrets out of test fixtures and captures (redact before committing).
- A password change (`--change-password`) rewraps the database bootstrap, the node keystore, the recovery seal, the existing master secret, and the backup wrapping key in one step. A failure puts the previous bytes back. It does not change the SQLCipher key, derive a new master, or change the `acp-id` or node certificates. An archive written before the change still opens with the password that wrote it. An archive written after it is format 2 and restores on a new machine with the new password and the phrase.
- Push credentials for the Forgejo mirror are only ever used through a temporary git credential helper that's deleted afterwards.

## Plugins and quarantine
- An external plugin is a gzipped tar named `<name>-<os>-<architecture>-<version>.tar.gz` that contains `package.toml`. `version` is a quoted semver string (`version = "1.2.3"`). The app never downloads a plugin. Built-ins are linked into the binary and are not packages.
- Before `dlopen`, unpack the archive, check `package.toml`, and require the `sha256` field on each `[install]` `binaries` and `libraries` entry (`path` and `sha256`) to match that file. A top-level `[[libraries]]` array is not read. There is no sidecar hash. The archive name's name and version match the manifest. `os` and `architecture` match the host. Every path is relative and stays inside the package. The official package sets `[meta] official = "redstar"`. `x` and `youtube` stay reserved.
- Any failure of that check means the plugin is not loaded. The app logs it as a potential security threat and moves the archive to `~/.acp/quarantined/plugins/`. That directory is not scanned. A rustc (ABI) or plugin-crate mismatch refuses registration and does not quarantine the package. `--doctor` prints that rustc and core-version pin. Authors read it only there.
- Quarantine is `~/.acp/quarantined/<kind>/`. Plugins are the first kind. `--quarantine`, `--quarantine-show`, `--quarantine-restore`, and `--quarantine-delete` are the headless forms. Account › Services is the window. `plugins.reload` is not bound to a key.
- Reload unloads the loaded libraries and runs that same gate. A package that passes is opened. A failure of the archive check unloads it and quarantines it. A version mismatch refuses the new library and does not quarantine the archive. A call already in progress keeps the previous library mapped until it returns.
- A sha256 shipped inside the same package catches corruption and an accidental mismatch. It does not catch a package that was rewritten and rehashed. A user-pinned hash or signatures are possible later hardening.

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
- Brands: the logo, the title, and the motto. The built-in `default` brand is ACP Red Star. The built-in `plain` brand is a neutral title and a star with no motto (`--brand plain`). User files are `~/.acp/brands/<name>.toml` and override a built-in of the same name. A brand is independent of the theme. Credits and the other surprises keep their own text.
- Decentralization uses libp2p.

## Where a pull request goes

- A pull request that touches the ACP Red Star service opens on <https://github.com/m3talsmith/acp--red-star-service>, not on this repo. That surface is the scraper, login and one-time codes, the LiveView socket, sync against the site, compose and publish, interactions, Spaces, live updates, account writes, notification parsing from the site, HAR and capture docs, and site fixtures.
- This repo only takes core and white-label pull requests: traits, the plugin loader and gate, brands, themes, the encrypted database, identity, the terminal shell, and the rest of the core.
- After the split has landed, port each still-open service pull request on this repo as a new draft on `acp--red-star-service`, then close the original with a comment that links the new draft. Do not close the original before that draft exists. Do not close it without the link. Delete the stale branch only after the pull request is closed.
- The service repo carries the same safety rules in its own `AGENTS.md`. The history scrub is approved for this public repo only, and only after step (c) has merged and the service tree is on the private repo. The private repo does not get that exception.
- X and YouTube are not the ACP Red Star service. They stay in this repo unless Michael says otherwise.

## Services, input and UX
- Every service is its own crate behind the Service and capability traits, Red Star included. The UI and the CLI offer an action only when the focused or selected service implements that capability. A gated or not-ready action shows the reason.
- Until the service split lands, the in-tree Red Star client is on by default. Every other service is off until the user turns it on in Services preferences. After the split, the shipped core does not contain the ACP Red Star client. That service is a package the user installs in `~/.acp/plugins`. A package that passes the gate is loaded. A missing `~/.acp/services/redstar.toml` does not load a package. See `architecture/PLAN-SERVICE-SPLIT.md`.
- Credentials, keys, tokens, and cookies are sealed in the encrypted database and are never stored in a file. Other per-service settings live in `~/.acp/services/<service>.toml`.
- A service starts on the user's logged-in session. The user can make that service's API the default. A metered API has a monthly cap and a per-run cap, an estimate, and a confirm before spending.
- A write across services asks once and sends to every service where that write is ready. The others are skipped. The preview and `--dry-run` name each skipped service and the reason. Cancel is the default.
- Every action that notifies implements the Notification trait.
- Plugins use the registrar pattern in `architecture/PLAN-SCRAPERS.md`, including the version and ABI check. An installed package that passes the gate is loaded. A plugin is never downloaded by the app.
- Input is modal. Focus sets the mode. Global keys are Ctrl-<key>. Every other key is a mode hotkey. Help, tooltips, the action bar, and docs show the active keymap, not a hardcoded key. Bindings live in `~/.acp/keymap.toml`, reload in the app, and edit in Preferences. The design is `architecture/PLAN-MODES-KEYMAP.md`.

## Docs
- README: the current version under the banner, install instructions near the top, then the full `--help` block. Keep the help block identical to the real `--help` output.
- `docs/USAGE.md` covers every flag with examples; update it alongside any CLI change.
- Keep `architecture/` plans current: when a phase ships, update its status and version numbers.
