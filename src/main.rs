mod config;
mod db;
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
            "/vessels",
            get(handlers::vessels::list).post(handlers::vessels::create),
        )
        .route("/vessels/table", get(handlers::vessels::table_fragment))
        .route("/vessels/{id}/edit", get(handlers::vessels::edit_fragment))
        .route(
            "/vessels/{id}",
            axum::routing::put(handlers::vessels::update).delete(handlers::vessels::delete),
        )
        .route(
            "/batches",
            get(handlers::batches::list).post(handlers::batches::create),
        )
        .route(
            "/batches/{id}",
            get(handlers::batches::detail)
                .put(handlers::batches::update)
                .delete(handlers::batches::delete),
        )
        .route(
            "/batches/{id}/events/picking",
            axum::routing::post(handlers::batches::add_picking),
        )
        .route(
            "/batches/{id}/events/juicing",
            axum::routing::post(handlers::batches::add_juicing),
        )
        .route(
            "/batches/{id}/events/measurement",
            axum::routing::post(handlers::batches::add_measurement),
        )
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!("listening on http://{}", config.bind_addr);
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
