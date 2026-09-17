use std::collections::HashMap;

use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Query, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use serde::Deserialize;

use crate::charts::{render_line_chart, render_season_timeline, Series};
use crate::dates;
use crate::error::AppError;
use crate::models::{BatchOption, BatchTimelineRow, ChartSection, MetricOption, SeasonOption};
use crate::state::AppState;

const METRICS: [(&str, &str); 5] = [
    ("specific_gravity", "Specific Gravity"),
    ("ph", "pH"),
    ("temperature_c", "Temperature (°C)"),
    ("volume_l", "Volume (L)"),
    ("score", "Taste Score"),
];

/// Query params arrive as repeated `batch_id=1&batch_id=2&metric=ph`
/// pairs, which serde_urlencoded (what axum's typed `Query` extractor
/// uses) can't collect into a `Vec<T>` field — confirmed this directly
/// rather than assuming, given how the last assumption-not-verified
/// mistake went. Parsing the raw query string by hand sidesteps that
/// limitation entirely; every value here is either numeric or one of
/// our own fixed metric slugs, so no percent-decoding is needed.
fn parse_selection(raw: &str) -> (Vec<i64>, Vec<String>) {
    let mut batch_ids = Vec::new();
    let mut metrics = Vec::new();

    for pair in raw.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("");
        match key {
            "batch_id" => {
                if let Ok(id) = value.parse::<i64>() {
                    batch_ids.push(id);
                }
            }
            "metric" => metrics.push(value.to_string()),
            _ => {}
        }
    }

    (batch_ids, metrics)
}

async fn fetch_measurement_series(
    db: &sqlx::SqlitePool,
    batch_id: i64,
    metric: &str,
) -> Result<Vec<(String, f64)>, sqlx::Error> {
    let rows = match metric {
        "specific_gravity" => {
            sqlx::query!(
                r#"SELECT e.occurred_at, me.specific_gravity as "value!: f64"
                   FROM measurement_events me JOIN events e ON e.id = me.event_id
                   WHERE e.batch_id = ? AND me.specific_gravity IS NOT NULL
                   ORDER BY e.occurred_at"#,
                batch_id
            )
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| (r.occurred_at, r.value))
            .collect()
        }
        "ph" => {
            sqlx::query!(
                r#"SELECT e.occurred_at, me.ph as "value!: f64"
                   FROM measurement_events me JOIN events e ON e.id = me.event_id
                   WHERE e.batch_id = ? AND me.ph IS NOT NULL
                   ORDER BY e.occurred_at"#,
                batch_id
            )
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| (r.occurred_at, r.value))
            .collect()
        }
        "temperature_c" => {
            sqlx::query!(
                r#"SELECT e.occurred_at, me.temperature_c as "value!: f64"
                   FROM measurement_events me JOIN events e ON e.id = me.event_id
                   WHERE e.batch_id = ? AND me.temperature_c IS NOT NULL
                   ORDER BY e.occurred_at"#,
                batch_id
            )
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| (r.occurred_at, r.value))
            .collect()
        }
        "volume_l" => {
            sqlx::query!(
                r#"SELECT e.occurred_at, me.volume_l as "value!: f64"
                   FROM measurement_events me JOIN events e ON e.id = me.event_id
                   WHERE e.batch_id = ? AND me.volume_l IS NOT NULL
                   ORDER BY e.occurred_at"#,
                batch_id
            )
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| (r.occurred_at, r.value))
            .collect()
        }
        "score" => sqlx::query!(
            r#"SELECT e.occurred_at, te.score as "value!: i64"
               FROM tasting_events te JOIN events e ON e.id = te.event_id
               WHERE e.batch_id = ?
               ORDER BY e.occurred_at"#,
            batch_id
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| (r.occurred_at, r.value as f64))
        .collect(),
        _ => Vec::new(),
    };

    Ok(rows)
}

#[derive(Template, WebTemplate)]
#[template(path = "dashboard/_results.html")]
struct ResultsTemplate {
    charts: Vec<ChartSection>,
}

#[derive(Template, WebTemplate)]
#[template(path = "index.html")]
struct IndexTemplate {
    batches: Vec<BatchOption>,
    metrics: Vec<MetricOption>,
    charts: Vec<ChartSection>,
    seasons: Vec<SeasonOption>,
    timeline_svg: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "dashboard/_season_overview.html")]
struct SeasonOverviewTemplate {
    seasons: Vec<SeasonOption>,
    timeline_svg: String,
}

#[derive(Deserialize)]
pub struct SeasonOverviewQuery {
    season_id: Option<i64>,
}

