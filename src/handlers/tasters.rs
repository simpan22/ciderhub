//! Owner page for the friends who log comparisons on /taste: add a
//! taster, copy their invite link, revoke it or issue a new one. See
//! docs/specs/friend-tasting-page.md.

use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Form;
use serde::Deserialize;

use crate::error::AppError;
use crate::state::AppState;

pub struct TasterRow {
    pub id: i64,
    pub name: String,
    pub link_path: String,
    pub revoked: bool,
    pub comparisons: i64,
    pub last_active: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "tasters/list.html")]
struct ListTemplate {
    rows: Vec<TasterRow>,
}

#[derive(Template, WebTemplate)]
#[template(path = "tasters/_table_body.html")]
struct TableBodyTemplate {
    rows: Vec<TasterRow>,
}

/// 32 random bytes from the OS, hex-encoded — unguessable, and safe to
/// put in a URL path as-is.
fn new_token() -> Result<String, AppError> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| anyhow::anyhow!("getrandom failed: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

async fn fetch_rows(db: &sqlx::SqlitePool) -> Result<Vec<TasterRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT t.id as "id!", t.name as "name!", t.token as "token!", t.revoked_at,
               COUNT(ce.event_id) as "comparisons!: i64",
               MAX(e.occurred_at) as "last_active?: String"
        FROM tasters t
        LEFT JOIN comparison_events ce ON ce.taster_id = t.id
        LEFT JOIN events e ON e.id = ce.event_id
        GROUP BY t.id
        ORDER BY t.revoked_at IS NOT NULL, t.name
        "#
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| TasterRow {
            id: r.id,
            name: r.name,
            link_path: format!("/taste/join/{}", r.token),
            revoked: r.revoked_at.is_some(),
            comparisons: r.comparisons,
            last_active: r.last_active,
        })
        .collect())
}

pub async fn list(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    Ok(ListTemplate { rows: fetch_rows(&state.db).await? })
}

#[derive(Deserialize)]
pub struct NewTasterForm {
    name: String,
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<NewTasterForm>,
) -> Result<impl IntoResponse, AppError> {
    let name = form.name.trim();
    if !name.is_empty() {
        let token = new_token()?;
        sqlx::query!("INSERT INTO tasters (name, token) VALUES (?, ?)", name, token)
            .execute(&state.db)
            .await?;
    }
    Ok(TableBodyTemplate { rows: fetch_rows(&state.db).await? })
}

/// Stops the link and every cookie already issued from it at once,
/// since the middleware looks the token up on every request.
pub async fn revoke(State(state): State<AppState>, Path(id): Path<i64>) -> Result<impl IntoResponse, AppError> {
    sqlx::query!(
        "UPDATE tasters SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND revoked_at IS NULL",
        id
    )
    .execute(&state.db)
    .await?;
    Ok(TableBodyTemplate { rows: fetch_rows(&state.db).await? })
}

/// Replaces the token, so the old link and its cookies stop working.
/// Also reinstates a revoked taster — the way to let someone back in.
pub async fn new_link(State(state): State<AppState>, Path(id): Path<i64>) -> Result<impl IntoResponse, AppError> {
    let token = new_token()?;
    sqlx::query!("UPDATE tasters SET token = ?, revoked_at = NULL WHERE id = ?", token, id)
        .execute(&state.db)
        .await?;
    Ok(TableBodyTemplate { rows: fetch_rows(&state.db).await? })
}
