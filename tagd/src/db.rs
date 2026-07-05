use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use rusqlite::{Connection, params};

use tagd_core::tagger::TaggerResponse;

/// A connection to the tag database.
/// 
/// SQLite handles locking, so just call Db::open wherever a connection is needed.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Opens a new connection to the tagd database.
    /// 
    /// SQLite handles locking, so each thread should hold its own `Db` rather
    /// than sharing one behind a mutex.
    pub fn open() -> Result<Db> {
        let conn = Connection::open(db_path())?;
        
        // journal_mode persists in the file header; foreign_keys and busy_timeout are
        // per-connection and must be reapplied on every new connection.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        
        conn.execute_batch(include_str!("schema.sql"))?;
        
        Ok(Db { conn })
    }

    /// Saves tags for a single file to the tag database.
    pub fn set_tags(&self, path: &str, response: &TaggerResponse) -> Result<()> {
        let file_id: i64 = self.conn.query_row(
            "INSERT INTO files (path) VALUES (?1)
             ON CONFLICT(path) DO UPDATE SET path=excluded.path
             RETURNING id",
            params![path],
            |row| row.get(0),
        )?;

        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO tags (file_id, key, value, source_tagger, mtime_at_tag)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(file_id, key, source_tagger) DO UPDATE SET
               value=excluded.value,
               mtime_at_tag=excluded.mtime_at_tag",
        )?;
        for (k, v) in &response.tags {
            stmt.execute(params![file_id, k, v, response.tagger, response.mtime_at_tag])?;
        }
        
        Ok(())
    }

    /// Returns paths of files matching (source tagger, key, value).
    /// 
    /// The mtime of the file when this specific tag was applied is returned alongside its path.
    pub fn query_files_by_qualified_tag(
        &self,
        tagger: &str,
        key: &str,
        value: &str,
    ) -> Result<Vec<(String, i64)>> {
        // (file_id, key, source) is the PK, so at most one row per file matches;
        // no dedup needed. Returns each file's path and the mtime the tag is valid for.
        let mut stmt = self.conn.prepare_cached(
            "SELECT f.path, t.mtime_at_tag FROM files f JOIN tags t ON t.file_id = f.id
             WHERE t.source_tagger = ?1 AND t.key = ?2 AND t.value = ?3",
        )?;
        let rows = stmt
            .query_map(params![tagger, key, value], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<rusqlite::Result<Vec<(String, i64)>>>()?;
        
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
            .parent().unwrap()
            .join("target/debug/tags.db");
        return path;
    }

    #[cfg(not(debug_assertions))]
    {
        PathBuf::from("/var/lib/tagd/tags.db")
    }
}