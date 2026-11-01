CREATE TABLE folders (
    id TEXT PRIMARY KEY NOT NULL,
    parent_id TEXT REFERENCES folders(id),
    name TEXT NOT NULL,
    tint TEXT,
    mark TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    collapsed INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX folders_by_parent ON folders (parent_id, position);

ALTER TABLE connections ADD COLUMN folder_id TEXT REFERENCES folders(id);

INSERT INTO folders (id, name, position)
SELECT lower(hex(randomblob(16))), group_name, row_number() OVER (ORDER BY group_name COLLATE NOCASE)
FROM (SELECT DISTINCT group_name FROM connections WHERE group_name <> '');

UPDATE connections
SET folder_id = (SELECT f.id FROM folders f WHERE f.parent_id IS NULL AND f.name = connections.group_name)
WHERE group_name <> '';
