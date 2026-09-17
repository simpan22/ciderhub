mod charts;
mod config;
mod db;
mod dates;
mod error;
mod handlers;
mod models;
mod state;

use axum::routing::get;
use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env();
    let db = db::connect(&config.database_url).await?;
    let state = AppState { db };

    let app = Router::new()
        .route("/", get(handlers::dashboard::index))
        .route("/dashboard/season-overview", get(handlers::dashboard::season_overview))
        .route("/healthz", get(handlers::dashboard::healthz))
        .route(
            "/seasons",
            get(handlers::seasons::list).post(handlers::seasons::create),
        )
        .route("/seasons/table", get(handlers::seasons::table_fragment))
        .route("/seasons/{id}/edit", get(handlers::seasons::edit_fragment))
        .route(
            "/seasons/{id}",
            axum::routing::put(handlers::seasons::update).delete(handlers::seasons::delete),
        )
        .route(
            "/trees",
            get(handlers::trees::list).post(handlers::trees::create),
        )
        .route("/trees/table", get(handlers::trees::table_fragment))
        .route("/trees/{id}/edit", get(handlers::trees::edit_fragment))
        .route(
            "/trees/{id}",
            axum::routing::put(handlers::trees::update).delete(handlers::trees::delete),
        )
        .route(
            "/units",
            get(handlers::units::list).post(handlers::units::create),
        )
        .route("/units/table", get(handlers::units::table_fragment))
        .route("/units/{id}/edit", get(handlers::units::edit_fragment))
        .route(
            "/units/{id}",
            axum::routing::put(handlers::units::update).delete(handlers::units::delete),
        )
        .route(
            "/batches",
            get(handlers::batches::list).post(handlers::batches::create),
        )
        .route("/batches/suggest-code", get(handlers::batches::suggest_code))
        .route(
            "/batches/{id}",
            get(handlers::batches::detail)
                .put(handlers::batches::update)
                .delete(handlers::batches::delete),
        )
        .route("/batches/{id}/timeline", get(handlers::batches::timeline_fragment))
        .route(
            "/batches/{id}/events/juicing",
            axum::routing::post(handlers::batches::add_juicing),
        )
        .route(
            "/batches/{batch_id}/events/juicing/{event_id}/edit",
            get(handlers::batches::edit_juicing_fragment),
        )
        .route(
            "/batches/{batch_id}/events/juicing/{event_id}",
            axum::routing::put(handlers::batches::update_juicing),
        )
        .route(
            "/batches/{id}/events/additive",
            axum::routing::post(handlers::batches::add_additive),
        )
        .route(
            "/batches/{batch_id}/events/additive/{event_id}/edit",
            get(handlers::batches::edit_additive_fragment),
        )
        .route(
            "/batches/{batch_id}/events/additive/{event_id}",
            axum::routing::put(handlers::batches::update_additive),
        )
        .route(
            "/batches/{id}/events/measurement",
            axum::routing::post(handlers::batches::add_measurement),
        )
        .route(
            "/batches/{batch_id}/events/measurement/{event_id}/edit",
            get(handlers::batches::edit_measurement_fragment),
        )
        .route(
            "/batches/{batch_id}/events/measurement/{event_id}",
            axum::routing::put(handlers::batches::update_measurement),
        )
        .route(
            "/batches/{id}/events/racking",
            axum::routing::post(handlers::batches::add_racking),
        )
        .route(
            "/batches/{batch_id}/events/racking/{event_id}/edit",
            get(handlers::batches::edit_racking_fragment),
        )
        .route(
            "/batches/{batch_id}/events/racking/{event_id}",
            axum::routing::put(handlers::batches::update_racking),
        )
        .route(
            "/batches/{id}/events/bottling",
            axum::routing::post(handlers::batches::add_bottling),
        )
        .route(
            "/batches/{batch_id}/events/bottling/{event_id}/edit",
            get(handlers::batches::edit_bottling_fragment),
        )
        .route(
            "/batches/{batch_id}/events/bottling/{event_id}",
            axum::routing::put(handlers::batches::update_bottling),
        )
        .route(
            "/batches/{id}/events/failure",
            axum::routing::post(handlers::batches::add_failure),
        )
        .route(
            "/batches/{id}/events/tasting",
            axum::routing::post(handlers::batches::add_tasting),
        )
        .route(
            "/batches/{batch_id}/events/tasting/{event_id}/edit",
            get(handlers::batches::edit_tasting_fragment),
        )
        .route(
            "/batches/{batch_id}/events/tasting/{event_id}",
            axum::routing::put(handlers::batches::update_tasting),
        )
        .route("/login", axum::routing::post(handlers::auth::login))
        .nest_service("/static", ServeDir::new("static"))
        .layer(axum::middleware::from_fn(handlers::auth::require_auth))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!("listening on http://{}", config.bind_addr);
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
