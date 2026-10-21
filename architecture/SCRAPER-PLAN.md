# Plan: read-only X and YouTube sources for ACP Red Star

Status: **design only, for Michael's review.** Nothing in this file is implemented.
Target path in the repo: `architecture/SCRAPER-PLAN.md`. This draft was written on the agent box. It was not committed or pushed.
Base: `main` at `28759588` (crate `3.5.0`, about 12 features under `[Unreleased]`).
Companion docs: [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) (§6 Feeds, §7.5 records, §7.14.13 rules and alerts, §7.14.14 post source) and [PLAN-4.0.md](./PLAN-4.0.md) (the research corpus).

Facts about the X API, the YouTube Data API and yt-dlp were checked on **2026-10-21** against docs.x.com, developers.google.com and the yt-dlp README (release `2026.08.19`). They change often. Re-check them before each phase: the X pricing and rate-limit pages, which X GraphQL operations are gated, the yt-dlp release notes (`max_comments` order, `live_chat`, EJS and JS runtime, PO tokens), and the YouTube quota calculator.

---

## 0. Goal and approved approach

Michael wants two more **read-only** sources, X and YouTube. They work like the Red Star scraper, but have no compose, vote, reply, bookmark-push or any other write. The approach he approved:

- **One source module per site.** Each is built on the shared core crate (`acp_red_star_node_core`, 3.2.0).
- **One database.** Accounts, posts, replies and media go into the same tables. Exports (json, html, tar.gz) are the same. `--said-by`-style filters, the TUI reader views and search work the same way, plus a **source switcher**.
- **YouTube:**
  - **yt-dlp** for channels, playlists, video metadata, transcripts or subtitles, and comments.
  - This feeds the PLAN-4.0 corpus (post source, then the Haz stream corpus).
  - The **YouTube Data API v3** is an optional alternative backend.
- **X:** two paths. The official **X API v2** with Michael's bearer token comes first. A **logged-in X web session** is the fallback, with cookies imported into `~/.acp`. Credentials never go into logs.

Non-goals:
- Writing to X or YouTube.
- Live audio or video (Spaces audio, livestream playback).
- DMs.
- Any other user's private data.
- A cloud model.
- Changing how Red Star works.

---

## 1. How this fits the current code

| Area today | File | What the new sources reuse or change |
|---|---|---|
| Post model | `src/scrape/mod.rs` `Post` | Reused. It gains `source`, provenance fields and `kind` (§3). |
| Schema and migrations | `src/storage/schema.rs`, `db.rs` `ensure_posts_schema` | The same additive pattern: a `pragma_table_info` check, then `ALTER TABLE ... ADD COLUMN`. |
| Encrypted DB | `src/crypto/atrest.rs`, `open_db_flags` | No change. New tables live in the same SQLCipher file and go into backups automatically. |
| Session | `src/auth/session.rs` (`~/.acp/session.json`, 0600, cookie values never logged) | The X web session uses the same `SavedSession` shape, but is **sealed** (§4.3). |
| Preferences | `src/auth/prefs.rs` `Prefs` (`[ui]`, `[sync]`, `[theme]`) | Adds `[sources]`, `[x]` and `[youtube]` tables, non-secret values only (§4). |
| Secrets interface | core `store.rs` `SecretStore`, `seal_blob`/`open_blob` | Holds the X bearer token, X cookies and the YouTube API key (§4.3). |
| HTTP | `src/http.rs` (client posing as Firefox, shared jar) | Gets a separate client per source. The X API client uses honest headers (§6.1). |
| Media | `src/media.rs` (`download_all`, `canonical_url`, `url_key`) | Reused for X images and video and YouTube thumbnails. yt-dlp handles video and audio files (§8). |
| Crawl frontier | `CRAWL_SCHEMA` (3.34) | The same idea, but in a per-source cursor table (§7). |
| Jobs | TUI jobs run as children of the headless binary and stream JSON lines | Scrapes of the new sources are ordinary jobs. The database key goes over the same private pipe. |
| Rules and alerts | `src/rules` (`Condition::Source`, `same_source`) | `x` and `youtube` become real source values (§9). |
| Export | `src/export`, `EXPORT_FORMAT_VERSION = 2` | Bumps to **3**, adding source fields as PLAN-4.0 already expected (§3.5). |
| Writes | `src/site/*` `SiteAction`, `CAPTURE_BLOCK` | Every write path refuses a post whose `source != 'redstar'` (§10.4). |
| Tests | `tests/support/mock.rs` (axum fake acp.us), cucumber `tests/features/*.feature`, `ACP_BASE_URL` | Gets the same kind of mocks per source, plus a fake yt-dlp (§11). |

---

## 2. Source interface

### 2.1 Where the code lives

The core crate's rule is that it "does not open the SQLite database, draw the terminal, or talk to the network." Keep that rule. So:

- **`acp_red_star_node_core::source`** (new module, no I/O). Contains:
  - `SourceKind`, namespaced ids (`SourceId`), and the record types (`AccountRec`, `ItemRec`, `MediaRec`, `SegmentRec`).
  - `Target`, `Cursor`, `Budget` and `Capabilities`.
  - The canonical CBOR encoding of an `ItemRec`, so the P9 `PostRecord` (§7.5 of the 3.0 plan) can carry any source later.
- **`crates/acp_red_star_source_x`** and **`crates/acp_red_star_source_youtube`** (new workspace crates). Each:
  - implements the trait,
  - owns its HTTP client or its yt-dlp subprocess, and its parsers,
  - depends on core plus `reqwest`, `serde_json` and `tokio`,
  - never touches SQLite.
- **Root crate `src/sources/`**. Glue code that:
  - maps records into `Post`, `Profile` and `Attachment`,
  - runs the sync loop (§7) and writes cursors,
  - calls the rules engine,
  - handles CLI and TUI wiring.

The root crate stays the only owner of the database.

> Open question Q1: two new crates, or modules `src/sources/{x,youtube}` in the root crate? Crates keep the core rule and let a future node or relay reuse the parsers. Modules are less ceremony. This plan assumes crates.

### 2.2 The trait

Async functions in traits are stable well below MSRV 1.88. They aren't `dyn`-compatible, so dispatch goes through an enum (`AnySource`) instead of `Box<dyn>`.

