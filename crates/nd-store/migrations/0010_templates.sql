ALTER TABLE connections ADD COLUMN template TEXT NOT NULL DEFAULT '{"kind":"none"}';
ALTER TABLE connections ADD COLUMN raw_fields TEXT NOT NULL DEFAULT '{}';
