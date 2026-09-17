# Spec: Season overview timeline

Status: Planning — ready for review

## Problem

The dashboard (`/`) currently only offers per-metric line charts
(specific gravity, pH, ...), which need at least one measurement or
tasting event to show anything. There's no way to see, at a glance,
where every batch in a season stands relative to each other in time —
which ones started early, which are still fermenting, which are
already bottled — without opening each batch individually and reading
its timeline.

## Decisions

1. **A new section on the existing dashboard (`/`), not a separate
   page.** The todo describes this as part of "the dashboard." It sits
   alongside the existing metrics-chart section as a second,
   independent block — its own season picker, unrelated to the
   batch/metric checkboxes used by the line charts.
2. **One season at a time**, chosen from a `<select>` (reuses the same
   `year DESC` season ordering used elsewhere), defaulting to the most
   recent season. Switching seasons re-renders via htmx
   (`hx-get="/dashboard/season-overview?season_id=..." hx-trigger="change"`),
   same pattern as the batch-code suggestion on the New batch form.
3. **Bars are built from existing event data — no new schema.** A
   batch's start is already `MIN(events.occurred_at)` (per
   [[batch-start-date]]); its bottling date is the `bottling` event's
   `occurred_at`. No new table or column is needed; this is purely a
   new read query plus a new SVG renderer.
4. **Color encodes phase, not batch identity** — the opposite of the
   existing line charts (where color = batch, since every series there
   is the same metric). Here every bar is the same "thing" (a batch),
   so color is free to carry more useful information: which part of
   the process a given stretch of time was in. Batch identity is
   carried by the row label instead. Four fixed phase colors, matching
   `compute_status`'s four non-planning states:
   - `fermenting` — from the batch's start until its first `racking`
     event, or until bottling if it's never racked.
   - `conditioning` — from the first `racking` event until bottling.
     Only appears for batches that actually have a racking event.
   - `bottled` — a thin end-cap marker at the bottling date itself
     (bottling is a point in time, not a phase with duration — nothing
     is logged *after* bottling that this view tracks, so there's no
     "bottled" span to shade, only the moment it happened).
   - `failed` — from the batch's start until its `failed` event,
     replacing whatever phase colors would otherwise apply. Matches
     `compute_status`'s override: a failed batch reads as failed for
     its whole bar, not "fermenting-then-failed."
5. **Batches still in progress extend to today**, in their current
   phase's color, since there's no bottling date yet to end at. Not
   visually distinguished from a finished phase (no hatching/dashing) —
   the row label's status word (already computed elsewhere) is enough
   context, and hobby-scale batch counts don't need more than that.
6. **Shared x-axis across every row in the season**, per the todo's
   explicit wording ("even though the timelines are duplicated they
   should represent the same time interval"). The axis spans from
   shortly before the earliest included batch's start to shortly after
   the latest included end (bottling date, failure date, or today for
   in-progress batches, whichever is latest) — a small fixed padding
   (e.g. a few days) on each side, not a percentage, since gravity/pH
   charts already establish the "pad a little past the data" pattern.
7. **Batches with no events yet (`status == "planning"`) are
   excluded** — there's no start date to place them at, and an empty
   bar would be misleading rather than informative.

## Backend

- `fetch_batch_phases(db, season_id) -> Vec<BatchTimelineRow>`:
  - One query for the season's batches, one for all their events
    (grouped in memory by batch, same N+1-avoidance shape already used
    in `list()`/`fetch_timeline()`).
  - For each batch with at least one event, compute:
    - `start` = `MIN(occurred_at)`.
    - `racking_at` = the first `racking` event's `occurred_at`, if any.
    - `bottling_at` = the `bottling` event's `occurred_at`, if any.
    - `failed_at` = the `failed` event's `occurred_at`, if any.
    - `end` = `failed_at`, else `bottling_at`, else today.
  - Skip batches with no events (decision 7).
- `render_season_timeline(rows: &[BatchTimelineRow]) -> String` (new
  function in `src/charts.rs`, alongside `render_line_chart`): computes
  the shared axis (decision 6), draws one horizontal row per batch —
  a background line/track, one or two colored `<rect>` segments per
  the phase boundaries above, a `<text>` label with the batch code at
  the row's start. A fixed legend (phase name → color) below the
  chart, same as the existing line-chart legend's markup but keyed by
  phase instead of batch.
- `GET /dashboard/season-overview?season_id=N`: parses `season_id`
  (plain scalar — the earlier `serde_urlencoded` Vec limitation
  doesn't apply here, so the typed `Query` extractor is fine), calls
  `fetch_batch_phases`, renders the fragment. Used both by the
  season-select's `change` handler and by the initial dashboard render
  (same "compute the default season's version up front" pattern as
  Feature 8's code suggestion).

## Frontend

- `templates/dashboard/_season_overview.html` (new): season `<select>`
  (`hx-get`, `hx-trigger="change"`, `hx-target` the results container,
  `hx-swap="outerHTML"`) plus the rendered `<svg>` timeline and its
  legend.
- `templates/index.html`: includes the new section above or below the
  existing stats-picker section — a separate `<section class="card">`,
  not merged into the existing form.

## Edge cases

- **Season has zero batches, or every batch is still in `planning`**:
  empty-state message ("no batches with logged events yet"), not a
  broken/empty `<svg>`.
- **A batch's only event is its own `failed` event** (start == failed
  date, zero-width span): render a small dot at that point, same
  "single point → circle, not a line" fallback already used in
  `render_line_chart`.
- **A batch has a `bottling` event after its `failed` event** (i.e.,
  bottling was logged despite the batch being marked failed — possible
  today since nothing prevents logging events after failure): the bar
  still ends at the failure date per decision 4; the later bottling
  event is simply not reflected in this view. This is an unusual data
  state, not a case worth adding validation for here.
- **Racking event logged after bottling** (out-of-order data entry):
  the `conditioning` segment would extend past the bar's `end`:
  clip the segment's rendered end to `end` rather than drawing past
  it, so the SVG never draws a phase segment beyond the bar itself.

## Out of scope

- Editing events from this view (it's read-only, like the line
  charts).
- Cross-season comparison (one season at a time, per decision 2).
- Any new "ended"/"completed" date beyond what's already computed for
  [[batch-start-date]] and the existing `bottling` event.