```rust
/// Which service a record came from. Stored as text in `posts.source`.
pub enum SourceKind { RedStar, X, YouTube }          // "redstar" | "x" | "youtube"

/// Namespaced id: Red Star ids stay bare; others are "x:…" / "yt:…" (§3.2).
pub struct SourceId { pub kind: SourceKind, pub native: String, pub sub: Option<SubId> }

pub enum Target {
    Account(String),          // X handle or user id; YouTube @handle, channel id or URL
    Search(String),           // X query; YouTube search terms (optional, §5.6)
    Playlist(String),         // YouTube playlist id or URL
    Item(String),             // one X post id / URL; one YouTube video id / URL
    Conversation(String),     // X conversation_id (replies of one thread)
}

pub struct Capabilities {
    pub writes: bool,                 // always false for X and YouTube
    pub replies: bool, pub media: bool, pub transcripts: bool, pub comments: bool,
    pub live_chat: bool, pub metrics: bool, pub edit_history: bool,
}

pub struct Page {
    pub accounts: Vec<AccountRec>,    // authors seen on this page (upserted)
    pub items: Vec<ItemRec>,          // posts, replies, videos, comments
    pub segments: Vec<SegmentRec>,    // transcript turns (YouTube)
    pub next: Option<Cursor>,         // None = this direction is exhausted
    pub cost: Spend,                  // requests, resources billed, quota units
}

pub trait Source {
    fn kind(&self) -> SourceKind;
    fn capabilities(&self) -> Capabilities;
    /// Turn a user string into a canonical target (handle → user id, URL → video id).
    async fn resolve(&self, t: &Target) -> Result<Resolved>;
    /// One page, newest first. `stop_at` lets incremental sync end early (§7).
    async fn page(&self, t: &Resolved, cur: &Cursor, stop_at: Option<&str>,
                  budget: &mut Budget) -> Result<Page>;
    /// Per-item extras that cost more: full thread, transcript, comments.
    async fn expand(&self, item: &SourceId, what: Expand, budget: &mut Budget) -> Result<Page>;
}
```

- `Budget` carries the run caps: the post limit, seconds, X spend, YouTube quota units and the web daily cap. Every adapter decrements it **before** a request and returns `BudgetExhausted` rather than overspending.
- `Cursor` is opaque, serialisable text:
  - an X `pagination_token`, `next_token` or `since_id`,
  - a GraphQL `cursor-bottom`,
  - a YouTube playlist index or Data API `pageToken`.
- Red Star doesn't have to move onto the trait in this work. A thin `RedStarSource` wrapper can come later.

### 2.3 Record mapping

| Concept | Red Star (today) | X | YouTube |
|---|---|---|---|
| Account (`profiles`) | nick | `@handle`, user id, name, bio, avatar, banner, metrics | channel (`@handle`, `UC…` id), title, description, avatar, banner, subscriber count |
| Root post | post with title | post that isn't a reply | **video**: title, description as body, `posted_at` = upload or release date |
| Reply | reply (`parent_post_id` = root, `in_reply_to_id` = parent) | reply: `conversation_id` → root, `referenced_tweets[replied_to]` → parent | **comment**: parent = video; a reply to a comment sets `in_reply_to_id` to that comment |
| Quote / repost | `quoted_post` | `quoted` / `retweeted` reference (repost stored as a link, not a copy, §5.3) | n/a |
| Media | attachments | photos, video and GIF variants | thumbnail. Optional video or audio file (§8) |
| Text body | ProseMirror → text | `note_tweet.text` when present (long posts), else `text`; entities expanded | description, comment text |
| Score | `score` | like count (raw metrics in `metrics_json`) | like count (views etc. in `metrics_json`) |
| Tags | `post_tags` | `entities.hashtags` | video `tags` plus `#hashtags` in the description |
| Mentions | `mentions` | `entities.mentions` → `x:<handle>` | `@handle` in comments (best effort) |
| Transcript | n/a | n/a | **segment rows** (§3.3) |

---

## 3. Schema changes and migrations

All changes are **additive**, so 3.0's SQLCipher file is migrated in place. There's no re-key, no second database, and `~/.acp/ai/data/rs.database` isn't renamed. Each change follows the `ensure_posts_schema` pattern, so it is idempotent and safe on every open.

### 3.1 `posts`: provenance columns (the PLAN-4.0 groundwork)

This lands the §7.14.14 sketch early. That section allows "a 3.x minor before 4.0".

```sql
ALTER TABLE posts ADD COLUMN source TEXT NOT NULL DEFAULT 'redstar';   -- redstar | x | youtube
ALTER TABLE posts ADD COLUMN source_id TEXT NOT NULL DEFAULT '';        -- native id at the source
ALTER TABLE posts ADD COLUMN source_url TEXT NOT NULL DEFAULT '';       -- canonical permalink
ALTER TABLE posts ADD COLUMN source_start_ms INTEGER;                   -- transcript segment start
ALTER TABLE posts ADD COLUMN source_end_ms INTEGER;                     -- transcript segment end
ALTER TABLE posts ADD COLUMN speaker TEXT NOT NULL DEFAULT '';          -- 4.0 diarization label
ALTER TABLE posts ADD COLUMN kind TEXT NOT NULL DEFAULT '';             -- post|reply|video|comment|segment|chat
ALTER TABLE posts ADD COLUMN lang TEXT NOT NULL DEFAULT '';
ALTER TABLE posts ADD COLUMN metrics_json TEXT NOT NULL DEFAULT '';     -- likes, reposts, views… as fetched
ALTER TABLE posts ADD COLUMN fetched_via TEXT NOT NULL DEFAULT '';      -- liveview | x-api | x-web | yt-dlp | yt-api
CREATE INDEX IF NOT EXISTS posts_source_idx ON posts(source, posted_at);
CREATE INDEX IF NOT EXISTS posts_source_id_idx ON posts(source, source_id);
```

- Existing rows keep `source = 'redstar'`. An optional backfill sets `source_id = post_id` and `source_url = https://acp.us/rs/post/…`, as §7.14.14 already allows.
- `kind` can be derived for old rows: `parent_post_id = ''` gives `post`, otherwise `reply`.
- `post_versions.source` stays the edit-observation column (`site|peer|local|draft`). X edit history is stored there with `source = 'site'`, read as "observed from the origin service". No table rebuild is needed.

### 3.2 Namespaced ids and nicks (decision)

`posts.post_id` already has a **global** unique index, and bookmarks, research items, tags, mentions, attachments and rule firings all key on that text. So:

- **Red Star ids stay bare.** These are 22-character base64url ids. No existing row, bookmark, research item or export changes.
- **New sources are prefixed:**
  - `x:<tweet id>`, for example `x:1849…`.
  - `yt:<video id>`, for example `yt:dQw4w9WgXcQ`.
  - `yt:<video id>/c:<comment id>` for comments.
  - `yt:<video id>/t:<start_ms>` for transcript segments.
  - `yt:<video id>/chat:<message id>` for live-chat replay.
  - The prefix can't collide with Red Star ids, because base64url never contains `:`.
- **Nicks are namespaced the same way**, `x:alice` and `yt:@somechannel`:
  - `profiles.nick` (the primary key) and `posts.nick` hold the prefixed form.
  - The nick bar, `--said-by`, rules `Author` and `RepliesTo`, alert "watch user" and Feeds nick rules all keep working unchanged and can't merge two different people named `alice`.
  - The UI strips the prefix and shows a source badge instead.
