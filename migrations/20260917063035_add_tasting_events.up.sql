CREATE TABLE tasting_events (
    event_id      INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    score         INTEGER NOT NULL CHECK (score BETWEEN 0 AND 10),
    tasting_notes TEXT
);
