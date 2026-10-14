# Spec: Tasting page for friends

Status: Done — deployed 2026-10-14

## Problem

Taste comparisons ([[bradley-terry-taste-rating]]) can only be logged
from a batch's detail page in the main app. That page is cluttered for
someone who just wants to say "this one was better", and the main app
sits behind one shared password that unlocks everything. Friends should
be able to log comparisons on their phones without seeing, or being able
to change, anything else.

## Decisions

1. **A separate, phone-first page at `/taste`,** with its own minimal
   layout (no main-app navigation). It has one job: pick two ciders,
   tap which was better, done.

2. **Personal invite links, one per friend** (settled in review). You
   create a taster by name in the main app and get a secret link like
   `https://cider.simonochamanda.se/taste/join/3f9c…`. Opening it:
   - sets a `ciderhub_taster` cookie (HttpOnly, SameSite=Lax, one
     year, `Path=/taste` so the browser never even sends it to other
     pages) holding that taster's token;
   - redirects straight to `/taste`, so the token doesn't stay in the
     address bar or browser history any longer than needed. The
     response also sends `Referrer-Policy: no-referrer`.

   No passwords to remember. Revoking a taster stops their link and
   cookie from working immediately, and their earlier comparisons are
   kept.

3. **Taster access is limited to `/taste` routes.** The auth middleware
   gets a second, narrower check: a valid, non-revoked taster cookie
   lets a request through only if its path is `/taste` or starts with
   `/taste/`. Everything else still needs the owner's password cookie,
   exactly as today. The owner's cookie also works on `/taste`, so you
   can use the simple page yourself.

4. **Every comparison records who logged it.** It's stored as a new
   nullable `comparison_events.taster_id`, where `NULL` means you,
   logging from the main app or from `/taste` with your own cookie. The
   main app's timeline shows it ("Better than 2026-C — by Anna").

5. **Everyone's comparisons count towards one shared rating.**
   Bradley-Terry handles comparisons from different people fine.
   Per-taster ratings or filtering are out of scope, and the data
   needed to add them later is recorded from day one.

6. **Friends see batch codes, not ratings** (settled in review: batches
   are shown by code, not blind). The page lists bottled batches by
   code only. It never shows ratings, scores or other people's
   comparisons, so nobody is nudged towards agreeing with earlier
   results. If you want a blind tasting, pour the glasses yourself and
   tell people which code is which only afterwards.

7. **Friends can undo but not edit.** After saving, a friend sees their
   own comparisons from today, each with an "Undo" button. Undo
   deletes that comparison. It works only on the taster's own
   comparisons and only on the day they were logged. Anything older is
   corrected by you in the main app, where you can already edit
   comparisons.

8. **The date is always today.** There is no date field. A friend logs
   a comparison while tasting, so asking for a date would only add a
   chance to get it wrong.

## The page

Built from top to bottom for one thumb on a phone:

```
┌─────────────────────────────┐
│ CiderHub tasting   Hi Anna  │
│                             │
│ First cider                 │
│ [2026-A] [2026-B] [2026-C]  │  ← big tappable chips (radio buttons)
│ [2026-D]                    │
│                             │
│ Second cider                │
│ [2026-A] [2026-B] [2026-C]  │
│ [2026-D]                    │
│                             │
│ Note (optional) [________]  │
│                             │
│ Which did you prefer?       │
│ [ First ] [ Same ] [Second] │  ← each one submits immediately
│                             │
│ ✓ Saved: 2026-A over 2026-C │
│                             │
│ Your comparisons today      │
│ 2026-A over 2026-C  [Undo]  │
│ 2026-B ≈ 2026-D     [Undo]  │
└─────────────────────────────┘
```

- **One tap to save:** the three outcome buttons are the form's submit
  buttons (`name="outcome"` with values `win`/`tie`/`loss`), so there's
  no separate "Save" step.
- **Selections stay after saving,** so comparing one cider against
  several others is just "tap a new second cider, tap an outcome". The
  note field clears.
- **Same cider twice:** choosing it disables that cider in the other
  row. The server also rejects it (422), as the main app already does.
