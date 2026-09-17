# Spec: Bottle count on the batch list

Status: Planning — ready for review

## Problem

[[batch-list-columns]] added a Bottled (date) column but not the
bottle count itself — how many bottles came out of that bottling run,
which is already recorded on `bottling_events.bottle_count` but only
visible by opening the batch.

## Decisions

1. **Computed from `bottling_events`, no new schema** — same principle
   as every other batch-list column.
2. **"If relevant" = if the batch has been bottled.** Shown only when
   at least one bottling event exists; `—` otherwise, same convention
   already used for Bottled/ABV/Aged on this list.
3. **Summed across bottling events**, in case a batch is bottled in
   more than one session (nothing prevents logging a second bottling
   event today) — matches how Trees/Additives already aggregate across
   potentially multiple source events rather than assuming exactly
   one, even though `bottled_on` itself stays keyed to the *first*
   bottling event's date, per [[batch-list-columns]]'s existing
   decision.

## Backend

- `src/handlers/batches.rs`, `list()`: one more grouped query,
  `SELECT e.batch_id, be.bottle_count FROM bottling_events be JOIN
  events e ON e.id = be.event_id`, summed in memory per batch
  (same anti-N+1 shape as every other column here).
- `BatchListItem` gains `bottle_count: Option<i64>` (`None` when no
  bottling event exists, distinct from `Some(0)` which can't happen
  since `bottle_count > 0` is enforced by the column's own `CHECK`).

## Frontend

- `templates/batches/list.html`: a "Bottles" column, placed next to
  Bottled, showing `{{ b.bottle_count.map(...) }}` /
  `b.bottle_count.as_ref().map(...).unwrap_or("—")`-style rendering —
  in practice `{% if let Some(n) = b.bottle_count %}{{ n }}{% else %}—{% endif %}`,
  same pattern already used for ABV/Aged.

## Out of scope

- Bottle size/volume totals — the todo asks for a count, not a volume;
  `bottle_size_ml` stays detail-page-only.
