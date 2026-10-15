//! SQL schema for every table, kept in one place.

/// `posts` table (the original spec schema) plus the unique `post_id` index.
pub const POSTS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id TEXT NOT NULL,
            parent_post_id TEXT NOT NULL,
            nick TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            posted_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS posts_post_id_uq ON posts(post_id);";

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
