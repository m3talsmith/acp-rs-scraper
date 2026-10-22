# Plan: white-label core, and the ACP Red Star service in its own repo

Status: **decided 2026-10-22.** This is the plan. It does not move code, and it does not rewrite history. The scrub at the bottom is approved. Do not run it until step (c) has merged and the service tree is on the private repo. This repository is public, which is why the spec stays in the tree and ready.

Base of this write-up: `main` at `3c84eb2e` (crate `3.34.0`). No version bump.

The app stays Red Star. Michael is renaming this GitHub repository and its path to a Red Star name. This doc says "this repo" for the core tree. The placeholder for the GitHub path is `<core-repo>`, the same token as the `repository` field in `Cargo.toml` and `crates/acp_red_star_node_core/Cargo.toml`. Links inside the tree stay relative. After the rename, one patch pull request replaces that placeholder and the remaining repository URLs and badges in `README.md`, `docs/INSTALL.md`, `docs/USAGE.md`, `docs/GETTING-STARTED.md`, `SECURITY.md`, `architecture/PLAN-3.0-CLIENT.md`, and `.github/workflows/tag-on-main.yml`. Historical posts under `docs/history/` stay as written. GitHub redirects the old URL, so that patch does not change git remotes and does not push for the rename. The compatibility command in `Cargo.toml` is a separate name from the repository path and is not part of that patch.

Companion: [PLAN-SCRAPERS.md](./PLAN-SCRAPERS.md) (the 2026-10-21 trait design). This doc overrides that file only where the two disagree, and only for the ACP Red Star service. X and YouTube are unchanged.

## What Michael decided

- The ACP Red Star client (scraper, login, LiveView, sync, compose, interactions, Spaces, live updates, account writes, site notification parsing) leaves this repo. The shipped app is a white-label core. No acp.us scraping, site HTTP, or site websocket is compiled into that binary.
- Service source lives only in the private repo <https://github.com/m3talsmith/acp--red-star-service> (note the double dash). Default branch `main`. Nothing is pushed straight to that `main`. The first service tree is a draft pull request there.
- The core repo keeps the white-label core, the plugin gate, and the trait crate. The service crate depends on the trait crate, pinned by git revision (or a path, while both trees are local) until the crate is published.
- Every pull request that touches the service surface opens on `acp--red-star-service`. This repo only takes core and white-label pull requests. After the split has merged, still-open service pull requests here are ported, then closed with a link to the new draft. See `AGENTS.md`.
- A history scrub of this public repo is approved. Do not run it until step (c) has merged and the service tree is on `acp--red-star-service`. The spec below is the one to run. It is not permission to force-push before that. Do not contact GitHub Support from an agent session.
- The default built-in brand stays ACP Red Star. Changing that default is on hold. Options are at the end. Pull request #57 is not changed.

`#59` (Red Star as an in-tree service crate) is not this split. Do not merge it as written. Traits and the package gate from that branch can be reused in step (a). The Red Star crate does not stay in this repo. X (`#60`) and YouTube (`#61`) are not the ACP service and do not move to the private repo unless Michael says so. They are not part of the scrub.

## Trait crate

The plugin ABI does not go in `acp_red_star_node_core`. That crate is the P9 node (HPKE, SLIP-39, CBOR). Putting the ABI there would make the private repo depend on it.

New workspace member: `crates/acp_rs_plugin`.

- It owns `Service`, the capability traits, `CapabilityId`, records, `ConfigSchema`, `PluginDeclaration`, `PluginRegistrar`, `declare_plugin!`, and the object-safe `DynService` adapter.
- `core_version` in the declaration is this crate's `CARGO_PKG_VERSION`, not the root crate's version. `--doctor` prints this crate's rustc pin and that version.
- The private repo depends on it by git revision until it is published. The service is not a workspace member of this repo. The git pin uses `<core-repo>` until the rename patch fills in the path.
- The name is confirmed when the crate is published. Until then the path is `crates/acp_rs_plugin`.

