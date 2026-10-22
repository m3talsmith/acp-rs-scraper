//! SQL schema for every table, kept in one place.

/// `posts` table (the original spec schema) plus the unique `post_id` index.
///
/// Reply relationships:
/// - `parent_post_id`: the thread root (`""` for root posts). Unchanged since 1.0.
/// - `in_reply_to_id`: the immediate parent, i.e. the comment a reply answers
///   (its first `postRef`), else the thread root; `""` for root posts.
///
/// Older databases gain `in_reply_to_id` through [`POSTS_MIGRATIONS`].
pub const POSTS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL,
            parent_post_id TEXT NOT NULL,
            in_reply_to_id TEXT NOT NULL DEFAULT '',
            nick TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            posted_at TEXT NOT NULL,
            body_state TEXT NOT NULL DEFAULT 'unknown',
            body_json TEXT NOT NULL DEFAULT '',
            score INTEGER
        );
        CREATE UNIQUE INDEX IF NOT EXISTS posts_post_id_uq ON posts(post_id);";

/// Run after [`POSTS_SCHEMA`] when `posts` lacks `in_reply_to_id`: add the
/// column and point existing replies at their thread root (the best known parent).
pub const POSTS_ADD_IN_REPLY_TO: &str = "ALTER TABLE posts ADD COLUMN in_reply_to_id TEXT NOT NULL DEFAULT '';
        UPDATE posts SET in_reply_to_id = parent_post_id WHERE in_reply_to_id = '' AND parent_post_id <> '';";

/// Run when `posts` lacks `body_state` (databases before 1.26.0). Values:
/// `full` (complete body), `truncated` (only the feed's shortened preview
/// was available) and `unknown` (stored before 1.26.0, could be either;
/// `--refetch-truncated` checks these against their thread page).
pub const POSTS_ADD_BODY_STATE: &str = "ALTER TABLE posts ADD COLUMN body_state TEXT NOT NULL DEFAULT 'unknown';";

/// Run when `posts` lacks `body_json` (databases before 2.21.0). The ProseMirror
/// body, compact JSON; `""` until a thread page or a feed item supplies it.
pub const POSTS_ADD_BODY_JSON: &str = "ALTER TABLE posts ADD COLUMN body_json TEXT NOT NULL DEFAULT '';";

/// Run when `posts` lacks `score` (databases before 2.22.0). NULL until a thread page sends one.
pub const POSTS_ADD_SCORE: &str = "ALTER TABLE posts ADD COLUMN score INTEGER;";

/// Tags on a post (`#strike`). Filled from mention nodes and from `#word` in the
/// body. Created on first use. `tag` is stored without the `#`.
pub const POST_TAGS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS post_tags (
            post_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (post_id, tag)
        );
        CREATE INDEX IF NOT EXISTS post_tags_tag_idx ON post_tags(tag);";

/// Indexes on the relationship columns (safe to re-run).
pub const POSTS_MIGRATIONS: &str = "CREATE INDEX IF NOT EXISTS posts_parent_idx ON posts(parent_post_id);
        CREATE INDEX IF NOT EXISTS posts_in_reply_to_idx ON posts(in_reply_to_id);
        CREATE INDEX IF NOT EXISTS posts_body_state_idx ON posts(body_state);";

/// `profiles` table: one row per scraped nick, upserted every scrape.
pub const PROFILES_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS profiles (
            nick TEXT PRIMARY KEY,
            user_id TEXT,
            display_name TEXT,
            bio TEXT,
            avatar_url TEXT,
            avatar_path TEXT,
            banner_url TEXT,
            banner_path TEXT,
            location TEXT,
            role TEXT,
            joined_at TEXT,
            followers INTEGER,
            following INTEGER,
            posts_count INTEGER,
            raw_json TEXT NOT NULL,
            scraped_at TEXT NOT NULL
        );";

/// `mentions` table: which posts mention which nick, and how.
pub const MENTIONS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS mentions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL,
            mentioned_nick TEXT NOT NULL,
            match_type TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS mentions_uq ON mentions(post_id, mentioned_nick, match_type);";

