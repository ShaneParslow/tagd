use std::path::PathBuf;

use anyhow::Result;
use rusqlite::{Connection, params};
use tagd_core::TaggerResponse;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn new() -> Result<Db> {
        let conn = Connection::open(db_path())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Db { conn })
    }

    pub fn set_tags(&self, path: &str, response: &TaggerResponse) -> Result<()> {
        let file_id: i64 = self.conn.query_row(
            "INSERT INTO files (path) VALUES (?1)
             ON CONFLICT(path) DO UPDATE SET path=excluded.path
             RETURNING id",
            params![path],
            |row| row.get(0),
        )?;

        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO tags (file_id, key, value, source, mtime_at_tag, tagged_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%s','now'))
             ON CONFLICT(file_id, key, source) DO UPDATE SET
               value=excluded.value,
               mtime_at_tag=excluded.mtime_at_tag,
               tagged_at=excluded.tagged_at",
        )?;
        for (k, v) in &response.tags {
            stmt.execute(params![file_id, k, v, response.tagger, response.mtime_at_tag])?;
        }
        Ok(())
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
