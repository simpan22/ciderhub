use std::collections::HashMap;

use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;

use crate::dates;
use crate::error::AppError;
use crate::models::{
    AddAdditiveForm, AddBottlingForm, AddMeasurementForm, AddRackingForm, BatchListItem,
    MonthOption, NewBatchForm, Season, TimelineRow, Tree, TreeWeightField, TreeYield,
    UpdateBatchForm,
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
    batch_id: i64,
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
    batch_id: i64,
    tree_yields: Vec<TreeYield>,
    timeline: Vec<TimelineRow>,
    tree_fields: Vec<TreeWeightField>,
    months: Vec<MonthOption>,
    today_day: u32,
    today: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/_edit_juicing.html")]
struct EditJuicingTemplate {
    batch_id: i64,
    event_id: i64,
    months: Vec<MonthOption>,
    day: u32,
    output_volume_l: f64,
    equipment: Option<String>,
    notes: Option<String>,
    tree_fields: Vec<TreeWeightField>,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/_edit_additive.html")]
struct EditAdditiveTemplate {
    batch_id: i64,
    event_id: i64,
    occurred_at: String,
    substance: String,
    amount: f64,
    unit: String,
    notes: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/_edit_measurement.html")]
struct EditMeasurementTemplate {
    batch_id: i64,
    event_id: i64,
    occurred_at: String,
    specific_gravity: Option<f64>,
    ph: Option<f64>,
    temperature_c: Option<f64>,
    volume_l: Option<f64>,
    tasting_notes: Option<String>,
    notes: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/_edit_racking.html")]
struct EditRackingTemplate {
    batch_id: i64,
    event_id: i64,
    occurred_at: String,
    volume_l: f64,
    loss_l: Option<f64>,
    notes: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "batches/_edit_bottling.html")]
struct EditBottlingTemplate {
    batch_id: i64,
    event_id: i64,
    occurred_at: String,
    bottle_count: i64,
    bottle_size_ml: i64,
    carbonation_method: Option<String>,
    notes: Option<String>,
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

/// Builds the tree checkbox list for the juicing form: `selected` maps a
/// tree id to its weight (or `None` if included but unweighed) for the
/// trees that are part of this event; every other tree renders unchecked.
fn build_tree_fields(all_trees: &[Tree], selected: &HashMap<i64, Option<f64>>) -> Vec<TreeWeightField> {
    all_trees
        .iter()
        .map(|t| TreeWeightField {
            tree_id: t.id,
            tree_name: t.name.clone(),
            checked: selected.contains_key(&t.id),
            weight_kg: selected.get(&t.id).copied().flatten(),
        })
        .collect()
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
        SELECT b.id as "id!", b.code, b.name, b.notes, s.year as season_year
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
    let juicings = sqlx::query!(
        r#"
        SELECT e.id as "event_id!", e.occurred_at, e.notes, je.output_volume_l, je.yield_pct, je.equipment
        FROM juicing_events je
        JOIN events e ON e.id = je.event_id
        WHERE e.batch_id = ?
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let juicing_tree_rows = sqlx::query!(
        r#"
        SELECT jtw.event_id, t.name as tree_name, jtw.weight_kg
        FROM juicing_tree_weights jtw
        JOIN juicing_events je ON je.event_id = jtw.event_id
        JOIN events e ON e.id = je.event_id
        JOIN trees t ON t.id = jtw.tree_id
        WHERE e.batch_id = ?
        ORDER BY t.name
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let mut trees_by_event: HashMap<i64, Vec<(String, Option<f64>)>> = HashMap::new();
    for row in juicing_tree_rows {
        trees_by_event
            .entry(row.event_id)
            .or_default()
            .push((row.tree_name, row.weight_kg));
    }

    let measurements = sqlx::query!(
        r#"
        SELECT e.id as "event_id!", e.occurred_at, e.notes, me.specific_gravity, me.ph, me.temperature_c, me.volume_l, me.tasting_notes
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
        SELECT e.id as "event_id!", e.occurred_at, e.notes, ae.substance, ae.amount, ae.unit
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
        SELECT e.id as "event_id!", e.occurred_at, e.notes, re.volume_l, re.loss_l
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
        SELECT e.id as "event_id!", e.occurred_at, e.notes, be.bottle_count, be.bottle_size_ml, be.carbonation_method
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
        SELECT t.name as tree_name, SUM(jtw.weight_kg) as "total_kg!: f64"
        FROM juicing_tree_weights jtw
        JOIN juicing_events je ON je.event_id = jtw.event_id
        JOIN events e ON e.id = je.event_id
        JOIN trees t ON t.id = jtw.tree_id
        WHERE e.batch_id = ? AND jtw.weight_kg IS NOT NULL
        GROUP BY t.id, t.name
        ORDER BY t.name
        "#,
        batch_id
    )
    .fetch_all(db)
    .await?;

    let mut timeline: Vec<TimelineRow> = Vec::new();

    for j in juicings {
        let tree_parts: Vec<String> = trees_by_event
            .remove(&j.event_id)
            .unwrap_or_default()
            .into_iter()
            .map(|(name, weight)| match weight {
                Some(w) => format!("{name} ({w:.1} kg)"),
                None => name,
            })
            .collect();
        let trees_note = if tree_parts.is_empty() {
            String::new()
        } else {
            format!(" from {}", tree_parts.join(", "))
        };
        let yield_note = match j.yield_pct {
            Some(pct) => format!(" ({pct:.0}% yield)"),
            None => String::new(),
        };
        let equipment_note = match j.equipment {
            Some(eq) if !eq.is_empty() => format!(" via {eq}"),
            _ => String::new(),
        };
        let mut summary = format!(
            "Pressed{trees_note} to {:.1} L{yield_note}{equipment_note}",
            j.output_volume_l
        );
        if let Some(notes) = j.notes.filter(|n| !n.is_empty()) {
            summary.push_str(&format!(" — {notes}"));
        }
        timeline.push(TimelineRow {
            event_id: j.event_id,
            occurred_at: j.occurred_at,
            kind_label: "Juicing",
            kind_slug: "juicing",
            summary,
        });
    }

    for a in additives {
        let mut summary = format!("{} {} {}", a.amount, a.unit, a.substance);
        if let Some(notes) = a.notes.filter(|n| !n.is_empty()) {
            summary.push_str(&format!(" — {notes}"));
        }
        timeline.push(TimelineRow {
            event_id: a.event_id,
            occurred_at: a.occurred_at,
            kind_label: "Additive",
            kind_slug: "additive",
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
            event_id: m.event_id,
            occurred_at: m.occurred_at,
            kind_label: "Measurement",
            kind_slug: "measurement",
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
            event_id: r.event_id,
            occurred_at: r.occurred_at,
            kind_label: "Racking",
            kind_slug: "racking",
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
            event_id: b.event_id,
            occurred_at: b.occurred_at,
            kind_label: "Bottling",
            kind_slug: "bottling",
            summary,
        });
    }

    timeline.sort_by(|a, b| a.occurred_at.cmp(&b.occurred_at));

    Ok(TimelineTemplate {
        batch_id,
        tree_yields,
        timeline,
    })
}

pub async fn list(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT b.id as "id!", b.code, b.name, s.year as season_year
        FROM batches b
        JOIN seasons s ON s.id = b.season_id
        ORDER BY b.code ASC
        "#
    )
    .fetch_all(&state.db)
    .await?;

    // One query for every batch's event types, grouped in memory, instead
    // of one query per batch row.
    let event_type_rows = sqlx::query!("SELECT batch_id, event_type FROM events")
        .fetch_all(&state.db)
        .await?;
    let mut event_types_by_batch: HashMap<i64, Vec<String>> = HashMap::new();
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
    let tree_fields = build_tree_fields(&trees, &HashMap::new());

    Ok(DetailTemplate {
        batch_id: meta.batch.id,
        batch: meta.batch,
        tree_yields: timeline.tree_yields,
        timeline: timeline.timeline,
        tree_fields,
        months: dates::month_options(),
        today_day: dates::today_day(),
        today: dates::today_iso(),
    }
    .into_response())
}

/// Fresh, unedited timeline fragment — used by the "Cancel" button on an
/// event's edit form to drop back out of edit mode without saving.
pub async fn timeline_fragment(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    Ok(fetch_timeline(&state.db, id).await?)
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

// ---- juicing (picking is expressed as tree contributions here) ----

/// Pulls the fixed scalar fields plus the raw tree checkbox/weight pairs
/// out of a juicing add/edit submission. Raw `HashMap` parsing (rather
/// than a typed `Form<T>`) is needed because the tree fields are keyed
/// by each tree's id (`use_tree_3`, `weight_tree_3`, ...), a set that
/// isn't known until we've looked up which trees exist.
struct JuicingSubmission {
    month: u32,
    day: u32,
    output_volume_l: f64,
    equipment: Option<String>,
    notes: Option<String>,
    tree_weights: Vec<(i64, Option<f64>)>,
}

fn parse_juicing_submission(
    raw: &HashMap<String, String>,
    all_trees: &[Tree],
) -> anyhow::Result<JuicingSubmission> {
    let get_required = |key: &str| -> anyhow::Result<String> {
        raw.get(key)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("missing required field: {key}"))
    };
    let get_optional = |key: &str| -> Option<String> {
        raw.get(key)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let month: u32 = get_required("month")?.parse()?;
    let day: u32 = get_required("day")?.parse()?;
    let output_volume_l: f64 = get_required("output_volume_l")?.parse()?;
    let equipment = get_optional("equipment");
    let notes = get_optional("notes");

    let mut tree_weights = Vec::new();
    for tree in all_trees {
        if raw.contains_key(&format!("use_tree_{}", tree.id)) {
            let weight = get_optional(&format!("weight_tree_{}", tree.id))
                .and_then(|s| s.parse::<f64>().ok());
            tree_weights.push((tree.id, weight));
        }
    }

    Ok(JuicingSubmission {
        month,
        day,
        output_volume_l,
        equipment,
        notes,
        tree_weights,
    })
}

fn compute_yield_pct(output_volume_l: f64, tree_weights: &[(i64, Option<f64>)]) -> Option<f64> {
    let total_weight: f64 = tree_weights.iter().filter_map(|(_, w)| *w).sum();
    if total_weight > 0.0 {
        let raw_pct = output_volume_l / total_weight * 100.0;
        Some((raw_pct * 10.0).round() / 10.0)
    } else {
        None
    }
}

pub async fn add_juicing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(raw): Form<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let season_year = fetch_season_year(&state.db, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("batch not found"))?;
    let all_trees = fetch_trees(&state.db).await?;
    let submission = parse_juicing_submission(&raw, &all_trees)?;
    let occurred_at = dates::season_date(season_year, submission.month, submission.day)?;
    let yield_pct = compute_yield_pct(submission.output_volume_l, &submission.tree_weights);

    let mut tx = state.db.begin().await?;

    let event = sqlx::query!(
        "INSERT INTO events (batch_id, event_type, occurred_at, notes) VALUES (?, 'juicing', ?, ?)",
        id,
        occurred_at,
        submission.notes
    )
    .execute(&mut *tx)
    .await?;

    let event_id = event.last_insert_rowid();

    sqlx::query!(
        "INSERT INTO juicing_events (event_id, output_volume_l, yield_pct, equipment) VALUES (?, ?, ?, ?)",
        event_id,
        submission.output_volume_l,
        yield_pct,
        submission.equipment
    )
    .execute(&mut *tx)
    .await?;

    for (tree_id, weight) in submission.tree_weights {
        sqlx::query!(
            "INSERT INTO juicing_tree_weights (event_id, tree_id, weight_kg) VALUES (?, ?, ?)",
            event_id,
            tree_id,
            weight
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, id).await?;
    Ok(timeline)
}

pub async fn edit_juicing_fragment(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT e.occurred_at, je.output_volume_l, je.equipment, e.notes
        FROM juicing_events je
        JOIN events e ON e.id = je.event_id
        WHERE e.id = ? AND e.batch_id = ?
        "#,
        event_id,
        batch_id
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Ok((StatusCode::NOT_FOUND, "event not found").into_response());
    };

    let tree_weight_rows = sqlx::query!(
        "SELECT tree_id, weight_kg FROM juicing_tree_weights WHERE event_id = ?",
        event_id
    )
    .fetch_all(&state.db)
    .await?;
    let selected: HashMap<i64, Option<f64>> = tree_weight_rows
        .into_iter()
        .map(|r| (r.tree_id, r.weight_kg))
        .collect();

    let all_trees = fetch_trees(&state.db).await?;
    let (month, day) = dates::month_day_of(&row.occurred_at)?;

    Ok(EditJuicingTemplate {
        batch_id,
        event_id,
        months: dates::month_options_selected(month),
        day,
        output_volume_l: row.output_volume_l,
        equipment: row.equipment,
        notes: row.notes,
        tree_fields: build_tree_fields(&all_trees, &selected),
    }
    .into_response())
}

pub async fn update_juicing(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
    Form(raw): Form<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let season_year = fetch_season_year(&state.db, batch_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("batch not found"))?;
    let all_trees = fetch_trees(&state.db).await?;
    let submission = parse_juicing_submission(&raw, &all_trees)?;
    let occurred_at = dates::season_date(season_year, submission.month, submission.day)?;
    let yield_pct = compute_yield_pct(submission.output_volume_l, &submission.tree_weights);

    let mut tx = state.db.begin().await?;

    sqlx::query!(
        "UPDATE events SET occurred_at = ?, notes = ? WHERE id = ? AND batch_id = ?",
        occurred_at,
        submission.notes,
        event_id,
        batch_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE juicing_events SET output_volume_l = ?, yield_pct = ?, equipment = ? WHERE event_id = ?",
        submission.output_volume_l,
        yield_pct,
        submission.equipment,
        event_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!("DELETE FROM juicing_tree_weights WHERE event_id = ?", event_id)
        .execute(&mut *tx)
        .await?;

    for (tree_id, weight) in submission.tree_weights {
        sqlx::query!(
            "INSERT INTO juicing_tree_weights (event_id, tree_id, weight_kg) VALUES (?, ?, ?)",
            event_id,
            tree_id,
            weight
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, batch_id).await?;
    Ok(timeline)
}

// ---- additive ----

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

pub async fn edit_additive_fragment(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT e.occurred_at, ae.substance, ae.amount, ae.unit, e.notes
        FROM additive_events ae
        JOIN events e ON e.id = ae.event_id
        WHERE e.id = ? AND e.batch_id = ?
        "#,
        event_id,
        batch_id
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Ok((StatusCode::NOT_FOUND, "event not found").into_response());
    };

    Ok(EditAdditiveTemplate {
        batch_id,
        event_id,
        occurred_at: row.occurred_at,
        substance: row.substance,
        amount: row.amount,
        unit: row.unit,
        notes: row.notes,
    }
    .into_response())
}

pub async fn update_additive(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
    Form(form): Form<AddAdditiveForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = state.db.begin().await?;

    sqlx::query!(
        "UPDATE events SET occurred_at = ?, notes = ? WHERE id = ? AND batch_id = ?",
        form.occurred_at,
        form.notes,
        event_id,
        batch_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE additive_events SET substance = ?, amount = ?, unit = ? WHERE event_id = ?",
        form.substance,
        form.amount,
        form.unit,
        event_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, batch_id).await?;
    Ok(timeline)
}

// ---- measurement ----

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

pub async fn edit_measurement_fragment(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT e.occurred_at, me.specific_gravity, me.ph, me.temperature_c, me.volume_l, me.tasting_notes, e.notes
        FROM measurement_events me
        JOIN events e ON e.id = me.event_id
        WHERE e.id = ? AND e.batch_id = ?
        "#,
        event_id,
        batch_id
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Ok((StatusCode::NOT_FOUND, "event not found").into_response());
    };

    Ok(EditMeasurementTemplate {
        batch_id,
        event_id,
        occurred_at: row.occurred_at,
        specific_gravity: row.specific_gravity,
        ph: row.ph,
        temperature_c: row.temperature_c,
        volume_l: row.volume_l,
        tasting_notes: row.tasting_notes,
        notes: row.notes,
    }
    .into_response())
}

pub async fn update_measurement(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
    Form(form): Form<AddMeasurementForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = state.db.begin().await?;

    sqlx::query!(
        "UPDATE events SET occurred_at = ?, notes = ? WHERE id = ? AND batch_id = ?",
        form.occurred_at,
        form.notes,
        event_id,
        batch_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE measurement_events SET specific_gravity = ?, ph = ?, temperature_c = ?, volume_l = ?, tasting_notes = ? WHERE event_id = ?",
        form.specific_gravity,
        form.ph,
        form.temperature_c,
        form.volume_l,
        form.tasting_notes,
        event_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, batch_id).await?;
    Ok(timeline)
}

// ---- racking ----

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

pub async fn edit_racking_fragment(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT e.occurred_at, re.volume_l, re.loss_l, e.notes
        FROM racking_events re
        JOIN events e ON e.id = re.event_id
        WHERE e.id = ? AND e.batch_id = ?
        "#,
        event_id,
        batch_id
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Ok((StatusCode::NOT_FOUND, "event not found").into_response());
    };

    Ok(EditRackingTemplate {
        batch_id,
        event_id,
        occurred_at: row.occurred_at,
        volume_l: row.volume_l,
        loss_l: row.loss_l,
        notes: row.notes,
    }
    .into_response())
}

pub async fn update_racking(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
    Form(form): Form<AddRackingForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = state.db.begin().await?;

    sqlx::query!(
        "UPDATE events SET occurred_at = ?, notes = ? WHERE id = ? AND batch_id = ?",
        form.occurred_at,
        form.notes,
        event_id,
        batch_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE racking_events SET volume_l = ?, loss_l = ? WHERE event_id = ?",
        form.volume_l,
        form.loss_l,
        event_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, batch_id).await?;
    Ok(timeline)
}

// ---- bottling ----

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

pub async fn edit_bottling_fragment(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT e.occurred_at, be.bottle_count, be.bottle_size_ml, be.carbonation_method, e.notes
        FROM bottling_events be
        JOIN events e ON e.id = be.event_id
        WHERE e.id = ? AND e.batch_id = ?
        "#,
        event_id,
        batch_id
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Ok((StatusCode::NOT_FOUND, "event not found").into_response());
    };

    Ok(EditBottlingTemplate {
        batch_id,
        event_id,
        occurred_at: row.occurred_at,
        bottle_count: row.bottle_count,
        bottle_size_ml: row.bottle_size_ml,
        carbonation_method: row.carbonation_method,
        notes: row.notes,
    }
    .into_response())
}

pub async fn update_bottling(
    State(state): State<AppState>,
    Path((batch_id, event_id)): Path<(i64, i64)>,
    Form(form): Form<AddBottlingForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut tx = state.db.begin().await?;

    sqlx::query!(
        "UPDATE events SET occurred_at = ?, notes = ? WHERE id = ? AND batch_id = ?",
        form.occurred_at,
        form.notes,
        event_id,
        batch_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE bottling_events SET bottle_count = ?, bottle_size_ml = ?, carbonation_method = ? WHERE event_id = ?",
        form.bottle_count,
        form.bottle_size_ml,
        form.carbonation_method,
        event_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let timeline = fetch_timeline(&state.db, batch_id).await?;
    Ok(timeline)
}
