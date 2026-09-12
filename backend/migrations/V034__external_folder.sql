-- Per-user settings for the optional external SQL-backed virtual folder.
-- A single row (id = 1). The external account password is stored encrypted
-- (AES-GCM) so the connection stays active across logins without retyping.
CREATE TABLE IF NOT EXISTS external_folder_settings (
    id             INTEGER PRIMARY KEY CHECK (id = 1),
    enabled        INTEGER NOT NULL DEFAULT 0,
    account_id     INTEGER,
    account_email  TEXT,
    system         INTEGER,
    enc_password   BLOB,
    enc_nonce      BLOB,
    connected_at   TEXT,
    updated_at     TEXT
);
