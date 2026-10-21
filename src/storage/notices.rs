//! Saved notifications in the encrypted database.
//!
//! A row is one notice. There is no cap. Clearing a row deletes it.

use crate::prelude::*;
use rusqlite::{params, Connection};

/// `notices` in the same database as posts and search history.
pub const NOTICES_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS notices (
            id INTEGER PRIMARY KEY,
            created_at TEXT NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL
        );";

/// One stored notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoticeRow {
    /// Database id. `0` before insert.
    pub id: i64,
    /// When it was raised, RFC 3339.
    pub created_at: String,
    /// `error`, `warning`, `sync`, or `job`.
    pub kind: String,
    /// Short heading.
    pub title: String,
    /// The rest of the text.
    pub body: String,
}

/// Create the table when it is missing.
pub fn ensure(c: &Connection) -> Result<()> {
    c.execute_batch(NOTICES_SCHEMA)?;
    Ok(())
}

/// Insert `row` and return its id. `row.id` is ignored.
pub fn insert(c: &Connection, row: &NoticeRow) -> Result<i64> {
    ensure(c)?;
    c.execute(
        "INSERT INTO notices (created_at, kind, title, body) VALUES (?1, ?2, ?3, ?4)",
        params![row.created_at, row.kind, row.title, row.body],
    )?;
    Ok(c.last_insert_rowid())
}

/// Newest first. An absent table is an empty list.
pub fn list(c: &Connection) -> Result<Vec<NoticeRow>> {
    if !crate::maintenance::table_exists(c, "notices")? { return Ok(Vec::new()); }
    let mut st = c.prepare(
        "SELECT id, created_at, kind, title, body FROM notices ORDER BY id DESC")?;
    let rows = st.query_map([], |r| {
        Ok(NoticeRow {
            id: r.get(0)?,
            created_at: r.get(1)?,
            kind: r.get(2)?,
            title: r.get(3)?,
            body: r.get(4)?,
        })
    })?;
    rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
}

/// Delete one row. A missing id deletes nothing.
pub fn delete(c: &Connection, id: i64) -> Result<()> {
    if !crate::maintenance::table_exists(c, "notices")? { return Ok(()); }
    c.execute("DELETE FROM notices WHERE id = ?1", params![id])?;
    Ok(())
}

/// Delete every saved notification.
pub fn clear(c: &Connection) -> Result<()> {
    if !crate::maintenance::table_exists(c, "notices")? { return Ok(()); }
    c.execute("DELETE FROM notices", [])?;
    Ok(())
}
