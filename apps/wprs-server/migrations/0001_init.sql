CREATE TABLE users (
  id INTEGER PRIMARY KEY,
  handle TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash TEXT,
  role TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'trusted', 'moderator', 'admin')),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended')),
  quota_bytes INTEGER NOT NULL,
  created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE identities (
  issuer TEXT NOT NULL,
  subject TEXT NOT NULL,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  PRIMARY KEY (issuer, subject)
) STRICT, WITHOUT ROWID;

CREATE TABLE invites (
  id INTEGER PRIMARY KEY,
  code_hash BLOB NOT NULL UNIQUE,
  role TEXT NOT NULL CHECK (role IN ('user', 'trusted', 'moderator', 'admin')),
  uses_left INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  created_by INTEGER REFERENCES users ON DELETE SET NULL
) STRICT;

CREATE TABLE setup_tokens (hash BLOB PRIMARY KEY, expires_at INTEGER NOT NULL) STRICT, WITHOUT ROWID;

CREATE TABLE sessions (
  id_hash BLOB PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  seen_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
CREATE INDEX sessions_user ON sessions(user_id);

CREATE TABLE devices (
  id TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  number INTEGER NOT NULL,
  platform TEXT NOT NULL CHECK (platform IN ('linux', 'android', 'other')),
  enc_key BLOB,
  sig_key BLOB,
  created_at INTEGER NOT NULL,
  seen_at INTEGER,
  UNIQUE (user_id, number)
) STRICT;

CREATE TABLE tokens (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  device_id TEXT REFERENCES devices ON DELETE CASCADE,
  name TEXT NOT NULL,
  hash BLOB NOT NULL UNIQUE,
  scopes TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  used_at INTEGER,
  expires_at INTEGER
) STRICT;
CREATE INDEX tokens_user ON tokens(user_id);

CREATE TABLE device_codes (
  code_hash BLOB PRIMARY KEY,
  user_code TEXT NOT NULL UNIQUE,
  platform TEXT NOT NULL,
  enc_key BLOB,
  sig_key BLOB,
  user_id INTEGER REFERENCES users ON DELETE CASCADE,
  state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'approved', 'denied')),
  interval_s INTEGER NOT NULL DEFAULT 5,
  polled_at INTEGER NOT NULL DEFAULT 0,
  expires_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE author_keys (
  fingerprint TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  public_key TEXT NOT NULL,
  created_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE packages (
  id TEXT PRIMARY KEY,
  owner_id INTEGER REFERENCES users ON DELETE SET NULL,
  created_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE versions (
  id INTEGER PRIMARY KEY,
  package_id TEXT NOT NULL REFERENCES packages,
  version TEXT NOT NULL,
  sha256 TEXT NOT NULL,
  size INTEGER NOT NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  license TEXT NOT NULL,
  tags TEXT NOT NULL,
  authors TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('pending', 'published', 'rejected', 'removed', 'revoked')),
  uploaded_by INTEGER REFERENCES users ON DELETE SET NULL,
  created_at INTEGER NOT NULL,
  published_at INTEGER,
  UNIQUE (package_id, version)
) STRICT;
CREATE INDEX versions_feed ON versions(state, published_at DESC, id DESC);
CREATE INDEX versions_uploader ON versions(uploaded_by);

CREATE TABLE uploads (
  id TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  size INTEGER NOT NULL,
  sha256 TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('open', 'received', 'failed', 'done')),
  errors TEXT,
  version_id INTEGER REFERENCES versions ON DELETE SET NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
CREATE INDEX uploads_user ON uploads(user_id, created_at);

CREATE TABLE cases (
  id INTEGER PRIMARY KEY,
  version_id INTEGER NOT NULL REFERENCES versions ON DELETE CASCADE,
  reason TEXT NOT NULL CHECK (reason IN ('upload', 'report', 'notice', 'counter')),
  detail TEXT NOT NULL DEFAULT '',
  priority INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'actioned', 'dismissed')),
  decision TEXT,
  statement TEXT,
  decided_by INTEGER REFERENCES users ON DELETE SET NULL,
  created_at INTEGER NOT NULL,
  decided_at INTEGER
) STRICT;
CREATE INDEX cases_queue ON cases(state, priority DESC, created_at);

CREATE TABLE notices (
  id TEXT PRIMARY KEY,
  case_id INTEGER REFERENCES cases ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK (kind IN ('copyright', 'illegal', 'counter')),
  name TEXT NOT NULL,
  email TEXT NOT NULL,
  location TEXT NOT NULL,
  work TEXT NOT NULL,
  explanation TEXT NOT NULL,
  signature TEXT NOT NULL,
  received_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE strikes (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  case_id INTEGER REFERENCES cases ON DELETE SET NULL,
  expires_at INTEGER NOT NULL
) STRICT;
CREATE INDEX strikes_user ON strikes(user_id, expires_at);

CREATE TABLE blocklist (
  sha256 TEXT PRIMARY KEY,
  reason TEXT NOT NULL,
  created_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE audit (
  id INTEGER PRIMARY KEY,
  at INTEGER NOT NULL,
  actor_id INTEGER,
  action TEXT NOT NULL,
  subject TEXT NOT NULL,
  detail TEXT NOT NULL DEFAULT ''
) STRICT;
CREATE INDEX audit_at ON audit(at);

CREATE TABLE repo_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  dirty INTEGER NOT NULL DEFAULT 1,
  signed_at INTEGER NOT NULL DEFAULT 0
) STRICT;
INSERT INTO repo_state (id) VALUES (1);

CREATE TABLE sync_heads (
  user_id INTEGER PRIMARY KEY REFERENCES users ON DELETE CASCADE,
  seq INTEGER NOT NULL DEFAULT 0,
  min_seq INTEGER NOT NULL DEFAULT 0,
  epoch INTEGER NOT NULL DEFAULT 0,
  encrypted INTEGER NOT NULL,
  escrow BLOB,
  used_bytes INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE sync_records (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  collection TEXT NOT NULL,
  item TEXT NOT NULL,
  seq INTEGER NOT NULL,
  epoch INTEGER NOT NULL,
  device_id TEXT NOT NULL,
  deleted INTEGER NOT NULL,
  hash BLOB NOT NULL,
  payload BLOB,
  updated_at INTEGER NOT NULL,
  UNIQUE (user_id, collection, item)
) STRICT;
CREATE UNIQUE INDEX sync_records_seq ON sync_records(user_id, seq);

CREATE TABLE mailbox (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users ON DELETE CASCADE,
  to_device TEXT NOT NULL REFERENCES devices ON DELETE CASCADE,
  from_device TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('key-share', 'apply', 'send', 'ping')),
  body BLOB NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
) STRICT;
CREATE INDEX mailbox_inbox ON mailbox(to_device, id);
