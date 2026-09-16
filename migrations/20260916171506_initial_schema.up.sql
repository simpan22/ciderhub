-- Reference / core entities

CREATE TABLE seasons (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    starts_on  TEXT NOT NULL,
    ends_on    TEXT
);

CREATE TABLE trees (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    variety    TEXT,
    planted_on TEXT,
    location   TEXT,
    notes      TEXT
);

CREATE TABLE vessels (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    capacity_l  REAL,
    kind        TEXT NOT NULL,
    active      INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1))
);

CREATE TABLE batches (
    id         INTEGER PRIMARY KEY,
    season_id  INTEGER NOT NULL REFERENCES seasons (id),
    code       TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'planning'
               CHECK (status IN ('planning', 'fermenting', 'conditioning', 'bottled', 'archived')),
    vessel_id  INTEGER REFERENCES vessels (id),
    started_on TEXT,
    notes      TEXT
);

CREATE INDEX idx_batches_season ON batches (season_id);
CREATE INDEX idx_batches_vessel ON batches (vessel_id);

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

CREATE TABLE racking_events (
    event_id       INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    from_vessel_id INTEGER REFERENCES vessels (id),
    to_vessel_id   INTEGER NOT NULL REFERENCES vessels (id),
    volume_l       REAL NOT NULL CHECK (volume_l > 0),
    loss_l         REAL
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
