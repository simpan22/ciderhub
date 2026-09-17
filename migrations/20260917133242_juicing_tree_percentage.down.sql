ALTER TABLE juicing_events ADD COLUMN yield_pct REAL;
ALTER TABLE juicing_tree_weights ADD COLUMN weight_kg REAL;
ALTER TABLE juicing_tree_weights DROP COLUMN percentage;
