use std::path::PathBuf;

use rusqlite::{Connection, params};

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn init() -> rusqlite::Result<Db> {
        let conn = Connection::open(db_path())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Db { conn })
    }

    /// Upsert file record, returns file_id
    pub fn upsert_file(&self, path: &str, mtime: i64) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO files (path, mtime_at_tag, tagged_at)
             VALUES (?1, ?2, strftime('%s','now'))
             ON CONFLICT(path) DO UPDATE SET mtime_at_tag=?2, tagged_at=strftime('%s','now')",
            params![path, mtime],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Store tags from a single tagger run
    pub fn set_tags(&self, file_id: i64, source: &str, tags: &[(String, String)]) -> rusqlite::Result<()> {
        // Clear old tags from this source for this file
        self.conn.execute(
            "DELETE FROM tags WHERE file_id = ?1 AND source = ?2",
            params![file_id, source],
        )?;
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO tags (file_id, key, value, source) VALUES (?1, ?2, ?3, ?4)"
        )?;
        for (k, v) in tags {
            stmt.execute(params![file_id, k, v, source])?;
        }
        Ok(())
    }
}

// Get path to tag database
fn db_path() -> PathBuf {
    // Runtime env override
    if let Ok(path) = std::env::var("TAGD_DB_PATH") {
        return PathBuf::from(path);
    }

    // Debug build default search path
    #[cfg(debug_assertions)]
    {
        // All workspace binaries end up here
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()  // workspace root
            .join("target/debug/tags.db");
        return path;
    }

    // Release build default search path
    #[cfg(not(debug_assertions))]
    {
        PathBuf::from("/var/lib/tagd/tags.db")
    }
}