- `profiles` also gains `source TEXT NOT NULL DEFAULT 'redstar'` and `source_user_id TEXT NOT NULL DEFAULT ''`. The X numeric id and the YouTube `UC…` id are the stable keys, because handles can change. A handle change on X updates `display`/`nick` through a rename map (§3.4).
- `research_items.kind` has a `CHECK` constraint (`post`, `reply`, `search-result`, `local-file`, `web-url`). Adding kinds would mean a table rebuild, so new items reuse `post` and `reply` with namespaced refs. The item's own `posts.kind` carries the detail (video, segment).

### 3.3 Transcript segments

Following PLAN-4.0, **one transcript turn is one `posts` row**:

- `source = 'youtube'`, `kind = 'segment'`, `parent_post_id = in_reply_to_id = 'yt:<video>'`.
- `source_start_ms` and `source_end_ms` hold the timing, and `speaker` stays `''` until 4.0 diarization.
- `nick` is the channel (`yt:@channel`), with `title = ''`.

Raw caption cues are merged into readable turns: sentence boundaries, at most about 30 s, at most about 600 characters. The merge is deterministic, so re-runs dedupe by `post_id`. The raw subtitle file (json3 or vtt) is kept as an attachment (`mime text/vtt` or `application/json`) so a later re-segmentation or human correction (`transcript_corrections`, 4.0) can start from the original.

- A new column `transcript_kind` on the video's `metrics_json` (`manual` | `auto` | `none`) records whether the captions were human or automatic. 4.0 needs that to decide what is "quotable".

### 3.4 New tables (created on first use)

```sql
-- What to keep in sync (also editable in preferences.toml; DB wins after first import).
CREATE TABLE IF NOT EXISTS source_targets (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source TEXT NOT NULL, kind TEXT NOT NULL,        -- account|search|playlist|item|conversation
  value TEXT NOT NULL, resolved_id TEXT NOT NULL DEFAULT '',
  options TEXT NOT NULL DEFAULT '{}',              -- replies, transcripts, comments, depth…
  enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL,
  UNIQUE (source, kind, value));

-- Incremental sync and resume (§7). One row per target and direction.
CREATE TABLE IF NOT EXISTS source_cursors (
  target_id INTEGER NOT NULL REFERENCES source_targets(id) ON DELETE CASCADE,
  direction TEXT NOT NULL CHECK (direction IN ('forward','backfill')),
  cursor TEXT NOT NULL DEFAULT '', newest_id TEXT NOT NULL DEFAULT '', oldest_id TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending',          -- pending|running|done|error|budget
  attempts INTEGER NOT NULL DEFAULT 0, wait_until INTEGER NOT NULL DEFAULT 0,
  last_error TEXT, updated_at TEXT NOT NULL,
  PRIMARY KEY (target_id, direction));

-- Per-item expensive stages (transcript, comments, live chat, media), resumable.
CREATE TABLE IF NOT EXISTS source_item_stages (
  post_id TEXT NOT NULL, stage TEXT NOT NULL,      -- transcript|comments|live_chat|media|thread
  status TEXT NOT NULL, cursor TEXT NOT NULL DEFAULT '', attempts INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL, PRIMARY KEY (post_id, stage));

-- Spend and quota ledger, so runs can refuse before a cap (§6).
CREATE TABLE IF NOT EXISTS source_usage (
  day TEXT NOT NULL, source TEXT NOT NULL, path TEXT NOT NULL,   -- x-api|x-web|yt-api|yt-dlp
  resource TEXT NOT NULL, count INTEGER NOT NULL DEFAULT 0,
  est_cost_micros INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (day, source, path, resource));

-- Handle history (X handles and YouTube @handles change; the numeric id is the key).
CREATE TABLE IF NOT EXISTS account_aliases (
  source TEXT NOT NULL, source_user_id TEXT NOT NULL, nick TEXT NOT NULL, seen_at TEXT NOT NULL,
  PRIMARY KEY (source, source_user_id, nick));

-- Sealed per-source secrets (§4.3). Value is seal_blob(db_key, aad=name, plaintext).
CREATE TABLE IF NOT EXISTS source_secrets (name TEXT PRIMARY KEY, sealed BLOB NOT NULL, updated_at TEXT NOT NULL);
```

`attachments` gains `kind TEXT NOT NULL DEFAULT ''` (`image|video|gif|thumbnail|subtitle|audio`), and `on_site` gets a documented meaning: "1 = this file exists on the origin service". New sources set it to 1.

### 3.5 Export and import

- `EXPORT_FORMAT_VERSION` 2 → **3**. Each post gains `source`, `source_id`, `source_url`, `source_start_ms`, `source_end_ms`, `speaker`, `kind`, `lang` and `metrics`. The copy embedded in the HTML export gains them too.
- Importing a v2 file treats a missing `source` as `redstar`, as PLAN-4.0 requires.
- A v2 client refuses a v3 file. It already bails on a newer `format_version`.
- The HTML export shows a source badge and a permalink. YouTube segments render as a timestamped transcript under the video, with each timestamp linking to `…watch?v=ID&t=Ns`.
- A search export's `search` object gains `sources: [...]`.

### 3.6 Rollback note

A 3.x binary from before this change opens the migrated database, because the columns are additive. It will, however, list `x:alice` and `yt:@chan` as if they were Red Star nicks. That is acceptable, but the changelog should say so. The writes guard (§10.4) ships in the **same** release as the schema, so no client that can write ever sees a non-Red-Star row without that guard.

---

## 4. Per-source auth and configuration

### 4.1 `preferences.toml` (hand-edited, non-secret only)

```toml
[sources]
default = "redstar"              # what the TUI and headless commands use without --source
enabled = ["redstar", "youtube", "x"]

[youtube]
backend = "yt-dlp"               # "yt-dlp" (default) or "api"
ytdlp_path = "yt-dlp"            # or an absolute path; ACP_YTDLP overrides (tests use this)
channels = ["@hazaldin"]         # seeded into source_targets on first run
playlists = []
transcripts = true
sub_langs = ["en.*"]             # yt-dlp --sub-langs
auto_subs = true                 # also take auto-generated captions, marked as such
comments = true
max_comments = 500               # per video; 0 = none, "all" allowed
comment_depth = 1                # 0 = top-level only, 1 = replies to top-level (YouTube max)
comment_sort = "top"             # "top" | "new"
live_chat = false                # replay of past livestream chat (§5.5)
download = "none"                # "none" | "audio" | "video" (§8)
max_height = 720
sleep_requests = 1.0             # seconds between yt-dlp requests
api_daily_quota = 10000          # only with backend = "api"

[x]
mode = "api"                     # "api" | "web" | "auto" (api first, then web where allowed)
users = []                       # handles to sync, without @
searches = []
include_replies = true           # replies by the user (their own)
include_conversations = false    # other people's replies under the user's posts (§5.4)
include_reposts = false
max_posts_per_run = 500
monthly_budget_usd = 10.0        # hard stop from the source_usage ledger
web_enabled = false              # the session fallback is opt-in (§6.2, §13)
web_delay_ms = 4000              # minimum gap between web requests, plus jitter
web_daily_post_cap = 2000        # far below X's 1,000,000/24 h damages threshold (§13)
```

