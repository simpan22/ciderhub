-- no-transaction
PRAGMA foreign_keys = OFF;

CREATE TABLE events_new (
    id          INTEGER PRIMARY KEY,
    batch_id    INTEGER NOT NULL REFERENCES batches (id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL
                CHECK (event_type IN (
                    'picking', 'juicing', 'additive', 'measurement',
                    'racking', 'bottling', 'note'
                )),
    occurred_at TEXT NOT NULL,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO events_new (id, batch_id, event_type, occurred_at, notes, created_at)
SELECT id, batch_id, event_type, occurred_at, notes, created_at FROM events
WHERE event_type NOT IN ('failed', 'tasting');

DROP TABLE events;

ALTER TABLE events_new RENAME TO events;

CREATE INDEX idx_events_batch_occurred ON events (batch_id, occurred_at);

PRAGMA foreign_key_check;

PRAGMA foreign_keys = ON;
