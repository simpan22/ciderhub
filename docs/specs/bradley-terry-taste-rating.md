# Spec: Comparative taste rating (Bradley-Terry)

Status: Done — deployed 2026-10-14 (production database rebuilt without the event_type CHECK first)

## Problem

The 0–10 tasting score ([[tasting-event]]) asks for an absolute
judgement, which is hard to give consistently: a "7" in March isn't
necessarily the same "7" in October. Judging two ciders side by side
is much easier ("A is clearly better than B"). The todo asks for a
per-batch taste score derived from those pairwise judgements via the
Bradley-Terry model, **separate from** the existing tasting score, with
a new event type for logging comparisons.

## Decisions

1. **New event type, `comparison`, with its own typed table.** It is
   the first event that involves *two* batches, while the `events`
   spine has a single `batch_id`. The spine row belongs to the batch
   the comparison was logged from, and the typed row references the
   other batch:

   - `events.batch_id` = "this" batch (the page it was logged on)
   - `comparison_events.other_batch_id` = the batch it was compared to
   - `comparison_events.outcome` ∈ `'win' | 'loss' | 'tie'`, from
     "this" batch's point of view

   The comparison shows on **both** batches' timelines. On the other
   batch's page it's rendered from that batch's point of view (outcome
   flipped). Storing the comparison once instead of as two mirrored
   rows means the two sides can never disagree.

2. **Ties are allowed** ("can't tell them apart" is a real outcome),
   and count as half a win for each side. That's the standard simple
   way to handle ties in Bradley-Terry. The heavier Davidson tie
   model isn't worth it at this data volume.

3. **The rating is computed, not stored.** Like `status`, start date
   and ABV, it's derived from the event log on each request, so it
   can't drift out of sync. With tens of batches and maybe hundreds of
   comparisons, fitting takes microseconds.

4. **One global fit across all seasons.** A 2026 batch compared with
   a 2025 batch is a valid data point, and a global fit lets ratings
   from different seasons sit on the same scale. Filtering by season
   is out of scope (see below).

5. **The fit is regularised with one virtual tie per batch.** Plain
   maximum-likelihood Bradley-Terry fails in two cases that are
   *normal* here:
   - a batch that has won every comparison gets an infinite strength;
   - batches in groups that have never been compared with each other
     have no common scale.

   The fix: every batch that has at least one comparison also gets one
   virtual **tie against a fixed "average batch"** with strength 1.
   This keeps every strength finite and ties all components to the
   same reference. It also pulls lightly-compared batches toward
   average, which is a sensible default given little evidence. The
   effect fades as real comparisons accumulate.

6. **Displayed as an Elo-style rating,**
   `rating = 1500 + 400 · log10(strength)`. An average batch is 1500,
   and a 400-point gap means roughly 10:1 odds of being preferred. It
   is deliberately on a different scale from the 0–10 score so the two
   can't be confused. (Settled in review: Elo-style, not "win %
   against an average batch".)

7. **Batches with no comparisons have no rating** (`—`), not 1500.
   The prior would put them at 1500, but showing that would claim
   knowledge we don't have.

8. **The "Log comparison" card shows only when the batch is
   `bottled`**, and the opponent dropdown lists only other `bottled`
   batches. This is the same gate as the tasting card (settled in
   review). Because `failed` overrides `bottled` in `compute_status`,
   a batch that was bottled and later marked failed drops out of the
   dropdown. Its existing comparisons still count towards the ratings.

## Data model

### Event-type CHECK constraint (needs an out-of-band script)