Unknown keys get the same warning as themes and feeds. A `[x]` or `[youtube]` table that is missing turns that source off. Nothing is fetched until a target exists.

### 4.2 Environment

These override for headless and CI use. Each is read once and never echoed:

- `ACP_X_BEARER`
- `ACP_YT_API_KEY`
- `ACP_YTDLP` (path)
- `ACP_X_API_BASE`, `ACP_X_WEB_BASE` and `ACP_YT_API_BASE` point at mocks, as `ACP_BASE_URL` does today.

### 4.3 Secrets

| Secret | How it gets in | Where it lives |
|---|---|---|
| X bearer token (app-only, OAuth 2.0) | `--x-token-set`, read from a no-echo prompt or stdin, **never argv** | `source_secrets` row `x.bearer`, sealed with `seal_blob` under the unlocked DB key |
| X web session (`auth_token`, `ct0`, `twid`, plus any others the app needs) | `--x-import-cookies FILE` (Netscape `cookies.txt` or a browser-export JSON). Only x.com cookies are kept; the file is not copied | `source_secrets` row `x.session` (`SavedSession` JSON, sealed) |
| YouTube Data API key | `--yt-api-key-set` (prompt or stdin) | `source_secrets` row `youtube.api_key` |
| YouTube cookies (only if a video needs sign-in) | `--yt-import-cookies FILE` | `source_secrets` row `youtube.cookies`. Given to yt-dlp as a **0600 temp file**, deleted after the run |

- Red Star's `session.json` stays plaintext 0600 today. §7.13.6 of the 3.0 plan already says sessions move into the keystore. Starting X sealed avoids adding a second plaintext credential file.
  > Open question Q2: does Michael prefer exact parity (`~/.acp/x-session.json`, 0600)? The plan recommends sealed.
- `--x-logout` and `--yt-forget-key` delete the rows.
- Backups (`.acpbackup`) contain the sealed rows, because they're inside the DB. That differs from Red Star, whose `session.json` is excluded. Recommend excluding them by deleting the `source_secrets` rows from the snapshot copy, so a restore asks for the token again.
  > Open question Q3.
- **The log masker** (`src/logging.rs`, the `--debug` redaction tests) gains:
  - the values of `Authorization: Bearer …` and `x-csrf-token`,
  - the cookies `auth_token`, `ct0`, `kdt`, `twid` and `att`,
  - `x-guest-token`, the value of `x-client-transaction-id`, and `guest_id`,
  - the `key=` query parameter (YouTube API),
  - yt-dlp's `--cookies` path and any `po_token`.
  - Every rule gets a unit test.
  - The pre-commit scan and `.gitleaks.toml` get rules for X bearer tokens (`AAAAAAAAAAAAAAAAAAAAA…`) and for `auth_token=`.

---

## 5. Source behaviour

### 5.1 YouTube via yt-dlp (default backend)

yt-dlp runs as a **subprocess**:
- arguments go through `Command::args` (no shell),
- a timeout is enforced,
- stderr goes to the job log after masking,
- stdout is parsed as JSON lines.

Minimum version check: `yt-dlp --version` must be at least a pinned floor. The app warns when it's older than about 90 days (§13).

| Step | Command (shape) | Stored as |
|---|---|---|
| List a channel, newest first | `yt-dlp --flat-playlist --dump-json --lazy-playlist https://www.youtube.com/@H/videos` (also `/streams` and `/shorts` per option) | ids, titles and dates. Stops at the first known id (§7) |
| List a playlist | `yt-dlp --flat-playlist --dump-json URL` | the same, in playlist order |
| Video metadata | `yt-dlp -J --skip-download --no-write-playlist-metafiles URL` | `posts` (`kind=video`), `profiles` (channel), tags, thumbnail URL |
| Transcript | add `--write-subs --write-auto-subs --sub-langs "en.*,-live_chat" --sub-format "json3/vtt/best" --skip-download -o <tmp>` | subtitle file → attachment; cues → segment rows (§3.3) |
| Comments | add `--write-comments --extractor-args "youtube:max_comments=500,all,all,10,1;comment_sort=top"` (the order is `max-comments,max-parents,max-replies,max-replies-per-thread,max-depth`) | `posts` (`kind=comment`); a reply's parent is the top-level comment |
| Live chat replay (opt-in) | `--write-subs --sub-langs live_chat --skip-download` (yt-dlp downloads replayed chat as a `live_chat` "subtitle" JSON) | `posts` (`kind=chat`, `source_start_ms` = video offset) |

Notes:
- **Pacing:** `--sleep-requests <yt.sleep_requests>` and `--sleep-subtitles 5`. Only one yt-dlp process runs at a time, and no parallel videos.
- **JavaScript runtime:** recent yt-dlp needs `yt-dlp-ejs` plus a runtime (deno recommended; node, bun and QuickJS are supported) for full YouTube support. Without one, some clients are skipped and formats can go missing. Metadata, captions and comments usually still work. `--doctor` reports which runtime it found. The app doesn't bundle one (§12).
- `--cookies-from-browser` isn't used, because it reads the browser's cookie store directly. Cookies come only from the user's explicit import.
- `--download-archive` isn't used. Our own `posts` table is the archive.

### 5.2 YouTube Data API v3 (optional backend)

`backend = "api"` with an API key gives:
- `channels.list` → uploads playlist id,
- `playlistItems.list` (50 per page),
- `videos.list` (batches of 50 ids),
- `commentThreads.list` and `comments.list` for replies.

Each list call costs **1 unit**. The default quota is **10,000 units per day**, reset at midnight Pacific time.

- **Transcripts aren't available this way:** `captions.download` (200 units) only works for videos the caller may edit. In API mode, transcripts still come from yt-dlp when it's installed. Otherwise they're marked `transcript_kind = none`.
- `search.list`: the current quota page lists it under its own small daily bucket. Older docs priced it at 100 units. The plan doesn't need search for channel and playlist sync.
- The `source_usage` ledger tracks units and refuses a run that would exceed `api_daily_quota`.

### 5.3 X API v2 (first path)

App-only bearer token (OAuth 2.0). Endpoints:

| Need | Endpoint | Notes (rate limits per 15 min, app / user) |
|---|---|---|
| Resolve a handle | `GET /2/users/by/username/:username` | → numeric id (the stable key), profile fields |
| A user's posts | `GET /2/users/:id/tweets` | 10,000 / 900. **Only the 3,200 most recent.** `exclude=retweets,replies` per options. `since_id` for incremental, `pagination_token` for backfill |
| Search, last 7 days | `GET /2/tweets/search/recent` | 450 / 300. 10–100 results, 512-character query |
| Full-archive search | `GET /2/tweets/search/all` | 1 per second, 300 per 15 min. 10–500 results, 1,024-character query. Available on pay-per-use |
| Thread replies | `search/recent` or `search/all` with `conversation_id:<root>` | recent only reaches back 7 days; older threads need full-archive |
| Post lookup | `GET /2/tweets?ids=` (100 per call) | 3,500 / 5,000 |
| Mentions of a user | `GET /2/users/:id/mentions` | 450 / 300. The most recent 800 |

