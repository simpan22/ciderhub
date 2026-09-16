use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Form;

use crate::error::AppError;
use crate::models::{Vessel, VesselForm};
use crate::state::AppState;

pub struct VesselRow {
    pub vessel: Vessel,
    pub editing: bool,
}

async fn fetch_rows(
    db: &sqlx::SqlitePool,
    editing_id: Option<i64>,
) -> Result<Vec<VesselRow>, sqlx::Error> {
    let vessels = sqlx::query_as!(
        Vessel,
        r#"SELECT id as "id!", name, capacity_l, kind, active as "active: bool" FROM vessels ORDER BY name"#
    )
    .fetch_all(db)
    .await?;

    Ok(vessels
        .into_iter()
        .map(|vessel| VesselRow {
            editing: Some(vessel.id) == editing_id,
            vessel,
        })
        .collect())
}

#[derive(Template, WebTemplate)]
#[template(path = "vessels/_table_body.html")]
struct TableBodyTemplate {
    rows: Vec<VesselRow>,
}

#[derive(Template, WebTemplate)]
#[template(path = "vessels/list.html")]
struct ListTemplate {
    rows: Vec<VesselRow>,
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
    Form(form): Form<VesselForm>,
) -> Result<impl IntoResponse, AppError> {
    let active = form.active.is_some();
    sqlx::query!(
        "INSERT INTO vessels (name, capacity_l, kind, active) VALUES (?, ?, ?, ?)",
        form.name,
        form.capacity_l,
        form.kind,
        active
    )
    .execute(&state.db)
    .await?;

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<VesselForm>,
) -> Result<impl IntoResponse, AppError> {
    let active = form.active.is_some();
    sqlx::query!(
        "UPDATE vessels SET name = ?, capacity_l = ?, kind = ?, active = ? WHERE id = ?",
        form.name,
        form.capacity_l,
        form.kind,
        active,
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
    sqlx::query!("DELETE FROM vessels WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}
