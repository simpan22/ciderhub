# Spec: Batch code auto-suggestion

Status: Planning — ready for review

## Naming note

The todo says "batch ids should be prefilled with `<year>-<letter>`."
This app has no user-facing "batch id" field — the closest match is
`batches.code` (e.g. `2026-A`), which is exactly the format described.
This spec treats "batch id" in the todo as `code`. Flag during review
if something else was actually meant.

## Problem

Today, `code` is a plain required text field on the "New batch" form
with no suggested value — the user always types the full code by hand
(`2026-A`, `2026-B`, ...), including remembering which letters are
already used for the season.

## Decisions

1. **Suggest, don't enforce.** The `code` field stays a normal editable
   text input; it's just pre-filled with the next likely value
   (`<year>-<next unused letter>`), same spirit as every other
   "sensible default, fully overridable" field in this app (dates
   defaulting to today, etc.).
2. **Per-season, not global.** "Next letter" means next unused letter
   *for that season* — `2026-A`/`2026-B` and `2027-A` can coexist
   without conflict, since the letter only needs to be unique within a
   year.
3. **Live-updates when the season selection changes.** The suggested
   code depends on which season is picked in the adjacent dropdown, and
   this app has no page-load-time way to know which season the user
   will pick. An htmx round-trip on the season `<select>`'s `change`
   event (`hx-get` to a small endpoint that returns just the next
   suggested code, `hx-target` the code input's `value`) keeps the
   suggestion correct without introducing hand-written JS or a new
   library — consistent with how every other dynamic-without-full-page-
   reload behavior in this app is already done.
4. **No wraparound past Z.** After `2026-Z`, there's no auto-suggestion
   (leave the field blank rather than guessing `2026-AA` or similar) —
   at hobby scale, 26 batches in one season is far beyond realistic, so
   this is a deliberately unhandled edge rather than added complexity
   for a case that won't occur.

## Backend

- `suggest_next_code(db, season_id) -> Option<String>`:
  1. `SELECT code FROM batches WHERE season_id = ?`.
  2. For each existing code matching the pattern `<year>-<single
     letter>` (the season's own year, from `seasons.year`), collect the
     used letters.
  3. Return `<year>-<first unused letter A..Z>`, or `None` if all 26
     are used or the season has no year context somehow.
  4. Codes that don't match the `<year>-<letter>` pattern (a batch
     manually named something else entirely) are ignored for the
     purposes of finding the next letter, not treated as errors.
- `GET /batches/suggest-code?season_id=1` → returns the suggested code
  as plain text (or a tiny fragment setting just the input's value),
  used both by the initial page render (seasons list's first/default
  entry) and the season-select's `change` handler.
- `list()` (the batches list handler, which renders the New batch
  form): computes the suggestion for whichever season is selected by
  default (the first option — seasons are already ordered `year DESC`,
  so this is the most recent season) and pre-fills the form's `code`
  input with it on first render.

## Frontend

- `templates/batches/list.html`: the season `<select>` gains
  `hx-get="/batches/suggest-code" hx-trigger="change" hx-target="#code-input" hx-swap="outerHTML"`
  (or a narrower swap of just the value, if htmx's `hx-target` on an
  `<input>` with a text response needs a wrapper — the exact swap
  mechanics are an implementation detail, not a spec-level decision).
  The `code` input gets `id="code-input"` and its initial `value` set
  to the server-computed default-season suggestion.

## Edge cases

- **User already typed a custom code, then changes season**: the
  live-update would overwrite their typing. Given this is a rare
  sequence (change season *after* already customizing the code) and
  the field remains freely editable afterward, this is accepted as a
  minor rough edge rather than adding "don't overwrite if the user has
  touched this field" tracking (which would need JS state, not just
  htmx).
- **Season has zero batches**: suggested code is `<year>-A`.
- **Deleting a batch**: doesn't need any special handling — the
  suggestion is always computed fresh from whatever batches currently
  exist for the season, so a freed-up letter (e.g. deleting `2026-B`)
  becomes suggestable again next time, which is reasonable.

## Out of scope

- Renumbering/re-lettering existing batches when one is deleted (e.g.
  collapsing `A, C` down to `A, B`) — codes are stable identifiers once
  created, not repacked.
