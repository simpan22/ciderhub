use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;

use crate::dates;
use crate::error::AppError;
use crate::models::{
    AddAdditiveForm, AddBottlingForm, AddJuicingForm, AddMeasurementForm, AddPickingForm,
    AddRackingForm, BatchListItem, MonthOption, NewBatchForm, Season, TimelineRow, Tree,
    TreeYield, UpdateBatchForm,
};
use crate::state::AppState;

/// Status is derived from which event types a batch has logged, most
/// advanced wins — there's no stored/editable status to drift out of
/// sync with the actual event history.
fn compute_status<'a>(event_types: impl IntoIterator<Item = &'a str>) -> &'static str {
    let mut has_fermenting_signal = false;
    let mut has_racking = false;
    let mut has_bottling = false;

    for t in event_types {
        match t {
            "juicing" | "additive" | "measurement" => has_fermenting_signal = true,
            "racking" => has_racking = true,
            "bottling" => has_bottling = true,
            _ => {}
        }
    }

    if has_bottling {
        "bottled"
    } else if has_racking {
        "conditioning"
    } else if has_fermenting_signal {
        "fermenting"
    } else {
        "planning"
    }
}

pub struct BatchDetail {
    pub id: i64,
    pub code: String,
    pub name: Option<String>,
    pub status: &'static str,
    pub season_year: i64,
    pub notes: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/list.html")]
struct ListTemplate {
    batches: Vec<BatchListItem>,
    seasons: Vec<Season>,
    today: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/_meta.html")]
struct MetaTemplate {
    batch: BatchDetail,
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
    tree_yields: Vec<TreeYield>,
    timeline: Vec<TimelineRow>,
    trees: Vec<Tree>,
    months: Vec<MonthOption>,
    today_day: u32,
    today: String,
}

async fn fetch_seasons(db: &sqlx::SqlitePool) -> Result<Vec<Season>, sqlx::Error> {
    sqlx::query_as!(Season, r#"SELECT id as "id!", year FROM seasons ORDER BY year DESC"#)
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

async fn fetch_event_types(db: &sqlx::SqlitePool, batch_id: i64) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT DISTINCT event_type FROM events WHERE batch_id = ?",
        batch_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(|r| r.event_type).collect())
}

async fn fetch_season_year(db: &sqlx::SqlitePool, batch_id: i64) -> Result<Option<i64>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT s.year as year FROM batches b JOIN seasons s ON s.id = b.season_id WHERE b.id = ?",
        batch_id
    )
    .fetch_optional(db)
    .await?;

    Ok(row.map(|r| r.year))
}

async fn fetch_meta(db: &sqlx::SqlitePool, id: i64) -> Result<Option<MetaTemplate>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT b.id, b.code, b.name, b.notes, s.year as season_year
        FROM batches b
        JOIN seasons s ON s.id = b.season_id
        WHERE b.id = ?
        "#,
        id
    )
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let event_types = fetch_event_types(db, id).await?;

    let batch = BatchDetail {
        id: row.id,
        code: row.code,
        name: row.name,
        status: compute_status(event_types.iter().map(String::as_str)),
        season_year: row.season_year,
        notes: row.notes,
    };

    Ok(Some(MetaTemplate { batch }))
}

