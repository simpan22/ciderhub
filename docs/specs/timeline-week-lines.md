# Spec: Weekly date gridlines on the Season overview timeline

Status: Planning — ready for review

## Problem

[[dashboard-timeline]]'s Season overview chart only labels its x-axis
at the two data extremes (earliest start, latest end) plus, since
[[timeline-month-lines]], month boundaries labeled `Jan`/`Feb`/etc.
Month granularity isn't fine enough to tell "which date is this bar
segment actually at" without counting pixels.

## Decisions

1. **Replaces the month gridlines, doesn't sit alongside them.** A
   weekly gridline labeled with an actual date (`Sep 10`) already
   tells you the month — keeping a separate, coarser `Jan`/`Feb`
   gridline system next to it would just be two overlapping label
   systems competing for the same strip of chart. One system, doing
   the more useful job, replaces the other. Flag during review if both
   were actually wanted simultaneously.
2. **Gridlines snap to Monday**, not to the axis's start date — same
   "snap to a real calendar boundary" reasoning `timeline-month-lines`
   used for the 1st of the month, so the grid reads as actual weeks,
   not an arbitrary offset from wherever the data happens to begin.
3. **Adaptive spacing, not always literally every 7 days.** A real
   cider season can span several months; at strict weekly spacing that
   means 15-30+ gridlines packed into a 640px-wide chart, with date
   labels overlapping into illegible noise — the opposite of "make it
   easier to see which dates are where." Same fixed-threshold approach
   already used for the metric charts' day ticks, scaled to weeks:
   span ≤ 8 weeks → every week, ≤16 weeks → every 2 weeks, ≤32 weeks →
   every 4 weeks, longer → every 8 weeks.
4. **Same visual treatment already established for gridlines on this
   chart** — light gray line (`#ddd6c8`), darker gray label
   (`#746f66`), full height from the plot's top down to the axis line.
   Nothing new to design here, just a different set of x-positions and
   label text.
5. **Label format: `Sep 10`** (short month + day, no year) — the
   Season overview is always scoped to one season/year already (picked
   from the dropdown above the chart), so a year in every gridline
   label would be redundant.

## Implementation

- `src/charts.rs`: replace `month_gridlines`/`MONTH_ABBREVS` (now
  unused once this lands) with `week_gridlines(axis_min, axis_max) ->
  Vec<(i64, String)>` — finds the first Monday on/after `axis_min`,
  then steps forward by the chosen interval (decision 3) using
  `chrono::Duration`, formatting each date as `%b %-d`. Same call site
  in `render_season_timeline`, same drawing code — only the tick
  source and label text change.

## Edge cases

- **Very short axis span** (a handful of days): same outcome as
  today's month gridlines in that situation — zero or one gridline is
  a fine result, not a bug; the existing start/end date labels already
  cover it.
- **A season spanning nearly a full year**: falls into the "every 8
  weeks" bucket — coarser than weekly, but still finer and more
  directly useful (actual dates) than the month labels it replaces.

## Out of scope

- The per-metric line charts (`render_line_chart`) — unaffected, they
  already have their own days-since-juicing tick system
  ([[plot-days-since-juicing]]).
