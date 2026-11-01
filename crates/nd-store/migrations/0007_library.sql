CREATE TABLE scripts (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    folder TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]',
    connection_id TEXT REFERENCES connections(id) ON DELETE SET NULL,
    sql TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE INDEX scripts_by_folder ON scripts (folder, name COLLATE NOCASE);

CREATE TABLE snippets (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    trigger TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL,
    dialect TEXT,
    position INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE notes (
    scope TEXT NOT NULL CHECK (scope IN ('connection', 'workspace')),
    scope_id TEXT NOT NULL,
    body TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (scope, scope_id)
) STRICT;
