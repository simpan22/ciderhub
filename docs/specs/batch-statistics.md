# Spec: Batch statistics dashboard

Status: Planning — largest/most open-ended item; needs scope
confirmation before implementation, more than the others in this list.

## Problem

There's no way to see trends over time (specific gravity through
fermentation, taste score across bottle-conditioning) or compare one
batch against another. Per-batch data exists (in `measurement_events`,
and once built, `tasting_events`) but is only ever shown as a flat
timeline, never plotted.

## Decisions

1. **Server-rendered SVG, no charting library.** Consistent with
   `vision.md`'s existing Phase 2 plan ("Gravity/ABV curve chart per
   batch (server-rendered SVG)... avoids a JS charting dependency").
   This todo extends that from "one fixed chart per batch" to
   "choose metrics, choose batches to compare" — the rendering
   approach doesn't need to change, just what data feeds it.
2. **Selection via plain forms/checkboxes + htmx, not client-side JS
   charting interactivity.** "Ability to select which data to show"
   is satisfiable by: pick batches (checkboxes) and metrics
   (checkboxes), submit (or auto-submit via `hx-trigger="change"`),
   server re-renders the SVG. No pan/zoom/hover-tooltip interactivity
   is implied by the todo text — flag during review if that's actually
   wanted, since that would need a different (JS-based) approach.
3. **New page, not folded into the existing `/` dashboard.** The
   existing dashboard (`handlers::dashboard::index`) is a placeholder
   ("Batches, trees, and events will show up here"). This feature
   becomes that page's real content, or a new `/stats` page linked from
   it — recommend making it *the* dashboard, since "dashboard with
   statistics" is literally the todo's wording, and the current index
   page has no other purpose yet.
4. **Metrics available to plot** (from data that exists or is planned):
   - Specific gravity (`measurement_events.specific_gravity`)
   - pH (`measurement_events.ph`)
   - Temperature (`measurement_events.temperature_c`)
   - Volume (`measurement_events.volume_l`)
   - Taste score (`tasting_events.score`, once `tasting-event.md` is
     built — this spec's chart code should be written against
     whichever of the two lands first without hard-blocking on the
     other)
   - ABV is *derived* (two gravity readings), not a stored value — per
     `vision.md`, it's "computed on read from two gravity measurements,
     not stored." Charting it means computing a running ABV estimate
     from the gravity series rather than reading a column directly —
     more involved than the others; treat as a stretch addition once
     the basic gravity/pH/temp/score charts work.
5. **X-axis is calendar date, not "days since start."** Simpler, and
   consistent with how the timeline already displays dates. Comparing
   two batches on the same calendar axis is also arguably more useful
   for a home setup (were both fermenting during the same heat wave?)
   than aligning them by day-zero.

## Backend

- `fetch_series(db, batch_id, metric) -> Vec<(String, f64)>` — one
  function per metric, or a single function taking a metric enum and
  dispatching to the right query (`specific_gravity`, `ph`,
  `temperature_c`, `volume_l` all come from `measurement_events`;
  `score` from `tasting_events`). Each returns `(occurred_at, value)`
  pairs, skipping nulls, ordered by date.
- `render_chart_svg(series: &[(batch_label, Vec<(date, value)>)]) ->
  String` — hand-rolled SVG: compute the combined min/max across all
  selected series for axis scaling, one `<polyline>` per
  batch+metric combination, a distinct color per batch (reuse the
  palette approach already used for `.status` badges — small, fixed,
  legible set of colors), simple axis tick labels.
- `GET /stats?batches=1,2&metrics=specific_gravity,ph` — parses the
  query params, fetches series for the cross product of selected
  batches × metrics, renders the SVG, returns the fragment (for
  htmx-driven re-render) or the full page (on direct navigation).

## Frontend

- Batch checklist (all batches, most recent season first — reuse the
  batch list's existing ordering) and metric checklist, both wired
  with `hx-get="/stats" hx-trigger="change" hx-target="#stats-chart"
  hx-include="[name=batches],[name=metrics]"` (htmx's `hx-include` to
  gather sibling checkbox state, since the changed checkbox alone
  doesn't carry the *other* selections).
- `#stats-chart` container holding the rendered `<svg>`, replaced on
  every selection change.
- A legend mapping color → batch (and metric, if multiple metrics are
  selected at once — worth deciding during review whether mixing
  metrics with different units, e.g. gravity and pH, on one shared
  y-axis is meaningful at all, or whether selecting multiple metrics
  should render multiple small charts stacked instead of one
  shared-axis chart. Recommend: **one chart per selected metric**,
  stacked vertically, each with its own y-axis scaled to its own
  data — avoids the meaningless-shared-axis problem entirely and is
  simpler to render correctly).

## Edge cases

- **No batches/metrics selected**: render an empty state ("select at
  least one batch and one metric"), not an empty or broken `<svg>`.
- **A selected batch has no data for a selected metric**: that
  batch's line is simply absent from that metric's chart — not an
  error, just nothing to draw.
- **Single data point for a series**: a `<polyline>` needs ≥2 points to
  draw a line; render a single dot (`<circle>`) instead when a series
  has exactly one point, rather than silently drawing nothing.

## Out of scope (for a first version)

- Interactive tooltips/hover values, zoom/pan — would require
  client-side JS charting, contrary to decision 1.
- Exporting the chart as an image/CSV.
- The derived-ABV chart (decision 4's stretch item) — land the direct
  metrics first.