`events.event_type` has a `CHECK (event_type IN (...))` that doesn't
include `'comparison'`. As documented in `deploy/README.md` ("Schema
changes that need `PRAGMA foreign_keys = OFF`"), changing it requires
a table rebuild, and that **cannot** be a sqlx migration without
cascading data loss. It has to be a standalone script based on
`deploy/fix_events_check_constraint.py`.

**Decision (settled in review): use this rebuild to drop the
`event_type` CHECK entirely** instead of adding one more value to it.
Each new event type has needed this risky manual step, while the only
writer is this app, using string literals it controls. Future event
types then need only a normal migration for their typed table.

New script: `deploy/rebuild_events_drop_type_check.py`. It follows the
template exactly: `foreign_keys = OFF` before `BEGIN`, then
create-copy-drop-rename, recreate `idx_events_batch_occurred`, run
`PRAGMA foreign_key_check`, and compare row counts per table. It must
be idempotent: if the CHECK is already gone, it does nothing.

> Pre-existing issue found while planning (not fixed here): a
> **fresh** database built only from `migrations/` still has the
> original CHECK (`'picking'`, no `'failed'`/`'tasting'`). Logging a
> failure or tasting on a fresh install fails today, and so will a
> comparison. Worth its own todo entry.

### Migration (a normal sqlx migration)

```sql
CREATE TABLE comparison_events (
    event_id       INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    other_batch_id INTEGER NOT NULL REFERENCES batches (id) ON DELETE CASCADE,
    outcome        TEXT NOT NULL CHECK (outcome IN ('win', 'loss', 'tie'))
);

CREATE INDEX idx_comparison_events_other_batch ON comparison_events (other_batch_id);
```

`other_batch_id <> events.batch_id` can't be expressed as a CHECK,
because it spans two tables, so the handler enforces it.

**Deleting a batch:** if the deleted batch is the `other_batch_id`
side, the FK cascade removes the `comparison_events` row but leaves its
`events` spine row behind on the surviving batch, with no typed row.
The timeline wouldn't show it (timeline queries join from the typed
tables), but it would still appear in `fetch_event_types` and the
list's event scan. `batches::delete` should therefore, in one
transaction, first delete
`events WHERE id IN (SELECT event_id FROM comparison_events WHERE other_batch_id = ?)`
and then the batch.

## Backend

### `src/ranking.rs` (new, pure, unit-tested)

```rust
pub enum Outcome { Win, Loss, Tie }
pub struct Comparison { pub a: i64, pub b: i64, pub outcome: Outcome } // outcome from a's view
pub struct Rating { pub rating: f64, pub comparisons: u32 }

pub fn fit(comparisons: &[Comparison]) -> HashMap<i64, Rating>
```

Algorithm: Hunter's (2004) MM / Zermelo iteration, with the virtual
average opponent fixed at strength `p₀ = 1`:

```
W_i   = wins_i + 0.5·ties_i + 0.5          (the +0.5 is the virtual tie)
p_i  ← W_i / ( Σ_j n_ij/(p_i+p_j) + 1/(p_i+p₀) )
```

Repeat until the largest relative change is below `1e-9`, or stop
after 10 000 iterations. No renormalisation is needed, because `p₀` is
fixed and anchors the scale. Convert with `1500 + 400·log10(p_i)`.

These are the first tests in the repo (`#[cfg(test)] mod tests`):
- one comparison, A beats B → A > 1500 > B, both finite
- A wins every comparison → finite rating
- an A/B pair and a separate C/D pair → both on the same scale; A
  and C (one win each) get equal ratings
- a tie-only history → everyone at 1500
- a symmetric cycle (A>B, B>C, C>A) → all equal
- known values: hand-check a 3-batch case against a reference
  computation (e.g. a few lines of Python), within a 1e-6 tolerance
- swapping a/b and flipping the outcome gives the same result

### `src/handlers/batches.rs`

- `fetch_comparative_ratings(db) -> HashMap<i64, Rating>`: one query
  over `comparison_events ⋈ events`, then `ranking::fit`.
- `add_comparison(batch_id, occurred_at, other_batch_id, outcome, notes)`:
  the standard transactional add-event shape. Return 422 if
  `other_batch_id == batch_id` or the batch doesn't exist.
