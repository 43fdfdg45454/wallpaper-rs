ALTER TABLE packages ADD COLUMN hidden TEXT;
ALTER TABLE packages ADD COLUMN hidden_at INTEGER;
UPDATE tokens SET scopes = 'upload sync' WHERE device_id IS NOT NULL;
ALTER TABLE sync_heads ADD COLUMN format INTEGER NOT NULL DEFAULT 0;
UPDATE tokens SET scopes = TRIM(REPLACE(' ' || scopes || ' ', ' devices ', ' ')) WHERE device_id IS NULL;
DELETE FROM tokens WHERE scopes = '';
