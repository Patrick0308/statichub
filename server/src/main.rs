use statichub_server::{db, storage, api, create_router};
use std::{net::SocketAddr, sync::Arc};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

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
    let base_url = std::env::var("BASE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let deploy_state = Arc::new(api::DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url,
    });

    let auth_state = Arc::new(api::AuthState::new(
        pool.clone(),
        std::env::var("GOOGLE_CLIENT_ID")
            .expect("GOOGLE_CLIENT_ID must be set"),
        std::env::var("GOOGLE_CLIENT_SECRET")
            .expect("GOOGLE_CLIENT_SECRET must be set"),
        std::env::var("GOOGLE_REDIRECT_URL")
            .unwrap_or_else(|_| "http://localhost:3000/auth/callback/google".to_string()),
        std::env::var("JWT_SECRET")
            .expect("JWT_SECRET must be set in production"),
    )?);

    // Build router
    let app = create_router(deploy_state, auth_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
