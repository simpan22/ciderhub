//! The friends' tasting page — see docs/specs/friend-tasting-page.md.
//! Reachable with the owner's cookie or a taster's invite-link cookie
//! (`auth::require_auth` decides which, and only lets taster cookies
//! through to `/taste` paths).

use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::http::header::{REFERRER_POLICY, SET_COOKIE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Extension, Form};
use serde::Deserialize;

use crate::dates;
use crate::error::AppError;
use crate::handlers::auth::{self, CurrentTaster, InvalidLinkTemplate, ONE_YEAR_SECS, TASTER_COOKIE_NAME};
use crate::handlers::batches::{fetch_bottled_batches, insert_comparison, validate_comparison};
use crate::models::BatchChoice;
use crate::ranking::Outcome;
use crate::state::AppState;

/// One of the current taster's comparisons from today, for the list
/// under the form.
pub struct TodayRow {
    pub event_id: i64,
    pub summary: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "taste/page.html")]
struct PageTemplate {
    name: Option<String>,
    choices: Vec<BatchChoice>,
    flash: Option<Flash>,
    today_rows: Vec<TodayRow>,
}

#[derive(Template, WebTemplate)]
#[template(path = "taste/_results.html")]
struct ResultsTemplate {
    flash: Option<Flash>,
    today_rows: Vec<TodayRow>,
}

pub struct Flash {
    pub ok: bool,
    pub message: String,
}

/// `GET /taste/join/{token}`: swaps the secret link for a cookie, then
/// redirects so the token doesn't linger in the address bar.
pub async fn join(State(state): State<AppState>, Path(token): Path<String>) -> Result<Response, AppError> {
    if auth::find_taster(&state.db, &token).await?.is_none() {
        return Ok((StatusCode::NOT_FOUND, InvalidLinkTemplate).into_response());
    }

    let mut response = Redirect::to("/taste").into_response();
    let cookie = format!("{TASTER_COOKIE_NAME}={token}; Path=/taste; HttpOnly; SameSite=Lax; Max-Age={ONE_YEAR_SECS}");
    let headers = response.headers_mut();
    headers.insert(SET_COOKIE, HeaderValue::from_str(&cookie).map_err(anyhow::Error::from)?);
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    Ok(response)
}

/// Today's comparisons logged by this taster (`None` = the owner), newest first.
async fn fetch_today(db: &sqlx::SqlitePool, taster_id: Option<i64>) -> Result<Vec<TodayRow>, sqlx::Error> {
    let today = dates::today_iso();
    let rows = sqlx::query!(
        r#"
        SELECT e.id as "event_id!", b.code as batch_code, ob.code as other_code, ce.outcome
        FROM comparison_events ce
        JOIN events e ON e.id = ce.event_id
        JOIN batches b ON b.id = e.batch_id
        JOIN batches ob ON ob.id = ce.other_batch_id
        WHERE ce.taster_id IS ? AND e.occurred_at = ?
        ORDER BY e.id DESC
        "#,
        taster_id,
        today
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(TodayRow {
                event_id: r.event_id,
                summary: describe(&r.batch_code, &r.other_code, Outcome::parse(&r.outcome)?),
            })
        })
        .collect())
}

fn describe(first: &str, second: &str, outcome: Outcome) -> String {
    match outcome {
        Outcome::Win => format!("{first} over {second}"),
        Outcome::Loss => format!("{second} over {first}"),
        Outcome::Tie => format!("{first} ≈ {second}"),
    }
}

pub async fn page(
    State(state): State<AppState>,
    Extension(CurrentTaster(taster)): Extension<CurrentTaster>,
) -> Result<impl IntoResponse, AppError> {
    Ok(PageTemplate {
        choices: fetch_bottled_batches(&state.db).await?,
        today_rows: fetch_today(&state.db, taster.as_ref().map(|t| t.id)).await?,
        name: taster.map(|t| t.name),
        flash: None,
    })
}

/// `outcome` is from the first cider's point of view: the "First" /
/// "Same" / "Second" buttons send `win` / `tie` / `loss`.
#[derive(Deserialize)]
pub struct CompareForm {
    first_batch_id: i64,
    second_batch_id: i64,
    outcome: String,
    #[serde(default)]
    notes: Option<String>,
}

/// Always answers 200 with the results fragment, errors included as a
/// message — htmx doesn't swap 4xx responses by default, and a friend
/// on a phone needs to see why nothing was saved.
pub async fn compare(
    State(state): State<AppState>,
    Extension(CurrentTaster(taster)): Extension<CurrentTaster>,
    Form(form): Form<CompareForm>,
) -> Result<impl IntoResponse, AppError> {
    let taster_id = taster.as_ref().map(|t| t.id);
    let flash = save(&state.db, taster_id, &form).await?;
    Ok(ResultsTemplate {
        flash: Some(flash),
        today_rows: fetch_today(&state.db, taster_id).await?,
    })
}

async fn save(db: &sqlx::SqlitePool, taster_id: Option<i64>, form: &CompareForm) -> Result<Flash, AppError> {
    let error = |message: &str| Flash { ok: false, message: message.to_string() };

    if form.first_batch_id == form.second_batch_id {
        return Ok(error("Pick two different ciders."));
    }
    let Ok(outcome) = validate_comparison(db, form.first_batch_id, form.second_batch_id, &form.outcome).await? else {
        return Ok(error("That didn't work — reload the page and try again."));
    };

    // Unlike the main app, the server enforces the bottled-only rule
    // here: a batch can be marked failed while a friend has the page open.
    let bottled = fetch_bottled_batches(db).await?;
    let code = |id: i64| bottled.iter().find(|b| b.id == id).map(|b| b.code.clone());
    let (Some(first), Some(second)) = (code(form.first_batch_id), code(form.second_batch_id)) else {
        return Ok(error("One of those ciders can't be compared any more — reload the page."));
    };

    let notes = form.notes.as_deref().map(str::trim).filter(|n| !n.is_empty());
    insert_comparison(
        db,
        form.first_batch_id,
        form.second_batch_id,
        outcome,
        &dates::today_iso(),
        notes,
        taster_id,
    )
    .await?;

    Ok(Flash {
        ok: true,
        message: format!("Saved: {}", describe(&first, &second, outcome)),
    })
}

/// `DELETE /taste/comparisons/{event_id}`: undo, only for this
/// taster's own comparisons from today. Deleting the spine row cascades
/// to the comparison_events row.
pub async fn undo(
    State(state): State<AppState>,
    Extension(CurrentTaster(taster)): Extension<CurrentTaster>,
    Path(event_id): Path<i64>,
) -> Result<Response, AppError> {
    let taster_id = taster.as_ref().map(|t| t.id);
    let today = dates::today_iso();
    let deleted = sqlx::query!(
        r#"
        DELETE FROM events
        WHERE id = ? AND event_type = 'comparison' AND occurred_at = ?
          AND id IN (SELECT event_id FROM comparison_events WHERE taster_id IS ?)
        "#,
        event_id,
        today,
        taster_id
    )
    .execute(&state.db)
    .await?;

    if deleted.rows_affected() == 0 {
        return Ok((StatusCode::NOT_FOUND, "comparison not found").into_response());
    }

    Ok(ResultsTemplate {
        flash: Some(Flash { ok: true, message: "Undone.".to_string() }),
        today_rows: fetch_today(&state.db, taster_id).await?,
    }
    .into_response())
}
