pub mod db;
pub mod storage;
pub mod models;
pub mod error;
pub mod api;

use axum::{routing::{get, post}, Router};
use std::sync::Arc;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

pub fn create_router(deploy_state: Arc<api::DeployState>, auth_state: Arc<api::AuthState>) -> Router {
    let auth_routes = Router::new()
        .route("/auth/login/google", post(api::login_google))
        .route("/auth/callback/google", get(api::callback_google))
        .route("/auth/status/:session_id", get(api::auth_status))
        .with_state(auth_state);

    let deploy_routes = Router::new()
        .route("/api/deploys/anonymous", post(api::create_anonymous_deploy))
        .fallback(get(api::serve_static_file))
        .with_state(deploy_state);

    Router::new()
        .route("/health", get(health_check))
        .merge(auth_routes)
        .merge(deploy_routes)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
}

async fn health_check() -> &'static str {
    "OK"
}