/// `attachments` table: downloaded media per post, unique per (post_id, url).
/// `on_site` is 1 when that file is part of the Red Star copy. Scraped files
/// came from the site, so the default is 1. A local publish can set it to 0.
pub const ATTACHMENTS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL,
            url TEXT NOT NULL,
            local_path TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            on_site INTEGER NOT NULL DEFAULT 1
        );
        CREATE UNIQUE INDEX IF NOT EXISTS attachments_post_url_uq ON attachments(post_id, url);";

/// Older `attachments` tables gain `on_site` (default 1: the file came from the site).
pub const ATTACHMENTS_ADD_ON_SITE: &str = "ALTER TABLE attachments ADD COLUMN on_site INTEGER NOT NULL DEFAULT 1;";

/// `bookmarks` table (2.1.0). One row per bookmarked post:
/// - `remote`: the site has it bookmarked too (seen in its bookmarks feed, or pushed).
/// - `deleted`: removed locally but still bookmarked on the site; the next
///   `--sync-bookmarks` removes it there and drops the row.
///
/// Rows are added by `--bookmark` / the TUI (local, `remote = 0`) and by
/// `--sync-bookmarks` (from the site, `remote = 1`). Created on first use.
pub const BOOKMARKS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS bookmarks (
            post_id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            remote INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0
        );";

/// One row of a post's edit history (2.24.0). `source` is `site` for versions
/// read from `/rs/post/{thread}/{id}/edits`. `peer`, `local` and `draft` are
/// reserved for later local-first drafts (§7.14.8) so that table is not rebuilt.
/// `version_hash` is a stable hash of `body_json` and `edited_at`.
pub const POST_VERSIONS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS post_versions (
            post_id TEXT NOT NULL,
            version_hash TEXT NOT NULL,
            body_json TEXT NOT NULL,
            edited_at TEXT NOT NULL,
            observed_at TEXT NOT NULL,
            source TEXT NOT NULL CHECK (source IN ('site', 'peer', 'local', 'draft')),
            PRIMARY KEY (post_id, version_hash)
        );
        CREATE INDEX IF NOT EXISTS post_versions_post_idx ON post_versions(post_id, edited_at);";

/// A post written on this machine before it is sent (§7.14.8).
/// `state` is `draft`, `local`, `queued`, or `published`.
pub const LOCAL_POSTS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS local_posts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            thread_id TEXT NOT NULL DEFAULT '',
            state TEXT NOT NULL CHECK (state IN ('draft','local','queued','published')),
            site_post_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );";

/// Offline send queue (§2.2). `payload` is JSON. `sending` is left in place
/// if the process stops, so the next send reads back before it tries again.
pub const OUTBOX_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS outbox (
            id TEXT PRIMARY KEY,
            action TEXT NOT NULL,
            payload TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('draft','queued','sending','sent','failed','cancelled')),
            created_at TEXT NOT NULL,
            confirmed_at TEXT,
            attempts INTEGER NOT NULL DEFAULT 0,
            last_error TEXT NOT NULL DEFAULT '',
            result_post_id TEXT
        );";

/// A named site read (Top, and later the other read-parity feeds), in the
/// order the site returned. `post_ids` is a JSON array of post ids. The posts
/// themselves live in `posts`, so the view still opens offline. Created on first use.
pub const SITE_FEEDS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS site_feeds (
            key TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            post_ids TEXT NOT NULL
        );";

/// Research projects and their items (see `research`). Items are unique per
/// project, kind and ref; deleting a project deletes its items.
pub const RESEARCH_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS research_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE,
            description TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS research_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES research_projects(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK (kind IN ('post', 'reply', 'search-result', 'local-file', 'web-url')),
            ref TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            meta TEXT NOT NULL DEFAULT '{}',
            content TEXT NOT NULL DEFAULT '',
            local_path TEXT NOT NULL DEFAULT '',
            added_at TEXT NOT NULL,
            UNIQUE (project_id, kind, ref)
        );
        CREATE INDEX IF NOT EXISTS research_items_project_idx ON research_items(project_id);";

/// Wrapped account data keys, one row per epoch. The raw key is not stored.
/// `issuer` is `local` until an authorizer exists.
pub const KEY_EPOCHS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS key_epochs (
            epoch INTEGER PRIMARY KEY,
            created_at TEXT NOT NULL,
            reason TEXT NOT NULL,
            issuer TEXT NOT NULL,
            wraps BLOB NOT NULL
        );";