This replaces PLAN-SCRAPERS §2.1's row that put the ABI in `acp_red_star_node_core::service`, and the row that linked `crates/acp_red_star_service_redstar` into this binary. The trait methods and the records in §2.2–§2.4 stay the contract.

A package in `~/.acp/plugins` that passes the gate is loaded. Missing `~/.acp/services/redstar.toml` does not load a package. That sentence in PLAN-SCRAPERS §2.5 applied to a built-in linked into the binary. It does not apply to the package.

## What stays in the core

These paths are the white-label app. They are not moved, and they are not in the scrub path list.

| Area | Paths |
|---|---|
| Identity and recovery | `src/crypto/`, `crates/acp_red_star_node_core/` |
| Encrypted database, local notices, history of stored rows | `src/storage/` (including `src/storage/notices.rs`) |
| Local notification list | `src/notify.rs`, `src/tui/widgets/notices.rs` |
| Preferences that are not a site session | `src/auth/prefs.rs` (re-homed before the scrub; see below) |
| Brands, slogan, logo, banner | `src/branding.rs`, `assets/acp-logo.png`, `assets/acp-logo.svg`, `assets/banner-red-star.jpg`, `assets/red-star.png`, `assets/red-star.svg`, `assets/social-acp-logo.png`, `assets/social-red-star.png`, and `src/brands.rs` when #57 lands |
| Wallpapers and the empty-body animation | `assets/wallpapers/`, `assets/empty-body.acpanim.gz`, `assets/empty-body.NOTICE.md` |
| Surprises | `src/eggs.rs`, `docs/.secrets/` |
| Themes, including download of a theme file from GitHub | `src/themes/` |
| TUI shell | `src/tui/` except the site login modal and the site interaction tests listed under "moves" |
| Search, export, import, maintenance of stored data | `src/export/`, `src/import.rs`, `src/maintenance/`, local search in the TUI and CLI |
| Local outbox queue | `src/outbox.rs` (the queue is core; sending a row to the site is the service) |
| Rules over stored rows | `src/rules/` |
| Research store | `src/research/` |
| Local bookmarks | the local table in `src/bookmarks/` (`--sync-bookmarks` against the site is the service) |
| Node protocol | `docs/PROTOCOL.md` |
| Political history posts | `docs/history/` |
| Packaging, changelog history, the export example name `comrades` | kept; history text is not rewritten except where the replace-text spec hits a site endpoint or frame literal |

Public pages stay as written: `https://acp.us/join`, `https://acp.us/program`, `https://acp.us/docs`. The slogan "The future belongs to the working class" stays. "Neon Lust" stays.

## What moves to acp--red-star-service

Whole trees:

- `src/liveview/` — LiveView socket and diff render
- `src/scrape/` — profile, feeds, thread, mentions, reads, refetch, site-wide crawl, edits
- `src/site/` — attach, delete, edit, ProseMirror, repost, subscribe, upload, vote, write
- `src/auth/session.rs` — cookie jar on disk
- `src/auth/mod.rs` — email and one-time-code login (after `prefs` has moved out)
- `src/media.rs` — post media JSON from the site
- `src/tui/modals/login.rs`
- `tests/fixtures/site/`
- `tests/support/mock.rs` and `tests/login_ws.rs`
- `architecture/INTERACTIONS-CAPTURE.md`, `architecture/UPLOAD-CAPTURE.md`, `docs/LIVE-PUBLISH-TEST.md`
- Generated rustdoc of those modules under `docs/api/` (the paths in `architecture/scrub-site.paths`)

Site Gherkin, moved with the service (the mock server moves with them):

- `tests/features/bodies.feature`
- `tests/features/edit.feature`
- `tests/features/login.feature`
- `tests/features/replies.feature`
- `tests/features/scrape.feature`
- `tests/features/sync.feature`
- `tests/features/truncated.feature`

Site TUI tests:

