# acp-rs-scraper

> **Full usage guide:** [docs/USAGE.md](docs/USAGE.md) · Install/uninstall: [docs/INSTALL.md](docs/INSTALL.md) · API docs: [docs/index.html](docs/index.html)

Rust 2024 CLI. It logs in to https://acp.us/rs (Red Star forum) as Firefox, scrapes all posts by a nick plus the comments on them, and stores them in SQLite.

    cargo run --release

Flow: reads `email` from `~/.acp/preferences.toml` (prompts and creates it if missing) -> LiveView `send-login-email` -> prompts for the code -> LiveView `submit-code` -> HTTP POST of the `phx-trigger-action` form to `/auth/user/otp/sign_in` -> checks that `/rs` no longer redirects to `/auth` -> prompts for a nick -> scrapes -> writes `~/.acp/ai/data/rs.database` (set by `DB_REL_PATH`).

## How it works
- **Login:** the site is Phoenix LiveView. The client keeps the rendered tree (join `rendered` merged with each reply's `diff`) and turns it back into HTML. From that HTML it reads the code form's event, field and `phx-target`, and the hidden inputs of the `phx-trigger-action` form. It then POSTs those inputs plus `_csrf_token` as a normal form, using the same cookie jar.
- **Profile:** `/rs/@{nick}` has `div#forum/Feed[data-props]` JSON (`posts`, `next_cursor`, `at_end`). Further pages come from the LiveView over the websocket, `pushEvent("page-next", {cursor})`. That name and payload were found in the site's `Feed-*.js` bundle. The client repeats it until `at_end`, or until the cursor stops changing.
- **Threads:** `/rs/post/{id}` has `div#forum/Thread[data-props]` with `replies`. More replies (best effort): if the page has a `[phx-hook=InfiniteScroll]` element with `phx-target`, the client pushes `next-page` (`{}`) to it. That is the generic hook from `app-*.js`. It then collects reply objects found in the returned diff.

## Data mapping
- **Root posts:** a feed item whose `thread_id` is missing or equals its own `id`. Stored with `parent_post_id = ""`.
- **The nick's replies in other people's threads:** stored with `parent_post_id = thread_id`.
- **Replies in the nick's threads:** stored with `parent_post_id = thread id` and `nick` = the reply author's username. They get `title = ""`; root posts keep their own `title`.
- **`content`:** the ProseMirror `body` flattened to text, or `body_text_summary` if that comes out empty.
- **`posted_at`:** `inserted_at`.
- **Re-runs:** a unique index on `post_id` plus `INSERT OR IGNORE`, so running again adds no duplicate rows.

Crates: `scraper` (the crate named `rust_scraper` is a TUI crawler app, not an HTML-parsing library), reqwest with cookies, tokio-tungstenite, rusqlite (bundled), toml/serde, regex.

## Paging and debugging
- **Where a reply arrives:** the site bundles phoenix_live_view 1.2.12. With that version, a hook `pushEvent` reply comes back in `phx_reply` `response.diff.r`. The scraper reads it from there, falling back to `response.r`. The `r`/`e`/`t` keys are removed before the diff is merged into the page state.
- **Feed:** sends `page-next {cursor}` repeatedly. It uses the reply if it has `posts`; otherwise it rebuilds the page and reads `forum/Feed` `data-props` again. It dedupes by id and stops at `at_end`, after 2 pages with nothing new, or when the cursor stops changing.
- **Threads:** sends `next-page {}` (the InfiniteScroll hook), with that element's `phx-target` as cid if it has one. It only tries if a hook element exists or `root.replies_count` is greater than the replies loaded. New replies are taken from the reply and from the rebuilt `forum/Thread` `data-props`, and it stops once nothing new comes back.
- **Debugging:** `cargo run -- --debug` or `ACP_DEBUG=1` prints every websocket frame sent and received to stderr, cut at 2000 characters.

## Export, attachments, search
Every scrape is still saved to SQLite. Progress messages go to stderr, so stdout carries only the export.

    cargo run --release -- --nick alice                          # JSON to stdout
    cargo run --release -- --nick alice --format html --output alice.html   # writes ./attachments/ beside it
    cargo run --release -- --nick alice --compress --tar-gz                 # ~/.acp/ai/data/exports/alice-<ts>.tar.gz
    cargo run --release -- --nick alice --format html --compress --tar-gz --name alice-archive --output /tmp/alice-archive.tar.gz
    cargo run --release -- --search "general strike"             # search the local DB; no login
    cargo run --release -- --search strike --nick alice --format html --compress --tar-gz --name strike-posts
    cargo run --release -- --format html --compress --tar-gz --name my-export   # -> ~/.acp/ai/data/exports/my-export.tar.gz

- `--format json|html` (default `json`). `--output <path>` is a file path; with `--compress` it is the archive path. `--name <name>` sets the file basename, the archive name, the folder inside the archive and the HTML title. The default name is `<nick>-<YYYYmmdd-HHMMSS>`, or `search-<slug>-<ts>` for a search. `--no-attachments` skips downloads. `--debug` logs LiveView traffic.
- **Compression:** `--compress` plus exactly one format flag; `--tar-gz` is the only one so far (more, such as `--zip`, can be added later). `--compress` without a format flag, a format flag without `--compress`, or two format flags is an error. Without `--compress`, nothing is compressed.
- **Folder layout:** an archive contains `<name>/<name>.<ext>` and `<name>/attachments/`.
- **JSON:** `{title, nick, exported_at, posts:[{post_id, parent_post_id, nick, title, content, posted_at, attachments:[{url, kind, mime_type, local_path}], children:[...]}]}`. When output goes to stdout, `local_path` is the absolute path of the saved copy. When it goes to a file or bundle, the path is `attachments/<file>`.
- **HTML:** a single page with threads and nested replies, plus `<img>`, `<video>` and `<audio>` tags that point at relative attachment paths.
- **Attachments:** URLs are taken from `post.media[]` (`url`, and `thumb_url` for video posters) and any `attachments`/`files`/`images` arrays. Body node attributes `src`, `url` and `href` are also used when they point at media.acp.us, posts-media.acp.us or Backblaze. Each URL is downloaded once, using the logged-in client, into `~/.acp/ai/data/attachments/<hash>.<ext>`. The extension comes from the Content-Type, otherwise from the URL. Failed downloads print a warning and are skipped. Each is recorded in a new `attachments` table (`post_id, url, local_path, mime_type`, unique on `(post_id, url)`); the `posts` table is unchanged.
- **Search:** case-insensitive `LIKE` on title and content, with `--nick` as an optional author filter. The export includes each match plus its parent and direct children, along with their stored attachments.
- **Embeds:** YouTube, X, TikTok, Instagram and Facebook embeds are `embed` body nodes. They are third-party players and are not downloaded.

## Offline DB export and import
    cargo run --release -- --export all.json                                  # whole DB
    cargo run --release -- --export alice.html --format html --nick alice
    cargo run --release -- --export strike.tar.gz --search strike --compress --tar-gz --name strike
    cargo run --release -- --import all.json                                  # plain JSON
    cargo run --release -- --import alice.html                                # HTML export (reads the embedded JSON)
    cargo run --release -- --import strike.tar.gz --compress --tar-gz         # archive

- **`--export <FILE>`** dumps the local DB with no login: all posts, or filtered by `--nick` and/or `--search`. It works with `--format`, `--compress --tar-gz` and `--name`, and it replaces `--output` for DB exports (the two can't be combined). `--search` without `--export` still writes to stdout or `--output`, so `--search q --output f` is the same as `--export f --search q`. `--search` exports include each match's parent and direct children; full and `--nick` exports include just those posts.
- **`--import <FILE>`** reads an earlier export into the DB with no login. Posts are added with `INSERT OR IGNORE` on `post_id`. Attachments are copied into `~/.acp/ai/data/attachments/`, their `local_path` is rewritten, and they are deduped by `(post_id, url)`.
  - **Compression comes from the flags, not the file name.** `--compress --tar-gz` means the file is unpacked as tar.gz, then the first `.json` (or else `.html`) inside is imported. Without those flags the file is read as plain JSON or HTML; which one is decided from the content.
  - If the file starts with the gzip bytes `1f 8b` and no compression flags are given, you get a clear error, and the same happens the other way round.
- **Export format:** JSON exports include `"format": "acp-rs-export"` and `"format_version": 1`. Import refuses an unknown format or a newer version. HTML exports carry the same JSON in `<script type="application/json" id="acp-data">`, so they can be imported again.
- **What can't be combined:** `--import` can't be used with `--export`, `--search`, `--nick`, `--output`, `--format`, `--name` or `--no-attachments`, and `--export` can't be used with `--output`.

## Multiple nicks
    cargo run --release -- --nicks @haz,@m3talsmith --format html --compress --tar-gz
    cargo run --release -- --export both.json --nicks haz,m3talsmith
    cargo run --release -- --search strike --nicks "@haz, @m3talsmith"

- **`--nicks <LIST>`** takes a comma-separated list. A leading `@` is optional, spaces are trimmed, empty entries are dropped and repeats are removed (ignoring case).
- **`--nick <NICK>`** is a one-nick shortcut for `--nicks`; the two can't be used together. With neither, the program asks for a nick (a comma-separated list is accepted there too).
- **Scraping:** all nicks share one login session and run one after another. If a nick fails, a warning is printed and the next one runs. A summary per nick (items, root posts, replies, new DB rows) is printed to stderr. The run fails only if every nick fails.
- **Export:** one export covers every nick that succeeded. Its default name is `haz+m3talsmith-<ts>`, or `multi-<ts>` for more than 4 nicks or names longer than 40 characters.
- **Filtering:** with `--export` or `--search`, `--nick`/`--nicks` keeps posts by any listed nick (ignoring case).

## Mentions and profiles
Mentions are **on by default**. Use `--no-mentions` to skip. There is no site-wide search in the public JS (no `/rs/search`, only composer `mention-query` autocomplete and `show-info` hover cards), so mentions are found by crawling the Latest feed at `/rs` (capped by `--crawl-limit`, default 500), then verifying each candidate by fetching its thread.

Match types (stored in `mentions(post_id, mentioned_nick, match_type)` with a unique key):
- `mention` — ProseMirror `mention` node whose `attrs.id` (user_id) or `attrs.label` matches
- `nick` — whole-word, case-insensitive plain-text occurrence of the username (no `@` required)
- `display_name` — whole-word match of the profile display name (skipped if shorter than 3 characters)

Profiles are scraped from each nick's `/rs/@{nick}` page (data-props / og: tags / feed author fields) into `profiles` (upsert on `nick`): display name, user_id, bio, avatar/banner URLs and local paths, location, role, joined_at, follower/following/post counts, `raw_json`, `scraped_at`. Avatar and banner are downloaded as attachments when attachments are enabled.

JSON/HTML exports include `profiles` and `mentions` (`format_version` 2; import still accepts version 1). HTML shows a profile header per nick and a Mentions section.

    cargo run --release -- --nick alice --no-mentions
    cargo run --release -- --nicks @haz,@m3talsmith --crawl-limit 200

## Incremental scraping
Re-runs avoid duplicates beyond `INSERT OR IGNORE`:
- Feed paging stops when a page's post_ids are all already in the DB (use `--full` to force a complete re-crawl).
- Threads already stored are skipped unless the feed's `replies_count` is higher than the children already in the DB (or `--full`).
- Attachment downloads skip posts whose `post_id` is already known (files on disk are still reused by URL hash).
- The same incremental stop applies to the mentions Latest-feed crawl.
- Before scraping, stderr lists each target as `known (display, N posts[, profile scraped])` or `new`.

## List known nicks
    cargo run --release -- --list-nicks
    cargo run --release -- --list-nicks --format json

Prints nick, display name, post count, latest `posted_at`, whether a profile row exists, and `scraped_at`. Table by default; JSON when `--format json` is given explicitly.


## Feed mode
    cargo run --release -- --nicks @haz                 # Latest (default)
    cargo run --release -- --nicks @haz --latest        # same
    cargo run --release -- --nicks @haz --trending      # Trending tab

- **`--latest` (default):** site-wide crawls (the mentions crawl) use `https://acp.us/rs/latest`. Chronological, so incremental early-stop on all-known pages still applies.
- **`--trending`:** site-wide crawls use `https://acp.us/rs` (the site's default Trending tab; `/rs/trending` is 404). Not chronological, so incremental early-stop is **disabled** — paging runs until `at_end` or `--crawl-limit`, with dedupe only. Use `--full` only matters for thread/attachment skipping here.
- **Profiles** always use Latest at `/rs/@{nick}` (profile tabs are Latest and Top; there is no Trending on profiles). `--trending` does not change profile scrapes.
- The two flags are mutually exclusive. Verified routes: `/rs` (Trending), `/rs/latest`, `/rs/top`, `/rs/@nick/top`.

## Logging
    cargo run --release -- --nick alice -q          # quiet: no progress
    cargo run --release -- --nick alice -v          # verbose: per-page / per-thread
    cargo run --release -- --nick alice --debug     # wire log (implies verbose)

- **`--quiet` / `-q`:** suppress progress and summaries on stderr. Prompts and errors still show. Conflicts with `-v` / `--debug`.
- **`--verbose` / `-v`:** extra detail (new posts per page, thread reply counts, skipped known items).
- **`--debug`:** raw LiveView frames (also `ACP_DEBUG=1`); implies verbose.

## License

Licensed under the GNU General Public License v3.0 or later. See [LICENSE](LICENSE).