/// One batch's phase boundaries, resolved from its events — the start
/// (first event of any kind), the first racking/bottling/failed event
/// dates if present, and the effective end of its bar (failed, else
/// bottled, else today). Batches with no events yet are left out
/// entirely, same reasoning as batch-start-date's `None`: there's
/// nothing to place them at.
async fn fetch_batch_phases(db: &sqlx::SqlitePool, season_id: i64) -> Result<Vec<BatchTimelineRow>, sqlx::Error> {
    let batches = sqlx::query!(
        r#"SELECT id as "id!", code FROM batches WHERE season_id = ? ORDER BY code"#,
        season_id
    )
    .fetch_all(db)
    .await?;

    if batches.is_empty() {
        return Ok(Vec::new());
    }

    let events = sqlx::query!(
        r#"
        SELECT e.batch_id as "batch_id!", e.event_type, e.occurred_at
        FROM events e
        JOIN batches b ON b.id = e.batch_id
        WHERE b.season_id = ?
        ORDER BY e.occurred_at
        "#,
        season_id
    )
    .fetch_all(db)
    .await?;

    #[derive(Default)]
    struct Acc {
        start: Option<String>,
        racking_at: Option<String>,
        bottling_at: Option<String>,
        failed_at: Option<String>,
    }

    let mut by_batch: HashMap<i64, Acc> = HashMap::new();
    for row in events {
        let acc = by_batch.entry(row.batch_id).or_default();
        if acc.start.is_none() {
            acc.start = Some(row.occurred_at.clone());
        }
        match row.event_type.as_str() {
            "racking" if acc.racking_at.is_none() => acc.racking_at = Some(row.occurred_at),
            "bottling" if acc.bottling_at.is_none() => acc.bottling_at = Some(row.occurred_at),
            "failed" if acc.failed_at.is_none() => acc.failed_at = Some(row.occurred_at),
            _ => {}
        }
    }

    let today = dates::today_iso();
    let rows = batches
        .into_iter()
        .filter_map(|b| {
            let acc = by_batch.remove(&b.id)?;
            let start = acc.start?;
            // `.max(start.clone())` guards against a future-dated event
            // (typo, or a deliberately backdated-forward entry) making
            // an in-progress batch's `end` (today) land before its own
            // `start` — every consumer of this row assumes end >=
            // start. ISO `YYYY-MM-DD` strings compare lexicographically
            // in chronological order, so plain `max` is correct here.
            let end = acc
                .failed_at
                .clone()
                .or_else(|| acc.bottling_at.clone())
                .unwrap_or_else(|| today.clone())
                .max(start.clone());
            Some(BatchTimelineRow {
                code: b.code,
                start,
                racking_at: acc.racking_at,
                bottling_at: acc.bottling_at,
                failed_at: acc.failed_at,
                end,
            })
        })
        .collect();

    Ok(rows)
}

/// Shared by the full dashboard render and the htmx fragment endpoint
/// so both always agree on which season is selected by default (the
/// most recent one) and how the timeline for it is built.
async fn build_season_overview(
    db: &sqlx::SqlitePool,
    season_id: Option<i64>,
) -> Result<(Vec<SeasonOption>, String), sqlx::Error> {
    let season_rows = sqlx::query!(r#"SELECT id as "id!", year FROM seasons ORDER BY year DESC"#)
        .fetch_all(db)
        .await?;

    let selected_season_id = season_id.or_else(|| season_rows.first().map(|s| s.id));

    let seasons = season_rows
        .iter()
        .map(|s| SeasonOption {
            id: s.id,
            year: s.year,
            selected: Some(s.id) == selected_season_id,
        })
        .collect();

    let timeline_svg = match selected_season_id {
        Some(id) => render_season_timeline(&fetch_batch_phases(db, id).await?),
        None => "<p class=\"muted\">No seasons yet.</p>".to_string(),
    };

    Ok((seasons, timeline_svg))
}

pub async fn season_overview(
    State(state): State<AppState>,
    Query(query): Query<SeasonOverviewQuery>,
) -> Result<impl IntoResponse, AppError> {
    let (seasons, timeline_svg) = build_season_overview(&state.db, query.season_id).await?;
    Ok(SeasonOverviewTemplate { seasons, timeline_svg })
}

async fn build_charts(
    db: &sqlx::SqlitePool,
    batch_ids: &[i64],
    metrics: &[String],
) -> Result<Vec<ChartSection>, sqlx::Error> {
    let batch_codes: Vec<(i64, String)> = sqlx::query!("SELECT id as \"id!\", code FROM batches")
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| (r.id, r.code))
        .collect();

    let mut charts = Vec::new();
    for (slug, label) in METRICS {
        if !metrics.iter().any(|m| m == slug) {
            continue;
        }

        let mut series = Vec::new();
        for &batch_id in batch_ids {
            let points = fetch_measurement_series(db, batch_id, slug).await?;
            if points.is_empty() {
                continue;
            }
            let batch_code = batch_codes
                .iter()
                .find(|(id, _)| *id == batch_id)
                .map(|(_, code)| code.clone())
                .unwrap_or_else(|| format!("#{batch_id}"));
            series.push(Series { batch_code, points });
        }

        let svg = if series.is_empty() {
            "<p class=\"muted\">No data for the selected batches.</p>".to_string()
        } else {
            render_line_chart(&series)
        };

        charts.push(ChartSection { label, svg });
    }

    Ok(charts)
}

pub async fn index(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
) -> Result<Response, crate::error::AppError> {
    let (selected_batches, selected_metrics) = parse_selection(&raw_query.unwrap_or_default());

    let charts = build_charts(&state.db, &selected_batches, &selected_metrics).await?;

    if headers.get("HX-Request").is_some() {
        return Ok(ResultsTemplate { charts }.into_response());
    }

    let all_batches = sqlx::query!("SELECT id as \"id!\", code FROM batches ORDER BY code")
        .fetch_all(&state.db)
        .await?;

    let batches = all_batches
        .into_iter()
        .map(|b| BatchOption {
            checked: selected_batches.contains(&b.id),
            id: b.id,
            code: b.code,
        })
        .collect();

    let metrics = METRICS
        .iter()
        .map(|(slug, label)| MetricOption {
            slug,
            label,
            checked: selected_metrics.iter().any(|m| m == slug),
        })
        .collect();

    let (seasons, timeline_svg) = build_season_overview(&state.db, None).await?;

    Ok(IndexTemplate {
        batches,
        metrics,
        charts,
        seasons,
        timeline_svg,
    }
    .into_response())
}

pub async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "database unavailable"),
    }
}