- `src/tui/tests/delete.rs`, `edit.rs`, `reads.rs`, `repost.rs`, `site_attachments.rs`, `subscribe.rs`, `sync.rs`, `vote.rs`

Core Gherkin that stays here: `offline.feature`, `research.feature`, `themes.feature`, `eggs.feature`, `tui.feature`.

## Mixed files, and the re-home rule

`git filter-repo --invert-paths` deletes a path from every commit, including the tip. A path that still exists when the scrub runs cannot be listed: the kept file would disappear with the old one. Step (c) re-homes anything the core still needs onto a path that never held site code, and deletes the site code, before the scrub. The live scrub re-reads `architecture/scrub-site.paths` against that tip and skips any path that is still present.

| File | Why it is mixed | What step (c) does |
|---|---|---|
| `src/auth/mod.rs` | Login and one-time-code flow, and it is the module parent of `prefs.rs` | Move `src/auth/prefs.rs` to `src/prefs.rs`. Then the whole `src/auth/` tree can leave. The rehearsal did not do this: `login` and `login_flow` were still in the rewritten tip. |
| `src/auth/prefs.rs` | Core preferences | Moves to `src/prefs.rs` before any path delete of `src/auth/`. |
| `src/http.rs` | Firefox-impersonating site client. `src/themes/download.rs` calls `crate::http::client` for a theme GET | Core gets a plain GET client on a new path. `src/http.rs` is scrubbed only after nothing in the core imports it. |
| `src/config.rs` | `DEFAULT_BASE_URL`, `/rs` URL builders, the LiveView websocket helper, media hosts, site attachment caps, beside TUI and database constants | Site constants move with the service. Core constants stay. The replace-text spec redacts endpoint literals that remain in history. |
| `src/bookmarks/mod.rs` | Local bookmark table, plus sync against the site | Local store stays. Site sync moves. |
| `src/cli.rs`, `src/app.rs`, `src/tui/state.rs`, `tests/e2e.rs` | They wire site actions and the mock | Site arms move with the service. The core binary still builds. |
| `tests/support/mod.rs` | Two lines: `pub mod mock;` | Not path-deleted until e2e no longer names `support`. |
| `tests/features/bookmarks.feature`, `tests/features/outbox.feature` | Local behaviour plus site frames | Split onto new filenames before any path delete. Do not path-delete these names while a core scenario still uses them. |
| `src/service.rs` | Today's outbox stub (`Outbox`, `LocalPost`, id `redstar`) | Replaced by the trait crate and the in-tree adapter in step (a), then the adapter goes away in step (c). |
| `src/lib.rs` module docs | The crate docs name the site, LiveView, and `/rs` | Rewritten when the modules leave. The replace-text spec covers the endpoint literals in history. |

`docs/api` rustdoc for preferences stays. Rustdoc for session, login, scrape, liveview, http, and media is in the path list.

## Traits and the data they exchange

The shapes are PLAN-SCRAPERS §2.2–§2.4. Short form of what crosses the crate boundary:

| Trait | Core passes in | Service returns |
|---|---|---|
| `Service` | nothing on the hot path | `ServiceId`, display name, `implemented()` as `&[CapabilityId]`, `schema()` as `&[Field]`, `availability(cap, RuntimeCtx)` |
| `Authenticator` | route (`Session` or `Api`) and the schema fields the user filled | sealed secret material the core stores; the service does not write the cookie file itself after step (b) |
| `Reader` | `Target` (`Account`, `Search`, `Playlist`, `Item`, `Conversation`), opaque `Cursor`, optional stop id, `Budget` | `Page`: `accounts: Vec<AccountRec>`, `items: Vec<ItemRec>`, `segments: Vec<SegmentRec>`, `next: Option<Cursor>`, `cost: Spend` |
| `Live` | the open view's `SourceId` | incremental `ItemRec`s for that view |
| `Notification` | a stored row or a local event | a notice the core keeps in `src/storage/notices.rs` |
| `Post`, `Reply`, `Upload` | local draft (body, attachments the core already has) | the service's native id, or a not-ready state |
| `Vote`, `Subscribe`, `Edit`, `Delete`, `Repost` | `SourceId` plus the capability's payload | success, or `Gated` / `NeedsAuth` |
| `Research` | namespaced id | the core's research store is the implementation for local adds; a service does not own that table |
| `Feeds` | a feed rule over stored rows, or a remote refresh target | rows the core inserts |
| `MediaFetcher` | a URL the service already resolved | bytes the core stores under the attachment path |
| `CostMeter` | the run's caps | `Spend` estimate and ledger delta |
| `Spaces` | a `Target` and a `Cursor` | a `Page` of Red Star Spaces. This is not an X Spaces read |

