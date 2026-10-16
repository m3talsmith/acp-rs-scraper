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
            body_state TEXT NOT NULL DEFAULT 'unknown'
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
