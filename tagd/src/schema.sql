CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE
);

-- A tag on a file. Tags can be added individually at any time by a tagger.
-- They are only valid for mtime_at_tag = file mtime.
--
-- Stale tags (file mtime > mtime_at_tag) are not immediately removed since they may
-- still be useful in some capacity. If file mtime >> mtime_at_tag they may be removed (todo).
-- Maybe also if they are orphaned (tagger providing key is no longer loaded) and old?
-- Would require tagged_at, which is probably good to store regardless.
-- Could also just have orphans pruned manually from the socket.
--
-- Different taggers can provide the same key.
-- They are disambiguated by qualifying with the source tagger if necessary.
-- (ex. Db::query_files_by_qualified_tag).
CREATE TABLE IF NOT EXISTS tags (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    source_tagger TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    mtime_at_tag INTEGER NOT NULL,
    PRIMARY KEY (file_id, source_tagger, key)
);

CREATE INDEX IF NOT EXISTS idx_tags_kv ON tags(key, value);
