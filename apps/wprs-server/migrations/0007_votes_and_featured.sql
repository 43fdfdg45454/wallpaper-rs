CREATE TABLE votes (
  package_id TEXT NOT NULL REFERENCES packages ON DELETE CASCADE,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  value INTEGER NOT NULL CHECK (value IN (-1, 1)),
  at INTEGER NOT NULL,
  PRIMARY KEY (package_id, user_id)
) STRICT, WITHOUT ROWID;
CREATE INDEX votes_user ON votes(user_id, at);
CREATE TABLE vote_changes (
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  at INTEGER NOT NULL
) STRICT;
CREATE INDEX vote_changes_user ON vote_changes(user_id, at);
CREATE TABLE featured (
  package_id TEXT PRIMARY KEY REFERENCES packages ON DELETE CASCADE,
  note TEXT,
  at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
