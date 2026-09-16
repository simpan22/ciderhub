use askama::Template;
use askama_web::WebTemplate;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::state::AppState;

#[derive(Template, WebTemplate)]
#[template(path = "index.html")]
struct IndexTemplate {
    db_status: &'static str,
}

pub async fn index(State(state): State<AppState>) -> impl IntoResponse {
    let db_status = match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => "connected",
        Err(_) => "unavailable",
    };

    IndexTemplate { db_status }
}

pub async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "database unavailable"),
    }
}
