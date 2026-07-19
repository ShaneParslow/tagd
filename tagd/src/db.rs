use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use tagd_core::tagger::TaggerResponse;

/// A connection to the tag database.
///
/// SQLite handles locking, so just call Db::open wherever a connection is needed.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Opens a new connection to the tag database.
    ///
    /// SQLite handles locking, so each thread should hold its own `Db` rather
    /// than sharing one behind a mutex.
    pub fn open() -> Result<Db> {
        let conn = Connection::open(db_path()).context("Failed to open connection to database")?;

        // journal_mode persists in the file header; foreign_keys and busy_timeout are
        // per-connection and must be reapplied on every new connection.
        conn.pragma_update(None, "journal_mode", "WAL")
            .context("Failed to set journal_mode=WAL on db")?;

        conn.pragma_update(None, "foreign_keys", true)
            .context("Failed to set foreign_keys=true on db")?;

        conn.busy_timeout(Duration::from_secs(5))
            .context("Failed to set busy_timeout on db")?;

        conn.execute_batch(include_str!("schema.sql"))
            .context("Failed to execute schema.sql")?;

        Ok(Db { conn })
    }

    /// Saves tags for a single file to the tag database.
    pub fn set_tags(&self, path: &str, response: &TaggerResponse) -> Result<()> {
        let file_id: i64 = self
            .conn
            .query_row(
                "INSERT INTO files (path) VALUES (?1)
             ON CONFLICT(path) DO UPDATE SET path=excluded.path
             RETURNING id",
                params![path],
                |row| row.get(0),
            )
            .context("Failed to get file id from db")?;

        let mut stmt = self
            .conn
            .prepare_cached(
                "INSERT INTO tags (file_id, source_tagger, key, value, mtime_at_tag)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(file_id, key, source_tagger) DO UPDATE SET
               value=excluded.value,
               mtime_at_tag=excluded.mtime_at_tag",
            )
            .context("Failed to prepare tag insert statement")?;
        for (k, v) in &response.tags {
            stmt.execute(params![
                file_id,
                response.tagger,
                k,
                v,
                response.mtime_at_tag
            ])
            .context("Failed to execute tag insert statement")?;
        }

        Ok(())
    }

    /// Returns paths of files matching tag key/value, qualified by source tagger.
    ///
    /// The mtime of the file when this specific tag was applied is returned alongside its path.
    pub fn query_files_by_qualified_tag(
        &self,
        tagger: &str,
        key: &str,
        value: &str,
    ) -> Result<Vec<(String, i64)>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT f.path, t.mtime_at_tag FROM files f JOIN tags t ON t.file_id = f.id
             WHERE t.source_tagger = ?1 AND t.key = ?2 AND t.value = ?3",
            )
            .context("Failed to prepare select statement")?;

        let rows = stmt
            .query_map(params![tagger, key, value], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .context("Failed to execute select statement")?
            .collect::<rusqlite::Result<Vec<(String, i64)>>>()
            .context("Failed to collect paths")?;

        Ok(rows)
    }
}

fn db_path() -> PathBuf {
    if let Ok(path) = std::env::var("TAGD_DB_PATH") {
        return PathBuf::from(path);
    }

    #[cfg(debug_assertions)]
    {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("target/debug/tags.db");
        return path;
    }

    #[cfg(not(debug_assertions))]
    {
        PathBuf::from("/var/lib/tagd/tags.db")
    }
}