- **Fields requested every time:**
  - `tweet.fields=created_at,author_id,conversation_id,in_reply_to_user_id,referenced_tweets,entities,public_metrics,lang,attachments,note_tweet,edit_history_tweet_ids`
  - `expansions=author_id,attachments.media_keys,referenced_tweets.id,referenced_tweets.id.author_id`
  - `media.fields=url,preview_image_url,variants,type,width,height,alt_text`
  - `user.fields=username,name,description,profile_image_url,created_at,public_metrics,verified`
- **Pricing (verified 2026-10-21). New developers have no tiers.** Pay-per-use credits:
  - Posts: Read **$0.005 per post returned**.
  - User: Read **$0.010** per user.
  - Owned reads (your own data) $0.001.
  - Repeated reads of the same resource within one **24-hour UTC day are deduplicated** (a "soft guarantee").
  - Pay-per-use is capped at **3,000,000 post reads per billing month**.
  - Legacy Basic and Pro subscriptions were migrated to pay-per-use in June and September 2026.
  - Example: syncing 10 accounts × 200 new posts a month is about 2,000 reads ≈ **$10 a month**. A first backfill of 3,200 posts each for 10 accounts is about 32,000 reads ≈ **$160**.
- **Cost guard:** before a run, estimate the maximum reads (pages × `max_results`) and show it. The CLI asks `Spend up to $X.XX? [y/N]` unless `--yes`. The run stops on `monthly_budget_usd` from the ledger. Responses are counted per resource returned, not per request.
- **Rate limits:** read `x-rate-limit-limit`, `-remaining` and `-reset` on every response. Sleep until `reset` when `remaining == 0`. A 429 sets `wait_until` on the cursor and ends the job cleanly with a resumable notice. One request at a time per app.
- **Reposts** are stored as a link row (`kind=post`, body "↻ reposted x:<id>"), plus the original post when expansions include it.
- **Edit history:** `edit_history_tweet_ids` → `post_versions`.

### 5.4 X logged-in web session (fallback)

The web path is used only when `mode = "web"`, or when `mode = "auto"` and **both** of these hold:
- the API can't serve the request: no token, budget exhausted, beyond 3,200 posts, or a search older than what the token allows;
- `web_enabled = true`.

It reads with Michael's imported cookies. This mirrors how the Red Star scraper reads with his session:

- **Transport:** the web app's `https://x.com/i/api/graphql/<queryId>/<Operation>` endpoints. These include `UserByScreenName`, `UserTweets`, `UserTweetsAndReplies`, `TweetDetail` and `SearchTimeline`. Each request sends:
  - the public web bearer,
  - `x-csrf-token` = `ct0`,
  - `x-twitter-auth-type: OAuth2Session`,
  - the session cookies.
