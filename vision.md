# CiderHub — Vision & Implementation Plan

## 1. Purpose

CiderHub is a self-hosted tool for tracking the full lifecycle of home cider
production: from picking and juicing apples, through fermentation and
measurement, to bottling. It replaces a notebook/spreadsheet with a
structured event log and, eventually, live environmental telemetry from the
fermentation area.

Guiding principles:

- **Event-sourced logging first.** Everything that happens (a picking, a
  pressing, a gravity reading, a racking) is recorded as a timestamped event
  attached to a batch. The UI is a thin layer over this log.
- **Small, boring, reliable stack.** A single Rust binary + a single SQLite
  file. No separate frontend build pipeline, no JS framework, no external
  services required to run it.
- **Professional feel, minimal ceremony.** Clean server-rendered pages,
  htmx for interactivity, no client-side routing or state management.
- **Extensible toward sensors.** The data model and ingestion path should
  accommodate continuous automated readings (temperature/humidity) from day
  one, even though hardware comes later.

## 2. Domain Model

Core entities:

- **Season** — a cider-making year/campaign (e.g. "2026"), mostly for
  grouping and reporting.
- **Batch** — the central object. A batch of cider moving through
  picking → juicing → fermentation → bottling. Has a name/code, season,
  status (e.g. `planning`, `fermenting`, `conditioning`, `bottled`,
  `archived`), and free-text notes.
- **Tree** — a physical apple tree the user owns (label, variety —
  nullable, since several are of unknown type — planting year, location
  in the garden/orchard, notes). A fixed, small, manually-maintained
  list (four today, room to grow).
- **Event** — a timestamped log entry attached to a batch, ordered on a
  shared timeline. Rather than one generic table with a JSON payload, each
  event *type* is its own small, fully-typed table (see §4) that shares
  the same `events` spine for ordering and batch association. Types:
  - `picking` — apples harvested from a specific tree into this batch
    (tree reference, weight). A batch fed by several trees just gets one
    `picking` event per tree; this is also how a batch's tree
    composition is derived — no separate lot/blend entity needed.
  - `juicing` — pressing event (input weight, output volume, yield %,
    equipment used).
  - `additive` — yeast pitch, nutrient, campden tablets, etc. (substance,
    amount).
  - `measurement` — a point-in-time reading tied to a batch: specific
    gravity, pH, temperature, volume, tasting notes (ABV is calculated
    from two gravity measurements, not stored).
  - `racking` — transfer between vessels, with volume/loss.
  - `bottling` — final packaging (bottle count/size, carbonation method).
  - `note` — free-text log entry with optional photo.
- **Vessel** — a fermenter/carboy/tank, so batches can be tracked as
  occupying physical equipment over time.
- **Sensor** — a registered device (id, location/label, type of readings
  it produces).
- **SensorReading** — a high-frequency, structured time-series row
  (sensor_id, timestamp, metric, value), decoupled from the manual Event
  log because of very different volume/shape.

Relationships: a `Batch` has many `Event`s; a `picking` event references
exactly one `Tree`, so a `Batch`'s tree composition falls out of its
picking events (many-to-many via the event log, not a separate join
table); a `Batch` occupies a `Vessel` over a date range; `SensorReading`s
belong to a `Sensor`, and a `Sensor` can optionally be associated with a
`Vessel` or a general location (e.g. "fermentation room") rather than a
specific batch.

## 3. Architecture

```
┌─────────────────────────────┐
│           Browser           │
│  HTML + htmx + minimal CSS  │
└──────────────┬───────────────┘
               │ HTTP (server-rendered fragments)
┌──────────────▼───────────────┐
│         Rust backend         │
│  axum (HTTP) + askama (tmpl) │
│  sqlx (async SQLite driver)  │
├───────────────────────────────┤
│   Ingestion endpoint (future) │
│   for sensor push (HTTP/MQTT) │
└──────────────┬───────────────┘
               │
        ┌──────▼──────┐
        │  SQLite file │
        └──────────────┘
```

**Backend**
- **Framework:** `axum` — async, minimal, integrates well with `tower`
  middleware (logging, compression, auth later).
- **Templates:** `askama` (compile-time checked, fast, no runtime template
  parsing) rendering HTML fragments for htmx swaps and full pages on
  initial load.
- **Database access:** `sqlx` with the `sqlite` feature — compile-time
  checked queries, async, migrations via `sqlx migrate`.
- **Migrations:** plain `.sql` files under `migrations/`, run automatically
  on startup in dev, and via `sqlx migrate run` in deploy.
- **Validation:** `validator` or hand-rolled checks at the form-handling
  boundary; keep domain invariants (e.g. non-negative weights) enforced in
  Rust, not just DB constraints.
- **Config:** a single `config.toml` or env vars for db path, bind
  address, and (later) auth secret.

**Frontend**
- Server-rendered HTML, htmx for partial updates (submit a form, swap in
  updated batch timeline without a full reload).
- No SPA framework, no bundler. A single hand-written CSS file (or a
  lightweight classless/utility base like Pico.css, self-hosted rather
  than CDN-loaded, to keep it fully offline-capable).
- Charts (gravity curve, temperature over time) rendered server-side to
  SVG initially (avoids a JS charting dependency); revisit if
  interactivity is needed later.

**Deployment**
- Single statically-linked binary + a SQLite file + a `migrations/`
  directory alongside it.
- Runs as a `systemd` unit on a small home server / Raspberry Pi.
- Backups: periodic copy of the SQLite file (litestream is an option
  later for continuous replication, not needed for v1).

## 4. Data Model Sketch (SQLite)

