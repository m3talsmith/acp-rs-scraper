# Plan: modular services for ACP Red Star

Status: **design, revised with Michael's 2026-10-21 answers.** A second pass the same day settled captions, comments, live chat, combined views, global actions, notifications, plugins, and the one confirm for a mixed write. Three plugin questions stay open (§15). Nothing in this file is implemented.
Target path in the repo: `architecture/PLAN-SCRAPERS.md`.
Base: `main` at `b1a8222c` (crate `3.5.0`, pull request #45 merged). Version numbers in this plan are relative labels until the versioning cleanup settles the crate version.
Companion docs: [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) (§6 Feeds, §7.5 records, §7.14.13 rules and alerts, §7.14.14 post source) and [PLAN-4.0.md](./PLAN-4.0.md) (the research corpus).

Facts about the X API, the YouTube Data API and yt-dlp were checked on **2026-10-21** against docs.x.com, developers.google.com and the yt-dlp README (release `2026.08.19`). They change often. Re-check them before each phase: the X pricing and rate-limit pages, which X GraphQL operations are gated, the yt-dlp release notes (`max_comments` order, `live_chat`, EJS and JS runtime, PO tokens), and the YouTube quota calculator.

Michael accepts the churn of making Red Star one service among others. His words: "It's worth it."

---

## 0. Goal and decisions

Red Star, X and YouTube are **independent services**. They are separate crates, registered in one app, and the scheduler runs the enabled ones **in parallel**. More services will be added the same way. Red Star is not a special case inside the UI: it is the service that is on by default.

Decisions from 2026-10-21, which override any older sentence in this file:

- **Separate crates.** One crate per service, including Red Star. Parsers and network clients do not live only as modules of the root crate.
- **Plugins.** Each service crate registers through one registrar, the shape in Michael F. Bryan's *Plugins in Rust*: a plugin declaration, a registrar, a `declare_plugin!` macro, dynamic loading with `libloading`, and a rustc / core-version check. Rust has no stable ABI, so a mismatch is refused. Built-ins, including Red Star, are linked statically and still register that way, without `dlopen`. An external service is a `cdylib` the user places in `~/.acp/plugins/`. A plugin runs with the app's privileges, so the user enables it explicitly and the app never downloads one (§2.1).
- **Captions.** The user lists the locales they want. The default list is English first. Every available caption track in those locales is scraped. A locale the video does not have is skipped quietly, and the locales that were available are recorded (§3.3, §5.1).
- **Comments.** The full depth is scraped by default. Optional settings cap the depth and the count (§5.1).
- **Live chat replays** are in scope: `kind = chat` rows, shown on the Chat tab (§5.1, §14).
- **Combined views.** Mixed and grouped are both offered. The default is mixed, sorted by time: the updated time when the item has one, otherwise the posted time (§10).
- **Keys.** Modal input ships in v3, before launch. The design is [PLAN-MODES-KEYMAP.md](./PLAN-MODES-KEYMAP.md). The source switcher is a Ctrl global. The default chord in that keymap is `ctrl-]`, and it is remappable (§10.1).
- **Global actions.** Bookmarks, site search, and edit history are app actions. They are not capability traits (§2.3).
- **Notification.** Every action that notifies implements `Notification`, local toasts included. Phase A refactors the existing notices, rule engine, and alert-template code onto that trait if the split needs it (§2.3, §9).
- **What the user turns on.** Red Star (the party's web app at acp.us) is enabled by default. Every other service starts off. The user turns one on in the Services preferences screen, or with the CLI and config equivalents, and fills in that service's details only then.
- **Same settings shape everywhere.** Every service exposes its fields in the TUI and in the CLI or config. A `ConfigSchema` describes the fields. The TUI and the CLI render that schema, so a new service does not grow a hand-built settings form.
- **Secrets and config are split.** Keys, tokens, cookies and credentials are sealed in the encrypted database and are never written to a file and never printed. Other settings live in `~/.acp/services/<service>.toml`.
- **Routes.** The default route is the user's logged-in account (web session). The user can make that service's API the default instead. A service with more than one route exposes the same trio: route choice, credentials, pacing. A service with one route skips the route choice. Red Star has one route today (its web session).
- **X web session.** The whole read surface stays, search and replies included. §13 keeps the Terms of Service and account-suspension warning. The first time the user enables that session, the app shows the warning and waits for an acknowledgement. Pacing and daily caps are settings, not constants.
- **Spend.** Every service used through a paid or quota-metered API has a monthly cap and a per-run cap, a cost estimate, and a confirmation before spending. Cancel is the default.
- **Who to follow.** Nicks are per service, the same idea as Red Star. The user picks a nick that is valid on that service. `--nick`, `--said-by`, alerts and the other reader flags apply per service, with namespaced ids.
- **Media.** Video, audio and transcripts are all available. A storage cap bounds what is kept on disk.
- **Writes.** As a service makes a write possible, it gets the same set Red Star is growing: post, reply, upload, vote or like, subscribe or follow, edit, delete, repost. Reads ship before writes. Every write follows `AGENTS.md`: a mocked test, a preview, a confirmation whose default is Cancel, and a `--dry-run` that prints exactly what would be sent.
- **One confirm.** A write that spans services uses a single confirm. It sends to every service where that write is Ready. The others are skipped. The preview and `--dry-run` name each skipped service and why: not captured, not logged in, or disabled. Cancel stays the default (§2.4).
- **X cookies** are sealed in the encrypted database. There is no `~/.acp/x-session.json`.
- **Backups strip secrets.** A `.acpbackup` snapshot drops `source_secrets` before it is packed, so a restore asks for tokens and cookies again.
- **Peer sync.** X and YouTube records stay out of P9 replication by default.

Non-goals for the first phases:

- DMs, and any other user's private data.
- A cloud model.
- Spaces audio and livestream playback. Listing a live item is in scope when the Live capability says so. Playing the stream is not.
- Sending a write for a service that has not implemented that capability, or whose runtime state is still gated.

---

## 1. How this fits the current code

| Area today | File | What changes |
|---|---|---|
| Post model | `src/scrape/mod.rs` `Post` | Stays the shared row. It gains `source`, provenance fields and `kind` (§3). Service crates do not own this struct. |
| Red Star scrape, login, writes | `src/scrape`, `src/auth`, `src/site` | Move behind capability traits in Phase A. Behaviour stays the same. The code then lives in the Red Star service crate. |
| Schema and migrations | `src/storage/schema.rs`, `db.rs` `ensure_posts_schema` | Additive columns and the new tables (§3). The root crate remains the only writer. |
| Encrypted DB | `src/crypto/atrest.rs` | Same SQLCipher file. `source_secrets` is sealed with the unlocked database key and is omitted from backups. |
| Session | `src/auth/session.rs` (`~/.acp/session.json`) | Phase A still reads this file, so today's tests pass. Phase B seals the cookie into `source_secrets` and stops using the file. |
| Preferences | `src/auth/prefs.rs` | App-wide UI stays in `preferences.toml`. Red Star's service settings move to `~/.acp/services/redstar.toml` (§4). |
| Secrets interface | core `store.rs` `SecretStore`, `seal_blob` / `open_blob` | Holds every service secret. Plaintext never enters a TOML file, the log, or argv. |
| HTTP | `src/http.rs` | One client per service. The X API client uses its own headers. The X web client uses the Firefox identity the imported cookies came from. |
| Media | `src/media.rs` | Shared downloader for images and files the service has already resolved to a URL. yt-dlp stays inside the YouTube crate. |
| Crawl frontier | `CRAWL_SCHEMA` | Red Star's whole-site crawl becomes the Red Star `Reader` implementation. Other services use `source_cursors` (§7). |
| Jobs | TUI jobs, private pipe for the database key | The scheduler is the parent of those jobs. Each enabled service is one job, and jobs run together. |
| Rules and alerts | `src/rules` | `x` and `youtube` become real source values (§9). |
| Export | `src/export`, format version 2 | Bumps to **3** when non-Red-Star rows can exist (§3.5). |
| Writes | `src/site/*`, `CAPTURE_BLOCK` | Become per-capability runtime gates (§2.4). A Red Star write stays gated exactly as it is today until its own capture exists. |
| Tests | mock server, cucumber, `ACP_BASE_URL` | Phase A must stay green with no behaviour change. Later phases add a mock per service (§11). |

The core crate still does not open SQLite, draw the terminal, or talk to the network. Service crates may talk to the network. Only the root crate opens the database.

---

## 2. Traits, capabilities, registry, scheduler

### 2.1 Crates and plugins

| Crate | Role |
|---|---|
| `acp_red_star_node_core::service` | Traits, ids, records, `ConfigSchema`, capability ids, and the plugin ABI (`PluginDeclaration`, `PluginRegistrar`, `declare_plugin!`). No I/O. |
| `crates/acp_red_star_service_redstar` | Today's Red Star client, behind the traits. Linked into the binary. |
| `crates/acp_red_star_service_x` | X session route and X API route. Linked into the binary. |
| `crates/acp_red_star_service_youtube` | yt-dlp session route and Data API route. Linked into the binary. |
| Root crate | Registry, plugin loader, scheduler, SQLite, TUI, CLI. Maps records into `Post` and `Profile`. |

Every service lives in its own crate, including Red Star. The host loads them through one registrar. The shape is the one in Michael F. Bryan's *Plugins in Rust*: a `PluginDeclaration` the plugin exports, a registrar the host implements, a macro that fills the declaration, `libloading` to open a `cdylib`, and a check of the rustc version and the core crate version. Rust has no stable ABI. A plugin built with another compiler, or against another version of the crate that defines these traits, is not safe to call, so the host refuses it.

**Declaration.** The core crate owns the ABI.

```rust
pub struct PluginDeclaration {
    pub rustc_version: &'static str,
    pub core_version: &'static str,
    pub register: unsafe extern "C" fn(&mut dyn PluginRegistrar),
}

pub trait PluginRegistrar {
    fn register_service(&mut self, service: Box<dyn DynService>);
}
```

`declare_plugin!` does the job of that article's `export_plugin!`. It emits `#[no_mangle] pub static plugin_declaration`. `rustc_version` comes from the core crate's build script (`RUSTC_VERSION`, read with the `rustc_version` crate). `core_version` is that crate's `CARGO_PKG_VERSION`: the crate that defines the traits, not the root crate's version. One plugin may call `register_service` more than once.

**Object-safe boundary.** Authors still implement `Service`, `Reader`, `Post`, and the other traits in §2.3, including their `async fn` methods. Those async methods are not object-safe, and a Rust trait object cannot cross a `cdylib` unless the methods are. `DynService` is the object-safe adapter the host stores. Each async capability on it returns `Pin<Box<dyn Future<Output = Result<…>> + Send + '_>>`. `declare_plugin!` generates the adapter and boxes the futures. `implemented()` and `CapabilityId` stay the same ids, so §2.4 does not learn a second vocabulary. The host does not call a capability the service left off `implemented()`. A call that arrives anyway returns a not-implemented error.

The object file has no type for the `plugin_declaration` symbol. The host's cast is `unsafe`. The version check is what makes that cast honest, and it runs before `register`.

**Built-ins.** Red Star, X, and YouTube are workspace crates linked into the binary. Startup calls each crate's `plugin_declaration.register` on this same registrar. There is no `dlopen` for a built-in, and no version compare against itself: that code was just compiled with the app. Built-ins still enter only through `register_service`, so the registry has one list.

**External plugins.** An out-of-tree service crate is built as a `cdylib`. The user places the library in `~/.acp/plugins/` (`*.so`, `*.dylib`, or `*.dll`). The app does not download plugins and does not copy them into that directory. At startup the root crate:

1. Opens each library with `libloading`.
2. Reads the `plugin_declaration` symbol.
3. Refuses the plugin when `rustc_version` or `core_version` differs from this binary. It does not call `register`. The Services screen lists the path, the versions the plugin declared, and the versions this binary required. The rest of the app starts.
4. Otherwise calls `register`. Each service is stored in a proxy that holds `Arc<Library>`, the same idea as that article's proxy: the library has to outlive every service object that came out of it. Dropping the service must not unload the library.
5. Leaves the library mapped until the process exits. This plan does not unload or reload a plugin while the app is running. A file replaced on disk is picked up on the next launch. Unloading under a live object would call into unmapped code.

A plugin's crate depends on the core crate only. The supported interface does not hand it a SQLite connection, the database key, or the root crate. Built-ins register before any file is opened. A later `id()` that collides with one already registered is refused, and the first registration stays. A plugin cannot replace Red Star, X, or YouTube, and two plugins cannot share an id.

**Config, secrets, and TOML.** Once a service is on the registrar, built-in or plugin, `schema()` is the same `ConfigSchema` as §2.2 and §4. Fields marked `Config` live in `~/.acp/services/<id>.toml`. Fields marked `Secret` seal into `source_secrets` as `<id>.<field>`. The host reads the file and unseals the secrets, then passes the values in `RuntimeCtx`. A plugin does not get a second config format. A discovered plugin stays **off** until the user enables it, the same rule as X and YouTube. Red Star stays the built-in that is on when its file is missing.

**Allocator.** Freeing memory with a different allocator is undefined. Both sides use Rust's system allocator, and the rustc check is what makes that the same allocator. The host drops `Box<dyn DynService>` itself. It does not take a separate `free_plugin` function from the library. A mismatched plugin never reaches `register`, so the host never frees that library's boxes.

**Security.** A loaded plugin is native code inside this process. It runs with the app's full privileges: the unlocked database, any secret the host has already opened, the network, the filesystem, and the memory of the other services. This design has no sandbox. The gates are the ones above. The user places the file, the version check passes, and the user enables that service by an explicit action. A file sitting in `~/.acp/plugins/` is not enabled. The app never auto-downloads a plugin. No signature check is in this plan; whether one should be is open (§15).

Phase A does not scan `~/.acp/plugins/`, so today's runs do not load extra code. The loader arrives in Phase B with the Services screen.

### 2.2 Service and the shared types

```rust
pub struct ServiceId(pub &'static str); // "redstar" | "x" | "youtube"

pub trait Service {
    fn id(&self) -> ServiceId;
    fn display_name(&self) -> &'static str;
    /// The capability traits this crate implements. Fixed for the life of the binary.
    fn implemented(&self) -> &'static [CapabilityId];
    fn schema(&self) -> &'static [Field];
    /// Logged-in, gated, over cap, and so on. Changes between calls.
    fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability;
}

pub struct SourceId { pub service: ServiceId, pub native: String, pub sub: Option<SubId> }

pub enum RouteKind { Session, Api }

pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub kind: FieldKind,          // bool, text, int, enum, string list, secret
    pub store: Store,             // Config → TOML, Secret → sealed DB
}
```

`SourceKind` in stored rows uses the same strings as `ServiceId`: `redstar`, `x`, `youtube`.

Namespaced ids stay as in §3.2: Red Star ids bare, `x:…`, `yt:…`. Nicks are `x:alice` and `yt:@channel`. The UI strips the prefix and draws a badge.

### 2.3 Capability traits

A feature is a capability trait. A service implements it or it does not. Red Star's current features are split onto these traits in Phase A, with no change to what the user can do.

| Trait | What it means | Red Star today |
|---|---|---|
| `Authenticator` | Routes (`Session`, and `Api` when the service has one), each with its own fields in the schema | Session only. No route picker. |
| `Reader` | Incremental, resumable scrape of accounts, searches, threads | The scraper, including whole-site |
| `Live` | Updates for the open view without a full scrape | The LiveView socket |
| `Notification` | Every action that notifies, including local toasts | The app's toasts implement it. Phase A moves notices, the rule engine, and alert templates onto it when that split needs a refactor (§9). |
| `Post`, `Reply`, `Upload` | Compose and attachments | Implemented, send still gated where `CAPTURE_BLOCK` gates it |
| `Vote`, `Subscribe`, `Edit`, `Delete`, `Repost` | The interaction set | Stubs, gated until a capture exists |
| `Research` | Add the current item to a research project | App research store, keyed by namespaced id |
| `Feeds` | This service can refresh a feed rule | Red Star feeds when P7 exists. X and YouTube join then. |
| `MediaFetcher` | Video, audio, transcripts, thumbnails | Red Star attachments. YouTube and X in Phase D. |
| `CostMeter` | Estimate, ledger, monthly cap, per-run cap | Unused on Red Star. Required on a paid or quota API route. |

`Publisher` is not a separate grab-bag. Each write is its own trait, so the UI can hide Vote on a service that will never vote and still show Reply.

Bookmarks, site search, and edit history are **global actions**. They are not capability traits, and they are not facets of `Reader`. Their menus and flags stay available no matter which service is focused.

- A bookmark is a local save of any namespaced id. The Bookmarks menu that already exists (Show, Bookmark current, Sync with acp.us) stays that menu. When the remote push sends, it is still this same action, aimed at the item's own service, and it follows the write rules in `AGENTS.md`. It does not grow a `Bookmarks` trait, and §2.4 does not hide it.
- Site search reads the local database across the rows that are stored. A service's own remote search (X search, YouTube search) stays a `Reader` target (§5.6).
- Edit history is the `post_versions` view of the focused item. It is not a trait.

`Reader` page shape (one page, newest first, stop early for incremental sync):

```rust
pub struct Page {
    pub accounts: Vec<AccountRec>,
    pub items: Vec<ItemRec>,
    pub segments: Vec<SegmentRec>,
    pub next: Option<Cursor>,
    pub cost: Spend,
}

pub trait Reader: Service {
    async fn resolve(&self, t: &Target) -> Result<Resolved>;
    async fn page(&self, t: &Resolved, cur: &Cursor, stop_at: Option<&str>,
                  budget: &mut Budget) -> Result<Page>;
    async fn expand(&self, item: &SourceId, what: Expand, budget: &mut Budget) -> Result<Page>;
}
```

`Budget` carries that service's run caps. The adapter decrements it before a request and returns `BudgetExhausted` rather than overspending. `Cursor` is opaque text (an X token, a GraphQL cursor, a YouTube `pageToken` or playlist index).

Suggested `Target`: `Account`, `Search`, `Playlist`, `Item`, `Conversation`. A service that cannot express one of those does not advertise it. YouTube has no X-style conversation target. X has no playlist target.

### 2.4 Capability discovery

Two layers.

**Static.** `implemented()` is the set of traits the crate has. It does not depend on login or caps. The CLI builds flag groups from it. The TUI builds menus from it. Help for that service lists only this set.

**Runtime.** `availability(cap, ctx)` is one of:

| State | Meaning | Control |
|---|---|---|
| `Ready` | Usable now | Shown and enabled |
| `NeedsAuth` | Implemented, not logged in, or the token is missing | Shown, disabled. Tooltip: "Sign in to X" (or the service's words) |
| `Gated` | Implemented, not sent until a capture is in the tree | Shown, disabled. Tooltip: "Gated until captured" |
| `Limited` | Over the monthly cap, the per-run cap, the daily cap, the storage cap, or a pacing pause | Shown, disabled. Tooltip names the cap |
| `Off` | The user has not enabled the service | The service's controls are absent |

Rules for the active or selected service:

- The trait is absent: the menu item, keybinding and flag are **hidden**. There is nothing to hover, so there is no tooltip. `--help` for that service does not list the flag.
- The trait is present but not `Ready`: the control is **shown and disabled**, with a tooltip that says why.

Rules for a combined view. Both layouts exist (§10): mixed by time, which is the default, and grouped by service. These rules apply to both.

- A control is shown when **any service currently in the view** implements the trait.
- It is enabled when the highlighted item's service is `Ready` for that trait, or when a multi-selection contains at least one `Ready` item.
- The tooltip names the services that will be skipped and why ("X does not implement edit", "YouTube vote is gated until captured").
- The action never crosses services. An X id is not sent to Red Star's vote hook.
- The preview, and `--dry-run`, print one block per selected item: the payload that would be sent, or the skip reason. Nothing is sent for a skip. `--dry-run` does not ask and does not connect.
- One confirm covers every service where that write is `Ready`. Cancel is the default. `--yes` affirms that one confirm. It does not ask again per service.
- Every service that is not `Ready` is skipped, and nothing is sent for it. The preview, `--dry-run`, and the confirm name each skipped service and why, in these words: **not captured** (`Gated`), **not logged in** (`NeedsAuth`), or **disabled** (the service is off). A service that does not implement the trait is named too ("X does not implement edit"). A service over a cap is not `Ready` either; the line names the cap.

A shared flag (`--nick`, `--said-by`, `--limit`, `--sync`) runs on every **enabled** service that implements `Reader`, using that service's namespaced ids. A flag for a capability only some services implement (`--vote`) follows the All-view rule above.

### 2.5 Registry and scheduler

The registry is the list of services that registered at startup (built-ins, then any external plugin that passed the version check) plus the user's enabled set. A refused plugin is listed and is not in this set (§2.1).

- Missing `~/.acp/services/redstar.toml` means Red Star is **on**.
- Missing `x.toml` or `youtube.toml` means that service is **off**. Creating the file with `enabled = false` is the same as off.
- Enabling a service opens its schema. Secret fields go to the database through a no-echo prompt. Config fields go to the TOML file.

The scheduler starts one job per enabled service that has a due target. Jobs run at the same time. Each service has its own rate limit, and that limit is not shared with the others. SQLite writes commit per page, so two services can insert without sharing a crawl cursor. Within one service the service's own pacing still holds (X web stays serial; only one yt-dlp process at a time).

`--sync` with no service runs every enabled service together. `--sync --source x` runs only X.

---

## 3. Schema changes and migrations

All changes are additive. The SQLCipher file stays `~/.acp/ai/data/rs.database`. Each change follows `ensure_posts_schema`: a `pragma_table_info` check, then `ALTER TABLE`, and it is safe to run on every open.

Phase A does **not** apply the non-Red-Star columns. It only moves Red Star behind the traits. The columns land with the first phase that can store another service's rows (Phase C).

### 3.1 `posts`: provenance columns

This is the §7.14.14 sketch, landed as a 3.x change before 4.0.

```sql
ALTER TABLE posts ADD COLUMN source TEXT NOT NULL DEFAULT 'redstar';
ALTER TABLE posts ADD COLUMN source_id TEXT NOT NULL DEFAULT '';
ALTER TABLE posts ADD COLUMN source_url TEXT NOT NULL DEFAULT '';
ALTER TABLE posts ADD COLUMN source_start_ms INTEGER;
ALTER TABLE posts ADD COLUMN source_end_ms INTEGER;
ALTER TABLE posts ADD COLUMN speaker TEXT NOT NULL DEFAULT '';
ALTER TABLE posts ADD COLUMN kind TEXT NOT NULL DEFAULT '';
ALTER TABLE posts ADD COLUMN lang TEXT NOT NULL DEFAULT '';
ALTER TABLE posts ADD COLUMN metrics_json TEXT NOT NULL DEFAULT '';
ALTER TABLE posts ADD COLUMN fetched_via TEXT NOT NULL DEFAULT '';
CREATE INDEX IF NOT EXISTS posts_source_idx ON posts(source, posted_at);
CREATE INDEX IF NOT EXISTS posts_source_id_idx ON posts(source, source_id);
```

`fetched_via` is `liveview`, `x-api`, `x-web`, `yt-dlp` or `yt-api`.

- Existing rows keep `source = 'redstar'`. An optional backfill sets `source_id = post_id` and `source_url` to the Red Star permalink.
- `kind` for old rows: empty `parent_post_id` means `post`, otherwise `reply`.
- `post_versions.source` stays the edit-observation column (`site|peer|local|draft`). X edit history, when a later write or a read of edits exists, uses `source = 'site'` to mean "seen on the origin service". No table rebuild.

### 3.2 Namespaced ids and nicks

`posts.post_id` is globally unique, and bookmarks, research items, tags, mentions, attachments and rule firings key on that text.

- **Red Star ids stay bare** (22-character base64url). No existing row or export changes.
- **Other services are prefixed:** `x:<id>`, `yt:<video id>`, `yt:<video>/c:<comment>`, `yt:<video>/t:<start_ms>`, `yt:<video>/chat:<message>`. Base64url never contains `:`, so the prefix cannot collide.
- **Nicks** are `x:alice` and `yt:@somechannel`. `profiles.nick` and `posts.nick` store the prefixed form. `--said-by`, the nick bar, rule authors and "watch user" keep working and cannot merge two people named alice. The UI hides the prefix and shows a badge.
- `profiles` gains `source TEXT NOT NULL DEFAULT 'redstar'` and `source_user_id TEXT NOT NULL DEFAULT ''`. The X numeric id and the YouTube `UC…` id are the stable keys. A handle change is recorded in `account_aliases`.
- `research_items.kind` keeps its current `CHECK`. New items use `post` or `reply` with a namespaced ref. `posts.kind` carries video, comment, segment.

### 3.3 Transcript segments

One transcript turn is one `posts` row, as PLAN-4.0 describes:

- `source = 'youtube'`, `kind = 'segment'`, `parent_post_id = in_reply_to_id = 'yt:<video>'`.
- `source_start_ms` and `source_end_ms` hold the timing. `speaker` stays empty until 4.0 diarization.
- `nick` is the channel.

Cues merge into turns on sentence boundaries, at most about 30 seconds and about 600 characters. The merge is deterministic, so a re-run dedupes on `post_id`. The raw subtitle file is an attachment (`text/vtt` or `application/json`) so a later re-segmentation can start from the original.

The user lists caption locales in the service file. The default list is English first (`caption_locales = ["en"]`). Every available caption track in those locales is scraped, manual and automatic. A requested locale the video does not offer is skipped quietly: not an error, not a notice, and not a failed stage.

The video's `metrics_json` records `transcript_kind` (`manual`, `auto`, or `none`) for what was stored, plus `caption_locales_available` (every locale the source offered) and the locales that were actually stored. When none of the requested locales were available, the stage still finishes, `transcript_kind` is `none`, and the available list is still recorded.

### 3.4 New tables

```sql
CREATE TABLE IF NOT EXISTS source_targets (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source TEXT NOT NULL, kind TEXT NOT NULL,
  value TEXT NOT NULL, resolved_id TEXT NOT NULL DEFAULT '',
  options TEXT NOT NULL DEFAULT '{}',
  enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL,
  UNIQUE (source, kind, value));

CREATE TABLE IF NOT EXISTS source_cursors (
  target_id INTEGER NOT NULL REFERENCES source_targets(id) ON DELETE CASCADE,
  direction TEXT NOT NULL CHECK (direction IN ('forward','backfill')),
  cursor TEXT NOT NULL DEFAULT '', newest_id TEXT NOT NULL DEFAULT '', oldest_id TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending',
  attempts INTEGER NOT NULL DEFAULT 0, wait_until INTEGER NOT NULL DEFAULT 0,
  last_error TEXT, updated_at TEXT NOT NULL,
  PRIMARY KEY (target_id, direction));

CREATE TABLE IF NOT EXISTS source_item_stages (
  post_id TEXT NOT NULL, stage TEXT NOT NULL,
  status TEXT NOT NULL, cursor TEXT NOT NULL DEFAULT '', attempts INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL, PRIMARY KEY (post_id, stage));

CREATE TABLE IF NOT EXISTS source_usage (
  day TEXT NOT NULL, source TEXT NOT NULL, path TEXT NOT NULL,
  resource TEXT NOT NULL, count INTEGER NOT NULL DEFAULT 0,
  est_cost_micros INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (day, source, path, resource));

CREATE TABLE IF NOT EXISTS account_aliases (
  source TEXT NOT NULL, source_user_id TEXT NOT NULL, nick TEXT NOT NULL, seen_at TEXT NOT NULL,
  PRIMARY KEY (source, source_user_id, nick));

CREATE TABLE IF NOT EXISTS source_secrets (
  name TEXT PRIMARY KEY, sealed BLOB NOT NULL, updated_at TEXT NOT NULL);
```

`source_secrets.name` is `<service>.<field>`, for example `x.bearer`, `x.session`, `youtube.api_key`, `youtube.cookies`, `redstar.session`. The blob is `seal_blob` under the unlocked database key. The plaintext is never stored elsewhere.

`attachments.kind` gains `image|video|gif|thumbnail|subtitle|audio`. `on_site = 1` means the file exists on the origin service.

The TOML file is the user's editable copy of non-secret targets (nicks, channels, pacing). On startup the registry copies enabled targets into `source_targets` if the row is missing. After that, a disable in the Services screen and a disable in the file are the same `enabled` flag. The database does not keep a second secret copy of those settings.

### 3.5 Export and import

- Format version 2 becomes **3** in the phase that first writes a non-Red-Star row. Each post gains the provenance fields. The HTML copy gains them too.
- A v2 import treats a missing `source` as `redstar`.
- A v2 client already refuses a newer `format_version`.
- HTML shows a badge and a permalink. Segments render as a timestamped transcript under the video.
- A search export's `search` object gains `sources`.

### 3.6 Older binaries

A binary from before the new columns still opens the file. It will show `x:alice` as if it were a Red Star nick. The changelog for that phase says so. The same phase refuses every Red Star `SiteAction` whose id is namespaced, before any frame is built.

---

## 4. Configuration, secrets, and migration

### 4.1 Two stores

`ConfigSchema` marks every field.

| `store` | Examples | Where it goes |
|---|---|---|
| `Config` | `enabled`, `route`, pacing, spend caps, nicks, media options, caption languages | `~/.acp/services/<service>.toml` |
| `Secret` | bearer token, API key, session cookies, CSRF cookie | `source_secrets`, sealed |

The Services screen and the CLI walk the same schema. A secret field is a no-echo prompt or a stdin read, then `seal_blob`. It is never written into the TOML, never placed in argv, and never printed. If a TOML file contains a key the schema marks `Secret`, load fails with the key name only, and the value is not logged.

Suggested service ids and files: `redstar`, `x`, `youtube`.

```toml
# ~/.acp/services/redstar.toml
enabled = true
# one route, so there is no `route` key
nicks = ["alice"]
mentions = true
bookmarks = true

# ~/.acp/services/x.toml
enabled = false
route = "session"            # "session" (default) or "api"
users = []                   # handles, without @
searches = []
include_replies = true
include_conversations = false
include_reposts = false
max_posts_per_run = 500
monthly_budget_usd = 10.0   # API route only; ignored on session
per_run_budget_usd = 2.0
web_delay_ms = 4000
web_daily_post_cap = 2000

# ~/.acp/services/youtube.toml
enabled = false
route = "session"            # "session" = yt-dlp (default), "api" = Data API
ytdlp_path = "yt-dlp"        # ACP_YTDLP overrides
channels = []
playlists = []
transcripts = true
audio = true
video = true
caption_locales = ["en"]     # English first. Every available track in each locale.
comments = true
max_comments = 0             # 0 = no count cap. A positive value caps the count.
comment_depth = 0            # 0 = full depth. A positive value caps the depth.
comment_sort = "top"
live_chat = true
max_storage_gb = 20
sleep_requests = 1.0
api_daily_quota = 10000
api_per_run_quota = 2000
```

Unknown keys warn, the same way theme files do. A service with one route rejects a `route` key.

### 4.2 What stays in `preferences.toml`

`preferences.toml` remains the **app** file. After Phase B it keeps:

- `[ui]`: theme, theme mode, theme download, colour, list order, search-history limit, empty-body animation, the app-wide `writes` switch (`ACP_WRITES` still wins).
- `[ui.notify]`: do-not-disturb and per-kind switches for the app's `Notification` implementation (local toasts).
- `[theme]`: the legacy colour keys, still ignored after the one-time conversion.
- `[sync] auto` and `[sync] interval_minutes`: when the scheduler runs. The nicks it used to store move out.

It loses, because they are Red Star service settings:

- Top-level `email` (the remembered login address). It moves to `redstar.toml` as config, not as a secret. It is not a token.
- `[sync] nicks`, `[sync] mentions`, `[sync] bookmarks`.

Phase B copies those values into `redstar.toml` on first launch, then removes them from `preferences.toml`. If `redstar.toml` already exists, the file wins and the old keys are still removed. A missing `redstar.toml` is created with `enabled = true` and those copied values. X and YouTube files are not created until the user turns the service on.

`session.json` is read once in that same launch, sealed as `redstar.session`, and no longer consulted. The file is left in place until that seal succeeds, then deleted. Phase A does not do this, so today's session tests stay valid through modularization.

### 4.3 Environment

Read once, never echoed. These override the sealed value for a headless run and are not written back:

- `ACP_X_BEARER`
- `ACP_YT_API_KEY`
- `ACP_YTDLP`
- `ACP_X_API_BASE`, `ACP_X_WEB_BASE`, `ACP_YT_API_BASE` for mocks, the same role as `ACP_BASE_URL`

### 4.4 How a secret gets in

| Secret | Entry | Row |
|---|---|---|
| X bearer (API route) | `--x-token-set`, prompt or stdin, never argv | `x.bearer` |
| X web session (`auth_token`, `ct0`, `twid`, and any other cookie that route needs) | `--x-import-cookies FILE` (Netscape `cookies.txt` or a browser-export JSON). Only x.com cookies are kept. The file is not copied into `~/.acp` | `x.session` |
| YouTube Data API key | `--yt-api-key-set` | `youtube.api_key` |
| YouTube cookies, only when a video needs sign-in | `--yt-import-cookies FILE` | `youtube.cookies`, passed to yt-dlp as a 0600 temp file that `Drop` deletes |
| Red Star session | Existing login, then Phase B seal | `redstar.session` |

`--x-logout` and `--yt-forget-key` delete the rows. Logout of Red Star deletes `redstar.session`.

Backups copy the database and then **delete `source_secrets` from the snapshot** before packing. A restore has accounts and posts, and asks for credentials again. This is different from today's `session.json`, which is already outside the backup because it is a file.

The log masker (`src/logging.rs`) grows rules, each with a unit test, for `Authorization: Bearer`, `x-csrf-token`, the cookies `auth_token`, `ct0`, `kdt`, `twid` and `att`, `x-guest-token`, `x-client-transaction-id`, `guest_id`, a YouTube `key=` query parameter, a yt-dlp `--cookies` path, and `po_token`. The pre-commit scan and `.gitleaks.toml` gain a rule for an X bearer (`AAAAAAAAAAAAAAAAAAAAA…`) and for `auth_token=`.

### 4.5 Services screen

TUI: a Services preferences screen, opened from Account › Services. The list is every registered service, with Red Star on and the others off. Enabling one shows that service's schema. Secret rows say "set" or "not set" and never the value. Changing `route` is the same control on every multi-route service.

CLI, generated from the schema rather than hard-coded per field where the type allows it:

- `--services` prints id, display name, enabled, route, and whether each secret is set.
- `--service-enable x` / `--service-disable x`
- `--service-set x route api` for config fields
- The secret commands in §4.4, because a secret cannot ride in argv.

The first time X's session route is enabled, before any cookie is used, the app shows §13's warning (Terms of Service, the damages clause, account lock or suspension) and requires an acknowledgement. Cancel leaves the service off. The acknowledgement is a config field `session_ack = true` in `x.toml`, not a secret. The API route does not show that warning. It shows the spend confirm instead (§6).

---

## 5. Service behaviour

### 5.1 YouTube session route (yt-dlp, the default)

yt-dlp is a subprocess: `Command::args` only, a timeout, masked stderr, JSON on stdout. `yt-dlp --version` must meet a pinned floor. Older than about 90 days warns.

| Step | Command shape | Stored as |
|---|---|---|
| Channel, newest first | `yt-dlp --flat-playlist --dump-json --lazy-playlist https://www.youtube.com/@H/videos` (also `/streams` and `/shorts` when those options are on) | ids, titles, dates. Stops at the first known id |
| Playlist | `yt-dlp --flat-playlist --dump-json URL` | the same, playlist order |
| Video metadata | `yt-dlp -J --skip-download --no-write-playlist-metafiles URL` | `kind=video`, channel profile, tags, thumbnail URL |
| Transcript | `--write-subs --write-auto-subs --sub-langs` built from `caption_locales`, with `live_chat` excluded, `--sub-format "json3/vtt/best" --skip-download`. A bare `en` asks this route for `en.*` so every English track matches. Manual and automatic tracks in the listed locales are both stored. A missing locale is skipped quietly (§3.3) | subtitle attachment and segment rows |
| Comments | `--write-comments`. Full depth by default. `max_comments` and `comment_depth` are passed through only when the value is greater than 0. `comment_sort` selects top or new | `kind=comment` |
| Live chat replay | on by default: `--sub-langs live_chat`, separate from the caption locales | `kind=chat`, shown on the Chat tab |

Pacing: `--sleep-requests` from the service file, `--sleep-subtitles 5`, one yt-dlp process at a time. Recent yt-dlp needs a JS runtime (deno, node, bun or QuickJS). The app does not bundle one. `--doctor` reports which runtime it found. `--cookies-from-browser` is not used. The archive is the `posts` table, not `--download-archive`.

### 5.2 YouTube API route

`route = "api"` uses the Data API key:

- `channels.list`, `playlistItems.list` (50 per page), `videos.list` (50 ids), `commentThreads.list`, `comments.list`.
- Each list call costs 1 unit. The default project quota is 10,000 units per day, reset at midnight Pacific.
- `captions.download` cannot fetch other people's captions. On the API route, transcripts still come from yt-dlp when it is installed, otherwise `transcript_kind = none`.
- The ledger counts units and stops before `api_daily_quota` or `api_per_run_quota`. The run confirms first (§6).

### 5.3 X API route

Used only when `route = "api"`. App-only bearer, OAuth 2.0.

| Need | Endpoint | Notes |
|---|---|---|
| Resolve a handle | `GET /2/users/by/username/:username` | numeric id is the stable key |
| A user's posts | `GET /2/users/:id/tweets` | Only the 3,200 most recent. `since_id` forward, `pagination_token` back |
| Search, last 7 days | `GET /2/tweets/search/recent` | 512-character query |
| Full-archive search | `GET /2/tweets/search/all` | pay-per-use, higher cap on query length |
| Thread replies | `conversation_id:<root>` on those search endpoints | recent search reaches back 7 days |
| Post lookup | `GET /2/tweets?ids=` | up to 100 |
| Mentions | `GET /2/users/:id/mentions` | most recent 800 |

Fields requested every time: `tweet.fields` including `created_at`, `conversation_id`, `referenced_tweets`, `entities`, `public_metrics`, `note_tweet`, `edit_history_tweet_ids`; expansions for author, media and referenced tweets; `media.fields`; `user.fields`.

Pricing checked 2026-10-21. Pay-per-use, no new tiers: post read $0.005, user read $0.010, owned reads $0.001. The same resource inside one UTC day is deduplicated (a soft guarantee). The monthly platform cap is 3,000,000 post reads. The estimator's prices live in one dated table with a test.

Reposts, when `include_reposts` is on, are a link row plus the original when the expansion includes it. `edit_history_tweet_ids` can fill `post_versions` as a read. Sending an edit is a later write capability.

### 5.4 X session route (the default)

`route = "session"` reads with the imported cookies, including **search and replies**, not only the user's own timeline.

- Transport: `https://x.com/i/api/graphql/<queryId>/<Operation>` for `UserByScreenName`, `UserTweets`, `UserTweetsAndReplies`, `TweetDetail` and `SearchTimeline`.
- Each request sends the public web bearer, `x-csrf-token` equal to `ct0`, `x-twitter-auth-type: OAuth2Session`, and the session cookies.
- Query ids rotate. A stale id is a 404. Ids live in a tested table and are refreshed from the signed-in page's script bundle only after a 404, the same idea as Red Star's bundle-hash check. Failure stops that run with a notice. It does not silently switch to the API route.
- Operations that the web app only answers when the per-request client header is present (`SearchTimeline`, `UserTweetsAndReplies`, and others) are **in scope**, because search and replies are in scope. The header value comes from a captured logged-in session, recorded the way Red Star records a LiveView frame, and is redacted in fixtures. This plan does not specify a way to mint that header by attacking the client. If the site rejects the call, the run stops, raises a notice, and does not retry in a loop.
- Guest tokens are not used.
- Pacing comes from the service file: one request at a time, `web_delay_ms` plus 0–50% jitter, at most one page per request, and `web_daily_post_cap`. A 429, a rate-limit body, a login challenge or a locked account stops the X job for the day and raises a notice.
- The first enable still requires the §13 acknowledgement (§4.5), even though this route is the default once the service is on.

### 5.5 Defaults

| | Default |
|---|---|
| X: the account's own posts, and their replies when `include_replies` is on | on |
| X: other people's replies under those posts | off (`include_conversations`) |
| X: reposts | off |
| X: Spaces and Communities | not in the first read track |
| YouTube: uploads | on |
| YouTube: shorts and livestream VODs as metadata | on for uploads and shorts, streams tab off until the user turns it on |
| YouTube file and transcript availability | video, audio and transcripts are all available, bounded by `max_storage_gb` (§8) |
| YouTube captions | `caption_locales = ["en"]`. Every available track in each listed locale. A missing locale is skipped quietly, and the locales that were available are recorded |
| YouTube comments | full depth. `max_comments = 0` and `comment_depth = 0` mean no cap |
| YouTube live chat replay | on. Rows are `kind=chat` and the detail shows a Chat tab |

### 5.6 Search

`--x-search` uses the active X route. On the API route, `--x-archive` is full-archive search and goes through the spend confirm. On the session route it is `SearchTimeline`. YouTube search is available and is not a standing sync target, because the order is ranked rather than chronological.

---

## 6. Rate limits and spend caps

Every `CostMeter` route, before a run that would spend:

1. Estimate the maximum (pages × page size × the dated price, or quota units).
2. Show it.
3. Ask `Spend up to $X.XX? [y/N]` or the quota equivalent. Cancel is the default. `--yes` affirms. `--dry-run` prints the estimate and does not ask and does not connect.
4. Refuse when the estimate would pass the monthly cap or the per-run cap already stored in the service file.
5. Add actual usage to `source_usage` as responses arrive. Stop the job cleanly, with a resumable cursor, when a cap is hit (`Limited` in the UI).

| Path | Pace | Caps |
|---|---|---|
| X API | rate-limit headers; one request in flight; full-archive at most 1/s | `max_posts_per_run`, `per_run_budget_usd`, `monthly_budget_usd`, platform 3M posts / month |
| X session | `web_delay_ms` + jitter, serial | `web_daily_post_cap`; stop on a challenge |
| yt-dlp | `--sleep-requests`, one process | `--limit`, `--scrape-budget`, `max_storage_gb` for files |
| YouTube API | 1 unit per list | `api_per_run_quota`, `api_daily_quota` |

`--limit` and `--scrape-budget` apply to every service's reader. A 429 sets `wait_until` and ends that service's job. Other services keep running.

---

## 7. Incremental sync

Same idea as the Red Star whole-site crawl, per target, inside that service's job:

1. **Forward.** X API uses `since_id`. X session, yt-dlp and the YouTube API walk newest-first until a page of ids that are all known. `newest_id` advances only after the page commits.
2. **Backfill.** Continues from `oldest_id` until the source ends, the run cap, or the budget. `status = budget` means resume next time.
3. **Stages.** Transcript, comments, live chat and media are `source_item_stages` rows, each with its own cursor. They run only when that capability is on and `Ready`.
4. **One transaction per page** for accounts, items, tags, mentions and the cursor. `ON CONFLICT(post_id) DO UPDATE` keeps the non-blanking rules.
5. **Rules** run on the rows just committed (§9).
6. **`--full`** ignores `newest_id`.

Services do not wait on each other (§2.5).

---

## 8. Media

Video, audio and transcripts are available on every service that implements `MediaFetcher`. The storage cap is `max_storage_gb` in that service's file (YouTube's suggested default is 20). A new file that would pass the cap is skipped, the stage stays resumable, and the UI shows `Limited`.

- **X.** Photos from `media.url` at `?name=orig`. Video and GIF: the highest-bitrate `video/mp4` variant. Audio-only variants are kept when the media option asks for audio. `--no-attachments` skips file bytes and still stores the post text.
- **YouTube.** Thumbnails, avatar and banner are part of the read, because cards need them. Transcripts, audio (`bestaudio` to opus) and video (`bv*[height<=720]+ba` when ffmpeg is present) follow the service file. All three default **on**, and the cap is what stops a runaway channel.
- Files are named `<service>/<id>.<ext>` under the attachments directory.
- Export copies them as it does today.
- Nothing here uploads. Upload is the `Upload` capability in Phase E, per service, after reads.

---

## 9. Rules, alerts, Feeds, research, replication

- `Condition::Source` accepts `x` and `youtube`. Existing rules that mean Red Star keep meaning Red Star (`site` and `redstar`).
- `Author` matches namespaced nicks. The editor offers a service picker.
- New condition fields when the columns exist: `kind`, `transcript_kind`.
- "Watch a user" takes a service and a nick that is valid there. "Replies to me" uses that service's configured own nick. "Saved search" spans enabled services unless the rule names one.
- Finish, budget stop, 429, a challenge, and a missing yt-dlp notify through `Notification`. Local toasts are that trait, implemented by the app. A service implements the same trait when it has its own notice stream.
- Phase A refactors the existing notices, rule engine, and alert-template code onto `Notification` if the split needs it. The versioning-cleanup draft labels those features 3.27.0 (notices), 3.28.0 (the rule engine), and 3.29.0 (alert templates). Those numbers are that draft's labels. This plan does not retag them and does not wait on that draft.
- Feeds (P7, not built yet) gain an optional `sources` list. The share string still has no tokens and no cookies.
- Research items can point at any namespaced id. Research › Add current view follows §2.4.
- This plan is what gives PLAN-4.0 its post-source columns and YouTube transcripts. Diarization and `transcript_corrections` stay in 4.0. The channel list for a corpus is the user's `channels` in `youtube.toml`, not a hard-coded set.
- **P9.** The CBOR record may carry `source`, but X and YouTube rows are **excluded from peer sync by default**. Red Star replication is unchanged. A later change to §7.7 of the 3.0 plan can opt them in. Media blobs are not replicated with those rows while they are excluded.

---

## 10. CLI and TUI

Flags are generated from the capability set and the schema as far as clap allows. Hand-written flags stay for the actions that already exist (`--nick`, `--said-by`, `--new-post`, and the rest), and each one checks `availability` before it runs.

| Flag | Who sees it |
|---|---|
| `--source redstar\|x\|youtube\|all` | Everyone. Default is the enabled set. `all` means every enabled service. |
| `--service-enable`, `--service-disable`, `--services`, `--service-set` | Registry |
| `--nick`, `--said-by`, `--limit`, `--order`, `--full`, `--sync`, `--scrape-budget` | Every enabled `Reader`, namespaced |
| `--x-user`, `--x-search`, `--x-post`, `--x-conversations`, `--x-token-set`, `--x-import-cookies`, `--x-logout`, `--x-cost` | Only when X is registered. Hidden when X is off, except `--service-enable x` |
| `--yt-channel`, `--yt-playlist`, `--yt-video`, transcript and comment flags, `--yt-api-key-set`, `--yt-download` | Only when YouTube is registered. Download flags exist because video and audio are available |
| `--new-post`, `--reply`, `--vote`, `--subscribe`, `--delete`, `--edit`, `--repost`, upload | Shown or hidden per §2.4. They are not globally disabled for X and YouTube. They are hidden until that service implements the trait, and disabled with a tooltip while gated |

`--dry-run` prints planned reads (URLs or operation names, caps, cost estimate) or planned writes (the exact payload) and opens no connection. A write confirm defaults to Cancel.

TUI:

- **Services** preferences screen (§4.5).
- **Source switcher** for which service's rows are on screen: one service, or All. The choice is remembered in `[ui]`, because it is a view, not a service secret. View › Source and a palette entry exist either way. The key is the Ctrl global `view.source`. The default binding is `ctrl-]` in [PLAN-MODES-KEYMAP.md](./PLAN-MODES-KEYMAP.md), and the keymap can change it.
- **Nick bar** sections per enabled service, prefix hidden, badge shown.
- **Cards.** Badge, permalink, and a video detail with the tabs that service's capabilities allow (Description, Transcript, Comments, Chat). A missing capability omits the tab. YouTube's Chat tab shows `kind=chat` rows.
- **Actions** follow §2.4. Bookmarks, site search, and edit history stay global actions (§2.3). One confirm sends a write to every selected service where it is Ready and skips the others.
- **Start Scanning** shows the selected service's schema fields (nicks, channels, caps).
- **Help** has one topic per service plus a topic for capabilities and gates. Getting Started gains a short section. Every flag has a description.

Both combined layouts are offered. **Mixed** is the default: one list, newest first. The sort key is the item's updated time when it has one, and `posted_at` otherwise. For a Red Star post the updated time is the latest `post_versions.edited_at` that is a real timestamp. A `~NNNN` placeholder is not a time, and a row with no edit uses `posted_at`. **Grouped** keeps a section per service, and each section uses that same key. The choice is remembered in `[ui]`. Capability rules for both layouts are in §2.4.

### 10.1 Modal input

Modal input is in v3, required before the 3.0 launch. The design is [PLAN-MODES-KEYMAP.md](./PLAN-MODES-KEYMAP.md), and the 3.0 roadmap points at it. It is not part of phases A–E.

The app enters a mode for what is focused. Viewing a post enters post mode, and that mode's keys become valid. In post mode, `r` replies. Global keys stay, and they move behind a Ctrl-<key> pattern. Every other key is a mode hotkey.

A mode's keys are the capabilities of the focused object's service. Post mode on a Red Star post offers the Red Star capabilities that are `Ready`. The same mode on an X post offers X's capabilities. A capability the service does not implement has no key in that mode.

The source switcher is one of the Ctrl global keys. The default binding is `ctrl-]`. The keymap file can change it.

---

## 11. Testing

Tests never talk to acp.us, x.com, api.x.com, twitter.com, youtube.com, googleapis.com or googlevideo.com.

- Under `cfg(test)` and in cucumber, a service HTTP client **panics** if the host is one of those. The yt-dlp runner refuses a real `yt-dlp` when `ACP_TEST` is set.
- **Phase A.** The existing suite passes unchanged. New tests only assert that Red Star's `implemented()` set matches today's features and that a missing trait is not offered. No snapshot of user-visible copy changes.
- **Later phases.** Axum mocks for the X API, the X web operations (including search and replies), and the YouTube Data API. A `fake-yt-dlp` binary selected with `ACP_YTDLP` checks argv, rejects a secret in argv, and prints fixtures. Fixtures are synthetic. A real capture is redacted first, then scanned, the same rule as the Red Star fixtures.
- Gherkin covers: scrape, export v3, import into a throwaway `HOME`, incremental stop, resume after 429 or a cap, `--said-by` per service, deterministic transcript turns, a missing caption locale that is skipped quietly with the available locales recorded, a mixed write whose preview and `--dry-run` name each skipped service (not captured, not logged in, or disabled) and whose one confirm sends only the Ready services, an All-view selection that skips a service without that trait, the masker, and the backup snapshot containing no `source_secrets` plaintext and no sealed rows.
- Manual live checks, if Michael runs them, go in a checklist like `docs/LIVE-PUBLISH-TEST.md`: throwaway `HOME`, `--dry-run` first, one small target. Never from CI and never from an agent VM.

---

## 12. Packaging and runtime dependencies

yt-dlp is a runtime dependency of the YouTube service only. Red Star and X work without it. The app looks at `ytdlp_path`, `ACP_YTDLP`, then `$PATH`, and explains how to install it when the YouTube session route is enabled and the binary is missing.

| Package | Change, in the YouTube read phase |
|---|---|
| Debian | `Recommends: yt-dlp, ffmpeg`. `Suggests: deno \| nodejs` |
| Arch | optdepends for yt-dlp, deno, ffmpeg |
| Fedora | `Recommends: yt-dlp`. Suggests ffmpeg and nodejs |
| `scripts/install.sh` | A hint when yt-dlp is missing. It does not install it |

Packaging stays on the alpha gate in §4.5 of the 3.0 plan. This plan does not cut a release. X uses the `reqwest` already in the tree. `--doctor` reports yt-dlp version and age, the JS runtime, ffmpeg, whether each secret is set (never the value), the session acknowledgement, and the month's spend.

---

## 13. Risks

1. **X Terms of Service and account risk.** X's terms forbid crawling or scraping without prior written consent, and forbid working around technical limits. Since late 2024 they also state liquidated damages of **$15,000 per 1,000,000 posts** requested, viewed or accessed in any 24-hour period. The session route is that kind of access. It can lock or suspend the account, and the damages clause is a real exposure even at small volume. The user has decided to keep this route, including search and replies. The mitigations that remain are: the service is off until the user turns it on; the first enable shows this warning and requires an acknowledgement; pacing and the daily cap are configurable (the suggested cap is 2,000 posts, far under the threshold); there is no guest-token path, no account rotation and no proxy pool; a challenge stops the job for the day. The API route is there when the user would rather pay than use the session.
2. **X API cost.** Post reads are $0.005 and user reads are $0.010. A backfill is the expensive part. The 3,200-post timeline means older history needs full-archive search. Mitigations: the estimate, Cancel by default, both caps, the ledger, same-day dedup, and `--x-cost`. Prices changed more than once in 2026, so the table is dated and tested.
3. **X API and web churn.** Endpoints, query ids and rate limits move. Parsers accept unknown fields. A stale web query id refreshes once from the signed-in bundle and then stops if it still fails.
4. **yt-dlp breakage.** PO tokens, the JS challenge, "Sign in to confirm you're not a bot", IP throttling. Mitigations: a version floor, stable `info.json` keys, per-stage failure that keeps the item, the optional API route for metadata and comments, and a notice to run `yt-dlp -U`.
5. **YouTube terms.** Automated access outside the API is restricted. Comments and transcripts are other people's content. PLAN-4.0's rights note covers what may be stored and exported. The API route is the more compliant one where it can serve the request.
6. **Privacy of other people's posts.** They sit in a database that can replicate. X and YouTube rows stay out of P9 by default (§9).
7. **Mixed ids.** Namespaced ids avoid collisions. The phase that adds them also refuses a Red Star write aimed at a namespaced id. A unit test calls every Red Star write with an `x:` id and expects a refusal before a frame.
8. **Disk.** Video, audio and transcripts are available. `max_storage_gb` is the bound. The run skips new files at the cap instead of filling the disk.
9. **Credentials.** Tokens and cookies are long-lived. They are sealed, masked, absent from argv and from backups, and the yt-dlp cookie file is 0600 and deleted in `Drop`. Tests assert each of those.
10. **Plugins are native code.** A `cdylib` that passes the version check runs with the app's privileges: the unlocked database, opened secrets, the network, the filesystem, and other services' memory. There is no sandbox. The mitigations are the ones in §2.1: nothing is auto-downloaded, a file in `~/.acp/plugins/` stays off until the user enables it, a rustc or core-version mismatch is refused before `register`, and a duplicate id cannot replace a built-in.

---

## 14. Phases

Labels are relative. They are not crate versions. One feature is one minor when it ships, and the tag is created when that bump is on main. The versioning cleanup PR assigns the real numbers. Nothing here moves a numbered stage in the 3.0 plan.

| Label | Phase | What ships | Depends on |
|---|---|---|---|
| **A** | Red Star modularization | Extract today's Red Star behaviour behind the traits and into `acp_red_star_service_redstar`. Red Star registers through the registrar. No plugin-directory scan. If the trait split needs it, refactor notices, the rule engine, and alert templates onto `Notification` (versioning-cleanup labels 3.27.0, 3.28.0, and 3.29.0). No new flags, no settings migration, no schema change for other services. The existing test suite passes with no behaviour change. | — |
| **B** | Registry and Services | Services screen, schema-driven CLI, `~/.acp/services/*.toml`, sealed secrets, Red Star migration off `preferences.toml` and `session.json` (§4.2). The loader scans `~/.acp/plugins/`, refuses a rustc or core-version mismatch, and lists refused plugins. Red Star on by default. X and YouTube register statically and stay off. Enabling X's session shows the §13 acknowledgement and does not yet scrape. | A |
| **C-yt** | YouTube reads | Session route (yt-dlp metadata, channels, playlists, incremental and backfill, thumbnails) and the API route (quota meter and confirm). Comments at full depth, with optional depth and count caps. Live chat replays stored as `kind=chat` and shown on the Chat tab. Parallel with C-x. | B |
| **C-x** | X reads | Session route, including search and replies, with pacing, daily cap and the acknowledgement. API route with the spend confirm and ledger. Parallel with C-yt. | B |
| **D** | Media | Video, audio and transcripts for the services that implement `MediaFetcher`, plus the storage cap. Transcripts use `caption_locales` (English first). A missing locale is skipped quietly and the available locales are recorded. | C-yt for YouTube files. C-x for X video files. Either may land first for its own service. |
| **E** | Writes, per service | One minor per write capability per service, and only after that service's reads have shipped. Each stays `Gated` until its captured frames are in the tree. Mock, preview, one Cancel-default confirm for every Ready service, exact `--dry-run` that names each skip (§2.4). | That service's read phase |

YouTube and X reads are parallel tracks. Either may merge first. Writes do not start for a service before its reads. Red Star writes that are already gated stay gated in Phase A. Turning one on later is its own minor under E, still waiting on a capture, and is not required to unblock X or YouTube reads.

Not scheduled: X Spaces or Communities metadata, and Feeds `sources` filters once P7 exists.

---

## 15. Open questions

Resolved, and not re-opened: separate crates; X cookies sealed in the encrypted database; backups strip `source_secrets`; X and YouTube stay out of P9 peer sync by default; Red Star on and other services off; session route as the default; search and replies included on the X session; video, audio and transcripts available under a storage cap; reads before writes; caption locales (English first, every available track, a missing locale skipped quietly and the available locales recorded); comments at full depth with optional depth and count caps; live chat replays (`kind = chat`, Chat tab); both combined views, mixed by default, sorted by the updated time when the item has one and otherwise the posted time; the source switcher as the Ctrl global `view.source`, default `ctrl-]` in [PLAN-MODES-KEYMAP.md](./PLAN-MODES-KEYMAP.md); bookmarks, site search, and edit history as global actions; `Notification` on every notifying action, local toasts included; services registered through the plugin registrar; one confirm for a mixed write.

**One confirm.** A write selection uses a single confirm. It sends to every service where that write is Ready. Every other selected service is skipped. The preview and `--dry-run` name each skipped service and why: not captured, not logged in, or disabled. Cancel is the default. `--yes` affirms that one confirm. Nothing is sent for a skip (§2.4).

Still open, from the plugin design:

1. **Signatures.** Explicit enable and no auto-download are decided. Is a signature, or a hash the user pins, required before `dlopen`, or is placing the file and enabling the service the whole gate?
2. **Where authors read the pin.** A plugin must be built with the same rustc and the same core-crate version as the binary. Should `--doctor` print that pair, should a file ship beside the binary, or both?
3. **Reload.** Plugins load at startup and stay mapped until the process exits, because a service object must not outlive its library. Is the next launch the permanent rule, or is an in-process reload in scope later?
