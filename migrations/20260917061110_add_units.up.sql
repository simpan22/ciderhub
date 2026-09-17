CREATE TABLE units (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

-- Seed from existing data, lowercased and de-duplicated.
INSERT INTO units (name)
SELECT DISTINCT LOWER(TRIM(unit)) FROM additive_events;

ALTER TABLE additive_events ADD COLUMN unit_id INTEGER REFERENCES units (id);

UPDATE additive_events
SET unit_id = (SELECT id FROM units WHERE units.name = LOWER(TRIM(additive_events.unit)));

ALTER TABLE additive_events DROP COLUMN unit;
