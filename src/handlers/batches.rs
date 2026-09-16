use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;

use crate::error::AppError;
use crate::models::{
    AddJuicingForm, AddMeasurementForm, AddPickingForm, BatchListItem, NewBatchForm, Season,
    TimelineRow, Tree, TreeYield, UpdateBatchForm, Vessel,
};
use crate::state::AppState;

const STATUSES: [&str; 5] = [
    "planning",
    "fermenting",
    "conditioning",
    "bottled",
    "archived",
];

pub struct StatusOption {
    pub value: &'static str,
    pub selected: bool,
}

pub struct VesselOption {
    pub id: i64,
    pub name: String,
    pub selected: bool,
}

pub struct BatchDetail {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub status: String,
    pub season_name: String,
    pub vessel_name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/list.html")]
struct ListTemplate {
    batches: Vec<BatchListItem>,
    seasons: Vec<Season>,
    vessels: Vec<Vessel>,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/_meta.html")]
struct MetaTemplate {
    batch: BatchDetail,
    statuses: Vec<StatusOption>,
    vessels: Vec<VesselOption>,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/_timeline.html")]
struct TimelineTemplate {
    tree_yields: Vec<TreeYield>,
    timeline: Vec<TimelineRow>,
}

// Flattened rather than nesting `MetaTemplate`/`TimelineTemplate`: Askama's
// `{% include %}` splices the partial's AST into the parent and resolves
// field names against the parent's own context, so the parent needs these
// fields at the top level, not behind `meta.`/`timeline.`.
#[derive(Template, WebTemplate)]
#[template(path = "batches/detail.html")]
struct DetailTemplate {
    batch: BatchDetail,
    statuses: Vec<StatusOption>,
    vessels: Vec<VesselOption>,
    tree_yields: Vec<TreeYield>,
    timeline: Vec<TimelineRow>,
    trees: Vec<Tree>,
}

async fn fetch_seasons(db: &sqlx::SqlitePool) -> Result<Vec<Season>, sqlx::Error> {
    sqlx::query_as!(Season, "SELECT id, name, starts_on, ends_on FROM seasons ORDER BY starts_on DESC, name")
        .fetch_all(db)
        .await
}

async fn fetch_vessels(db: &sqlx::SqlitePool) -> Result<Vec<Vessel>, sqlx::Error> {
    sqlx::query_as!(
        Vessel,
        r#"SELECT id as "id!", name, capacity_l, kind, active as "active: bool" FROM vessels ORDER BY name"#
    )
    .fetch_all(db)
    .await
}

async fn fetch_trees(db: &sqlx::SqlitePool) -> Result<Vec<Tree>, sqlx::Error> {
    sqlx::query_as!(
        Tree,
        r#"SELECT id as "id!", name, variety, planted_on, location, notes FROM trees ORDER BY name"#
    )
    .fetch_all(db)
    .await
}

async fn fetch_meta(db: &sqlx::SqlitePool, id: i64) -> Result<Option<MetaTemplate>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT b.id, b.code, b.name, b.status, b.vessel_id, b.notes,
               s.name as season_name, v.name as "vessel_name?"
        FROM batches b
        JOIN seasons s ON s.id = b.season_id
        LEFT JOIN vessels v ON v.id = b.vessel_id
        WHERE b.id = ?
        "#,
        id
    )
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let all_vessels = fetch_vessels(db).await?;

    let batch = BatchDetail {
        id: row.id,
        code: row.code,
        name: row.name,
        status: row.status,
        season_name: row.season_name,
        vessel_name: row.vessel_name,
        notes: row.notes,
    };

    let statuses = STATUSES
        .iter()
        .map(|s| StatusOption {
            value: s,
            selected: *s == batch.status,
        })
        .collect();

    let vessels = all_vessels
        .into_iter()
        .map(|v| VesselOption {
            selected: Some(v.id) == row.vessel_id,
            id: v.id,
            name: v.name,
        })
        .collect();

