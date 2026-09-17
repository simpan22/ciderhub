# Spec: Percentage-per-tree on the juicing form

Status: Planning — ready for review

## Problem

The juicing form currently asks for an optional weight in kg per
selected tree — how much fruit that tree contributed. The todo wants
this replaced with a required percentage per tree instead, so the
app can work out how many liters of the final juice came from each
tree (`output_volume_l × percentage`), without needing anyone to
separately weigh fruit per tree at pressing time.

## Decisions

1. **Percentage replaces weight, it isn't added alongside it.** The
   todo says "instead of," and the two numbers answer different
   questions anyway (fruit weight in vs. share of juice out) — keeping
   both would mean maintaining two parallel, occasionally
   contradictory pictures of the same pressing.
2. **This breaks `yield_pct`** (`juicing_events.yield_pct`, "liters of
   juice per kg of fruit," `output_volume_l / total tree weight_kg *
   100`) — its only input, total fruit weight, no longer exists once
   weight_kg is gone. Rather than leave a column every future juicing
   event writes `NULL` to forever, **drop `yield_pct` too**. Nothing in
   the todo asks to preserve it, and a column nothing can compute
   anymore is exactly the kind of drift-prone dead data this app's
   design otherwise avoids. Flag during review if fruit-to-juice
   efficiency is actually still wanted — it would need its own,
   separate "total fruit weight" input if so, unrelated to per-tree
   juice share.
3. **Required per selected tree, not per se on the row** — same
   checkbox-then-value shape the form already uses, just the
   companion value becomes mandatory instead of optional whenever its
   checkbox is checked.
4. **Percentages for one juicing event's checked trees must sum to
   100 (±0.5 for rounding).** The whole point is computing liters per
   tree from the total output; percentages that don't sum to 100 would
   make that computation silently wrong rather than just imprecise, so
   this is validated server-side and rejected with a clear message,
   unlike most numeric fields in this app which trust the log as
   entered. A juicing event with **no** trees checked at all is
   unaffected — nothing to validate, same as today (e.g. juice bought
   in rather than pressed from tracked trees).
5. **No new `CHECK`/`NOT NULL` constraint at the schema level.**
   Adding one to an *existing* column requires SQLite to rebuild the
   table, which needs `foreign_keys` disabled mid-migration —
   something `sqlx`'s SQLite migration runner cannot actually do (see
   `deploy/README.md`'s "Schema changes that need `PRAGMA foreign_keys
   = OFF`" section, written after this exact mistake caused data loss
   earlier in this project). The column stays a plain nullable `REAL`;
   the 0–100 range and "required if checked" rule are enforced in the
   handler instead, same trust boundary as everywhere else in this
   app (validate at the edges, not by re-deriving DB constraints in
   two places).
6. **Existing rows default to an even split**, per the todo's explicit
   instruction — not derived from their old `weight_kg` values. That
   column was optional (many rows have no weight at all) and measured
   a different thing entirely, so there's no reliable conversion; an
   even split across whatever trees are already recorded for each
   juicing event is the honest "we don't actually know" default.
7. **"Liters per tree" replaces the existing "Tree yield" (total kg)
   card on the batch detail page** — same spot, same purpose (how much
   did each tree contribute), now expressed in the unit the todo
   actually asks for: `SUM(output_volume_l × percentage / 100)` per
   tree across the batch's juicing events, computed on read like every
   other derived figure in this app.

## Data model

New migration (plain `ALTER`/`UPDATE`, no `foreign_keys` toggling
needed — `ADD COLUMN` and `DROP COLUMN` are both safe under an open
transaction; only adding a constraint to an existing column isn't):

```sql
ALTER TABLE juicing_tree_weights ADD COLUMN percentage REAL;

UPDATE juicing_tree_weights
SET percentage = 100.0 / (
    SELECT COUNT(*) FROM juicing_tree_weights AS counted
    WHERE counted.event_id = juicing_tree_weights.event_id
);

ALTER TABLE juicing_tree_weights DROP COLUMN weight_kg;
ALTER TABLE juicing_events DROP COLUMN yield_pct;
```

## Backend (`src/handlers/batches.rs`)

- `parse_juicing_submission`: `tree_weights: Vec<(i64, Option<f64>)>`
  becomes `tree_percentages: Vec<(i64, f64)>` — for each checked tree,
  `percentage_tree_{id}` is now required and parsed as `f64`; reject
  (0, 100] out-of-range values. After collecting, if the set is
  non-empty, reject unless the sum is within 0.5 of 100.
- `compute_yield_pct` removed (decision 2); its call sites in
  `add_juicing`/`update_juicing` and the `INSERT`/`UPDATE` into
  `juicing_events` drop the `yield_pct` argument/column.
- `add_juicing`/`update_juicing`: insert into
  `juicing_tree_weights (event_id, tree_id, percentage)`.
- `TreeWeightField` → `TreePercentageField` (`percentage: Option<f64>`
  instead of `weight_kg`); `build_tree_fields` and
  `edit_juicing_fragment`'s query/hashmap follow the rename.
- `fetch_timeline`'s tree-yield query: replace
  `SUM(jtw.weight_kg) as total_kg` with
  `SUM(je.output_volume_l * jtw.percentage / 100.0) as total_l`,
  joining `juicing_events` for `output_volume_l`. `TreeYield.total_kg`
  → `total_l`.
- The juicing timeline row's summary (`fetch_timeline`): tree parts
  render as `{name} ({percentage:.0}%, {liters:.1} L)` instead of
  `{name} ({weight} kg)`; drop the `({pct:.0}% yield)` segment since
  `yield_pct` no longer exists.

## Frontend

- `templates/batches/detail.html` and `_edit_juicing.html`: the
  per-tree number input's `name` becomes `percentage_tree_{{
  t.tree_id }}`, placeholder `%` instead of `kg (optional)`, and
  gains `required` (enforced with plain HTML `required` for the
  common case, with the real 0–100/sum-to-100 check happening
  server-side regardless, same "HTML `required` is a UX nicety, the
  handler is the real validator" pattern already used elsewhere, e.g.
  the code field on the New batch form).
- `templates/batches/_timeline.html`: "Tree yield" list shows
  `{{ ty.total_l|fmt("{:.1}") }} L` instead of `{{ ty.total_kg }} kg`.

## Edge cases

- **A tree checked with 0% or a blank value**: rejected — required
  means required, not "required unless zero," since a 0% row
  contributes nothing to the total and just adds noise.
- **Percentages summing to 99% or 101%** (simple rounding by the
  person entering them): accepted within the ±0.5 tolerance in
  decision 4; anything further off is rejected with a message showing
  what it actually summed to, so it's obvious how to fix.
- **A juicing event with multiple existing tree rows but only one
  updated in isolation**: not applicable — the edit form always
  resubmits every checked tree's percentage together (same
  delete-then-reinsert-all pattern `update_juicing` already uses for
  `juicing_tree_weights`), so partial updates can't happen.

## Out of scope

- Any UI to auto-balance percentages (e.g. "split remaining evenly")
  — plain manual entry, like every other numeric field in this app.
- Preserving `yield_pct`/fruit-weight tracking as a separate concern
  (decision 2's flagged alternative).
