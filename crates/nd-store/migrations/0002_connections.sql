CREATE TABLE connections (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    group_name TEXT NOT NULL DEFAULT '',
    environment TEXT NOT NULL CHECK (environment IN ('local', 'development', 'staging', 'production')),
    color TEXT,
    target TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE history (
    id INTEGER PRIMARY KEY,
    connection_id TEXT NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    sql TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    elapsed_ms INTEGER NOT NULL,
    rows INTEGER,
    error TEXT
) STRICT;

CREATE INDEX history_by_connection ON history (connection_id, started_at DESC);
