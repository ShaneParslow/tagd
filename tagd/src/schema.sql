CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    mtime_at_tag INTEGER, -- epoch seconds
    tagged_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tags (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    source TEXT NOT NULL, -- which tagger produced this
    PRIMARY KEY (file_id, key, source)
);

CREATE INDEX IF NOT EXISTS idx_tags_kv ON tags(key, value);
-- Explicit: path is a critical lookup key
-- technically redundant since UNIQUE makes index in sqlite
CREATE INDEX IF NOT EXISTS idx_files_path ON files(path); 
