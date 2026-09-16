# Spec: Batch start date is implicit, not asked for

Status: Planning — ready for review

## Problem

The "New batch" form currently asks for a `started_on` date
(pre-filled to today, but editable), stored as its own column. This is
exactly the kind of separately-editable fact that can drift from
reality — the batch's real start is whenever its first event actually
happened, which is already recorded. The todo asks to stop asking for
it and derive it instead, mirroring the reasoning already applied to
`status` (computed from events, not stored).

## Decisions

1. **Drop `batches.started_on` entirely**, don't just stop asking for
   it in the form. Keeping an unused column that nothing writes to
   invites exactly the drift this change is meant to prevent (someone
   could still set it via direct DB access and it would then silently
   disagree with the real first-event date forever).
2. **Computed as `MIN(events.occurred_at)` for that batch.** `None`
   until the batch has at least one event — a brand new batch has no
   "started on" yet, which is correct: it hasn't started anything.
3. **Not used for sorting.** The batches list was recently changed to
   sort alphabetically by `code` (a separate, deliberate change) — this
   spec doesn't reintroduce date-based sorting. If chronological
   sorting is wanted later, that's a separate decision.

## Data model

New migration:

```sql
ALTER TABLE batches DROP COLUMN started_on;
```

## Backend

- `NewBatchForm`: remove `started_on`.
- `create()`: drop `started_on` from the `INSERT`.
- `fetch_meta`/`BatchDetail`: add `started_on: Option<String>`,
  populated via a new query (or folded into the existing meta query
  with a scalar subquery):
  ```sql
  SELECT MIN(occurred_at) FROM events WHERE batch_id = ?
  ```
- `list()`/`BatchListItem`: same computed value, if it's to be shown
  on the batches list too (see Frontend below) — reuse the
  batch→event-types grouping query's shape (one query for all batches'
  min dates, grouped in memory, rather than one query per row) the
  same way `compute_status` already avoids N+1 queries in `list()`.

## Frontend

- `templates/batches/list.html`: remove the "Started on" field from
  the New batch form entirely.
- `templates/batches/_meta.html`: display the computed value —
  `{% if let Some(started) = batch.started_on %}Started {{ started
  }}{% else %}Not started yet{% endif %}` — somewhere in the existing
  muted status line.
- Batches list table: optionally add a "Started" column showing the
  same computed value, if useful there too (not required by the todo,
  but cheap given the data's already being fetched for the detail
  page's equivalent — worth a quick decision during review rather than
  guessing).

## Edge cases

- **Batch with zero events**: `started_on` is `None`; the UI must
  handle this (a freshly created batch, before its first "Log
  juicing," previously would have shown *today's date* by default,
  which is arguably more misleading than an honest "not started yet").
- **Editing an event's date to be earlier than the current minimum**:
  automatically reflected next render, since it's a live `MIN(...)`
  query, not a cached/stored value — this is the entire point of
  computing it instead of storing it.

## Out of scope

- Anything for "ended on" / batch completion date — not requested,
  and `status` already communicates the terminal states
  (`bottled`/`failed`) without needing a separate date.
