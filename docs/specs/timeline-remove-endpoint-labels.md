# Spec: Remove the Season overview's start/end date labels

Status: Planning — ready for review

## Problem

`render_season_timeline` draws two exact-date labels at the plot's
far left/right edges (the earliest start and latest end among the
season's batches), from before [[timeline-week-lines]] added proper
weekly date gridlines across the whole axis. Now that every ~1-8 weeks
already has its own date label, these two extra ones are redundant and
can visually collide with the nearest weekly gridline's label when
they land close together (nothing prevents a batch's exact start/end
from sitting a day or two from a Monday gridline).

## Decision

Remove them outright — the weekly gridlines already give date context
along the whole axis, including near the actual start/end, so nothing
is lost, and it resolves the label-collision risk instead of working
around it.

## Implementation

- `src/charts.rs`, `render_season_timeline`: delete the
  `earliest_label`/`latest_label` tracking and the two `<text>`
  elements they feed at the end of the function. `min_day`/`max_day`
  (used for the axis padding math) are untouched — only the two label
  strings and their rendering go.
