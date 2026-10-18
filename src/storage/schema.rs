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
pub const ATTACHMENTS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL,
            url TEXT NOT NULL,
            local_path TEXT NOT NULL,
            mime_type TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS attachments_post_url_uq ON attachments(post_id, url);";

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
