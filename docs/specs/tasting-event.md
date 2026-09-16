# Spec: Tasting event

Status: Planning — ready for review

## Problem

Once bottled, there's no way to record how the cider actually turned
out — a rating and tasting notes over time (you might taste the same
batch again a month later as it conditions in the bottle).

## Decisions

1. **New event type, `tasting`**, with its own typed table — unlike
   the failure event, this one has real structured data (a numeric
   score) worth its own column for querying/charting later (see
   `batch-statistics.md`, which plots score over time).
2. **Score is an integer 0–10**, per the todo's literal wording. Not
   spec'ing half-points unless asked.
3. **Reuses the shared `events.notes` for tasting notes**, rather than
   adding a second notes-like column — this is exactly the same
   pattern `measurement_events.tasting_notes` uses today, so for
   consistency the free-text description of *how it tasted* should
   probably live in a dedicated `tasting_events.notes`-equivalent
   field distinct from the shared `events.notes` (which is meant for
   "notes about the event itself," e.g. "tasted straight from the
   bottle vs. decanted" — a logistics note, not a tasting-notes note).
   Concretely: `tasting_events.tasting_notes` (the flavor description)
   *and* the shared `events.notes` (everything else) both exist, same
   shape as `measurement_events`.
4. **Only shown once bottled.** The "Log tasting" card only appears on
   the batch detail page when `compute_status` is `"bottled"` (or
   `"failed"`, arguably — tasting a failed batch to confirm the
   problem is a real use case; worth confirming during review). Before
   that, there's nothing to taste yet.

## Data model

New migration:

```sql
CREATE TABLE tasting_events (
    event_id      INTEGER PRIMARY KEY REFERENCES events (id) ON DELETE CASCADE,
    score         INTEGER NOT NULL CHECK (score BETWEEN 0 AND 10),
    tasting_notes TEXT
);
```

`'tasting'` also needs adding to `events.event_type`'s `CHECK`
constraint — same trade-off as `batch-failure.md`: either do the full
table rebuild (and bundle it with the other two pending additions,
`'failed'` and this, if the rebuild is ever done) or leave it as
another unenforced-but-harmless value, consistent with the existing
`'picking'` precedent.

## Backend

- `add_tasting(batch_id, occurred_at, score, tasting_notes, notes)`:
  standard add-event shape, transactionally inserting into `events`
  then `tasting_events`.
- `edit_tasting_fragment` / `update_tasting`: same shape as every other
  editable event type added in the recent editing feature
  (`_edit_tasting.html`, pre-filled).
- Timeline summary: `"Scored {score}/10"` + `", {tasting_notes}"` if
  present + the usual `" — {notes}"` suffix if the shared notes field
  is set.
- `detail()` handler: pass whether to render the tasting card down to
  the template (`show_tasting_form: bool`, `batch.status == "bottled"`
  — reuses the status already computed for `_meta.html`).

## Frontend

- `templates/batches/detail.html`: new `<details>` card, "Log
  tasting," gated by `{% if batch.status == "bottled" %}` (matching
  the string-equality pattern already used elsewhere for status, e.g.
  in the seasons/status option rendering). Fields: date, score
  (`<input type="number" min="0" max="10" step="1">`), tasting notes,
  notes.
- `templates/batches/_edit_tasting.html`: mirrors the other five edit
  fragments.

## Edge cases

- **Score at the boundary (0 or 10)**: valid, not an error — `CHECK
  (score BETWEEN 0 AND 10)` is inclusive both ends.
- **Multiple tastings per batch**: expected and encouraged (taste
  again as it conditions) — no uniqueness constraint, timeline just
  shows each one chronologically.
- **Tasting a batch that isn't bottled**: the form is hidden by the
  status gate, but nothing stops direct API access from creating one
  anyway (same as every other event type — this app has no
  batch-lifecycle enforcement at the data layer, only at the UI). Not
  worth hardening for a single-user hobby tool unless that changes.

## Out of scope

- Structured tasting attributes (sweetness, tannin, carbonation as
  separate scored dimensions) — the todo asks for one overall score
  plus free text, not a multi-axis rubric.
