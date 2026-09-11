-- Per-user configurable shortcut shown below Settings in the navigation rail.
-- A single row (id = 1). `url` empty means no shortcut button is shown.
CREATE TABLE IF NOT EXISTS custom_nav_link (
    id         INTEGER PRIMARY KEY CHECK (id = 1),
    url        TEXT,
    icon       TEXT,
    updated_at TEXT
);
