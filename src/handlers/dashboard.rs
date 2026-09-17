use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::charts::{render_line_chart, Series};
use crate::models::{BatchOption, ChartSection, MetricOption};
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

    Ok(IndexTemplate {
        batches,
        metrics,
        charts,
    }
    .into_response())
}

pub async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "database unavailable"),
    }
}