- **Query ids rotate** with each web deploy. A stale id answers 404. Ids are read from a small, tested table and refreshed from the live JS bundle only when a 404 is seen (a "protocol self-check", like Red Star's bundle-hash gate). On failure the client falls back to read-only API mode with a clear message.
- **`x-client-transaction-id`:** as of mid-2026, X rejects some operations (`SearchTimeline`, `UserTweetsAndReplies`, `Followers`) with an empty 404 unless this per-request anti-bot header is valid. Others (`UserByScreenName`, `UserTweets`, `TweetDetail`) were reported ungated.
  - **This plan does not implement a generator** for that header. Building one is "working around technical limitations" and makes the ToS exposure worse (§13).
  - The web path therefore covers **ungated operations only**: profile, the user's posts timeline and thread detail.
  - Gated operations are reported as "not available through the web session".
  > Open question Q4: does Michael want the gated operations anyway?
- **Guest tokens:** the logged-out guest path reaches only a handful of operations now, and search isn't one of them. The plan doesn't use guest tokens. They are short-lived, rate-limited by IP and easily flagged.
- **Pacing:** one request at a time, `web_delay_ms` (default 4 s) plus 0–50 % jitter, at most 1 page of 20 posts per request, and `web_daily_post_cap` (default 2,000). A 429, a `Rate limit exceeded` body, a login challenge or a locked-account response stops at once, raises a notice and doesn't retry until the next day.
- **Headers:** the existing Firefox identity (`src/http.rs`) for parity with the browser the cookies came from.
- **Account safety:** this path uses Michael's real X account, which can be locked or suspended (§13). That is why it's off by default and capped.

### 5.5 Which kinds are included by default

| | Default | Option |
|---|---|---|
| X: the user's own posts and replies (`--said-by` semantics) | on | `include_replies` |
| X: others' replies under the user's posts | off (costs reads) | `include_conversations`, `--x-conversations` |
| X: reposts | off | `include_reposts` |
| X: Spaces, Communities | off (Q5) | later minors |
| YouTube: uploads | on | `/videos` |
| YouTube: livestream VODs and shorts | on and off | `/streams`, `/shorts` |
| YouTube: transcripts | on | `--transcripts` / `--no-transcripts` |
| YouTube: comments | on, top 500, depth 1 | `--comments N`, `--comment-depth` |
| YouTube: live chat replay | off (Q6) | `--live-chat` |

### 5.6 Searches

`--x-search` maps to recent search, or to full-archive with `--x-archive` (higher cost, with a confirm). YouTube search (`ytsearchN:` in yt-dlp) is supported but not stored as a sync target by default, because results are ranked and not chronological (like ranked Red Star feeds, §7).

---

## 6. Rate limiting summary

| Path | Pace | Hard caps | On limit |
|---|---|---|---|
| X API | headers; one request in flight; `full-archive` at most 1/s | `max_posts_per_run`, `monthly_budget_usd` (ledger), 3M per month | sleep to `reset` if under 15 min, else stop with a resumable cursor |
| X web | 4 s + jitter, serial | `web_daily_post_cap` (2,000), stop on any challenge | stop for the day, notice |
| yt-dlp | `--sleep-requests 1`, `--sleep-subtitles 5`, one process | `--limit` videos per run, `--scrape-budget` seconds | stop on HTTP 429 or "Sign in to confirm you're not a bot"; notice suggests cookies or waiting |
| YouTube API | 1 unit per list call | `api_daily_quota` | stop at the quota; resume after midnight PT |

The existing `--limit N` and `--scrape-budget SECS` apply to every source.

---

## 7. Incremental sync and resume

The same model as the 3.34 whole-site crawl, but per target:

1. **Forward (new items).**
   - X API: `since_id = newest_id`.
   - X web, yt-dlp and YouTube API: walk newest-first until **a page where every id is already known**, as Red Star's chronological feeds already do.
   - `newest_id` is advanced only after the page is committed.
2. **Backfill (older items).** Continues from `oldest_id` / `cursor` until the source is exhausted (X: the 3,200 limit or no `next_token`), the run limit or the budget. `status = budget` means "resume next run".
3. **Per-item stages** (`source_item_stages`): transcript, comments, live chat and media run after the item row exists. Each is resumable: the comment `continuation` and API `pageToken` are stored. `--refresh-comments` re-reads comments for videos younger than N days (comments keep arriving), using `comment_sort=new` and stopping at known comment ids.
4. **Transactions:** a page's accounts, items, tags, mentions and the cursor update commit in one SQLite transaction. A crash re-reads at most one page. `INSERT … ON CONFLICT(post_id) DO UPDATE` keeps the non-blanking rules already used for `body_json` and `score`.
5. **Rules:** after each commit, the rule engine evaluates the rows just written, with `source` set (§9).
6. **`--full`** ignores `newest_id` and walks again, as the crawl does.
7. **`--sync`:** `acprs --sync --source youtube` runs every enabled target of that source. `--sync --source all` runs Red Star, then YouTube, then X. The sources run one after another, never in parallel against one service.

---

## 8. Media download

- **X:**
  - Photos come from `media.url`, upgraded to `?name=orig`.
  - Video and GIF: the highest-bitrate `video/mp4` entry in `variants`.
  - Downloads go through `media::download_all` into the same attachments directory, deduped by `url_key`. Query strings are canonicalised, as in the 1.26.1 fix.
  - `--no-attachments` skips media, as it does today.
- **YouTube:**
  - Thumbnails (`maxresdefault`, then the best available) and the channel avatar and banner are on by default.
  - `download = "audio"`: `yt-dlp -f bestaudio -x --audio-format opus`. This prepares PLAN-4.0's local transcription. Off by default.
  - `download = "video"`: `-f "bv*[height<=720]+ba/b[height<=720]"`, merged by ffmpeg when present. Off by default; large.
  - The `[youtube] max_storage_gb` cap (default 20) refuses new files past it.
- Files are named `<source>/<id>.<ext>` under the attachments directory, so a Red Star `post_id` collision is impossible.
- Export with attachments copies them, as it does today.
- PDFs, audio and other files are never uploaded anywhere. These sources have no writes.

---

## 9. Rules, alerts, Feeds and the 4.0 corpus

- **Rule engine (3.32):**
  - `Condition::Source` accepts `x` and `youtube`. `same_source` keeps `site|redstar → site`, so existing user rules keep their meaning.
  - `Author` matches namespaced nicks (`x:alice`). The editor offers a source picker so users don't type prefixes.
  - New trigger fields: `kind` (video, comment, segment) and `transcript_kind`.
  - `rule_firings.source` records the source.
- **Alert templates (3.33):**
  - "Watch a user" takes a source.
  - "Replies to me" works per source once Michael's own handles are configured (`[x] me`, `[youtube] me`).
  - "Saved-search matches" spans every source unless filtered.
- **Notices (3.31):** finish, budget stop, 429, a challenge, and yt-dlp being too old or missing all raise notices.
- **Feeds (P7, not built yet):**
  - The `.acpfeed` rule model gains an optional `sources = ["redstar","x","youtube"]` filter and a `source` key on nick rules.
  - The share string still holds only rules: no tokens, no cookies.
  - Refreshing a feed from the site runs the matching source jobs.
  - This extends §6 of the 3.0 plan and doesn't change its numbering.
- **Research projects:**
  - Items can be X posts, videos or transcript segments (namespaced refs, §3.2).
  - Research › Add current view works across sources.
- **4.0 corpus:**
  - This plan delivers the post-source columns and YouTube transcript ingestion, the two prerequisites of the Haz stream corpus.
  - "Every Haz stream" is a target list (`[youtube] channels`, plus playlists) that Michael owns (Q7).
  - Diarization, human review and `transcript_corrections` stay in 4.0.
  - Auto captions are stored with `transcript_kind = auto`, so 4.0 never treats them as reviewed text.
- **P9 replication (later):** the CBOR `ItemRec` carries `source`, so X and YouTube records can replicate like Red Star posts. Site-observed precedence becomes "origin-observed". Media blobs follow §7.5.

---

## 10. CLI and TUI

### 10.1 CLI flags (clap, `help_heading` per group, same style as `--nick` and `--said-by`)

| Flag | Heading | Meaning |
|---|---|---|
| `--source redstar\|x\|youtube\|all` | Sources | Scopes scrape, sync, search, export, `--list-nicks`, `--remove-nick`. Default `[sources] default` |
| `--x-user HANDLE[,HANDLE…]` | X | Scrape these accounts (implies `--source x`) |
| `--x-search QUERY` | X | Recent search; `--x-archive` for full-archive (cost confirm) |
| `--x-post ID\|URL` | X | One post and, with `--x-conversations`, its thread |
| `--x-conversations` | X | Include other people's replies in the user's threads |
| `--x-mode api\|web\|auto` | X | Override `[x] mode` |
| `--x-token-set` / `--x-token-clear` | X | Read the bearer token from a prompt or stdin; never argv |
| `--x-import-cookies FILE` / `--x-logout` | X | Import or forget the web session |
| `--x-cost` | X | Print the month's ledger and the estimate for the next run; fetch nothing |
| `--yt-channel @H\|ID\|URL[,…]` | YouTube | Channel uploads (`--yt-tabs videos,streams,shorts`) |
| `--yt-playlist ID\|URL[,…]` | YouTube | A playlist |
| `--yt-video ID\|URL[,…]` | YouTube | Single videos |
| `--transcripts` / `--no-transcripts`, `--sub-langs LANGS`, `--no-auto-subs` | YouTube | Transcript stage |
| `--comments [N]` / `--no-comments`, `--comment-depth 0\|1`, `--comment-sort top\|new` | YouTube | Comment stage |
| `--live-chat` | YouTube | Live chat replay stage |
| `--yt-backend yt-dlp\|api`, `--yt-api-key-set`, `--yt-import-cookies FILE` | YouTube | Backend and credentials |
| `--yt-download none\|audio\|video` | YouTube | Media files beyond thumbnails |
| `--refresh-comments DAYS` | YouTube | Re-read comments on recent videos |
| `--targets-list`, `--target-add SOURCE KIND VALUE`, `--target-remove ID` | Sources | Saved sync targets |

- **Reused unchanged:** `--said-by`, `--limit`, `--order`, `--full`, `--scrape-budget`, `--format json,html`, `--output`, `--compress --tar-gz`, `--no-attachments`, `--search`, `--export`, `--sync`, `--dry-run` (which prints the planned requests and the cost estimate and opens no connection) and `--yes`.
- **Every write flag** (`--new-post`, `--reply`, `--vote`, `--subscribe`, `--delete`, `--edit`, `--repost`, `--bookmark` push) **conflicts with `--source x|youtube`** at the clap level, so the error appears before anything runs.

### 10.2 TUI

- **Source switcher:**
  - A header chip `Source: All ▾` (Red Star · X · YouTube · All).
  - View › Source in the menu, plus a palette entry.
  - The key is to be chosen against existing bindings; F8 is a candidate, since F5, F6 and F7 are taken or planned.
  - The choice persists in `[ui] source`. "All" interleaves by `posted_at`.
- **Nick bar:** grouped sections ★ Red Star, 𝕏 X and ▶ YouTube, each with counts. The prefix is hidden and the badge is shown instead.
- **Cards and detail:**
  - A source badge and a permalink.
  - X posts show metrics and media.
  - A **video detail** has tabs: Description · Transcript (timestamps; Enter opens the browser at `&t=`) · Comments (nested like Red Star replies) · Chat.
  - Segment search results open the video at that segment.
- **Read-only chrome:**
  - Compose, Reply, Quote, Vote, Subscribe, Delete, Edit and Repost are greyed out on non-Red-Star posts.
  - Their tooltip says "Read-only source (X)" and links to the `sources-read-only` Help topic. This follows the §4.3 tooltip rule.
  - Local bookmarks still work (`remote = 0`, never pushed).
- **Start Scanning:** gets a Source dropdown that swaps in that source's fields (handles, searches, channel, playlist, transcripts, comments, depth, budget).
- **Account menu:** X › Set token, Import cookies, Spending · YouTube › API key, Import cookies, yt-dlp status.
- **Help:** topics `source-x`, `source-youtube` and `sources-read-only`. Getting Started gains a short section. Every new flag gets a description (§4.4 rule).

---

## 11. Testing strategy

Nothing in the test suite talks to x.com, api.x.com, twitter.com, youtube.com, googleapis.com or googlevideo.com.

- **Guards:** under `cfg(test)` and in the cucumber harness, the source HTTP clients **panic** if the host is one of those domains. This is the same rule as `acp.us`. A test also asserts that the yt-dlp runner refuses a real `yt-dlp` binary when `ACP_TEST` is set.
- **Mocks** (`tests/support/`):
  - `mock_x_api.rs`: axum. Serves `/2/users/by/username`, `/2/users/:id/tweets`, `/2/tweets/search/recent|all` and `/2/tweets`, with paging, `since_id`, rate-limit headers, a scripted 429, and an invalid-token 401. It also asserts the `Authorization` header is present and never logged.
  - `mock_x_web.rs`: GraphQL `UserByScreenName`, `UserTweets` and `TweetDetail`. Includes a stale-queryId 404, a login challenge, and a missing `ct0` case.
  - `mock_yt_api.rs`: channels, playlistItems, videos, commentThreads, comments, and a quota-exceeded 403.
  - **`fake-yt-dlp`**: a tiny test binary (`tests/bin/fake_ytdlp.rs`) selected through `ACP_YTDLP`. It checks argv against expectations, rejects shell metacharacters, sleeps on cue, and prints fixture JSON or subtitle files. It also covers version-too-old, the "not a bot" error, and a mid-stream crash.
- **Fixtures** (`tests/fixtures/x/`, `tests/fixtures/youtube/`):
  - **Synthetic or minimised:** invented ids, handles and text that keep the real field shapes. These include long `note_tweet`, edits, quotes, media variants, json3 and vtt captions with overlaps, live-chat JSON, and nested comments.
  - A real capture is used only after redaction, gated by the existing `scan.sh` and gitleaks, as the Red Star reply capture was.
- **Gherkin features:** `x_api.feature`, `x_web.feature`, `youtube.feature`, `sources.feature`. Scenarios:
  - scrape → export JSON/HTML (v3) → import into a second throwaway HOME → same rows;
  - incremental sync stops at known ids;
  - resume after a 429 or a budget stop;
  - `--said-by` keeps only the user's own posts;
  - transcript segments are deterministic across runs;
  - comment depth;
  - a write flag with `--source x` is refused;
  - the TUI read-only tooltips;
  - the source switcher filters lists;
  - rules fire with `source = x`;
  - the masker hides bearer, cookies and API key in `--debug`.
- **Unit tests:**
  - the record mapping for each field;
  - id and nick namespacing round-trips;
  - the migration on a 3.5.0 fixture DB (encrypted), checking row counts and that Red Star rows are unchanged;
  - the cost estimator;
  - cue → turn merging;
  - the yt-dlp argv builder (no secrets in argv; cookies passed only as a temp file path);
  - the budget never going negative.
- **Manual live checks:** a `docs/LIVE-SOURCES-TEST.md` checklist in the style of LIVE-PUBLISH-TEST:
  - run by Michael on his own machine with a throwaway HOME (never the real `~/.acp`, where auto-sync is on);
  - first `--dry-run`, then one small target per path.
  - Never in CI and never from an agent VM.

---

## 12. Packaging and runtime dependencies

- **yt-dlp is a runtime dependency of the YouTube source only.** Red Star and X work without it. The app detects it (`ytdlp_path`, `ACP_YTDLP`, `$PATH`) and explains how to install it when it's missing.

| Package | Change |
|---|---|
| Debian (`debian/control`) | `Recommends: yt-dlp, ffmpeg`. `Suggests: deno \| nodejs`. Distro yt-dlp often lags; the docs recommend the upstream binary or `pipx install yt-dlp` and `yt-dlp -U` |
| Arch (`packaging/arch/PKGBUILD`) | `optdepends+=('yt-dlp: YouTube source' 'deno: full YouTube support in yt-dlp' 'ffmpeg: merge downloaded video and audio')` |
| Fedora (`packaging/fedora/acp-red-star.spec`) | `Recommends: yt-dlp`. `Suggests: ffmpeg-free`, `nodejs` |
| `scripts/install.sh` | Prints a hint when yt-dlp is missing; never installs it silently |

- A hard dependency was rejected: it would force Python onto users who don't want YouTube, and distro copies break whenever YouTube changes.
- Packaging stays on the alpha gate in §4.5 of the 3.0 plan. Nothing here cuts a release.
- No new native libraries are needed for X; it uses `reqwest` and `rustls`, which are already present.
- `--doctor` (planned in §7.14.5) gains:
  - the yt-dlp version and age,
  - the JS runtime it found,
  - whether ffmpeg is present,
  - X token presence (yes/no, never the value),
  - the web session's age,
  - the month's spend.

---

## 13. Risks

1. **X Terms of Service and anti-scraping.**
   - X's terms forbid crawling or scraping "in any form, for any purpose" without prior written consent, and forbid working around technical limits.
   - Since late 2024 they also include **liquidated damages of $15,000 per 1,000,000 posts** requested, viewed or accessed in any 24-hour period.
   - The **web-session path is exactly that kind of access.** It can get Michael's X account locked or suspended, and the damages clause is a real legal exposure, even at small volume.
   - Mitigations:
     - the API is the default;
     - the web path is opt-in (`web_enabled = false`) with a one-time warning screen;
     - only ungated operations are used, and there's no generator for anti-bot headers;
     - serial pacing;
     - a 2,000-post daily cap (a factor of 500 under the threshold);
     - no guest tokens, no account rotation, no proxies.
   - Michael should decide explicitly (Q4).
2. **X API cost.**
   - Pay-per-use bills each post returned ($0.005) and each user ($0.010). Backfills are the expensive part, and the 3,200-post timeline limit means older history needs full-archive search, which costs more requests.
   - Mitigations: the estimate plus confirm, `monthly_budget_usd`, the ledger, same-day dedup, `max_posts_per_run`, and `--x-cost`.
   - Prices changed twice in 2026, so the estimator's rates live in one constant table with a date and a test.
3. **X API churn.** Tiers were retired in 2026, endpoints and fields move, and the rate-limit tables change. The fixtures pin our parser to the shapes we tested. A schema drift is logged with the unknown fields, not fatal.
4. **yt-dlp breakage.**
   - YouTube changes often:
     - PO tokens,
     - the JS challenge (now requiring yt-dlp-ejs plus a runtime),
     - "Sign in to confirm you're not a bot",
     - IP throttling.
   - Mitigations:
     - a version floor and staleness warning;
     - parsing only stable `info.json` keys;
     - per-stage failure that doesn't lose the item;
     - a resumable stage table;
     - an optional Data API backend for metadata and comments;
     - a clear notice telling the user to run `yt-dlp -U`.
5. **YouTube terms.**
   - YouTube's terms also restrict automated access outside the API.
   - Comments and transcripts are other people's content.
   - The 4.0 "rights, permission and credit" note (PLAN-4.0 PROPOSED 8) should cover what may be stored and exported.
   - The Data API path is the more compliant one where it can serve the request.
6. **Privacy.** Collecting other people's X posts and YouTube comments into a database that may later replicate (P9) widens exposure. Non-Red-Star records should be **excluded from replication by default** until §7.7 of the 3.0 plan is revisited (Q9).
7. **Schema mixing.**
   - Namespaced ids avoid collisions, but older binaries will show mixed rows (§3.6).
   - The writes guard ships in the same release as the schema.
   - A bug that forgets the guard could try to "reply" to `x:123` on Red Star. A unit test sends every `SiteAction` with a non-Red-Star id and expects a refusal before any frame is built.
8. **Disk.** Video downloads and comment-heavy channels grow fast. The defaults (thumbnails only, 500 comments) and `max_storage_gb` keep it bounded.
9. **Credential handling.** Bearer tokens and cookies are long-lived. They are sealed, masked and never in argv or env dumps. The temp cookie file for yt-dlp is 0600 and deleted in a `Drop` guard. Tests assert all of this.

---

## 14. Phased plan and versioning

Michael's rule applies: **one minor per feature, a fix is a patch**, changelog plus tag on every bump.

**Prerequisite:** a version and changelog cleanup is **pending**. The crate is at 3.5.0 with about 12 features under `[Unreleased]`, and plan stage numbers (3.30–3.37) aren't real crate versions. So this plan uses **relative labels**. "S1 = next minor" means the first minor released *after* that cleanup lands; S2 is the one after, and so on. Final numbers are assigned when the cleanup decides the current version. Numbered 3.x stages in the 3.0 plan are not moved.

| Label | Feature | Contents | Depends on |
|---|---|---|---|
| **S1 (next minor)** | Source layer | Core `source` module and trait; post-source columns, namespaced ids and nicks, new tables; writes guard; export v3 and import; source switcher, badges and grouped nick bar; `--source`; rules `source` values; masker additions; mocks and harness | cleanup release |
| **S2 (next minor + 1)** | YouTube metadata via yt-dlp | Channels, playlists and videos; incremental and backfill; thumbnails; the yt-dlp runner, version check and fake; packaging Recommends | S1 |
| **S3 (+2)** | YouTube transcripts | Subtitles → segment rows; Transcript tab; `--transcripts`, `--sub-langs`; `transcript_kind` | S2 |
| **S4 (+3)** | YouTube comments | Comments and replies, depth and sort, `--refresh-comments`, the Comments tab | S2 |
| **S5 (+4)** | X API v2 | Token handling; users, search and post lookup; conversations; media; the cost estimator, ledger and confirm; rate-limit handling | S1 |
| **S6 (+5)** | X web-session fallback | Cookie import, GraphQL ungated operations, query-id refresh, pacing and caps, warning screen. **Only if Michael approves after reading §13** | S5 |
| **S7 (+6)** | YouTube Data API backend | API key; channels, playlists, videos and comments via the API; quota ledger | S2, S4 |
| **S8 (+7)** | Media files | YouTube `download = audio\|video`, storage cap, X video variants at best bitrate | S2, S5 |
| optional | YouTube live chat replay | `kind = chat` rows and a Chat tab | S3 (Q6) |
| optional | X Spaces / Communities metadata | Read-only listing (no audio) | S5 (Q5) |
| later, in P7 | Feeds across sources | `sources` filter in `.acpfeed` | Feeds core |

- Order rationale:
  - YouTube first: it has lower legal and account risk, no per-read cost, and it is the 4.0 corpus prerequisite.
  - X API before the web path, because the API is the sanctioned route.
  - Michael may reorder (Q8).
- Each minor ships with its tests, Help topics, changelog entry and a tag (the hook tags it).
- None changes Red Star behaviour, except S1's export bump and the source switcher.

---

## 15. Open questions for Michael

1. **Crates or modules:** two new workspace crates for X and YouTube (recommended), or modules in the root crate?
2. **X cookie storage:** sealed in the encrypted DB (recommended), or a 0600 `~/.acp/x-session.json` for parity with Red Star?
3. **Backups:** should `.acpbackup` include the sealed X token, X cookies and YouTube key, or strip them so a restore asks again (recommended)?
4. **X web session:** after reading §13 (ToS, the $15k-per-1M clause, account-suspension risk), keep the fallback? If yes, ungated operations only (recommended), or also search and replies, which need an anti-bot header generator?
5. **X scope:** which accounts and which searches? Michael's own handle for "replies to me"? Include other people's replies under those accounts' posts (costs reads)? Reposts? X Spaces metadata? Communities?
6. **YouTube scope:** which channels and playlists make up "every Haz stream" (PLAN-4.0)? Include shorts and livestream VODs? **Live chat replay** for streams? Comment depth (top-level only, or plus replies) and a cap per video (default 500, top-sorted)? Which caption languages, and should auto captions count?
7. **Media:** thumbnails only by default, or also audio (for 4.0 local transcription) or full video? What storage cap?
8. **Order:** YouTube first (S2–S4), then X (S5–S6), as proposed, or X first?
9. **Replication:** keep X and YouTube records out of P9 peer sync by default (recommended) until the 3.0 plan's §7.7 is revisited?
10. **Budget:** what monthly X API spend cap (default $10) and what per-run post cap? Is a first backfill of up to 3,200 posts per account worth about $16 per account?
11. **YouTube API:** does Michael have, or want, a Google Cloud project and API key for the optional backend, or yt-dlp only?
12. **Display:** in "All" mode, should Red Star, X and YouTube interleave by date (proposed), or always group by source?
13. **Key for the source switcher:** F8, or another binding?