async fn fetch_timeline(db: &sqlx::SqlitePool, batch_id: i64) -> Result<TimelineTemplate, sqlx::Error> {
    let pickings = sqlx::query!(
        r#"
        SELECT e.occurred_at, e.notes, t.name as tree_name, pe.weight_kg
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
        SELECT e.occurred_at, e.notes, je.input_weight_kg, je.output_volume_l, je.yield_pct, je.equipment
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
        SELECT e.occurred_at, e.notes, me.specific_gravity, me.ph, me.temperature_c, me.volume_l, me.tasting_notes
        FROM measurement_events me
        JOIN events e ON e.id = me.event_id
        WHERE e.batch_id = ?
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let additives = sqlx::query!(
        r#"
        SELECT e.occurred_at, e.notes, ae.substance, ae.amount, ae.unit
        FROM additive_events ae
        JOIN events e ON e.id = ae.event_id
        WHERE e.batch_id = ?
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let rackings = sqlx::query!(
        r#"
        SELECT e.occurred_at, e.notes, re.volume_l, re.loss_l
        FROM racking_events re
        JOIN events e ON e.id = re.event_id
        WHERE e.batch_id = ?
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let bottlings = sqlx::query!(
        r#"
        SELECT e.occurred_at, e.notes, be.bottle_count, be.bottle_size_ml, be.carbonation_method
        FROM bottling_events be
        JOIN events e ON e.id = be.event_id
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
        let mut summary = format!("{:.1} kg from {}", p.weight_kg, p.tree_name);
        if let Some(notes) = p.notes.filter(|n| !n.is_empty()) {
            summary.push_str(&format!(" — {notes}"));
        }
        timeline.push(TimelineRow {
            occurred_at: p.occurred_at,
            kind_label: "Picking",
            summary,
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
        let mut summary = format!(
            "{:.1} kg pressed to {:.1} L{yield_note}{equipment_note}",
            j.input_weight_kg, j.output_volume_l
        );
        if let Some(notes) = j.notes.filter(|n| !n.is_empty()) {
            summary.push_str(&format!(" — {notes}"));
        }
        timeline.push(TimelineRow {
            occurred_at: j.occurred_at,
            kind_label: "Juicing",
            summary,
        });
    }

    for a in additives {
        let mut summary = format!("{} {} {}", a.amount, a.unit, a.substance);
        if let Some(notes) = a.notes.filter(|n| !n.is_empty()) {
            summary.push_str(&format!(" — {notes}"));
        }
        timeline.push(TimelineRow {
            occurred_at: a.occurred_at,
            kind_label: "Additive",
            summary,
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
        if let Some(tasting_notes) = m.tasting_notes.filter(|n| !n.is_empty()) {
            parts.push(tasting_notes);
        }
        let mut summary = if parts.is_empty() {
            "measurement logged".to_string()
        } else {
            parts.join(", ")
        };
        if let Some(notes) = m.notes.filter(|n| !n.is_empty()) {
            summary.push_str(&format!(" — {notes}"));
        }
        timeline.push(TimelineRow {
            occurred_at: m.occurred_at,
            kind_label: "Measurement",
            summary,
        });
    }

    for r in rackings {
        let mut summary = format!("Racked {:.1} L", r.volume_l);
        if let Some(loss) = r.loss_l {
            summary.push_str(&format!(" (lost {loss:.1} L)"));
        }
        if let Some(notes) = r.notes.filter(|n| !n.is_empty()) {
            summary.push_str(&format!(" — {notes}"));
        }
        timeline.push(TimelineRow {
            occurred_at: r.occurred_at,
            kind_label: "Racking",
            summary,
        });
    }

    for b in bottlings {
        let mut summary = format!("{} × {} ml bottles", b.bottle_count, b.bottle_size_ml);
        if let Some(method) = b.carbonation_method.filter(|m| !m.is_empty()) {
            summary.push_str(&format!(" ({method})"));
        }
        if let Some(notes) = b.notes.filter(|n| !n.is_empty()) {
            summary.push_str(&format!(" — {notes}"));
        }
        timeline.push(TimelineRow {
            occurred_at: b.occurred_at,
            kind_label: "Bottling",
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
    let rows = sqlx::query!(
        r#"
        SELECT b.id, b.code, b.name, s.year as season_year
        FROM batches b
        JOIN seasons s ON s.id = b.season_id
        ORDER BY b.started_on DESC, b.code
        "#
    )
    .fetch_all(&state.db)
    .await?;

    // One query for every batch's event types, grouped in memory, instead
    // of one query per batch row.
    let event_type_rows = sqlx::query!("SELECT batch_id, event_type FROM events")
        .fetch_all(&state.db)
        .await?;
    let mut event_types_by_batch: std::collections::HashMap<i64, Vec<String>> =
        std::collections::HashMap::new();
    for row in event_type_rows {
        event_types_by_batch
            .entry(row.batch_id)
            .or_default()
            .push(row.event_type);
    }

    let batches = rows
        .into_iter()
        .map(|row| {
            let types = event_types_by_batch.get(&row.id).map(Vec::as_slice).unwrap_or(&[]);
            BatchListItem {
                status: compute_status(types.iter().map(String::as_str)),
                id: row.id,
                code: row.code,
                name: row.name,
                season_year: row.season_year,
            }
        })
        .collect();

    let seasons = fetch_seasons(&state.db).await?;

    Ok(ListTemplate {
        batches,
        seasons,
        today: dates::today_iso(),
    })
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<NewBatchForm>,
) -> Result<impl IntoResponse, AppError> {
    let result = sqlx::query!(
        "INSERT INTO batches (season_id, code, name, started_on) VALUES (?, ?, ?, ?)",
        form.season_id,
        form.code,
        form.name,
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
        tree_yields: timeline.tree_yields,
        timeline: timeline.timeline,
        trees,
        months: dates::month_options(),
        today_day: dates::today_day(),
        today: dates::today_iso(),
    }
    .into_response())
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateBatchForm>,
) -> Result<Response, AppError> {
    sqlx::query!(
        "UPDATE batches SET name = ?, notes = ? WHERE id = ?",
        form.name,
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
    let season_year = fetch_season_year(&state.db, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("batch not found"))?;
    let occurred_at = dates::season_date(season_year, form.month, form.day)?;

    let mut tx = state.db.begin().await?;

    let event = sqlx::query!(
        "INSERT INTO events (batch_id, event_type, occurred_at, notes) VALUES (?, 'picking', ?, ?)",
        id,
        occurred_at,
        form.notes
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
    let season_year = fetch_season_year(&state.db, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("batch not found"))?;
    let occurred_at = dates::season_date(season_year, form.month, form.day)?;

    let raw_yield_pct = form.output_volume_l / form.input_weight_kg * 100.0;
    let yield_pct = (raw_yield_pct * 10.0).round() / 10.0;

    let mut tx = state.db.begin().await?;

    let event = sqlx::query!(
        "INSERT INTO events (batch_id, event_type, occurred_at, notes) VALUES (?, 'juicing', ?, ?)",
        id,
        occurred_at,
        form.notes
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

pub async fn add_additive(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<AddAdditiveForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = state.db.begin().await?;

    let event = sqlx::query!(
        "INSERT INTO events (batch_id, event_type, occurred_at, notes) VALUES (?, 'additive', ?, ?)",
        id,
        form.occurred_at,
        form.notes
    )
    .execute(&mut *tx)
    .await?;

    let event_id = event.last_insert_rowid();

    sqlx::query!(
        "INSERT INTO additive_events (event_id, substance, amount, unit) VALUES (?, ?, ?, ?)",
        event_id,
        form.substance,
        form.amount,
        form.unit
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
        "INSERT INTO events (batch_id, event_type, occurred_at, notes) VALUES (?, 'measurement', ?, ?)",
        id,
        form.occurred_at,
        form.notes
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

pub async fn add_racking(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<AddRackingForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = state.db.begin().await?;

    let event = sqlx::query!(
        "INSERT INTO events (batch_id, event_type, occurred_at, notes) VALUES (?, 'racking', ?, ?)",
        id,
        form.occurred_at,
        form.notes
    )
    .execute(&mut *tx)
    .await?;

    let event_id = event.last_insert_rowid();

    sqlx::query!(
        "INSERT INTO racking_events (event_id, volume_l, loss_l) VALUES (?, ?, ?)",
        event_id,
        form.volume_l,
        form.loss_l
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, id).await?;
    Ok(timeline)
}

pub async fn add_bottling(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<AddBottlingForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = state.db.begin().await?;

    let event = sqlx::query!(
        "INSERT INTO events (batch_id, event_type, occurred_at, notes) VALUES (?, 'bottling', ?, ?)",
        id,
        form.occurred_at,
        form.notes
    )
    .execute(&mut *tx)
    .await?;

    let event_id = event.last_insert_rowid();

    sqlx::query!(
        "INSERT INTO bottling_events (event_id, bottle_count, bottle_size_ml, carbonation_method) VALUES (?, ?, ?, ?)",
        event_id,
        form.bottle_count,
        form.bottle_size_ml,
        form.carbonation_method
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, id).await?;
    Ok(timeline)
}
