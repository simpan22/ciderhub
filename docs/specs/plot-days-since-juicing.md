# Spec: Days-since-juicing x-axis for the metric charts

Status: Planning — ready for review

## Problem

The dashboard's per-metric charts (`render_line_chart` — specific
gravity, pH, temperature, volume, taste score) currently plot against
absolute calendar dates. Comparing two batches juiced weeks apart is
awkward: their curves sit at different points along the x-axis even if
they're at the same stage of fermentation. The todo wants the x-axis
to read "days after juicing" instead, so batches line up by process
stage rather than calendar date, plus per-point hover detail that
doesn't exist today.

## Scope

**Only `render_line_chart`** (the 5 metric charts on `/`). The Season
overview timeline (`render_season_timeline`) is a different kind of
chart — phase durations, not a metric measured over time — and isn't
mentioned by this todo; same "this chart only" scoping already applied
when month gridlines were added to the timeline.

## Decisions

1. **X-axis = days since the batch's own first `juicing` event**, not
   calendar date. This is what actually enables the comparison the
   todo asks for: two batches juiced on different dates can now share
   one axis, aligned by process stage instead of wall-clock time.
2. **Day 0 is always labeled "Fermentation Start"**, every other
   labeled tick is a plain day number, per the todo's explicit
   wording. The axis always includes day 0, even when a chart's actual
   data starts later (e.g. Taste Score readings, which only begin
   after bottling — possibly 60+ days in) — a consistent anchor across
   every chart, at the cost of sometimes-blank leading space on charts
   whose data starts late. Flag during review if Taste Score
   specifically should instead anchor at its own first data point
   rather than day 0 — this spec treats all five metrics uniformly
   rather than special-casing one.
3. **A batch with no `juicing` event can't be placed on this axis at
   all** (no day 0 to measure from) — excluded from every metric's
   chart, not silently plotted at day 0. Rare in practice (a batch
   whose only events are additives/measurements), but possible.
4. **Tick interval chosen from the axis's day-span**, same
   fixed-threshold approach [[timeline-month-lines]] already
   established for the Season overview's month gridlines, just in
   days instead of months — reusing that visual language (light gray
   lines, darker gray labels) for consistency across the app's charts:
   `≤14d → every 2d`, `≤30d → every 5d`, `≤90d → every 10d`,
   `≤180d → every 30d`, else every 60d.
5. **Every data point gets a dot**, not just the existing
   single-point fallback — this is what "events should be marked as
   dots" means literally, and gives every point a shape to attach a
   tooltip to.
6. **Hover tooltip = a native SVG `<title>` inside each point's
   `<circle>`.** Browsers render this as a built-in tooltip on hover
   with zero JS, consistent with `vision.md`'s server-rendered-SVG,
   no-charting-library stance — the same reasoning that ruled out
   pan/zoom/rich hover in `batch-statistics.md`. Content: the date,
   the day offset, the value (formatted to the same precision the
   batch timeline's measurement summary already uses — SG to 3
   decimals, pH to 2, temperature/volume to 1, score as `N/10`), and
   the event's own note if it has one (already joined from
   `events.notes` for the query these charts run, so it's essentially
   free to include).

## Backend

- `src/handlers/dashboard.rs`:
  - One query fetching every involved batch's first `juicing` date
    (`MIN(occurred_at)` grouped by `batch_id`, filtered to
    `event_type = 'juicing'`), fetched once in `build_charts` rather
    than per batch/metric.
  - `fetch_measurement_series` also selects `e.notes` and now returns
    day-offset-from-juicing (an `i64`, computed from the batch's
    juicing date resolved above) plus a ready-made tooltip string,
    instead of a raw date string — the value/precision formatting and
    notes-suffix logic moves here (matching the metric-specific
    formatting `fetch_timeline`'s measurement summary already uses),
    so `charts.rs` stays a dumb renderer that only knows about days
    and pre-formatted labels.
  - `build_charts`: skip a batch's series entirely (for every metric)
    if it has no resolved juicing date, per decision 3.
- `src/charts.rs`:
  - `Series`'s points become `(day: i64, value: f64, tooltip: String)`
    instead of `(date: String, value: f64)` — `render_line_chart` no
    longer parses dates itself; the axis is built directly from
    integer days, always widened to include `0`.
  - Add the tick-gridline pass (decision 4), reusing the same
    line+text drawing shape `month_gridlines`/its call site in
    `render_season_timeline` already established, adapted to days
    instead of calendar months.
  - Every point renders a `<circle>` with a nested `<title>` (decision
    6); the connecting `<polyline>` is unchanged when a series has 2+
    points.

## Edge cases

- **A batch+metric combination with zero points**: unchanged from
  today — simply absent from that metric's chart.
- **An event dated before the batch's own juicing event** (unusual,
  not prevented elsewhere in this app): yields a negative day offset,
  plotted as-is left of "Fermentation Start" — no special handling,
  same trust-the-log stance used throughout.
- **A single data point for a batch+metric**: gets its circle and
  tooltip like any other point; nothing for the polyline to connect,
  same outcome as today's single-point fallback, just reached via the
  general case now that every point draws a circle.

## Out of scope

- The Season overview timeline (decision/scope note above).
- Any JS-based interactivity (pan/zoom, a styled custom tooltip) —
  native SVG `<title>` only, per decision 6.
