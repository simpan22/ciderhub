-- Best-effort schema reversal only. Data in juicing_tree_weights is
-- not migrated back into picking_events — downgrading past this point
-- loses the per-tree weight fidelity the merged model recorded.

CREATE TABLE picking_events (
    event_id  INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    tree_id   INTEGER NOT NULL REFERENCES trees (id),
    weight_kg REAL NOT NULL CHECK (weight_kg > 0)
);

CREATE INDEX idx_picking_events_tree ON picking_events (tree_id);

ALTER TABLE juicing_events ADD COLUMN input_weight_kg REAL;

DROP TABLE juicing_tree_weights;
