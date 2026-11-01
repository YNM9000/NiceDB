ALTER TABLE scripts ADD COLUMN params TEXT NOT NULL DEFAULT '[]';

CREATE TABLE param_values (
    query TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    value TEXT NOT NULL,
    used_at INTEGER NOT NULL,
    PRIMARY KEY (query, name, kind, value)
) STRICT;

CREATE INDEX param_values_by_name ON param_values (name, used_at);
