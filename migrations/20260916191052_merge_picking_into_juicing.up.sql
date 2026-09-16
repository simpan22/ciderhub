-- Picking is no longer a separate event type. Which trees fed a batch,
-- and how much each contributed, is now recorded directly on the
-- juicing event that pressed them — weight per tree is optional (you
-- may know you used a tree without having weighed its share).
--
-- 'picking' is left as a harmless unused value in events.event_type's
-- CHECK constraint rather than rebuilding that table: SQLite requires
-- a full table-copy to change a CHECK constraint, and events has many
-- dependent child tables via FK — not worth the risk for an enum value
-- nothing will insert anymore.

CREATE TABLE juicing_tree_weights (
    id        INTEGER PRIMARY KEY,
    event_id  INTEGER NOT NULL REFERENCES juicing_events (event_id) ON DELETE CASCADE,
    tree_id   INTEGER NOT NULL REFERENCES trees (id),
    weight_kg REAL
);

CREATE INDEX idx_juicing_tree_weights_event ON juicing_tree_weights (event_id);
CREATE INDEX idx_juicing_tree_weights_tree ON juicing_tree_weights (tree_id);

-- Carry existing picking data forward: attach each picking event's
-- tree/weight to its batch's earliest juicing event, rather than losing
-- it when picking_events is dropped below. (SQLite can't correlate the
-- outer row inside a subquery's ORDER BY, only its WHERE, so this picks
-- the chronologically first juicing event per batch rather than the
-- closest-in-time one — equivalent in practice, since every batch here
-- has exactly one juicing event.)
INSERT INTO juicing_tree_weights (event_id, tree_id, weight_kg)
SELECT
    (
        SELECT je.event_id
        FROM juicing_events je
        JOIN events je_e ON je_e.id = je.event_id
        WHERE je_e.batch_id = pe_e.batch_id
        ORDER BY je_e.occurred_at ASC, je.event_id ASC
        LIMIT 1
    ) AS event_id,
    pe.tree_id,
    pe.weight_kg
FROM picking_events pe
JOIN events pe_e ON pe_e.id = pe.event_id
WHERE EXISTS (
    SELECT 1 FROM juicing_events je2
    JOIN events je2_e ON je2_e.id = je2.event_id
    WHERE je2_e.batch_id = pe_e.batch_id
);

ALTER TABLE juicing_events DROP COLUMN input_weight_kg;

DROP TABLE picking_events;
