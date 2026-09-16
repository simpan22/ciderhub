use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Form;

use crate::error::AppError;
use crate::models::{Season, SeasonForm};
use crate::state::AppState;

pub struct SeasonRow {
    pub season: Season,
    pub editing: bool,
}

async fn fetch_rows(
    db: &sqlx::SqlitePool,
    editing_id: Option<i64>,
) -> Result<Vec<SeasonRow>, sqlx::Error> {
    let seasons = sqlx::query_as!(
        Season,
        "SELECT id, name, starts_on, ends_on FROM seasons ORDER BY starts_on DESC, name"
    )
    .fetch_all(db)
    .await?;

    Ok(seasons
        .into_iter()
        .map(|season| SeasonRow {
            editing: Some(season.id) == editing_id,
            season,
        })
        .collect())
}

#[derive(Template, WebTemplate)]
#[template(path = "seasons/_table_body.html")]
struct TableBodyTemplate {
    rows: Vec<SeasonRow>,
}

#[derive(Template, WebTemplate)]
#[template(path = "seasons/list.html")]
struct ListTemplate {
    rows: Vec<SeasonRow>,
}

pub async fn list(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let rows = fetch_rows(&state.db, None).await?;
    Ok(ListTemplate { rows })
}

pub async fn table_fragment(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}

pub async fn edit_fragment(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let rows = fetch_rows(&state.db, Some(id)).await?;
    Ok(TableBodyTemplate { rows })
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<SeasonForm>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query!(
        "INSERT INTO seasons (name, starts_on, ends_on) VALUES (?, ?, ?)",
        form.name,
        form.starts_on,
        form.ends_on
    )
    .execute(&state.db)
    .await?;

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<SeasonForm>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query!(
        "UPDATE seasons SET name = ?, starts_on = ?, ends_on = ? WHERE id = ?",
        form.name,
        form.starts_on,
        form.ends_on,
        id
    )
    .execute(&state.db)
    .await?;

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query!("DELETE FROM seasons WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}