```sql
seasons(id, name, starts_on, ends_on)

trees(
  id, name, variety NULL, planted_on NULL,
  location, notes
)

vessels(id, name, capacity_l, kind, active)

batches(
  id, season_id, code, name, status,
  vessel_id NULL, started_on, notes
)

-- Thin spine: shared identity, batch association, and ordering for
-- every event type. Holds no type-specific data.
events(
  id, batch_id, event_type, occurred_at, created_at
)

-- One table per event type, each keyed 1:1 on events.id.
-- event_type on the spine says which of these to join.

picking_events(
  event_id PK/FK -> events.id,
  tree_id FK -> trees.id,
  weight_kg
)

juicing_events(
  event_id PK/FK -> events.id,
  input_weight_kg, output_volume_l, yield_pct, equipment
)

additive_events(
  event_id PK/FK -> events.id,
  substance, amount, unit
)

measurement_events(
  event_id PK/FK -> events.id,
  specific_gravity NULL, ph NULL, temperature_c NULL,
  volume_l NULL, tasting_notes NULL
)

racking_events(
  event_id PK/FK -> events.id,
  from_vessel_id FK NULL -> vessels.id,
  to_vessel_id FK -> vessels.id,
  volume_l, loss_l
)

bottling_events(
  event_id PK/FK -> events.id,
  bottle_count, bottle_size_ml, carbonation_method
)

note_events(
  event_id PK/FK -> events.id,
  text, photo_path NULL
)

sensors(id, label, location, kind)

sensor_readings(
  id, sensor_id, recorded_at, metric, value
)
-- high write volume: index on (sensor_id, recorded_at),
-- consider periodic downsampling/rollup table for long-term storage.
```

Design notes:
- This is class-table inheritance: `events` is the shared spine (id,
  batch, timestamp), and each event type gets its own table sharing that
  primary key. No JSON payload, no generic `numeric_value`/`text_value`
  columns — every column is named, typed, and `NOT NULL` where it should
  be, so `sqlx`'s compile-time query checking actually means something.
  The cost is one migration + one small table per new event type, which
  is cheap at the rate this app adds event types.
- Reading a batch's timeline means one query against `events` plus a
  join per type present (or one query per type, unioned) — trivial at
  this scale (tens to low thousands of events per batch, not millions).
- `event_type` on the spine is set redundantly with "which detail table
  has a row for this id" — enforced at the application layer rather than
  the database, since SQLite can't easily express "exactly one of these
  seven tables has a matching row" as a constraint. Kept simple over
  kept fully DB-enforced.
- `trees.variety` is nullable to support the several trees of unknown
  type — the UI should treat "unknown" as a first-class, common value
  rather than something to nag the user about.
- `sensor_readings` is intentionally separate from `events`: different
  volume profile (thousands of rows/day vs. a handful), different
  retention/rollup strategy, and no per-row user authorship.

## 5. Roadmap

**Phase 0 — Project scaffolding**
- Cargo workspace, axum server returning a static "hello" page.
- SQLite + sqlx wired up, first migration, health-check endpoint.
- Base layout template + minimal CSS.

**Phase 1 — Core logging (MVP)**
- CRUD for Seasons, Trees, Vessels, Batches.
- Batch detail page showing a chronological event timeline and a summary
  of which trees contributed (derived from its `picking` events).
- Log a `picking` event (tree, weight) — a batch can have several, one
  per tree it drew fruit from.
- Log a `juicing` event (input/output, computed yield %).
- Log a `measurement` event (specific gravity, pH, temp, tasting notes);
  ABV is computed on read from two gravity measurements, not stored.
- htmx-powered inline "add event" forms on the batch page (no reload).

**Phase 2 — Batch lifecycle & reporting**
- Racking and bottling event types; batch status transitions.
- Gravity/ABV curve chart per batch (server-rendered SVG).
- Season/batch overview dashboard (active batches, recent events).
- Search/filter across batches and events.
- Per-tree view: yield history across seasons, which batches it fed.

**Phase 3 — Sensors**
- `sensors` + `sensor_readings` tables and an authenticated ingestion
  endpoint (`POST /api/sensors/:id/readings`) for push-based devices
  (e.g. ESP32 running a small firmware, or a Raspberry Pi poller).
- Live "current conditions" widget on the dashboard (latest temp/humidity
  per location).
- Historical environment charts, overlaid with batch fermentation
  timelines (e.g. "was the room too warm during days 3–7 of batch X?").
- Rollup job to downsample old high-frequency readings.

**Phase 4 — Polish**
- Basic auth (single-user login) since the tool moves toward
  network-exposed sensor ingestion.
- Photo attachments on events (stored on disk, referenced by path).
- Export (CSV/JSON) of a batch's full history.
- Optional: simple alerting (e.g. notify if temperature out of range).

## 6. Open Questions

- **Auth model:** single shared password vs. no auth at all if it stays
  on a trusted LAN? (Affects whether Phase 3's ingestion endpoint needs a
  token from day one — recommend: yes, a static bearer token per sensor,
  even before full user auth exists.)
- **Units:** metric only, or toggle between metric/imperial? (Assumed
  metric throughout above; easy to add a display-layer conversion later.)
- **Multi-user/multi-site:** is this just for one person's setup, or
  should it eventually support multiple growers/locations? (Assumed
  single-site for now — keeps the data model much simpler.)
- **Sensor transport:** HTTP push (simplest, works with any
  microcontroller) vs. MQTT (better fit if more devices/subscribers show
  up later). Recommend starting with HTTP push and revisiting if a
  broker is already in use elsewhere.

## 7. Non-Goals (for now)

- No mobile app — the htmx web UI should be responsive enough for
  phone use in the field/cellar.
- No multi-tenancy or cloud hosting story — this is a self-hosted,
  single-installation tool.
- No inventory/recipe management for equipment/ingredients beyond what's
  needed to log events (e.g. not trying to be a full brewery ERP).
