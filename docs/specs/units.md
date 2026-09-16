# Spec: Units

Status: Planning — ready for review

## Problem

`additive_events.unit` is free-text (`TEXT NOT NULL`). Every "Log additive"
submission requires retyping the unit, and nothing stops "g", "G", and
"grams" from coexisting as different strings for the same real unit.

## Decisions

1. **A real `units` table**, referenced by FK, not a denormalized text
   column with app-level validation. This matches how the rest of the
   schema handles reference data (seasons, trees).
2. **Units are always stored lowercase.** The todo says to lowercase
   existing values on migration; for consistency this applies to new
   units too, not just the migrated ones — otherwise "Sugar" and "sugar"
   could still both exist going forward, defeating the point. This is a
   deviation-by-extension from the literal todo text, flagged here for
   confirmation.
3. **Dropdown, not free text, in the additive form.** A real `<select>`,
   per "select units from a dropdown list" — not a datalist/autocomplete
   text input (which would still permit case-drift typos). Adding a new
   unit is a separate, explicit action (see below), not something typed
   inline into the additive form.
4. **New units page** (`/units`), following the exact CRUD pattern
   already used for Seasons and Trees: htmx inline-edit list + add form.
   Delete is only allowed when no `additive_events` row references the
   unit (guard, not cascade) — deleting a unit out from under existing
   history would orphan those rows' display.
5. **Case-insensitive create-or-reuse**: submitting a name that already
   exists (case-insensitively) does not create a duplicate — the
   existing row is reused silently (no error shown), per the todo's
   explicit instruction.

## Data model

New migration (existing DB is live in production — this is a new file,
never an edit to an already-applied one):

```sql
CREATE TABLE units (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

-- Seed from existing data, lowercased and de-duplicated.
INSERT INTO units (name)
SELECT DISTINCT LOWER(TRIM(unit)) FROM additive_events;

ALTER TABLE additive_events ADD COLUMN unit_id INTEGER REFERENCES units (id);

UPDATE additive_events
SET unit_id = (SELECT id FROM units WHERE units.name = LOWER(TRIM(additive_events.unit)));

-- unit_id is backfilled for every existing row before this point, so
-- it's safe to make it required from here on.
ALTER TABLE additive_events DROP COLUMN unit;
```

`ALTER TABLE ... ADD COLUMN` can't add a `NOT NULL` column without a
default when the table already has rows, hence the two-step add-then-
backfill; sqlite also can't tighten a column to `NOT NULL` after the
fact without a full table rebuild, so `unit_id` stays nullable at the
schema level with the `NOT NULL` guarantee enforced at the application
layer (mirrors how `event_type`'s enum-completeness is already handled
by app logic, not a DB constraint, per the class-table-inheritance
design notes in `vision.md`).

## Backend

- `fetch_units(db) -> Vec<Unit>` (id, name), ordered by name.
- `find_or_create_unit(db, raw_name) -> i64`: trim + lowercase the
  input, `SELECT id FROM units WHERE name = ?`; if none, `INSERT` and
  use the new id. Used by the `/units` add handler.
- `/units` page: same shape as `seasons.rs`/`trees.rs` — `list`,
  `table_fragment`, `edit_fragment`, `create`, `update`, `delete`.
  `delete` first checks `SELECT COUNT(*) FROM additive_events WHERE
  unit_id = ?`; if nonzero, return the table fragment unchanged with an
  inline error rather than deleting.
- `add_additive` / `update_additive`: form field changes from
  `unit: String` to `unit_id: i64`; the `<select>` only offers ids that
  exist, so no create-or-reuse logic is needed on the additive form
  itself — that only happens on the `/units` page.
- Timeline formatting (`fetch_timeline`'s additive query) joins
  `units` for display: `SELECT ... u.name as unit FROM additive_events
  ae JOIN units u ON u.id = ae.unit_id ...` — the existing summary
  string format (`"{amount} {unit} {substance}"`) is unchanged.

## Frontend

- New `templates/units/list.html` + `_table_body.html`, copied from the
  Trees pattern (single `name` field instead of five).
- Nav link added to `base.html`.
- `templates/batches/detail.html`'s additive form and
  `_edit_additive.html`: `<input type="text" name="unit">` becomes
  `<select name="unit_id">{% for u in units %}<option value="{{
  u.id }}">{{ u.name }}</option>{% endfor %}</select>`. Both the add
  form (`DetailTemplate`) and the edit fragment
  (`EditAdditiveTemplate`) need a `units: Vec<Unit>` field, with the
  edit fragment's `<option>` pre-selected to match the event's current
  `unit_id`.

## Edge cases

- **No units exist yet**: the additive form's dropdown is empty and the
  form can't be usefully submitted. The app should nudge toward
  `/units` in this state (e.g. an inline "no units yet — add one" link
  where the dropdown would be) rather than rendering a submittable but
  meaningless empty `<select>`.
- **Renaming a unit** (edit-in-place) to a name that collides
  case-insensitively with a different existing unit: reject the rename
  server-side to avoid producing two rows sqlite would otherwise
  consider distinct (since uniqueness here is enforced by the app
  always writing lowercase, not by `COLLATE NOCASE`, a same-cased
  collision is possible if two units already differ only by case from
  the old bad data — worth a defensive check even though the seed step
  should have de-duplicated this).

## Out of scope

- Per-substance default units (e.g. auto-suggesting "g" for "Campden
  tablets"). Not requested; would need a substance vocabulary too.