`probe(cap, subject)` takes a `Target`, a `Draft`, or a `Page`. A capability left off `implemented()` is not-implemented. A capability that is implemented still refuses an empty subject or the wrong kind.

`schema()` for Red Star is `SESSION_SCHEMA`: `email` and `base_url`, both stored as config.

`DynService` is the object-safe adapter. Each async capability returns `Pin<Box<dyn Future<Output = Result<…>> + Send + '_>>`. `declare_plugin!` emits `plugin_declaration`. The host checks rustc and `acp_rs_plugin`'s version before `register`. A mismatch refuses registration and does not quarantine. A bad `package.toml` or a sha256 mismatch quarantines to `~/.acp/quarantined/plugins/`.

## Host API

The service does not open the database, and it does not depend on `acp_red_star_node_core`. `Host` in `acp_rs_plugin` is how a service reaches the core's HTTP client, configuration, storage, sealed secrets, bookmarks, outbox, export, import, rules, research, maintenance, notices, preferences, and the binary name. The core implements that trait in `src/host_api.rs`. `Registry::register_service` calls `attach` with it. A service stores the `Arc<dyn Host>`. The trait crate has no SQLCipher dependency.

HTTP calls take extra request headers and a cookie jar the host keeps per service id. The service reads and writes that jar. It does not pass a `Cookie` header. `save_page` writes `body_state` (`full`, `truncated`, `unknown`) and attachment rows. `known_ids` returns those post ids and the stored reply count of each thread root, which is what a trait read uses to skip an unchanged thread. `export` takes `ExportRequest`: JSON or HTML, a compressed archive, profile images, and a chosen set of post ids, so a service does not ship its own exporter. `save_outbox`, `enqueue_outbox`, and `cancel_outbox` write the outbox. `sync_bookmarks` and `bookmark_pushed` keep a site-bookmark id mapping in `bookmark_sites`. `Outbox` and `LocalPost` are traits with methods, not only capability ids. `impl_default_caps!(Ty, except Reader, Post)` takes a comma-separated list. The `test-host` feature exposes `MemHost`, an in-memory host, so a service test does not open SQLite. The plugin crate is `0.3.0`.

This binary is the headless CLI. The terminal UI starts the same flags in a child process. Those flags call a loaded service that implements the capability. Until one is loaded, fetching, login, and publishing are not in this binary. After the sha256 gate, the loader opens the verified library, checks the ABI and plugin-crate version, and calls `attach`. A mismatch is refused and is not quarantined. A quarantined file is never opened. `--plugins-reload` unloads and loads. It is not bound to a key. The LiveView socket client stays in the service. It is site-specific.

`RESERVED_IDS` is `redstar`, `x`, and `youtube`. `OFFICIAL_IDS` is `redstar` only. The official package sets `[meta] official = "redstar"`, and `claim_allowed` accepts that pair. `x` and `youtube` stay refused, including when the manifest sets `official` to its own name. This is an allow-list, not a signature. The repository does not ship a private key. A package that rewrites its own manifest can write the field. That is the same limit as a rewritten sha256.

Bookmarks, search of stored rows, and edit history stay global actions. They are not traits.