/// Certificate rows. Nothing writes these until enrollment exists.
pub const IDENTITY_CERTS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS identity_certs (
            serial TEXT PRIMARY KEY,
            subject TEXT NOT NULL,
            body BLOB NOT NULL
        );";

/// Revocation rows. Nothing writes these until revocation exists.
pub const REVOCATIONS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS revocations (
            seq INTEGER PRIMARY KEY,
            body BLOB NOT NULL
        );";

/// Quarantined records. Nothing writes these until revocation exists.
pub const QUARANTINE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS quarantine (
            id INTEGER PRIMARY KEY,
            observed_at TEXT NOT NULL,
            body BLOB NOT NULL
        );";

/// Verified site bundle and the last protocol check (3.4.0). The writes switch
/// lives in preferences, not here. Nothing stores a bundle until a capture
/// confirms one.
pub const SITE_STATE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS site_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );";

/// A post deleted through the loaded service. The `posts` row stays.
pub const TOMBSTONES_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS post_tombstones (
            post_id TEXT PRIMARY KEY,
            deleted_at TEXT NOT NULL
        );";

/// Site notification pane. `raw` stays empty. The service keeps the socket.
pub const SITE_NOTIFICATIONS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS site_notifications (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL DEFAULT '',
            actor TEXT NOT NULL DEFAULT '',
            post_id TEXT NOT NULL DEFAULT '',
            thread_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT '',
            read INTEGER NOT NULL DEFAULT 0,
            body TEXT NOT NULL DEFAULT '',
            raw TEXT NOT NULL DEFAULT ''
        );";

/// One space. A LiveKit token is not a column. Joining a room is not built.
pub const SPACES_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS spaces (
            slug TEXT PRIMARY KEY,
            post_id TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            state TEXT NOT NULL DEFAULT '',
            live INTEGER NOT NULL DEFAULT 0,
            scheduled_at TEXT NOT NULL DEFAULT '',
            ended_at TEXT NOT NULL DEFAULT '',
            recording_url TEXT NOT NULL DEFAULT '',
            fetched_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS spaces_post_idx ON spaces(post_id);";

/// Rules and the log of what they did (3.32.0). A system row is read-only.
/// `trigger`, `conditions`, and `actions` are canonical JSON. Lower `priority` runs first.
pub const RULES_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            origin TEXT NOT NULL CHECK (origin IN ('system','user')),
            enabled INTEGER NOT NULL DEFAULT 1,
            priority INTEGER NOT NULL DEFAULT 100,
            system_key TEXT,
            cloned_from TEXT,
            template_key TEXT,
            trigger TEXT NOT NULL,
            conditions TEXT NOT NULL,
            actions TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS rules_system_key ON rules(system_key) WHERE system_key IS NOT NULL;
        CREATE TABLE IF NOT EXISTS rule_firings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rule_id INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
            post_id TEXT,
            source TEXT NOT NULL,
            action TEXT NOT NULL,
            outcome TEXT NOT NULL,
            detail TEXT NOT NULL DEFAULT '',
            fired_at TEXT NOT NULL
        );";

/// Whole-site crawl frontier (3.34.0). One row per URL. `status` is `pending`,
/// `done`, or `error`. `wait_until` is a unix millisecond; a retry waits until then.
/// `chronological` feeds stop early once a page is already known. Ranked feeds do not.
pub const CRAWL_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS crawl_frontier (
            url TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            chronological INTEGER NOT NULL DEFAULT 0,
            priority INTEGER NOT NULL DEFAULT 2,
            status TEXT NOT NULL,
            attempts INTEGER NOT NULL DEFAULT 0,
            wait_until INTEGER NOT NULL DEFAULT 0,
            last_error TEXT,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS crawl_recent (
            url TEXT PRIMARY KEY
        );
        CREATE TABLE IF NOT EXISTS crawl_checkpoint (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            running INTEGER NOT NULL DEFAULT 0,
            finished_at TEXT,
            posts_saved INTEGER NOT NULL DEFAULT 0,
            pages INTEGER NOT NULL DEFAULT 0,
            crawl_delay_ms INTEGER NOT NULL DEFAULT 0,
            disallow TEXT NOT NULL DEFAULT ''
        );";

/// Backup bookkeeping inside the database. The archive files themselves live outside it.
pub const BACKUPS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS backups (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL,
            created_at TEXT NOT NULL
        );";