- **Fewer than two bottled batches:** the page shows "Nothing to
  compare yet" instead of the form.
- htmx swaps only the confirmation and "today" sections, so the
  selections stay without any extra JavaScript.

## Owner side: a "Tasters" page in the main app

New page `/tasters`, linked in the main navigation:

- **Add a taster** by name. The page then shows their invite link with
  a "Copy" button, ready to send in a message.
- **List of tasters:** name, number of comparisons logged, last
  active, and the link again (it can be copied later).
- **Revoke:** stops the link and any cookie already issued.
  **New link** replaces the token, e.g. if a link was forwarded by
  mistake; the old link stops working. On a revoked taster the same
  action is labelled **Reinstate**: it issues a new link and lets them
  back in. (Reinstating was added during implementation.)

## Data model

New migration:

```sql
CREATE TABLE tasters (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    token      TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    revoked_at TEXT
);

ALTER TABLE comparison_events ADD COLUMN taster_id INTEGER REFERENCES tasters (id);
```

- A plain `ADD COLUMN`. Unlike the CHECK change, it doesn't rebuild the
  table, so it's safe as a normal sqlx migration.
- Tasters are never deleted, only revoked, so `taster_id` never points
  at a missing row and the history of who logged what stays intact.
- **Tokens:** 32 random bytes from the operating system, hex-encoded.
  This uses the `getrandom` crate, which is already in `Cargo.lock`
  through other dependencies, so it only becomes a direct dependency.
  Tokens are stored in plain text so the owner page can show the link
  again. Anyone with the database file already has all the data, so
  hashing them would add little.

## Backend

- `src/handlers/auth.rs`:
  - `require_auth` lets `/taste/join/{token}` through without any
    cookie, and lets `/taste` and `/taste/*` through with a valid
    taster cookie. The taster lookup is one indexed query
    (`SELECT id, name FROM tasters WHERE token = ? AND revoked_at IS NULL`).
  - The resolved taster is added to the request extensions, so `/taste`
    handlers can read who is logged in. `None` means the owner.
- `src/handlers/taste.rs` (new):
  - `GET /taste/join/{token}`: validate, set the cookie, redirect to
    `/taste`. An unknown or revoked token gets a plain "This link is
    no longer valid — ask for a new one" page.
  - `GET /taste`: the page.
  - `POST /taste/comparisons`: add the comparison (first cider as the
    spine batch), with `taster_id` set. It reuses `validate_comparison`
    from `batches.rs`, which moves to a shared spot with the insert
    logic so the two entry points can't drift apart. It returns the
    confirmation and "today" sections.
  - `DELETE /taste/comparisons/{event_id}`: undo. It deletes the event
    only if it belongs to the current taster and was logged today,
    otherwise 404.
- `src/handlers/tasters.rs` (new): owner page, add / revoke / new link.
- `fetch_timeline` and the Overview tooltips append " — by {name}"
  when `taster_id` is set.

## Edge cases

- **A friend opens their link on a second device:** that's fine, each
  device gets its own cookie for the same taster.
- **A forwarded link:** whoever opens it logs as that friend. "New
  link" fixes it. Acceptable for a hobby app; the worst case is some
  junk comparisons, which you can see and edit in the main app.
- **A revoked friend's open page:** their next save gets the "link no
  longer valid" page instead of saving. Because htmx doesn't swap 4xx
  responses, the middleware answers htmx requests with `HX-Refresh`,
  so the page reloads into that message instead of silently doing
  nothing.
- **A batch stops being bottled** (marked failed) while someone has
  the page open: the server checks the status when saving and rejects
  it (422). The page shows "That cider can't be compared any more" and
  reloads the list.
- **Two friends tasting together on one phone:** both comparisons are
  logged as whoever's link that phone used. Out of scope; each friend
  should use their own phone.

## Out of scope

- A built-in blind tasting mode (anonymous glass labels mapped to
  batches).
- Per-taster ratings, or filtering the rating by taster.
- Friends logging 0–10 tasting scores. Only comparisons for now.
- Notifications, or any way for friends to see results.
