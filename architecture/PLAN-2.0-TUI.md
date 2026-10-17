# Plan: acp-rs-scraper 2.0 TUI

Status: **design only — do not implement until Michael reviews.**
Source of truth for UX: [SPECS.md](./SPECS.md) (includes Design expansions).

## Goal

Make the interactive default a terminal UI for browsing the local DB and
driving the same scrape/search/export/import/maintenance actions the CLI
already exposes — without breaking headless flag use.

## Constraints (from standing prefs)

- Work on alien-blue at this repo; commit when done/tested; **do not push**
- Opportunity-style commits; Keep a Changelog; GPL-3.0-or-later
- Public docs URL remains the GitHub mirror
- MSRV 1.88; edition 2024
- Minor bumps by feature count — **except this release is a deliberate 2.0.0**
  because default argv behavior changes

## Phase 0 — Agree the open questions in SPECS

Resolve the five open questions in SPECS before coding. Defaults if unstated:

1. Search filters Body in place with a clearable chip
2. Start Scanning while logged out → open Login, then continue on submit
3. Yes, persist cookies at `~/.acp/session` mode `0600`
4. No `--tui` required; document "no flags opens TUI"
5. Quit during scan → confirm modal (cancel scan + quit / keep scanning)

## Phase 1 — Session persistence (library, usable by CLI too)

- Save/load cookie jar next to preferences
- `logout()` clears jar + file
- `login()` unchanged UX; optional non-prompt path for TUI modal fields
- Unit tests: round-trip jar; "already logged in" short-circuit still works

## Phase 2 — CLI entry split

- `Args::wants_tui()` true when no mode/search/export/import/maint/generate flags
- `app::run` → `tui::run` vs existing headless paths
- Add scrape-intent clarity if needed so headless scrape remains obvious in help
- Update README / USAGE / `--help` examples: bare invocation opens TUI
- Cli tests for "empty argv ⇒ tui", "any scrape flag ⇒ not tui"

## Phase 3 — TUI skeleton

- Add `ratatui`, `crossterm` deps
- `src/tui/{mod,state,layout,event}.rs`: topbar, menu bar stub, nick list,
  body, footer/status
- Tick loop: draw → poll input → update state
- Empty DB and populated DB smoke paths against a temp database

## Phase 4 — Browse mode (offline)

- Nick bar from `list_nicks_db`
- Body: posts for selected nick (`search_db` / a dedicated `posts_for_nick`)
- Detail stack: push/pop, residual list strip, nested link open for ids in DB
- Render detail ≈ HTML export fields (meta, reply-to, body, attachments)
- Keyboard + mouse for list/detail navigation
- Unit tests for stack depth and link resolution

## Phase 5 — Menu → Args bridge

- Declarative command/option catalog derived from (or kept in sync with) clap
- Widgets: checkbox, dropdown, text field, button
- Start Scanning → build `Args` → run scrape on background task → status lines
- Search / Export / Import / Maintain wired similarly
- Help modal from `clap` help text
- Confirm modals for remove-nick / clear-all

## Phase 6 — Session UI

- Login modal (email + code) → existing auth, then persist
- Logout → clear → login modal
- Gate Start Scanning on session (per agreed open question #2)

## Phase 7 — Docs, changelog, release

- CHANGELOG 2.0.0: Added TUI default; Changed bare invocation; Session file
- README: screenshots optional later; document keys + default behavior
- Regenerate `docs/api` if public items added
- Full test suite + clippy; commit, **do not push**

## Suggested commit slices (opportunity style)

1. Persist login session for the next scrape
2. Open the terminal UI when you pass no flags
3. Browse every nick and post without leaving the terminal
4. Drive scrape and export from the menu bar
5. Release 2.0.0: the scraper you can read in

(Exact bundling can collapse if review prefers fewer commits.)

## Risks

| Risk | Mitigation |
|---|---|
| Nested detail layout fights small terminals | Min size check; collapse menu options; max stack depth |
| Scrape progress vs UI responsiveness | Dedicated task + channel of status events |
| Flag catalog drifts from clap | Generate help from `Command`; catalog tests against `Args` fields |
| Cookie format / security | `0600`, document in README; never log cookie values |
| Gherkin tests assume prompt-on-empty | Update e2e expectations for TUI vs headless |

## Done when

Michael can run `acp-rs-scraper` with no flags, browse haz/m3talsmith posts,
open nested reply details, log in from a modal, start a scan from the menu,
and still use every 1.20 CLI flag headlessly — with tests green and 2.0.0
committed locally.
