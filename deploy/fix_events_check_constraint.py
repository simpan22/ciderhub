#!/usr/bin/env python3
"""
One-off manual schema fix: rebuilds events.event_type's CHECK constraint
to add 'failed' and 'tasting' and drop the unused 'picking'.

This CANNOT be run as a normal sqlx migration: sqlx's SQLite migration
runner always wraps a migration's SQL in a transaction regardless of a
"-- no-transaction" marker (verified against sqlx 0.8.6 source — that
marker is only honored by other database backends). SQLite can't
disable `PRAGMA foreign_keys` inside an open transaction, and dropping
(or renaming away) an FK-parent table while foreign_keys=ON immediately
cascades and destroys every ON DELETE CASCADE child row — even if the
parent is recreated and renamed back moments later. Confirmed this
empirically (twice) before writing this script.

Run this directly against a database file, with the app's service
stopped, before that database is ever started with a binary that
expects the new CHECK constraint to already be in place.

Usage: python3 fix_events_check_constraint.py /path/to/ciderhub.db
"""
import sqlite3
import sys


def main():
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <path-to-db>", file=sys.stderr)
        sys.exit(1)

    db_path = sys.argv[1]
    conn = sqlite3.connect(db_path)

    cur = conn.cursor()
    cur.execute("SELECT sql FROM sqlite_master WHERE name = 'events'")
    current_schema = cur.fetchone()
    if current_schema and "'tasting'" in current_schema[0] and "'picking'" not in current_schema[0]:
        print("events table already has the target schema — nothing to do.")
        return

    # Foreign keys OFF *before* any transaction starts — this is the
    # part that must never happen inside a sqlx-managed migration.
    conn.execute("PRAGMA foreign_keys = OFF")
    assert conn.execute("PRAGMA foreign_keys").fetchone() == (0,), "foreign_keys did not turn off"

    conn.execute("BEGIN")
    conn.execute("""
        CREATE TABLE events_new (
            id          INTEGER PRIMARY KEY,
            batch_id    INTEGER NOT NULL REFERENCES batches (id) ON DELETE CASCADE,
            event_type  TEXT NOT NULL
                        CHECK (event_type IN (
                            'juicing', 'additive', 'measurement',
                            'racking', 'bottling', 'note', 'failed', 'tasting'
                        )),
            occurred_at TEXT NOT NULL,
            notes       TEXT,
            created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        )
    """)
    conn.execute("""
        INSERT INTO events_new (id, batch_id, event_type, occurred_at, notes, created_at)
        SELECT id, batch_id, event_type, occurred_at, notes, created_at FROM events
        WHERE event_type != 'picking'
    """)
    conn.execute("DROP TABLE events")
    conn.execute("ALTER TABLE events_new RENAME TO events")
    conn.execute("CREATE INDEX idx_events_batch_occurred ON events (batch_id, occurred_at)")
    conn.commit()

    conn.execute("PRAGMA foreign_keys = ON")

    # Verify nothing was lost or orphaned.
    issues = conn.execute("PRAGMA foreign_key_check").fetchall()
    if issues:
        raise SystemExit(f"foreign_key_check found issues after rebuild: {issues}")

    for table in [
        "juicing_events", "additive_events", "measurement_events",
        "racking_events", "bottling_events", "note_events",
    ]:
        count = conn.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
        print(f"{table}: {count} rows")

    print("events table rebuilt successfully.")


if __name__ == "__main__":
    main()
