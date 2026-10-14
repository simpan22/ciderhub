-- Friends who log taste comparisons from the /taste page via a
-- personal invite link (see docs/specs/friend-tasting-page.md). Never
-- deleted, only revoked, so comparison_events.taster_id always points
-- at a real row and the history of who logged what stays intact.
CREATE TABLE tasters (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    -- 32 random bytes, hex-encoded. Stored in plain text so the owner
    -- page can show the invite link again.
    token      TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    revoked_at TEXT
);

-- NULL = logged by the owner (main app, or /taste with the owner's
-- cookie). A plain ADD COLUMN doesn't rebuild the table, so unlike the
-- event_type CHECK change it's safe as a normal migration.
ALTER TABLE comparison_events ADD COLUMN taster_id INTEGER REFERENCES tasters (id);

CREATE INDEX idx_comparison_events_taster ON comparison_events (taster_id);
