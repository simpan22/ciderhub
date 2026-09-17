# Spec: Month gridlines on the Season overview timeline

Status: Planning — ready for review

## Problem

[[dashboard-timeline]]'s Season overview chart has an x-axis labeled
only at its two extremes (earliest start, latest end) — there's no way
to tell how far apart two dates on the chart actually are without
guessing.

## Decisions

1. **Vertical gridline at the 1st of every month the axis spans**,
   drawn from the top of the plot area down through the bar rows to
   the x-axis — same shared axis every row already sits on, so one set
   of gridlines works for the whole chart.
2. **Label each line with the month's first three letters** (`Jan`,
   `Feb`, ...), per the todo's own wording. Placed just above the axis
   line, same position the existing start/end date labels use.
3. **Light gray lines, darker gray labels** — a deliberately lower
   visual weight than the axis line itself (`#8899aa`) and the phase
   bars, so gridlines read as background structure, not data. Reuses
   the muted text color (`#746f66`, already used for the axis's
   date labels) for the labels; a lighter gray (`#ddd6c8`, matching
   the app's warm-neutral palette rather than a cold gray) for the
   lines themselves.
4. **This chart only** — the per-metric line charts
   (`render_line_chart`) aren't in scope; nothing in the todo mentions
   them, and their x-axis already reads fine at hobby-scale point
   counts (few enough points that the first/last date labels are
   sufficient context).

## Implementation

- `src/charts.rs`, `render_season_timeline`: compute every month
  boundary (`chrono::NaiveDate` at day 1) falling within
  `[axis_min, axis_max]`, using `checked_add_months(Months::new(1))`
  to step forward from the axis start's month. For each, draw a
  `<line>` from `MARGIN_TOP` to `axis_y` at that day's x-coordinate,
  and a `<text>` label above `axis_y` (reusing the same y position as
  the existing start/end date labels, since gridline labels replace
  the need to also show those — see edge ccase below).
- A small 3-letter month-name array local to `charts.rs` (`["Jan",
  "Feb", ...]`) — distinct from `dates::MONTH_NAMES` (full names, used
  for form `<select>` options), not worth sharing between two
  different display formats.

## Edge cases

- **Very short axis span** (a handful of days, e.g. a single
  just-started batch): at most one or two month boundaries fall in
  range, possibly zero if the whole span sits inside one month — the
  existing start/end date labels already cover that case, so no
  gridlines is a fine outcome, not a bug.
- **Gridline lands very close to the existing start/end date label**:
  acceptable minor visual overlap at hobby-scale data volumes; not
  worth adding label-collision avoidance for this.

## Out of scope

- Week or day gridlines — the todo asks for months only.
- Making gridline density adaptive to axis span (e.g. switching to
  year lines for a multi-year view) — this chart is scoped to one
  season at a time, so spans are never more than ~1 year wide.
