# Spec: Mark batches as failed

Status: Planning — ready for review

## Problem

A batch can fail (infection, off flavors, whatever) partway through.
There's currently no way to record that — it would just sit showing
whatever its last real lifecycle status was (`fermenting`,
`conditioning`, ...) forever, which is misleading.

## Decisions

1. **Failure is an event, not a stored flag.** `status` was
   deliberately made a *computed* property of the event log (see the
   design note in `vision.md` §2/§4) specifically so it can't drift
   out of sync with what actually happened. A bare `batches.failed
   BOOLEAN` column would reintroduce exactly the kind of
   separately-editable fact that decision was meant to avoid. A
   `failed` event type fits the existing pattern instead: it has a
   date (when it was noticed/declared) and can carry a note (why).
2. **No dedicated typed table.** A failure event's only meaningful
   payload is "why," which the shared `events.notes` column already
   provides (same reasoning as `note_events`, which is nearly this
   minimal already). Adding `failed` to `events.event_type`'s allowed
   values is enough; no `failure_events` table needed.
3. **Failure is a terminal, overriding status.** If a batch has any
   `failed` event, `compute_status` returns `"failed"` regardless of
   what other events exist before or after it — a fermenting batch
   that failed shouldn't still read as "fermenting." This means
   `compute_status`'s priority order becomes: `failed` > `bottled` >
   `conditioning` > `fermenting` > `planning`.
4. **Doesn't block further logging.** Marking a batch failed doesn't
   prevent adding more events to it (e.g. a final measurement taken to
   confirm the problem, or a closing note). This is a simplifying
   choice, not a strong requirement — flag if the opposite is wanted.
5. **UI placement: batch-level action, not a "Log X" card.** This is
   conceptually closer to Delete (a batch-lifecycle action) than to
   routine logging (picking/measuring/etc.), so it belongs on the
   `_meta.html` card near the Delete button, not as a seventh
   `<details>` card in the routine logging list.

## Data model

New migration (adds a value to an existing `CHECK` constraint on
`events.event_type`). As established in the picking/juicing merge
migration, SQLite can't `ALTER` a `CHECK` constraint without a full
table rebuild, and `events` has many dependent child tables via FK —
the same trade-off applies again here. Two real options:

- **(a)** Do the full rebuild properly this time (create
  `events_new` with the updated constraint, copy rows, drop old, rename,
  recreate indexes, with `PRAGMA foreign_keys=OFF` toggled outside any
  transaction around the copy). More correct, more risk, more testing
  needed against a copy of the production DB first.
- **(b)** Skip the constraint change, same as the precedent already
  set for `'picking'` remaining an unused-but-allowed value. The
  application never needs the database to reject an invalid
  `event_type` it would never generate itself.

Recommendation: **(b)**, consistent with the existing precedent, unless
there's a specific reason to finally do the rebuild (in which case it's
worth doing once for both the leftover `'picking'` cleanup and this
addition together, not twice).

```sql
-- No schema change under option (b) beyond what compute_status needs
-- to know about — 'failed' just becomes a value the app writes.
```

## Backend

- `compute_status`: add a `has_failed` flag, checked first, returning
  `"failed"` before any other branch.
- New handler `add_failure(batch_id, occurred_at, notes)`: inserts a
  single `events` row with `event_type = 'failed'`. No typed-table
  insert.
- `_meta.html`'s existing PUT-based batch update is unaffected; this is
  a new, separate POST endpoint (`POST /batches/{id}/events/failure`),
  matching the shape of every other "add event" handler minus the
  typed-table half.
- Timeline formatting: a `failed` row's summary is just its notes (or
  "Batch marked as failed" if no notes given), `kind_label: "Failed"`.

## Frontend

- `_meta.html`: a small form near the Delete button — a date input, an
  optional reason text field, and a "Mark as failed" button, styled
  distinctly (danger-adjacent, like the Delete button) since it's a
  significant, rare action, not routine logging.
- `.status` badge styling: give `"failed"` its own color (the existing
  `.status` class is a flat neutral badge for every value today —
  worth a dedicated red/warning treatment here specifically, similar
  to how `.danger` buttons already read differently from primary ones).

## Edge cases

- **Un-failing a batch**: no explicit "undo" — since there's no
  delete-event feature yet (only add and edit exist), the only way to
  reverse a mistaken failure mark today would be editing that event's
  date into the future/deleting it via direct DB access. Worth noting
  as a gap; likely low priority for a rare, deliberate action, but flag
  for confirmation.
- **Multiple failure events on one batch**: allowed (no uniqueness
  constraint) — harmless, since status only checks *existence*, not
  count. The timeline would just show every failure note logged.

## Out of scope

- Preventing new events from being logged after failure (see decision
  4 — deliberately not doing this).
- A "resolved"/un-failed follow-up event type. Not requested.
