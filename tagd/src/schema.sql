CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE
);

-- A tag on a file. Tags can be added individually at any time by a tagger. They are only valid for mtime_at_tag = file mtime.
CREATE TABLE IF NOT EXISTS tags (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    source TEXT NOT NULL,           -- tagger that emitted this tag
    mtime_at_tag INTEGER NOT NULL,  -- file version that this tag is valid for (mtime of file from tagger)
    tagged_at INTEGER NOT NULL,
    PRIMARY KEY (file_id, key, source)
);

CREATE INDEX IF NOT EXISTS idx_tags_kv ON tags(key, value);
-- TODO: prune orphaned tags (tags with source not in active tagger registry) and stale tags (mtime_at_tag < current file mtime) past a certain threshold
