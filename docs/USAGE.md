# acp-rs-scraper: usage guide

`acp-rs-scraper` logs in to the Red Star forum (https://acp.us/rs) as Firefox and scrapes one or more nicks' posts, the replies in their threads, the posts that mention them, and their profiles into a local SQLite database. It downloads attachments and exports to JSON or HTML, optionally compressed. Search, export, import, `--list-nicks`, `--remove-nick` and `--clear-all` run offline.

- [Install and build](#install-and-build)
- [First run and login](#first-run-and-login)
- [Where data lives](#where-data-lives)
- [Flags](#flags)
- [Workflows](#workflows)
- [Database schema](#database-schema)
- [Export format (version 2)](#export-format-version-2)
- [Troubleshooting and debugging](#troubleshooting-and-debugging)
- [Known limitations](#known-limitations)
- [Contributing](#contributing)
- [API docs](#api-docs)

## Install and build

For distro packages (Arch, Debian, Fedora and derivatives), `scripts/install.sh` and uninstalling, see [INSTALL.md](INSTALL.md). Building by hand:

You need Rust 1.88 or newer (`rust-version` in Cargo.toml: edition 2024, plus let-chains in the `scraper` dependency) (install with [rustup](https://rustup.rs)). SQLite is compiled in (rusqlite `bundled`) and TLS uses rustls, so there are no system libraries to install.

    git clone https://forgejo.acp.us/m3talsmith/acp-rs-scraper
    cd acp-rs-scraper
    cargo build --release
    ./target/release/acp-rs-scraper --help

You can also run it through cargo with `cargo run --release -- <flags>`. The first build sets git's `core.hooksPath` to `.githooks` (see [Contributing](#contributing)).

## First run and login

1. Run a scrape, for example `acp-rs-scraper --nick alice`.
2. **Email:** the tool reads `email` from `~/.acp/preferences.toml`. If the file doesn't exist, it creates `~/.acp/`, asks for your email, and saves it:

       email = "you@example.com"

   To use a different account, edit or delete that file.
3. The tool asks the site to email you login details (the same form as the "Create account or log in" button).
4. **Code:** enter the verification code from the email (e.g. `KWMRUV`), or paste the login link instead.
5. The tool submits the code, completes the sign-in form the site sends back, and checks that `/rs` no longer redirects to `/auth`.
6. If you gave no `--nick`/`--nicks`, it asks for a nick; a comma-separated list is accepted.

The session lasts for one run only (an in-memory cookie jar), so each scrape logs in again. Search, export, import, `--list-nicks`, `--remove-nick` and `--clear-all` never log in.

## Where data lives

| What | Path |
|---|---|
| Preferences | `~/.acp/preferences.toml` |
| Database | `~/.acp/ai/data/rs.database` |
| Attachments (persistent copies) | `~/.acp/ai/data/attachments/<hash>.<ext>` |
| Default export / archive location | `~/.acp/ai/data/exports/` |

Every scrape is saved to the database. Exports are written on top of that.

## Flags

These are grouped the same way as `--help`. `-h`/`--help` prints help and `-V`/`--version` prints the version.

### Mode

| Flag | Meaning |
|---|---|
| `--latest` | Site-wide crawls use the Latest tab (`/rs/latest`). **Default.** It is chronological, so stopping early once pages are already known works. |
| `--trending` | Site-wide crawls use the Trending tab (`/rs`, the site's own default). It isn't chronological, so the early stop is disabled; paging runs until the end of the feed or `--crawl-limit`. |

The two can't be combined. Profiles always use their Latest feed (`/rs/@nick`) in both modes.

### Scraping

| Flag | Meaning |
|---|---|
| `--nick <NICK>` | One nick; the `@` is optional. Can't be used with `--nicks`. |
| `--nicks <LIST>` | A comma-separated list, e.g. `@haz,@m3talsmith`. Spaces are trimmed, `@` is optional, and repeats are removed (ignoring case). |
| `--no-mentions` | Skip the mentions crawl, which is on by default. |
| `--crawl-limit <N>` | The most site-feed posts to scan when looking for mentions (default 500). |
| `--no-attachments` | Don't download media, avatars or banners. |
| `--full` | Re-crawl everything: ignore known post ids when paging, refetch every thread, and process attachments for every post (files already on disk are still reused). |

**"Posts & replies":** every `--nick`/`--nicks` scrape walks both profile tabs. It reads the default **Posts** feed, then the **Posts & replies** tab, which holds the nick's replies in other people's threads, and pages each to the end. Items are merged by `post_id`, so nothing is stored twice; the database's unique `post_id` index enforces this too. Replies are stored with `parent_post_id` set to their thread's root post. Incremental runs stop on each tab once a page is entirely already-known posts; `--full` walks both tabs completely. The per-nick summary shows how many items came only from the replies tab.

### Search / Export (offline)

| Flag | Meaning |
|---|---|
| `--list-nicks` | List the nicks in the database: display name, post count, latest post, whether a profile was scraped and when. Prints a table, or JSON with `--format json`. |
| `--search <Q>` | Case-insensitive search of title and content. Exports each match plus its parent post and direct replies. Writes to stdout or `--output`. |
| `--export <FILE>` | Dump the database (everything, or filtered by `--nick`/`--nicks` and/or `--search`) to FILE. Takes the place of `--output`. |
| `--name <NAME>` | Sets the export's file name, archive name, folder inside the archive, and HTML title. |

`--nick`/`--nicks` also filter `--search` and `--export` (posts by any listed nick).

Default names: `<nick>-<YYYYmmdd-HHMMSS>`, `nick1+nick2-<ts>` for several nicks (`multi-<ts>` for more than 4 nicks or more than 40 characters), `search-<slug>-<ts>` for a search, and `db-<ts>` for a full dump.

### Maintenance (offline)

| Flag | Meaning |
|---|---|
| `--remove-nick <LIST>` | Delete nicks from the database. Comma-separated, `@` optional, same parsing as `--nicks`; matching ignores case. |
| `--keep-replies` | With `--remove-nick`: keep other people's replies in the removed nicks' threads. |
| `--clear-all` | Delete the database file and the attachments directory. |
| `--dry-run` | Show what would be removed; change nothing. |
| `-y`, `--yes` | Skip the confirmation prompt. |

These can't be combined with each other's opposite (`--remove-nick` vs `--clear-all`) or with any scraping, search, export, import or output flag.

**What `--remove-nick` deletes**, all in one SQLite transaction:
- every post authored by the nicks;
- **other people's replies in those nicks' threads** (replies whose `parent_post_id` is one of the removed root posts), because they only exist as children of the thread. Pass `--keep-replies` to keep them; they then remain in the database as orphans. The nick's own replies in other people's threads are always removed, and the rest of those threads stays;
- `attachments` rows of every removed post;
- `mentions` rows where the nick is `mentioned_nick`, plus mention rows that pointed at a removed post;
- the nick's `profiles` row.

After the commit, attachment, avatar and banner files are deleted from disk **only if no remaining `attachments` row or profile still references them**, and only if they are inside `~/.acp/ai/data/attachments/`. Nicks with nothing stored are listed as "not in database". A summary of what was removed is printed.

**What `--clear-all` deletes:** `~/.acp/ai/data/rs.database` (and any `-wal`/`-shm`/`-journal` sidecar files) and the whole `~/.acp/ai/data/attachments/` directory. **It does not touch `~/.acp/preferences.toml` or `~/.acp/ai/data/exports/`**; your saved email and previous exports stay.

**Confirmation:**
- `--remove-nick` prints the counts first, then asks `Proceed? [y/N]`.
- `--clear-all` lists the paths and sizes, then requires typing `yes`.
- Prompts go to stderr. `--yes` skips them.
- If stdin is not a terminal (a pipe, cron, CI), the command refuses to run unless `--yes` is given.
- `--dry-run` prints the same preview (and, for `--remove-nick`, the files it would delete) and exits without changing anything.

### Import (offline)

| Flag | Meaning |
|---|---|
| `--import <FILE>` | Read an earlier export back into the database. Plain `.json` or `.html` by default; add `--compress --tar-gz` for an archive. |

Whether the file is unpacked depends on the flags, not the file name. If the file is gzip but no flags were given, or the other way round, you get a clear error. HTML exports are read through the JSON embedded in them. Can't be combined with the scrape, search and output flags.

### Output

| Flag | Meaning |
|---|---|
| `--format json\|html` | Export format (default `json`). |
| `--output <PATH>` | Output file. With `--compress` it's the archive path. Defaults to stdout. |

- **Stdout:** attachment paths are the absolute paths under `~/.acp/ai/data/attachments/`.
- **File:** an `attachments/` folder is written next to the file, and paths are relative.
- **Stdout stays clean:** progress and prompts go to stderr, so you can pipe it.

### Compression

| Flag | Meaning |
|---|---|
| `--compress` | Compress the export. Needs exactly one format flag. |
| `--tar-gz` | Format: `.tar.gz`. Needs `--compress`. |

An archive contains `<name>/<name>.<ext>` and `<name>/attachments/`. It's written to `--output`, or to `~/.acp/ai/data/exports/<name>.tar.gz` by default. `--tar-gz` is the only format for now.

### Debug

| Flag | Meaning |
|---|---|
| `-q`, `--quiet` | Hide progress and summaries. Prompts and errors still show. |
| `-v`, `--verbose` | Show detail per page and per thread, and counts of skipped items. |
| `--debug` | Print every LiveView websocket message (cut at 2000 characters). Turns on verbose too. `ACP_DEBUG=1` does the same. |

`-q` can't be combined with `-v` or `--debug`.

## Workflows

```bash
# One nick, JSON to stdout (prompts for login)
acp-rs-scraper --nick alice > alice.json

# Several nicks in one session, HTML bundle with attachments
acp-rs-scraper --nicks @haz,@m3talsmith --format html --compress --tar-gz --name comrades
# -> ~/.acp/ai/data/exports/comrades.tar.gz

# Skip mentions; or scan fewer feed posts for them
acp-rs-scraper --nick alice --no-mentions
acp-rs-scraper --nick alice --crawl-limit 200

# Latest (default) vs Trending for the site-wide mentions crawl
acp-rs-scraper --nicks @haz --latest
acp-rs-scraper --nicks @haz --trending --crawl-limit 300

# Incremental re-run (default) vs full re-crawl
acp-rs-scraper --nick alice          # stops paging at known posts, skips unchanged threads
acp-rs-scraper --nick alice --full   # refetches everything

# What's in the DB?
acp-rs-scraper --list-nicks
acp-rs-scraper --list-nicks --format json

# Offline search and export
acp-rs-scraper --search "general strike" --format html --output strike.html
acp-rs-scraper --search strike --nicks alice,bob > strike.json
acp-rs-scraper --export all.json
acp-rs-scraper --export alice.html --format html --nick alice
acp-rs-scraper --export strike.tar.gz --search strike --compress --tar-gz --name strike

# Round trip into another machine / fresh DB
acp-rs-scraper --export backup.tar.gz --compress --tar-gz
acp-rs-scraper --import backup.tar.gz --compress --tar-gz
acp-rs-scraper --import all.json
acp-rs-scraper --import alice.html

# Maintenance
acp-rs-scraper --remove-nick @spam,@bot --dry-run
acp-rs-scraper --remove-nick alice                  # prompts with counts
acp-rs-scraper --remove-nick alice --keep-replies -y
acp-rs-scraper --clear-all                          # type 'yes'; prefs and exports are kept

# Logging levels
acp-rs-scraper --nick alice -q
acp-rs-scraper --nick alice -v
acp-rs-scraper --nick alice --debug 2> wire.log
```

When the mentions crawl is on, these steps happen in order:
1. Each nick's profile and feed are scraped.
2. The site feed (Latest or Trending) is crawled up to `--crawl-limit` posts.
3. Each likely post is checked by loading its whole thread.
4. Confirmed matches are stored with their threads, plus a row in `mentions`.

A summary for each nick is printed to stderr at the end.

## Database schema

```sql
-- Unchanged from the original spec. post_id is unique (index posts_post_id_uq).
CREATE TABLE posts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id TEXT NOT NULL,          -- 22-char base64url site id
  parent_post_id TEXT NOT NULL,   -- '' for thread roots; thread id for replies
  nick TEXT NOT NULL,             -- author username
  title TEXT NOT NULL,            -- '' for replies
  content TEXT NOT NULL,          -- ProseMirror body flattened to text
  posted_at TEXT NOT NULL         -- inserted_at (ISO 8601)
);

CREATE TABLE attachments (        -- unique (post_id, url)
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id TEXT NOT NULL,
  url TEXT NOT NULL,
  local_path TEXT NOT NULL,
  mime_type TEXT NOT NULL
);

CREATE TABLE mentions (           -- unique (post_id, mentioned_nick, match_type)
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id TEXT NOT NULL,
  mentioned_nick TEXT NOT NULL,
  match_type TEXT NOT NULL        -- 'mention' | 'nick' | 'display_name'
);

CREATE TABLE profiles (           -- upserted on every scrape
  nick TEXT PRIMARY KEY,
  user_id TEXT, display_name TEXT, bio TEXT,
  avatar_url TEXT, avatar_path TEXT, banner_url TEXT, banner_path TEXT,
  location TEXT, role TEXT, joined_at TEXT,
  followers INTEGER, following INTEGER, posts_count INTEGER,
  raw_json TEXT NOT NULL, scraped_at TEXT NOT NULL
);
```

How each mention type is decided:
- **`mention`:** an @mention in the post whose user id or label matches the nick.
- **`nick`:** the nick appears as a whole word, ignoring case, so `haz` does not match `hazard`.
- **`display_name`:** the profile display name appears as a whole word. Display names shorter than 3 characters are skipped.

A nick's own posts can also be recorded as mentions of that nick.

## Export format (version 2)

```json
{
  "format": "acp-rs-export",
  "format_version": 2,
  "title": "<name>",
  "nick": "@alice, @bob",
  "exported_at": "2026-10-15T04:30:00-04:00",
  "profiles": [ { "nick": "alice", "user_id": "...", "display_name": "...", "bio": "...",
                  "avatar_url": "...", "avatar_path": "...", "banner_url": null, "banner_path": null,
                  "location": null, "role": null, "joined_at": null,
                  "followers": null, "following": null, "posts_count": null, "scraped_at": "..." } ],
  "mentions": [ { "post_id": "...", "mentioned_nick": "alice", "match_type": "mention" } ],
  "posts": [ {
      "post_id": "...", "parent_post_id": "", "nick": "alice", "title": "...",
      "content": "...", "posted_at": "...",
      "attachments": [ { "url": "...", "kind": "image", "mime_type": "image/png",
                         "local_path": "attachments/abc123.png" } ],
      "children": [ /* replies, same shape */ ]
  } ]
}
```

- **HTML exports** are a single page. They show a profile header per nick, threads with nested replies, embedded media and a Mentions section. The same JSON is embedded in `<script type="application/json" id="acp-data">`, which is how HTML exports can be imported again.
- **Versions:** import accepts versions 1 (no profiles or mentions) and 2, and refuses newer versions or unknown formats.

## Troubleshooting and debugging

- **Login errors:**
  - Run with `--debug` and look at the `submit-code` reply and the sign-in POST (its field names are printed with `-v`).
  - Remove `~/.acp/preferences.toml` to enter a different email.
- **"feed paging stopped before at_end":**
  - Rerun with `--debug`.
  - Check the `page-next` replies: posts should be in `response.diff.r`, or in a rebuilt `forum/Feed` data-props.
- **Re-run finds nothing new:** that's the incremental stop at work. Use `--full`.
- **Attachment warnings:** failed downloads are skipped. Rerun later; files already on disk are reused.
- **Import errors about gzip:** add `--compress --tar-gz`, or remove them if the file is plain JSON or HTML.
- **Inspect the database directly:**

      sqlite3 ~/.acp/ai/data/rs.database 'select nick,count(*) from posts group by nick'

- **Check the output:** stdout carries only the export, so `acp-rs-scraper ... | jq .` works.

## Known limitations

- **Untested against the live site:**
  - Paging through a feed (`page-next {cursor}`).
  - Loading more thread replies through the InfiniteScroll `next-page` event (a guess).
  - Profile fields beyond author info, data-props and `og:` tags.
  - Whether media hosts require the login cookies.
- **Mentions are limited by the crawl:** there is no site-wide search in the site's JavaScript. Only `--crawl-limit` posts of the chosen feed are scanned, so older mentions, and replies inside threads whose root isn't in that window, can be missed.
- **"Posts & replies" tab:** the tab is only rendered for logged-in users and isn't in the public JavaScript. The scraper finds it at runtime from the profile HTML, by its "Posts & replies" label or replies-named attributes, and follows it as a link (`href`/`data-phx-link`) or pushes its `phx-click` event with its `phx-value-*` params. This is covered by mocked tests but not yet verified against the live site. Run `--debug` to see the tab that was found (`[profile] posts & replies tab: ...`); if none is found, the scrape continues with the Posts tab only.
- **Trending feeds:** these aren't chronological, so incremental stopping is disabled there.
- **Embeds:** YouTube, X and other embedded players aren't downloaded.
- **Logins:** each scrape run needs a fresh email code.

## Contributing

The repo ships `.githooks/pre-commit`, a credential scanner, and `build.rs` sets `git config core.hooksPath .githooks` on the first `cargo build`.

The hook blocks commits that contain:
- **Secret files:** `preferences.toml`, `*.database`, `*.db`, `.env`, keys, cookies and similar.
- **Credential patterns:** private keys, cloud and GitHub tokens, JWTs, `_acp_key=` cookies and `password=`-style assignments.

It also runs `gitleaks` if installed. Generated API docs under `docs/api/` are scanned like everything else and currently pass cleanly. Never commit your database or preferences. Bypass with `git commit --no-verify` only if you're sure.

Licensed GPL-3.0-or-later (see `LICENSE`).

## API docs

The rendered rustdoc lives in [`docs/api/`](api/) (entry: [`docs/index.html`](index.html)). Regenerate with:

    scripts/gen-docs.sh
