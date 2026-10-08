-- Hatoba sync Worker: initial schema (spec section 5.3).
-- One Worker deployment serves exactly one user / one vault.
-- All timestamps are Unix epoch milliseconds. Only ciphertext is stored.

CREATE TABLE meta (
  id                  INTEGER PRIMARY KEY CHECK (id = 1),
  schema_version      INTEGER NOT NULL,
  kdf_salt            TEXT NOT NULL,
  kdf_params          TEXT NOT NULL,     -- JSON, stored and returned verbatim
  auth_hash           TEXT NOT NULL,     -- hex SHA-256(auth_key)
  protected_vault_key TEXT NOT NULL,
  recovery_vault_key  TEXT NOT NULL,
  recovery_auth_hash  TEXT NOT NULL,     -- hex SHA-256(recovery_auth)
  seq                 INTEGER NOT NULL DEFAULT 0,
  created_at          INTEGER NOT NULL
);

CREATE TABLE items (
  id         TEXT PRIMARY KEY,
  envelope   TEXT,                       -- NULL once deleted (tombstone)
  revision   INTEGER NOT NULL,
  seq        INTEGER NOT NULL,           -- global, monotonically increasing, used for incremental pull
  deleted    INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_items_seq ON items(seq);

CREATE TABLE sessions (
  token_hash  TEXT PRIMARY KEY,          -- hex SHA-256(session_token)
  device_id   TEXT NOT NULL,
  device_name TEXT NOT NULL,             -- opaque envelope encrypted with vault_key
  scope       TEXT NOT NULL DEFAULT 'full' CHECK (scope IN ('full', 'recovery')),
  created_at  INTEGER NOT NULL,
  last_seen   INTEGER NOT NULL,
  expires_at  INTEGER NOT NULL
);
-- At most one active session per device and scope.
CREATE UNIQUE INDEX idx_sessions_device_scope ON sessions(device_id, scope);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