- `edit_comparison_fragment` / `update_comparison`: same shape as the
  other edit pairs, but match the event with
  `(e.batch_id = ? OR ce.other_batch_id = ?)` so it can be edited from
  either batch's page. The form always shows and submits the outcome
  from the *viewing* batch's point of view. On save, the comparison is
  re-stored from that side: the viewing batch becomes the spine batch
  (`events.batch_id`) and the submitted batch becomes
  `other_batch_id`. This lets the other batch be changed from either
  page, which matters because the app has no way to delete an event.
  (Changed during implementation: the first draft didn't allow
  changing the opponent from the other side.)
- `fetch_timeline`: include comparisons where the batch is on either
  side. Summary from the viewing batch's point of view:
  `"Preferred over 2026-B"` / `"2026-B preferred"` /
  `"Tied with 2026-B"`, plus the usual `" — {notes}"`. Link the
  opponent code to its batch page. `kind_label: "Comparison"`,
  `kind_slug: "comparison"`.
- `list()`: call `fetch_comparative_ratings` and add
  `comparative_rating: Option<i64>` (rounded) to `BatchListItem`.
- `fetch_meta` / `_meta.html`: show
  `Comparative rating 1563 (#2 of 7 rated, 5 comparisons)`.
- `src/main.rs`: three routes:
  `POST /batches/{id}/events/comparison`,
  `GET /batches/{batch_id}/events/comparison/{event_id}/edit` and
  `PUT /batches/{batch_id}/events/comparison/{event_id}`, matching the
  tasting routes' methods.
- `src/models.rs`: `AddComparisonForm { occurred_at, other_batch_id, outcome, notes }`,
  used for both add and edit.
- Add and edit both respond with the timeline plus the meta card as an
  out-of-band swap, because the rating shown in the meta card changes
  too (the mirror image of `add_failure`'s response).

## Frontend

- `templates/batches/detail.html`: a new `<details class="card"
  id="log-comparison">` card next to "Log tasting", with the same
  status gate and the same `ciderhubAfterEventSubmit` behaviour. Like
  measurements, it **stays open after submit**, since comparisons tend
  to be logged several in a row during one tasting session (see
  [[streamlined-event-forms]]). Fields:
  - Date (same date input/prefill as the other cards)
  - "Compared with": `<select>` of other eligible batches, labelled
    by code (and season if there's more than one)
  - Outcome: three radios, "This batch was better" / "About the same"
    / "{other} was better"
  - Notes
- `templates/batches/_edit_comparison.html`: mirrors `_edit_tasting.html`.
- `templates/batches/_timeline.html`: render the opponent link in the
  summary.
- `templates/batches/list.html`: a new "Rating" column next to "Taste
  score", shown as `—` when there's no rating.

## Rollout order (important, because CI deploys on every push to `main`)

1. Write the rebuild script. Test it on a copy of the production DB
   and diff row counts for every table.
2. On the server: stop the service, back up the DB, run the script,
   start the service. The old binary works fine without the CHECK.
3. Only then merge the feature, including the new migration and the
   regenerated `.sqlx/`
   (`DATABASE_URL=sqlite://ciderhub.db cargo sqlx prepare`), so CI
   deploys it.

If step 3 lands before step 2, the app starts fine, but every "Log
comparison" submit fails the CHECK in production.

## Edge cases

- **The same pair compared several times**: expected and useful, since
  each one adds evidence. Disagreements (A>B one day, B>A another)
  just pull the two closer together.
- **Comparing with yourself**: rejected by the handler and excluded
  from the dropdown.
- **Taste changes as cider conditions**: an old comparison counts as
  much as a new one. This is accepted for now (see Out of scope).
- **Only one batch eligible**: hide the card, since there's nothing to
  compare against.
- **Deleting the other batch**: see "Deleting a batch" above.

## Out of scope

- Time-weighting or decay of older comparisons.
- Per-season ratings or season filtering.
- Plotting the rating on the Overview / statistics charts. It's a
  single snapshot value, not a time series, so a "rating over time"
  chart would mean refitting at each point in time. That can be a
  follow-up.
- Comparing more than two ciders at once (ranking, Plackett-Luce).
- Uncertainty bands or confidence intervals on the rating.
- A dedicated "tasting session" page for logging many pairs at once.
  The card staying open covers most of that need.
