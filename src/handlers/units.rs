use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Form;

use crate::error::AppError;
use crate::models::{Unit, UnitForm};
use crate::state::AppState;

pub struct UnitRow {
    pub unit: Unit,
    pub editing: bool,
    pub in_use: bool,
}

async fn fetch_rows(
    db: &sqlx::SqlitePool,
    editing_id: Option<i64>,
) -> Result<Vec<UnitRow>, sqlx::Error> {
    let units = sqlx::query_as!(Unit, r#"SELECT id as "id!", name FROM units ORDER BY name"#)
        .fetch_all(db)
        .await?;

    let mut rows = Vec::with_capacity(units.len());
    for unit in units {
        let in_use = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM additive_events WHERE unit_id = ?",
            unit.id
        )
        .fetch_one(db)
        .await?
            > 0;

        rows.push(UnitRow {
            editing: Some(unit.id) == editing_id,
            unit,
            in_use,
        });
    }

    Ok(rows)
}

/// Trims and lowercases `raw_name`, then finds an existing unit with that
/// name or creates one. Used so submitting "Sugar" when "sugar" already
/// exists reuses the existing row instead of creating a case-drifted
/// duplicate.
async fn find_or_create_unit(db: &sqlx::SqlitePool, raw_name: &str) -> Result<i64, sqlx::Error> {
    let name = raw_name.trim().to_lowercase();

    if let Some(id) = sqlx::query_scalar!(r#"SELECT id as "id!" FROM units WHERE name = ?"#, name)
        .fetch_optional(db)
        .await?
    {
        return Ok(id);
    }

    let result = sqlx::query!("INSERT INTO units (name) VALUES (?)", name)
        .execute(db)
        .await?;
    Ok(result.last_insert_rowid())
}

#[derive(Template, WebTemplate)]
#[template(path = "units/_table_body.html")]
struct TableBodyTemplate {
    rows: Vec<UnitRow>,
}

#[derive(Template, WebTemplate)]
#[template(path = "units/list.html")]
struct ListTemplate {
    rows: Vec<UnitRow>,
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
    Form(form): Form<UnitForm>,
) -> Result<impl IntoResponse, AppError> {
    find_or_create_unit(&state.db, &form.name).await?;

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<UnitForm>,
) -> Result<impl IntoResponse, AppError> {
    let name = form.name.trim().to_lowercase();

    // Refuse a rename that would collide with a *different* existing
    // unit — sqlite's UNIQUE constraint would catch this too, but only
    // as an opaque error; checking first lets it fail quietly instead.
    let collision = sqlx::query_scalar!(r#"SELECT id as "id!" FROM units WHERE name = ? AND id != ?"#, name, id)
        .fetch_optional(&state.db)
        .await?;

    if collision.is_none() {
        sqlx::query!("UPDATE units SET name = ? WHERE id = ?", name, id)
            .execute(&state.db)
            .await?;
    }

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let in_use = sqlx::query_scalar!("SELECT COUNT(*) FROM additive_events WHERE unit_id = ?", id)
        .fetch_one(&state.db)
        .await?
        > 0;

    if !in_use {
        sqlx::query!("DELETE FROM units WHERE id = ?", id)
            .execute(&state.db)
            .await?;
    }

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}
