ALTER TABLE device_codes ADD COLUMN client TEXT NOT NULL DEFAULT '';
ALTER TABLE device_codes ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sync_heads ADD COLUMN key_id TEXT;
ALTER TABLE sync_heads ADD COLUMN rotate INTEGER NOT NULL DEFAULT 0;
UPDATE tokens SET scopes = 'upload sync devices' WHERE device_id IS NOT NULL;
