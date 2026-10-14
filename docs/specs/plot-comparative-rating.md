# Spec: Plot the comparative rating on the Overview page

Status: Done — deployed 2026-10-14

## Problem

The Bradley-Terry rating ([[bradley-terry-taste-rating]]) shows only
as a single current number, on the batch list and the batch page. It
can't be selected as a metric on the Overview page, so there's no way
to see how a batch's rating developed or to compare batches' ratings
side by side.

## Decisions

1. **A sixth metric checkbox, "Comparative Rating,"** next to "Taste
   Score," using the existing chart: one line per selected batch, with
   days since juicing on the x axis. There's no new chart type and no
   change to how batches and metrics are selected.

2. **Each point is the rating as it stood right after that day's
   comparisons.** A rating is a fit over *all* comparisons, so there
   is no stored per-event value to plot the way `score` is. For each
   date on which a batch was compared, the chart refits using every
   comparison up to and including that date, across all batches, and
   plots that batch's rating.
   - A batch gets a point on dates **it** was compared. It isn't
     replotted every time two *other* batches are compared, even though
     that moves its rating slightly. Otherwise a batch's line would
     gain dots on days nothing happened to it.
   - **One closing point** (changed during implementation): those
     other comparisons mean a batch's rating after its own last
     comparison can differ from its current rating (by about 2.5
     points in a test case). When it does, the line gets one extra
     final point at the date of the latest comparison overall, with
     the tooltip "after comparisons between other batches". The first
     draft of this spec claimed the line always ended at the current
     rating without this, which was wrong.
   - So the last point of each line always equals the rating on the
     batch list and batch page (the same fit over the same
     comparisons).
   - Earlier points don't change when later comparisons are logged.
     Each point shows what was known at that date, not a re-evaluation
     with hindsight.

3. **Tooltip:** `2026-10-12 (Day 40): Rating 1631 — better than 2026-X`.
   It uses the same "date (Day N): value" shape as the other metrics,
   followed by that day's comparisons for this batch from its own point
   of view ("better than" / "worse than" / "tied with"), separated by
   `; ` if there were several. Comparison notes are left out to keep
   the tooltip short; they're on the batch timeline.

4. **Same exclusions as every other metric:** a batch with no juicing
   event has no day 0, so it doesn't appear (see
   `plot-days-since-juicing.md`). A selected batch that has never been
   compared draws no line. If no selected batch has any, the existing
   "No data for the selected batches." message shows.

## Backend

- `src/ranking.rs`: new pure function

  ```rust
  /// For each date (ascending) on which a batch was compared, its
  /// rating from a fit over every comparison up to and including it,
  /// plus the closing entry described above (`compared: false`).
  pub fn history(comparisons: &[DatedComparison]) -> HashMap<i64, Vec<HistoryEntry>>
  ```

  `DatedComparison` is `Comparison` plus `occurred_at`. It refits once
  per distinct comparison date, which is a few dozen fits of a few
  dozen batches and takes microseconds. New unit tests:
  - the last entry per batch matches `fit` over all comparisons;
  - a batch has entries on dates it was compared, plus at most one
    closing entry;
  - later comparisons don't alter earlier entries.
- `src/handlers/dashboard.rs`:
  - `METRICS` gains `("rating", "Comparative Rating")`.
  - `build_charts` handles `rating` separately from the per-batch
    `fetch_measurement_series` loop. Rating points depend on every
    batch's comparisons, not just the selected batch's, so they're
    loaded with one query (comparisons ⋈ events ⋈ both batch codes)
    and passed through `ranking::history` once per request, only when
    the metric is selected. Each point is then placed on the same
    days-since-juicing axis and becomes a `SeriesPoint`.
  - `format_metric_value`: `"rating" => format!("Rating {value:.0}")`.
- No template, schema, or migration changes. `index.html` already
  renders the metric checkboxes from `METRICS`.

## Edge cases

- **Several comparisons on the same day:** one point after all of
  them, and the tooltip lists each.
- **Editing a comparison's date or outcome:** the whole history is
  recomputed on every request, so the chart always matches the
  current log.
- **Y axis:** `render_line_chart` already scales to the data's range.
  Ratings bunch around 1500 with only a few comparisons, so the axis
  zooms in on small differences. That's the same behaviour as the
  other metrics, and the tooltip shows the exact value.

## Out of scope

- A snapshot chart of every batch's current rating (bars sorted by
  rating). The batch list already shows current ratings. Could be a
  follow-up if a visual ranking is wanted.
- A reference line at 1500 (average).
- Uncertainty bands.