Availability values stay `Ready`, `NeedsAuth`, `Gated`, `Limited`, `Off`.

## Extraction steps

Three pull requests, in order. Michael merges. No force-push in these three.

**(a) Traits, loader, and an in-tree adapter.** On this repo. Add `crates/acp_rs_plugin`. Add the package gate (tar name `<name>-<os>-<arch>-<version>.tar.gz`, `package.toml`, sha256 per binary and library, quarantine, `plugins.reload`, `--doctor` pin) as specified in `AGENTS.md` and PLAN-SCRAPERS §2.1. Red Star still runs through an adapter in this repo, so today's Gherkin stays green. No behaviour change. This is a core pull request.

**(b) The service crate, in the private repo.** Draft pull request on `acp--red-star-service`, not a push to its `main`. The crate implements the traits. Behaviour matches the adapter. `AGENTS.md` in that repo copies the safety rules from this repo: no force-push, tags only from `main` via the hook, mocked Gherkin, never commit credentials or raw captures. It also copies the routing rule. The history scrub is approved for this public repo only, after step (c) has merged and the service tree is on the private repo. The private repo does not get that exception. The dependency is a git revision of `acp_rs_plugin` (a path override is fine on a machine that has both trees). Pin `rev` to a commit that contains `crates/acp_rs_plugin`. Combined Gherkin runs against the mock in that repo.

**(c) Core ships without Red Star.** Normal pull request on this repo. Delete the service code, the site fixtures, the HAR and capture docs, and the site-specific docs listed above. Re-home the mixed files first. The core build does not feature-enable the service; the default build has no Red Star code. Core tests are `cargo test` with no service present. The Gherkin that talks to the mock moves with step (b) and passes there. Core Gherkin (offline search, export, import, themes, recovery, rules preview against stored rows, eggs) passes here.

After (c) is merged, and the service tree is on `acp--red-star-service`:

1. Port still-open service pull requests as new drafts on the private repo: `#49` interactions (`cursor/captured-interactions-2e75`), `#51` Spaces (`cursor/spaces-view-2e75`), `#52` live (`cursor/live-updates-2e75`), `#53` account (`cursor/account-writes-2e75`), and the Red Star half of `#59` if it is still open. `#58` is mixed (the Explore timeout is site-adjacent; the nick bar and theme samples are core) and is classified at port time, not copied wholesale. `#55` feeds: the local editor stays a core pull request; a site fetch is ported with the service. `#50` recovery and `#56` password re-key are core and stay here.
2. Close each original with a comment that links the new draft. Do not close before the port exists. Do not close without the link.
3. Delete the stale branch only after that pull request is closed.
4. Then, and only then, run the scrub below.

## How the service package is built

Archive name: `redstar-<os>-<arch>-<version>.tar.gz`.

`package.toml`:

```toml
[meta]
name = "redstar"
version = "0.0.0"   # the service crate's version, quoted semver
official = "redstar" # required for this reserved id. x and youtube cannot set this.

[install]
libraries = [
  { path = "libredstar.so", sha256 = "<64 lowercase hex digits of that file>" }
]
```

The gate reads `[install].libraries` and `[install].binaries` as arrays of tables. Each entry needs `path` and `sha256`. A top-level `[[libraries]]` array is not read. `[meta]` name and version match the archive name. `os` and `architecture` match the host. Every path is relative and stays inside the package. The sha256 is of the file, not a sidecar. `official = "redstar"` is how this package passes `RESERVED_IDS`.

CI on the private repo:

- builds the cdylib against the pinned `acp_rs_plugin` revision
- runs the service tests and the moved Gherkin against the mock (nothing contacts acp.us)
- packs the tar.gz, writes `package.toml` with the sha256 of the library, and checks that a core loader would accept it
- does not publish into this repo and does not push the package into this repo's releases

CI on this repo: `cargo test` with no service crate checked out. `--doctor` still prints the trait crate's rustc and version, which is how a package author sees the pin.