    Ok(Some(MetaTemplate {
        batch,
        statuses,
        vessels,
    }))
}

async fn fetch_timeline(db: &sqlx::SqlitePool, batch_id: i64) -> Result<TimelineTemplate, sqlx::Error> {
    let pickings = sqlx::query!(
        r#"
        SELECT e.occurred_at, t.name as tree_name, pe.weight_kg
        FROM picking_events pe
        JOIN events e ON e.id = pe.event_id
        JOIN trees t ON t.id = pe.tree_id
        WHERE e.batch_id = ?
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let juicings = sqlx::query!(
        r#"
        SELECT e.occurred_at, je.input_weight_kg, je.output_volume_l, je.yield_pct, je.equipment
        FROM juicing_events je
        JOIN events e ON e.id = je.event_id
        WHERE e.batch_id = ?
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let measurements = sqlx::query!(
        r#"
        SELECT e.occurred_at, me.specific_gravity, me.ph, me.temperature_c, me.volume_l, me.tasting_notes
        FROM measurement_events me
        JOIN events e ON e.id = me.event_id
        WHERE e.batch_id = ?
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let tree_yields = sqlx::query_as!(
        TreeYield,
        r#"
        SELECT t.name as tree_name, SUM(pe.weight_kg) as "total_kg!: f64"
        FROM picking_events pe
        JOIN events e ON e.id = pe.event_id
        JOIN trees t ON t.id = pe.tree_id
        WHERE e.batch_id = ?
        GROUP BY t.id, t.name
        ORDER BY t.name
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let mut timeline: Vec<TimelineRow> = Vec::new();

    for p in pickings {
        timeline.push(TimelineRow {
            occurred_at: p.occurred_at,
            kind_label: "Picking",
            summary: format!("{:.1} kg from {}", p.weight_kg, p.tree_name),
        });
    }

    for j in juicings {
        let yield_note = match j.yield_pct {
            Some(pct) => format!(" ({pct:.0}% yield)"),
            None => String::new(),
        };
        let equipment_note = match j.equipment {
            Some(eq) if !eq.is_empty() => format!(" via {eq}"),
            _ => String::new(),
        };
        timeline.push(TimelineRow {
            occurred_at: j.occurred_at,
            kind_label: "Juicing",
            summary: format!(
                "{:.1} kg pressed to {:.1} L{yield_note}{equipment_note}",
                j.input_weight_kg, j.output_volume_l
            ),
        });
    }

    for m in measurements {
        let mut parts = Vec::new();
        if let Some(sg) = m.specific_gravity {
            parts.push(format!("SG {sg:.3}"));
        }
        if let Some(ph) = m.ph {
            parts.push(format!("pH {ph:.2}"));
        }
        if let Some(temp) = m.temperature_c {
            parts.push(format!("{temp:.1}°C"));
        }
        if let Some(vol) = m.volume_l {
            parts.push(format!("{vol:.1} L"));
        }
        if let Some(notes) = m.tasting_notes.filter(|n| !n.is_empty()) {
            parts.push(notes);
        }
        let summary = if parts.is_empty() {
            "measurement logged".to_string()
        } else {
            parts.join(", ")
        };
        timeline.push(TimelineRow {
            occurred_at: m.occurred_at,
            kind_label: "Measurement",
            summary,
        });
    }

    timeline.sort_by(|a, b| a.occurred_at.cmp(&b.occurred_at));

    Ok(TimelineTemplate {
        tree_yields,
        timeline,
    })
}

pub async fn list(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let batches = sqlx::query_as!(
        BatchListItem,
        r#"
        SELECT b.id, b.code, b.name, b.status, s.name as season_name, v.name as "vessel_name?"
        FROM batches b
        JOIN seasons s ON s.id = b.season_id
        LEFT JOIN vessels v ON v.id = b.vessel_id
        ORDER BY b.started_on DESC, b.code
        "#
    )
    .fetch_all(&state.db)
    .await?;

    let seasons = fetch_seasons(&state.db).await?;
    let vessels = fetch_vessels(&state.db).await?;

    Ok(ListTemplate {
        batches,
        seasons,
        vessels,
    })
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<NewBatchForm>,
) -> Result<impl IntoResponse, AppError> {
    let result = sqlx::query!(
        "INSERT INTO batches (season_id, code, name, status, vessel_id, started_on) VALUES (?, ?, ?, 'planning', ?, ?)",
        form.season_id,
        form.code,
        form.name,
        form.vessel_id,
        form.started_on
    )
    .execute(&state.db)
    .await?;

    Ok(Redirect::to(&format!("/batches/{}", result.last_insert_rowid())))
}

pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let Some(meta) = fetch_meta(&state.db, id).await? else {
        return Ok((StatusCode::NOT_FOUND, "batch not found").into_response());
    };
    let timeline = fetch_timeline(&state.db, id).await?;
    let trees = fetch_trees(&state.db).await?;

    Ok(DetailTemplate {
        batch: meta.batch,
        statuses: meta.statuses,
        vessels: meta.vessels,
        tree_yields: timeline.tree_yields,
        timeline: timeline.timeline,
        trees,
    }
    .into_response())
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateBatchForm>,
) -> Result<Response, AppError> {
    sqlx::query!(
        "UPDATE batches SET status = ?, vessel_id = ?, notes = ? WHERE id = ?",
        form.status,
        form.vessel_id,
        form.notes,
        id
    )
    .execute(&state.db)
    .await?;

    let Some(meta) = fetch_meta(&state.db, id).await? else {
        return Ok((StatusCode::NOT_FOUND, "batch not found").into_response());
    };

    Ok(meta.into_response())
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query!("DELETE FROM batches WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    let mut response = StatusCode::OK.into_response();
    response
        .headers_mut()
        .insert("HX-Redirect", HeaderValue::from_static("/batches"));
    Ok(response)
}

pub async fn add_picking(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<AddPickingForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = state.db.begin().await?;

    let event = sqlx::query!(
        "INSERT INTO events (batch_id, event_type, occurred_at) VALUES (?, 'picking', ?)",
        id,
        form.occurred_at
    )
    .execute(&mut *tx)
    .await?;

    let event_id = event.last_insert_rowid();

    sqlx::query!(
        "INSERT INTO picking_events (event_id, tree_id, weight_kg) VALUES (?, ?, ?)",
        event_id,
        form.tree_id,
        form.weight_kg
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, id).await?;
    Ok(timeline)
}

pub async fn add_juicing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<AddJuicingForm>,
) -> Result<impl IntoResponse, AppError> {
    let raw_yield_pct = form.output_volume_l / form.input_weight_kg * 100.0;
    let yield_pct = (raw_yield_pct * 10.0).round() / 10.0;

    let mut tx = state.db.begin().await?;

    let event = sqlx::query!(
        "INSERT INTO events (batch_id, event_type, occurred_at) VALUES (?, 'juicing', ?)",
        id,
        form.occurred_at
    )
    .execute(&mut *tx)
    .await?;

    let event_id = event.last_insert_rowid();

    sqlx::query!(
        "INSERT INTO juicing_events (event_id, input_weight_kg, output_volume_l, yield_pct, equipment) VALUES (?, ?, ?, ?, ?)",
        event_id,
        form.input_weight_kg,
        form.output_volume_l,
        yield_pct,
        form.equipment
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, id).await?;
    Ok(timeline)
}

pub async fn add_measurement(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<AddMeasurementForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = state.db.begin().await?;

    let event = sqlx::query!(
        "INSERT INTO events (batch_id, event_type, occurred_at) VALUES (?, 'measurement', ?)",
        id,
        form.occurred_at
    )
    .execute(&mut *tx)
    .await?;

    let event_id = event.last_insert_rowid();

    sqlx::query!(
        "INSERT INTO measurement_events (event_id, specific_gravity, ph, temperature_c, volume_l, tasting_notes) VALUES (?, ?, ?, ?, ?, ?)",
        event_id,
        form.specific_gravity,
        form.ph,
        form.temperature_c,
        form.volume_l,
        form.tasting_notes
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, id).await?;
    Ok(timeline)
}
