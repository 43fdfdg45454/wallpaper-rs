CREATE TABLE sync_blobs (
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  id TEXT NOT NULL,
  size INTEGER NOT NULL,
  received INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, id)
) STRICT, WITHOUT ROWID;

CREATE VIRTUAL TABLE search USING fts5(package, title, description, tags, tokenize = 'trigram remove_diacritics 1');
INSERT INTO search (rowid, package, title, description, tags)
  SELECT id, package_id, title, description, tags FROM versions WHERE state = 'published';

CREATE TRIGGER search_added AFTER INSERT ON versions WHEN new.state = 'published' BEGIN
  INSERT INTO search (rowid, package, title, description, tags)
    VALUES (new.id, new.package_id, new.title, new.description, new.tags);
END;

CREATE TRIGGER search_changed AFTER UPDATE ON versions BEGIN
  DELETE FROM search WHERE rowid = old.id;
  INSERT INTO search (rowid, package, title, description, tags)
    SELECT new.id, new.package_id, new.title, new.description, new.tags WHERE new.state = 'published';
END;

CREATE TRIGGER search_removed AFTER DELETE ON versions BEGIN
  DELETE FROM search WHERE rowid = old.id;
END;
