ALTER TABLE additive_events ADD COLUMN unit TEXT;

UPDATE additive_events
SET unit = (SELECT name FROM units WHERE units.id = additive_events.unit_id);

ALTER TABLE additive_events DROP COLUMN unit_id;

DROP TABLE units;
