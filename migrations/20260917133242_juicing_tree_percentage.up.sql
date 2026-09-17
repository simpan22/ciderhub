-- Tree contribution is now recorded as a percentage of the juicing
-- event's total output volume, not a separate fruit-weight
-- measurement — liters per tree is then output_volume_l * percentage
-- / 100, computed on read like every other derived value in this app.
--
-- No CHECK/NOT NULL added at the schema level: SQLite can only add
-- those to an existing column by rebuilding the table, which requires
-- disabling foreign_keys mid-migration — a step sqlx's SQLite runner
-- can't actually perform (see deploy/README.md's "Schema changes that
-- need PRAGMA foreign_keys = OFF" section). The application enforces
-- 0 < percentage <= 100, required for every checked tree, instead.
ALTER TABLE juicing_tree_weights ADD COLUMN percentage REAL;

-- Default existing entries to an even split across each juicing
-- event's recorded trees, per the todo's explicit instruction — not
-- derived from the old weight_kg values, which weren't always present
-- (the column was optional) and measured a different thing (fruit
-- weight in, not juice-output share).
UPDATE juicing_tree_weights
SET percentage = 100.0 / (
    SELECT COUNT(*) FROM juicing_tree_weights AS counted
    WHERE counted.event_id = juicing_tree_weights.event_id
);

ALTER TABLE juicing_tree_weights DROP COLUMN weight_kg;

-- yield_pct (juice out / fruit in) can no longer be computed now that
-- per-tree fruit weight isn't tracked at all — dropping it rather than
-- leaving a column every future juicing event would just leave NULL
-- forever.
ALTER TABLE juicing_events DROP COLUMN yield_pct;