## History scrub

Approved. Do not run it yet. This repository is public, so once step (c) merges the old site client is still reachable from history until this scrub lands. The spec and the rehearsal below are the procedure. They are not permission to run it early.

Run it only after step (c) has merged and the service tree is on `acp--red-star-service` (a pushed branch there, not only a local copy). It does not apply to the private repo. `AGENTS.md` records the one force-push exception. Do not contact GitHub Support from an agent session.

Scope: Red Star site internals only. Wallpapers, backgrounds, brand assets, surprises, and core code stay. Political history docs stay. The export example name `comrades` stays.

### Spec

Paths: [`scrub-site.paths`](./scrub-site.paths). One path per line. A trailing slash is a directory prefix. Comment lines are not passed to filter-repo.

Replace-text: [`scrub-site-replacements.txt`](./scrub-site-replacements.txt). `literal:` and `regex:` only. A `glob:` line is a text pattern, not a filename filter, and must not be added back. Longer literals come first. There is no bare `https://acp.us` replacement, because that would eat `/join`, `/program`, and `/docs`. The regex `https://acp\.us(?!/join|/program|/docs)` rewrites the origin constant and leaves those three prefixes.

The replacement file, in full:

```
literal:wss://acp.us/live/websocket==>[redacted-site-endpoint]
literal:https://acp.us/live/websocket==>[redacted-site-endpoint]
literal:https://acp.us/rs==>[redacted-site-endpoint]
literal:media.acp.us==>[redacted-site-endpoint]
literal:posts-media.acp.us==>[redacted-site-endpoint]
literal:phx-trigger-action==>[redacted-site-frame]
literal:phx-submit==>[redacted-site-frame]
literal:phx-change==>[redacted-site-frame]
literal:phx-target==>[redacted-site-frame]
literal:phx_join==>[redacted-site-frame]
literal:phx_leave==>[redacted-site-frame]
literal:phx_reply==>[redacted-site-frame]
literal:user[otp]==>[redacted-site-field]
literal:submit-code==>[redacted-site-field]
literal:form[body_input]==>[redacted-site-field]
literal:_csrf_token==>[redacted-site-field]
regex:https://acp\.us(?!/join|/program|/docs)==>[redacted-site-endpoint]
```

Command to run from a fresh mirror after step (c) has merged and the service tree is on the private repo. Check the path list against that tip first, and skip any path that is still present (`PATH` includes the directory that holds `git-filter-repo`; 2.47.0 is the version the rehearsal used):

```sh
args=()
while IFS= read -r p; do
  case "$p" in ''|\#*) continue ;; esac
  # skip p if it still exists at the scrub tip
  args+=(--path "$p")
done < architecture/scrub-site.paths
git filter-repo --force --invert-paths "${args[@]}" \
  --replace-text architecture/scrub-site-replacements.txt
```

`--force` on filter-repo is the flag that tool wants when the clone has an origin. It then removes that origin. That flag is not a git force-push. Tags are rewritten in place: same names, new object ids. The live scrub force-pushes every branch and every tag once, with an explicit refspec, and only after the remaining service pull requests are ported or closed so a merge cannot put the old history back. Do not run that push before step (c) has merged.

### Rehearsal (throwaway, not published)

Run on a local clone of this workspace at `3c84eb2e`, not on a GitHub mirror, and not on `/workspace`. The clone contained the local branches and the 54 tags. Result commit `74bd3b55fed9f2ebd3d1547a724aa24950a8ff1c` is disposable. Do not push it. The live scrub runs on a fresh mirror of the post-split history, so every SHA below will differ.

| | Count |
|---|---|
| Commits in the pre-rewrite rev list | 296 |
| Commit rewritten to the null id | 1 (`ae9eae89`, "Note the edit capture that replaces an attachment.", empty once the capture note was path-deleted) |
| Commits reachable after the rewrite (`git rev-list --all`) | 295 |
| `origin/main` history before | 272 |
| Rewritten `main` history | 271 |
| Tags before and after | 54 names, zero missing, new object ids |
| Files at the tip before | 1883 |
| Files at the tip after | 1694 |
| Files removed from the tip | 189 |

