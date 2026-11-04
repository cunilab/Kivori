-- Public release data only; no waitlist records are changed by a refresh.
CREATE TABLE IF NOT EXISTS release_snapshots (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  payload TEXT NOT NULL CHECK (json_valid(payload)),
  synced_at INTEGER NOT NULL
);
