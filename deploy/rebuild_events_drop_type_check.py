#!/usr/bin/env python3
"""
One-off manual schema fix: rebuilds the events table without any CHECK
constraint on events.event_type (see
docs/specs/bradley-terry-taste-rating.md).

Every new event type so far ('failed', 'tasting', now 'comparison') has
needed this same risky table rebuild just to extend that list, while
the only writer is the app itself using event_type literals it
controls. Dropping the constraint means future event types only need a
normal migration for their own typed table.

Same reason as fix_events_check_constraint.py for why this CANNOT be a
sqlx migration: sqlx's SQLite runner always wraps migrations in a
transaction, PRAGMA foreign_keys can't be turned off inside one, and
dropping events while foreign_keys=ON cascades and destroys every
ON DELETE CASCADE child row (see deploy/README.md).

Run this directly against a database file, with the app's service
stopped. Safe to run more than once: it does nothing if the constraint
is already gone. Every table's row count is compared before and after
the rebuild, inside the transaction, and the rebuild is rolled back if
anything changed.

Usage: python3 rebuild_events_drop_type_check.py /path/to/ciderhub.db
"""
import sqlite3
import sys


def row_counts(conn):
    tables = [
        r[0]
        for r in conn.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
    ]
    return {t: conn.execute(f'SELECT COUNT(*) FROM "{t}"').fetchone()[0] for t in tables}


def main():
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <path-to-db>", file=sys.stderr)
        sys.exit(1)

    db_path = sys.argv[1]
    # Autocommit mode, so the explicit BEGIN/COMMIT below are the only
    # transaction boundaries (no implicit ones from the sqlite3 module).
    conn = sqlite3.connect(db_path, isolation_level=None)

    current_schema = conn.execute("SELECT sql FROM sqlite_master WHERE name = 'events'").fetchone()
    if current_schema is None:
        raise SystemExit("no events table found — is this a CiderHub database?")
    if "CHECK" not in current_schema[0]:
        print("events table has no event_type CHECK constraint — nothing to do.")
        return

    # Foreign keys OFF *before* any transaction starts — this is the
    # part that must never happen inside a sqlx-managed migration.
    conn.execute("PRAGMA foreign_keys = OFF")
    assert conn.execute("PRAGMA foreign_keys").fetchone() == (0,), "foreign_keys did not turn off"

    before = row_counts(conn)

    conn.execute("BEGIN")
    try:
        conn.execute("""
            CREATE TABLE events_new (
                id          INTEGER PRIMARY KEY,
                batch_id    INTEGER NOT NULL REFERENCES batches (id) ON DELETE CASCADE,
                event_type  TEXT NOT NULL,
                occurred_at TEXT NOT NULL,
                notes       TEXT,
                created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            )
        """)
        conn.execute("""
            INSERT INTO events_new (id, batch_id, event_type, occurred_at, notes, created_at)
            SELECT id, batch_id, event_type, occurred_at, notes, created_at FROM events
        """)
        conn.execute("DROP TABLE events")
        conn.execute("ALTER TABLE events_new RENAME TO events")
        conn.execute("CREATE INDEX idx_events_batch_occurred ON events (batch_id, occurred_at)")

        issues = conn.execute("PRAGMA foreign_key_check").fetchall()
        if issues:
            raise RuntimeError(f"foreign_key_check found issues after rebuild: {issues}")

        after = row_counts(conn)
        if after != before:
            changed = {t: (before.get(t), after.get(t)) for t in set(before) | set(after) if before.get(t) != after.get(t)}
            raise RuntimeError(f"row counts changed during rebuild (before, after): {changed}")

        conn.execute("COMMIT")
    except Exception:
        conn.execute("ROLLBACK")
        raise
    finally:
        conn.execute("PRAGMA foreign_keys = ON")

    for table, count in after.items():
        print(f"{table}: {count} rows")
    print("events table rebuilt without the event_type CHECK constraint.")


if __name__ == "__main__":
    main()