Files removed from the tip, by group (the full list is [`scrub-site-rehearsal-removed.txt`](./scrub-site-rehearsal-removed.txt)):

| Group | Files |
|---|---|
| `docs/api` site rustdoc | 119 |
| `tests/fixtures` | 20 |
| `src/scrape` | 15 |
| `src/site` | 10 |
| `src/tui` site tests and the login modal | 9 |
| `tests/features` | 7 |
| `src/liveview` | 2 |
| capture docs (`INTERACTIONS-CAPTURE.md`, `UPLOAD-CAPTURE.md`, `LIVE-PUBLISH-TEST.md`) | 3 |
| `src/auth/session.rs`, `src/media.rs`, `tests/support/mock.rs`, `tests/login_ws.rs` | 4 |

Text at the rewritten tip:

- Gone: `wss://acp.us/live/websocket`, `https://acp.us/rs`, `media.acp.us`, `phx_join`, `phx-submit`, `_csrf_token`. `DEFAULT_BASE_URL` is `"[redacted-site-endpoint]"`.
- Pickaxe across the rewritten clone: 0 commits still introduce `phx_join`, `https://acp.us/rs`, or the websocket URL. 4 commits still mention `https://acp.us/join`. 2 mention Neon Lust. 6 mention the slogan.
- Kept on the tip: `src/eggs.rs`, `docs/.secrets/`, `src/auth/prefs.rs`, `docs/PROTOCOL.md`, `src/branding.rs`, 114 files under `assets/` including the wallpapers.
- Still present, because the path list does not delete mixed files that the core was still using: `src/auth/mod.rs` (`login` at line 529, `login_flow` at line 542) and `src/http.rs`. Selectors in the other mixed files remain, with the frame literals redacted. Step (c) removes that code. The rehearsal did not.

59 commits on `origin/main` at `3c84eb2e` touch the path list (generated `docs/api` excluded; it is produced from those modules):

`17f85941` `f8705511` `ae9eae89` `335e9574` `84ca8136` `98ce9928` `40795569` `6b59aa91` `d3745f15` `ec6e43ff` `227fdca4` `58bdd3b6` `db13d647` `71e07fa5` `af302bc6` `a3feaff3` `f9ffb5aa` `0ed8a091` `df1aa53f` `c4c13fff` `eb52d0e0` `12d3edd5` `6fcbd3cb` `358f68f1` `9df88962` `9219e665` `6ddd6b0f` `da9a3d26` `cd7fc7e9` `a60834a7` `803367e7` `bf4943ed` `31c3c4bf` `04c3d71b` `f641703a` `cf8fd9ef` `3692a822` `782333f7` `f3ed4c7d` `0400cd1b` `eb9e17f8` `c172c5c1` `067cf73b` `5a257214` `566a64d7` `49d1e73b` `891a3100` `affead9c` `c74739cb` `bfe17ba9` `4b535454` `ea50616b` `4dc6f2a4` `8c35334b` `f5b1fa19` `f3333f30` `7c4aed8d` `ad1b71b5` `41c6894b`

That list is a cross-check, not a substitute for the path list. The replace-text spec also rewrites endpoint and frame literals inside commits that never touched those paths.

Rewritten tag names from the rehearsal, with the throwaway short SHA. The live scrub keeps the same names and points each tag at its rewritten commit. There is no `v3.34.0` tag today.

