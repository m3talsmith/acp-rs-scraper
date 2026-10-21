//! Local SQLite storage at `~/.acp/ai/data/rs.database`.

pub mod db;
pub mod history;
pub mod notices;
pub mod schema;

#[cfg(test)]
mod tests;
