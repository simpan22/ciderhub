-- A batch's start date is now computed as MIN(events.occurred_at) for
-- that batch, not a separately-entered fact that can drift from what
-- actually happened — same reasoning as status. Verified that a plain
-- DROP COLUMN (unlike the events-table rebuild) doesn't touch any
-- other table's rows regardless of foreign_keys state, so this is
-- safe as a normal sqlx migration.
ALTER TABLE batches DROP COLUMN started_on;
