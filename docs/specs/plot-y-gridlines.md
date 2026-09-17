# Spec: Horizontal value gridlines on the metric charts

Status: Planning — ready for review

## Problem

`render_line_chart` (the dashboard's specific gravity/pH/temperature/
volume/score charts) only labels the y-axis at its two extremes (the
data's min and max). Reading off any value in between means visually
interpolating between two dots with no reference lines — the x-axis
already got day-tick gridlines in [[plot-days-since-juicing]], but the
y-axis never did.

## Scope

Only `render_line_chart`. The Season overview
(`render_season_timeline`) has no numeric y-axis — its rows are
categorical (one per batch) — so "grid lines to make it easier to read
values" doesn't apply there.

## Decisions

1. **Replaces the min/max-only y-labels, doesn't add to them.** Same
   reasoning as replacing the month gridlines with week gridlines: a
   proper set of evenly-spaced tick gridlines already covers what the
   old two fixed labels did, so keeping both would just be redundant
   labels at slightly different values.
2. **"Nice" tick step, not a fixed count or a fixed raw division.**
   Picks a step from {1, 2, 5} × a power of 10 closest to `span / 4`
   (aiming for roughly 4–6 gridlines), the standard "nice numbers"
   approach — avoids ugly steps like drawing gridlines every 0.137
   units just because that's `span / 4`.
3. **Label decimal places derived from the step itself**, not a fixed
   `.3` like today's min/max labels used unconditionally (visibly
   wrong for e.g. volume or temperature, which read more naturally to
   1 decimal). A step of `1` → 0 decimals, `0.1` → 1, `0.01` → 2, and
   so on — keeps `charts.rs` metric-agnostic (it still never learns
   which of the five metrics it's drawing) while still labeling each
   chart at a precision that matches its own data's scale.
4. **Light gray, full-width, drawn first** — same visual treatment and
   layering already established for every other gridline in this app
   (`#ddd6c8`, behind the data), per the todo's explicit "light gray
   and not too intrusive."

## Implementation

- `src/charts.rs`, `render_line_chart`: replace the two hardcoded
  min/max `<text>` labels with a loop over `y_ticks(y_min, y_max)`,
  each drawing a horizontal `<line>` across the plot width plus a
  label, mirroring how `day_ticks` already draws the x-axis's vertical
  gridlines.
- `y_ticks`/`nice_step`: small numeric helpers, no chrono/date
  involvement (unlike every other tick helper in this file) — a
  "nice numbers" step-size function is a common, short algorithm
  (round `span / target_count` to the nearest 1/2/5 × 10ⁿ), not
  something worth pulling in a crate for.

## Edge cases

- **All values identical** (e.g. a single reading, or several
  identical readings): `value_span` already gets a synthetic pad in
  this case (existing `value_pad` logic) — `y_ticks` runs on the
  padded range like everything else, so it still produces a sensible
  small set of ticks rather than one.
- **Very small span** (e.g. pH readings all within 0.05 of each
  other): the nice-step algorithm naturally picks a small step (down
  to whatever power of 10 fits), so gridlines stay meaningful rather
  than collapsing to one line or exploding to hundreds.

## Out of scope

- The Season overview (scope note above).
- Changing the x-axis day-tick gridlines — unaffected, already shipped
  in [[plot-days-since-juicing]].
