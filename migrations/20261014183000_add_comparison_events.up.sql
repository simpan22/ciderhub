-- Pairwise taste comparisons for the Bradley-Terry rating (see
-- docs/specs/bradley-terry-taste-rating.md). The events spine row
-- belongs to the batch the comparison was logged from; other_batch_id
-- is the batch it was compared against, and outcome is from the spine
-- batch's point of view. Stored once rather than as two mirrored rows,
-- so the two sides can never disagree.
--
-- Needs events.event_type to accept 'comparison', which is handled by
-- deploy/rebuild_events_drop_type_check.py (can't be a migration, see
-- deploy/README.md).
CREATE TABLE comparison_events (
    event_id       INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    other_batch_id INTEGER NOT NULL REFERENCES batches (id) ON DELETE CASCADE,
    outcome        TEXT NOT NULL CHECK (outcome IN ('win', 'loss', 'tie'))
);

CREATE INDEX idx_comparison_events_other_batch ON comparison_events (other_batch_id);
