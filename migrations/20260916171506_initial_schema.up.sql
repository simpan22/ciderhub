-- Reference / core entities

-- A season always runs calendar-year Jan 1 to Jan 1; no separate
-- start/end dates to enter.
CREATE TABLE seasons (
    id   INTEGER PRIMARY KEY,
    year INTEGER NOT NULL UNIQUE
);

CREATE TABLE trees (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    variety    TEXT,
    planted_on TEXT,
    location   TEXT,
    notes      TEXT
);

-- No status column: a batch's status is derived from which event types
-- it has logged (see compute_status in src/handlers/batches.rs), not
-- stored, so it can never drift out of sync with the actual event log.
-- No vessel tracking either: which physical container a batch sits in
-- isn't worth recording until bottling, which is its own event type.
CREATE TABLE batches (
    id         INTEGER PRIMARY KEY,
    season_id  INTEGER NOT NULL REFERENCES seasons (id),
    code       TEXT NOT NULL UNIQUE,
    name       TEXT,
    started_on TEXT,
    notes      TEXT
);

CREATE INDEX idx_batches_season ON batches (season_id);

-- Events: a thin spine shared by every typed event table below.

CREATE TABLE events (
    id          INTEGER PRIMARY KEY,
    batch_id    INTEGER NOT NULL REFERENCES batches (id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL
                CHECK (event_type IN (
                    'picking', 'juicing', 'additive', 'measurement',
                    'racking', 'bottling', 'note'
                )),
    occurred_at TEXT NOT NULL,
    -- Free-text note on the event itself, common to every event type,
    -- so it lives on the shared spine rather than duplicated per type.
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_events_batch_occurred ON events (batch_id, occurred_at);

CREATE TABLE picking_events (
    event_id  INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    tree_id   INTEGER NOT NULL REFERENCES trees (id),
    weight_kg REAL NOT NULL CHECK (weight_kg > 0)
);

CREATE INDEX idx_picking_events_tree ON picking_events (tree_id);

CREATE TABLE juicing_events (
    event_id        INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    input_weight_kg REAL NOT NULL CHECK (input_weight_kg > 0),
    output_volume_l REAL NOT NULL CHECK (output_volume_l > 0),
    yield_pct       REAL,
    equipment       TEXT
);

CREATE TABLE additive_events (
    event_id  INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    substance TEXT NOT NULL,
    amount    REAL NOT NULL CHECK (amount > 0),
    unit      TEXT NOT NULL
);

CREATE TABLE measurement_events (
    event_id         INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    specific_gravity REAL,
    ph               REAL,
    temperature_c    REAL,
    volume_l         REAL,
    tasting_notes    TEXT
);

-- Just the transfer itself (volume moved, volume lost to sediment) —
-- no vessel reference, since vessels aren't tracked as entities.
CREATE TABLE racking_events (
    event_id INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    volume_l REAL NOT NULL CHECK (volume_l > 0),
    loss_l   REAL
);

CREATE TABLE bottling_events (
    event_id            INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    bottle_count        INTEGER NOT NULL CHECK (bottle_count > 0),
    bottle_size_ml      INTEGER NOT NULL CHECK (bottle_size_ml > 0),
    carbonation_method  TEXT
);

CREATE TABLE note_events (
    event_id   INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    text       TEXT NOT NULL,
    photo_path TEXT
);

-- Sensors: separate high-volume telemetry path, decoupled from the
-- manually-authored event log above.

CREATE TABLE sensors (
    id       INTEGER PRIMARY KEY,
    label    TEXT NOT NULL UNIQUE,
    location TEXT,
    kind     TEXT NOT NULL
);

CREATE TABLE sensor_readings (
    id          INTEGER PRIMARY KEY,
    sensor_id   INTEGER NOT NULL REFERENCES sensors (id) ON DELETE CASCADE,
    recorded_at TEXT NOT NULL,
    metric      TEXT NOT NULL,
    value       REAL NOT NULL
);

CREATE INDEX idx_sensor_readings_sensor_time ON sensor_readings (sensor_id, recorded_at);
