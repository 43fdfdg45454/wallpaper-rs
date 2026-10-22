CREATE TABLE sources (
  id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL DEFAULT 'folder' CHECK (kind IN ('folder', 'provider')),
  uri TEXT NOT NULL UNIQUE, recursive INTEGER NOT NULL DEFAULT 1, scanned_at INTEGER, error TEXT);

CREATE TABLE items (
  id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE,
  source_id INTEGER REFERENCES sources(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('image', 'video', 'package')), path TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL, size INTEGER NOT NULL, mtime INTEGER NOT NULL, qhash BLOB NOT NULL,
  width INTEGER, height INTEGER, duration_ms INTEGER, audio INTEGER, color INTEGER, error TEXT,
  pkg_id TEXT, pkg_version TEXT, pkg_type TEXT, license TEXT,
  rating INTEGER NOT NULL DEFAULT 0 CHECK (rating BETWEEN 0 AND 5), favorite INTEGER NOT NULL DEFAULT 0,
  added_at INTEGER NOT NULL, used_at INTEGER, use_count INTEGER NOT NULL DEFAULT 0,
  missing INTEGER NOT NULL DEFAULT 0);
CREATE INDEX items_kind ON items(kind, added_at);
CREATE INDEX items_qhash ON items(qhash);
CREATE INDEX items_source ON items(source_id);

CREATE TABLE tags (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE);
CREATE TABLE item_tags (
  item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (item_id, tag_id)) WITHOUT ROWID;
CREATE INDEX item_tags_tag ON item_tags(tag_id);

CREATE VIRTUAL TABLE items_fts USING fts5(title, tags, path, content='', contentless_delete=1,
  tokenize='unicode61 remove_diacritics 2');
CREATE VIEW items_text AS
  SELECT i.id, i.title, coalesce((SELECT group_concat(t.name, ' ') FROM item_tags it
    JOIN tags t ON t.id = it.tag_id WHERE it.item_id = i.id), '') AS tags, i.path FROM items i;
CREATE TRIGGER items_inserted AFTER INSERT ON items BEGIN
  INSERT INTO items_fts(rowid, title, tags, path) SELECT * FROM items_text WHERE id = new.id;
END;
CREATE TRIGGER items_deleted AFTER DELETE ON items BEGIN
  DELETE FROM items_fts WHERE rowid = old.id;
END;
CREATE TRIGGER items_renamed AFTER UPDATE OF title, path ON items BEGIN
  DELETE FROM items_fts WHERE rowid = new.id;
  INSERT INTO items_fts(rowid, title, tags, path) SELECT * FROM items_text WHERE id = new.id;
END;
CREATE TRIGGER item_tags_inserted AFTER INSERT ON item_tags BEGIN
  DELETE FROM items_fts WHERE rowid = new.item_id;
  INSERT INTO items_fts(rowid, title, tags, path) SELECT * FROM items_text WHERE id = new.item_id;
END;
CREATE TRIGGER item_tags_deleted AFTER DELETE ON item_tags BEGIN
  DELETE FROM items_fts WHERE rowid = old.item_id;
  INSERT INTO items_fts(rowid, title, tags, path) SELECT * FROM items_text WHERE id = old.item_id;
END;

CREATE TABLE playlists (
  id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  query TEXT, rules TEXT NOT NULL DEFAULT '{}');
CREATE TABLE playlist_items (
  playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE, pos INTEGER NOT NULL,
  item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  PRIMARY KEY (playlist_id, pos)) WITHOUT ROWID;
CREATE INDEX playlist_items_item ON playlist_items(item_id);

CREATE TABLE presets (
  id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE,
  item_id INTEGER REFERENCES items(id) ON DELETE CASCADE, name TEXT NOT NULL COLLATE NOCASE,
  value TEXT NOT NULL);
CREATE UNIQUE INDEX presets_name ON presets(coalesce(item_id, 0), name);

CREATE TABLE profiles (
  id INTEGER PRIMARY KEY, uid TEXT NOT NULL UNIQUE, name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  outputs TEXT NOT NULL, options TEXT NOT NULL DEFAULT '{}', auto TEXT NOT NULL DEFAULT '{}',
  priority INTEGER NOT NULL DEFAULT 0);

CREATE TABLE rotation (
  output TEXT PRIMARY KEY, playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
  state TEXT NOT NULL, next_at INTEGER) WITHOUT ROWID;

CREATE TABLE history (
  at INTEGER NOT NULL, output TEXT NOT NULL, item_id INTEGER REFERENCES items(id) ON DELETE SET NULL);
CREATE INDEX history_output ON history(output);