`v1.0.0` `0f18f0cd`, `v1.4.0` `2c64d023`, `v1.12.0` `c8f77152`, `v1.15.0` `b5200c43`, `v1.16.0` `7830a42b`, `v1.17.0` `54c621f6`, `v1.20.0` `754b9412`, `v1.21.0` `e42ca20c`, `v1.22.0` `809f9ad1`, `v1.23.0` `b5f9a6cd`, `v1.24.0` `618ee075`, `v1.25.0` `6f6bf7f7`, `v1.25.1` `4c72a3b0`, `v1.26.0` `2445e302`, `v1.26.1` `a87691a3`, `v2.0.0` `6fe6211a`, `v2.1.0` `d2ebf8d3`, `v2.2.0` `a08ce8ae`, `v2.3.0` `f692be68`, `v2.4.0` `8415a806`, `v2.5.0` `75c8b439`, `v2.6.0` `85969a60`, `v2.7.0` `5a501853`, `v2.8.0` `a454b369`, `v2.9.0` `a546559f`, `v2.10.0` `6ebee8af`, `v2.11.0` `bc21184a`, `v2.11.1` `41caac13`, `v2.12.0` `f2b5401e`, `v2.13.0` `a437a7fd`, `v2.14.0` `66804406`, `v2.15.0` `e59f7d7f`, `v2.16.0` `1f0428b6`, `v2.17.0` `4ac08235`, `v2.18.0` `261aace5`, `v2.19.0` `a1719fb9`, `v2.20.0` `3b475c5b`, `v2.21.0` `67299dcd`, `v2.22.0` `2f7a15d3`, `v2.23.0` `04f594df`, `v2.24.0` `bdcd8c91`, `v2.24.1` `2ad11918`, `v2.25.0` `574c31da`, `v2.26.0` `64bb67ff`, `v2.27.0` `e8e1f678`, `v2.28.0` `dd55efc4`, `v3.0.0` `0ed7d0fd`, `v3.1.0` `db71d642`, `v3.2.0` `bb4d85c5`, `v3.3.0` `9ab4d52c`, `v3.3.1` `2dca06ac`, `v3.3.2` `16255dc2`, `v3.4.0` `2ab00d5a`, `v3.5.0` `f056b846`.

### What GitHub will still keep

After the scrub is pushed, GitHub can still serve the old commits from a closed pull request, a fork cache, or its own caches. Those objects are not reachable from the rewritten branches and tags. Only GitHub Support can purge them. Do not contact support from an agent session; Michael decides if he wants that request filed. The report after the live scrub names the new `main` SHA, the tag list, and which old SHAs a closed pull request can still see.

## Default brand (on hold)

Pull request #57 keeps the built-in `default` brand as ACP Red Star (logo, big ACP, title, "The future belongs to the working class"). `plain` stays opt-in: `acp-red-star --brand plain`, or `[ui] brand = "plain"` in `~/.acp/preferences.toml`. This plan does not change that.

Options, with no recommendation:

1. Keep #57. White-label is possible, and a first run still shows ACP until the user picks `plain` or another brand.
2. Ship `plain` as the built-in default, and let the ACP brand be one the service installs or the user picks. A missing `ui.brand` would change every current install. Avoiding that means a missing key keeps today's default, while a new install writes `brand = "plain"`.
3. The default follows the service. No Red Star package enabled means `plain`. The package enabled means the ACP brand. The core has to paint a header with no plugin loaded. Brand assets stay in the core, so the ACP brand can remain an optional built-in either way.

## Leaks this scrub is aimed at

Site internals that are still in the tree at `3c84eb2e`, outside the pure site directories:

- `src/config.rs` — origin, `/rs` builders, websocket URL, media hosts, attachment caps
- `src/lib.rs` crate docs — LiveView, login, `/rs`
- `src/http.rs` — the site client
- `src/auth/mod.rs` — the one-time-code login
- `CHANGELOG.md`, `README.md`, `docs/USAGE.md` — endpoint and frame names in shipped notes (the replace-text spec rewrites those literals; it does not delete the release notes)
- `docs/api/**` rustdoc generated from the site modules
- comments and fixtures named in the path list

Core comments that only mention the public site (`/join`, `/program`, `/docs`), the slogan, or the product name are not leaks of the client, and the spec leaves them.
