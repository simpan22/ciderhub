# Spec: Easier input forms (auto-advance on submit)

Status: Planning — ready for review

## Problem

Logging a full session (pick apples, press, pitch yeast) means manually
opening each `<details>` card in turn and re-entering roughly the same
date each time. The todo asks for: submitting a form collapses it and
opens the *next* form in the logging sequence, with that next form's
date pre-filled to the date just submitted — except measurement, which
should stay open (you typically log several measurements in a row, not
move on after one).

## Decisions

1. **Fixed sequence**: Juicing → Additive → Measurement → Racking →
   Bottling. This matches the order the cards already appear in on the
   page. Measurement is a sequence member for the purpose of "what
   comes after additive," but never auto-closes itself, and submitting
   it does not auto-open Racking — the todo's exception is about
   measurement's own behavior, not about skipping it in the chain.
2. **Client-side, not server-side.** The "Log X" cards live outside the
   `#batch-timeline` fragment that add/edit handlers already
   re-render, so implementing this server-side would mean restructuring
   the page so every submission re-renders all six cards — a much
   bigger change for a purely presentational behavior. A small vanilla
   JS helper reacting to htmx's `htmx:afterRequest` event is
   self-contained and doesn't touch the request/response cycle or add
   a library dependency.
3. **Date copying, not just "keep the last value"**: juicing stores
   month+day (year implied by season); every other type stores a full
   `YYYY-MM-DD`. Copying juicing's date into additive's date field (or
   vice versa) requires converting between the two shapes, which needs
   the batch's season year available client-side. This is added as a
   `data-season-year` attribute on a stable container element (e.g. the
   `<main class="content">` on the batch detail page), read once by the
   helper script.

## Behavior

On a successful submit (`event.detail.successful`) of a "Log X" form:

1. Read that form's date value(s) (`month`+`day`, or `occurred_at`).
2. Close that form's `<details>` (`open = false`) — **except**
   measurement, which never closes itself.
3. If there's a next type in the sequence after this one: set that next
   form's date field(s) from the value read in step 1 (converting
   month/day ⇄ full date via the season year as needed), then open its
   `<details>` (`open = true`).
4. Bottling has no "next" — it just closes (or stays as-is, since it's
   last).

Each `<details>` gets a stable `id` (`log-juicing`, `log-additive`,
`log-measurement`, `log-racking`, `log-bottling`) so the script can
address them directly. A small shared script,
`static/js/event-forms.js`, exports one function invoked from each
form's `hx-on::after-request`, replacing today's blanket
`this.reset()` — see the date-format spec for why a blanket reset is
also part of the problem here.

## Interaction with the date-format spec

This spec's "prefill next form with the submitted date" and the
date-format bug's "keep the date prefilled after submit" are the same
underlying fix: today's `hx-on::after-request="this.reset()"` wipes
every field, date included, back to page-load defaults. Implementing
this spec effectively resolves that half of the other bug too — see
`european-date-format.md`. They're kept as separate backlog items
per the todo list, but should probably be implemented together to
avoid two passes over the same `hx-on::after-request` wiring.

## Edge cases

- **Validation failure** (e.g. required field missing): htmx's
  `afterRequest` fires with `successful: false` for a non-2xx response;
  the advance-and-clear logic must only run on success, leaving a
  failed form open and untouched so the user can fix it.
- **Manually reordering**: if the user opens a later card out of
  sequence (e.g. Racking before Juicing) and submits it, the "next"
  logic still applies mechanically (Racking → Bottling) — there's no
  attempt to detect or correct for out-of-order usage.
- **First form of a session**: nothing pre-opens Juicing automatically
  on page load; this spec only governs what happens *after* a submit.

## Out of scope

- Remembering the last-used date across a full page reload/revisit —
  this is in-memory/DOM state for the current page view only.
