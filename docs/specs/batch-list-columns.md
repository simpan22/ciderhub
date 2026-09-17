# Spec: Batch list columns

Status: Planning — ready for review

## Problem

The batches list (`/batches`) currently shows Code, Name, Season, and
Status only. The todo asks for several more at-a-glance facts per
batch, all of which already exist somewhere in the event log but
require opening the batch to see today.

## Naming note

"Batch id" in the todo means `code` (e.g. `2026-A`), same naming note
as [[batch-code-prefill]] — the list already has this column, so it's
unchanged here.

## Decisions

1. **Every new column is computed from existing events, no new
   schema.** Same principle as `status` and [[batch-start-date]]: none
   of this is separately entered or stored, so it can never drift from
   what actually happened.
2. **Start date** = `MIN(events.occurred_at)` per batch, same value
   already computed for the batch detail page. `—` if the batch has no
   events yet.
3. **Bottling date** = the batch's `bottling` event's `occurred_at` (a
   batch has at most one in practice). `—` if not yet bottled.
4. **Trees used** = distinct tree names from `juicing_tree_weights`
   across all of the batch's juicing events, comma-separated, no
   weights or percentages. `—` if none recorded (a juicing event's
   tree/weight breakdown is optional today).
5. **Additives** = distinct `additive_events.substance` values for the
   batch, comma-separated, no amounts/units. `—` if none.
6. **Alcohol percentage — potential ABV from the starting gravity
   alone, not a two-point OG/FG delta.** The todo's own wording ("based
   on **first** SG measurement... otherwise omitted") describes a
   single-reading estimate, not the OG-minus-FG calculation
   `batch-statistics.md` mentions as a *later* stretch goal for the
   stats chart — this is a simpler, different number for a different
   place (a quick glance on the list, not a precise final reading).
   - Take the batch's earliest `measurement_events.specific_gravity`
     reading (by `occurred_at`).
   - Only trust it as the true starting gravity if its `occurred_at`
     equals the batch's *first* `juicing` event's `occurred_at` — i.e.
     it was measured on pressing day, before fermentation could have
     changed it. Any other timing means we don't actually know the
     starting gravity, so the estimate would be wrong, not just
     imprecise — omit rather than show a misleading number.
   - Formula: potential ABV% = `(OG - 1.000) × 131.25`, the standard
     homebrew approximation assuming full attenuation to ~1.000 SG.
     Flag during review if a different formula or assumption
     (e.g. requiring an actual final-gravity reading instead of
     assuming dryness) is wanted instead — this reading is a rough
     "how strong will this end up" estimate, not a lab-accurate value.
7. **Days since bottling** = `today − bottling_date` in whole days,
   shown only when a bottling date exists. Plain day count, no
   week/month conversion — consistent with the app's plain,
   unadorned display of other computed values (e.g. the "Started
   <date>" line added for [[batch-start-date]]).
8. **Wrap the table in a horizontal-scroll container.** Six more
   columns makes this too wide for a phone screen; `vision.md`
   explicitly wants the UI usable in the field, so the table gets an
   `overflow-x: auto` wrapper rather than being redesigned into cards
   (out of scope — a bigger visual change than this todo asks for).

## Backend

All in `list()` (`src/handlers/batches.rs`), extending the existing
batch-events query rather than adding N+1 per-row queries, same
pattern `compute_status` already uses:

- Extend the existing `SELECT batch_id, event_type FROM events` query
  to also select `occurred_at`. Walk it once (already ordered, or add
  `ORDER BY occurred_at`) building a per-batch accumulator: first
  `occurred_at` seen (= start), first `juicing` event's date, first
  `bottling` event's date. Reuses the same "first occurrence in a
  globally date-sorted stream wins" trick used for
  [[dashboard-timeline]]'s phase boundaries.
- One query for tree names: `SELECT DISTINCT e.batch_id, t.name FROM
  juicing_tree_weights jtw JOIN juicing_events je ON je.event_id =
  jtw.event_id JOIN events e ON e.id = je.event_id JOIN trees t ON
  t.id = jtw.tree_id`, grouped into `HashMap<i64, Vec<String>>`.
- One query for additive substances: `SELECT DISTINCT e.batch_id,
  ae.substance FROM additive_events ae JOIN events e ON e.id =
  ae.event_id`, grouped the same way.
- One query for gravity readings: `SELECT e.batch_id, e.occurred_at,
  me.specific_gravity FROM measurement_events me JOIN events e ON e.id
  = me.event_id WHERE me.specific_gravity IS NOT NULL ORDER BY
  e.occurred_at`, first-per-batch giving the earliest SG + its date,
  compared against that batch's first-juicing date from the
  accumulator above.
- `BatchListItem` gains `started_on: Option<String>`, `bottled_on:
  Option<String>`, `trees: Vec<String>`, `additives: Vec<String>`,
  `abv_pct: Option<f64>`, `days_since_bottling: Option<i64>`.

## Frontend

- `templates/batches/list.html`: wrap `<table class="card table">` in
  a `<div class="table-scroll">`; add columns Started, Bottled, Trees,
  ABV, Aged, Additives after the existing Status column. List-valued
  cells render as `{{ list|join(", ") }}` or `—` when empty. ABV
  rendered to one decimal place (`{{ "%.1f"|format(v) }}%`).
- `static/css/style.css`: `.table-scroll { overflow-x: auto; }`.

## Edge cases

- **Multiple juicing events for one batch** (e.g. a top-up pressing):
  "first juicing date" and "trees used" both consider all juicing
  events for the batch; the ABV same-day check only compares against
  the *first* one, per decision 6.
- **A juicing event with no tree/weight rows** (breakdown not
  recorded): contributes nothing to the Trees column, not an error.
- **SG measured on the same day as juicing but showing an implausible
  value** (e.g. someone mis-keys `0.something`): not validated here —
  same trust-the-log approach used everywhere else in the app.

## Out of scope

- Redesigning the list as cards instead of a table (decision 8).
- The two-point OG/FG ABV chart from `batch-statistics.md` — unrelated
  number, unrelated place.
- Sorting/filtering the list by any of the new columns.
