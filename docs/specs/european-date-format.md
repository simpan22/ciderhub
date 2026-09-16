# Spec: European date format

Status: Planning — needs a decision before work starts (see below)

## Problem, and a constraint that changes the fix

The todo asks for `dd/mm/yyyy` display instead of `mm/dd/yyyy`. The
complication: `<input type="date">` always stores its value as ISO
`YYYY-MM-DD` (already true everywhere in this app — this part is fine
and doesn't change), but the **displayed** text in the native date
picker is rendered by the browser according to the *browser/OS locale*,
not anything the page controls. A site cannot force a native date
input to display `dd/mm/yyyy` for a browser configured for
`en-US`. This isn't a bug in this app to begin with — it's a platform
behavior.

So "fix the display format" has two genuinely different implementations
depending on what's actually wanted:

### Option A — it's a browser locale issue, not an app bug (recommended)

Confirm the browser/OS this is being observed on is set to an
American locale, and that this is what's producing `mm/dd/yyyy`.
Changing the OS/browser locale (or just Chrome's language setting)
fixes it with zero code changes, and every other site's date inputs
too. No implementation needed on our side.

### Option B — replace native date inputs with day/month/year selects

If the format must be enforced regardless of the visiting browser's
locale, native `<input type="date">` has to go. Replace it with three
explicit fields — `<select day>`, `<select month>` (names, not
numbers, to make order-independent of locale entirely — this project
already does this for juicing's month picker), `<input type="number"
year>` — combined server-side into an ISO string, exactly like
`dates::season_date` already does for the juicing/picking-merged form,
just with an explicit year field added back for the event types that
need one (additive, measurement, racking, bottling).

Trade-off: loses the native calendar picker widget (tap a date on a
mini calendar) in exchange for full control over field order. Given
this app already uses month/day selects for juicing, this would make
date entry *consistent* across all event types, which is arguably a
UX win independent of the locale question.

**This spec doesn't pick between A and B** — that's a product decision
for review, not an implementation detail. Recommendation: try A first
(it's free); only build B if the actual browser locale is correct and
still shows the "wrong" order, or if the day/month/year field style is
wanted for its own sake regardless of locale.

## The second half of this bug: dates not staying prefilled

"If an event is created with a specific date, the form should keep
that date prefilled" describes a real, unrelated app bug: every
"Log X" form's `hx-on::after-request="if(event.detail.successful)
this.reset()"` resets *all* fields, including date, back to their
page-load default (today, or the current month for juicing) after
every submit. Logging several back-dated entries in a row (e.g.
catching up a week of measurements) currently means re-picking the
date every single time.

This is the same fix as `streamlined-event-forms.md`'s "prefill next
form with the submitted date" — both come down to replacing the blanket
`this.reset()` with logic that clears everything *except* the date
field(s), which get set to the just-submitted value rather than reset
to page-load defaults. Implement once, referenced from both specs.

## Decisions

1. Ship the "keep date prefilled" fix regardless of the A/B locale
   decision above — it's unambiguous and valuable on its own.
2. Don't build Option B speculatively. Get a read on which display is
   actually being seen (and on what browser/locale) before touching
   the date inputs themselves.

## Backend / Frontend changes (for the "keep date prefilled" half only)

- Replace each form's `this.reset()` with a small helper (shared with
  `streamlined-event-forms.md`'s script) that resets the form via
  `HTMLFormElement.reset()` and then immediately re-applies the
  submitted date value(s) on top — simplest correct sequencing, avoids
  hand-clearing every non-date field individually.
- No server/template changes needed for this half; it's purely the
  `hx-on::after-request` JS.

## Out of scope

- Any change to the stored date format — it is and remains ISO
  `YYYY-MM-DD` everywhere in the database and in URLs. Only the
  *display* is in question, and only for Option B if chosen.
