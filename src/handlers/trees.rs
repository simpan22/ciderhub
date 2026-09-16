use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Form;

use crate::error::AppError;
use crate::models::{Tree, TreeForm};
use crate::state::AppState;

pub struct TreeRow {
    pub tree: Tree,
    pub editing: bool,
}

async fn fetch_rows(
    db: &sqlx::SqlitePool,
    editing_id: Option<i64>,
) -> Result<Vec<TreeRow>, sqlx::Error> {
    let trees = sqlx::query_as!(
        Tree,
        r#"SELECT id as "id!", name, variety, planted_on, location, notes FROM trees ORDER BY name"#
    )
    .fetch_all(db)
    .await?;

    Ok(trees
        .into_iter()
        .map(|tree| TreeRow {
            editing: Some(tree.id) == editing_id,
            tree,
        })
        .collect())
}

#[derive(Template, WebTemplate)]
#[template(path = "trees/_table_body.html")]
struct TableBodyTemplate {
    rows: Vec<TreeRow>,
}

#[derive(Template, WebTemplate)]
#[template(path = "trees/list.html")]
struct ListTemplate {
    rows: Vec<TreeRow>,
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
    Form(form): Form<TreeForm>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query!(
        "INSERT INTO trees (name, variety, planted_on, location, notes) VALUES (?, ?, ?, ?, ?)",
        form.name,
        form.variety,
        form.planted_on,
        form.location,
        form.notes
    )
    .execute(&state.db)
    .await?;

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<TreeForm>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query!(
        "UPDATE trees SET name = ?, variety = ?, planted_on = ?, location = ?, notes = ? WHERE id = ?",
        form.name,
        form.variety,
        form.planted_on,
        form.location,
        form.notes,
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
    sqlx::query!("DELETE FROM trees WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    let rows = fetch_rows(&state.db, None).await?;
    Ok(TableBodyTemplate { rows })
}
