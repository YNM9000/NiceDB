CREATE TABLE audit (
    id INTEGER PRIMARY KEY,
    connection_id TEXT NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    at INTEGER NOT NULL,
    who TEXT NOT NULL,
    sql TEXT NOT NULL,
    rows INTEGER,
    error TEXT
) STRICT;

CREATE INDEX audit_by_connection ON audit (connection_id, at DESC);
