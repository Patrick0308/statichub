mod db;
mod models;
mod storage;
mod error;
mod api;

use axum::{Router, routing::{get, post}};
use std::{net::SocketAddr, sync::Arc};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "statichub_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Database setup
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:statichub.db".to_string());

    let pool = db::create_pool(&database_url).await?;

    tracing::info!("Database connected and migrations run");

    // Storage setup
    let storage_path = std::env::var("STORAGE_PATH")
        .unwrap_or_else(|_| "./var/statichub/deploys".to_string());

    let storage = Arc::new(storage::FilesystemStorage::new(storage_path.into())) as Arc<dyn storage::Storage>;

    // Shared state
    let deploy_state = Arc::new(api::DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/deploys/anonymous", post(api::create_anonymous_deploy))
        .with_state(deploy_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}
