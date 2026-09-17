-- no-transaction
-- Rebuilding events.event_type's CHECK constraint to add 'failed' and
-- 'tasting' (needed by the batch-failure and tasting-event features),
-- and to finally drop the long-unused 'picking' value now that nothing
-- inserts it.
--
-- SQLite can't ALTER a CHECK constraint in place — this requires the
-- documented create-copy-drop-rename procedure. events is the parent
-- of many ON DELETE CASCADE children (juicing_events, additive_events,
-- measurement_events, racking_events, bottling_events, note_events),
-- and dropping an FK parent while foreign_keys=ON immediately cascades
-- and destroys every child row referencing it — verified empirically
-- before writing this migration. Toggling the pragma requires being
-- outside a transaction, hence "-- no-transaction" opting this file
-- out of sqlx's automatic wrapping.
PRAGMA foreign_keys = OFF;

CREATE TABLE events_new (
    id          INTEGER PRIMARY KEY,
    batch_id    INTEGER NOT NULL REFERENCES batches (id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL
                CHECK (event_type IN (
                    'juicing', 'additive', 'measurement',
                    'racking', 'bottling', 'note', 'failed', 'tasting'
                )),
    occurred_at TEXT NOT NULL,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Excludes leftover 'picking' rows: their data was already carried into
-- juicing_tree_weights by the earlier merge migration, so the spine row
-- itself is inert (no typed table backs it, nothing renders it) and
-- safe to drop here rather than keep as permanent clutter.
INSERT INTO events_new (id, batch_id, event_type, occurred_at, notes, created_at)
SELECT id, batch_id, event_type, occurred_at, notes, created_at FROM events
WHERE event_type != 'picking';

DROP TABLE events;

ALTER TABLE events_new RENAME TO events;

CREATE INDEX idx_events_batch_occurred ON events (batch_id, occurred_at);

PRAGMA foreign_key_check;

PRAGMA foreign_keys = ON;
