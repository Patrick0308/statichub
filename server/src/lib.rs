pub mod db;
pub mod storage;
pub mod models;
pub mod error;
pub mod api;

use axum::{routing::{get, post}, Router};
use std::sync::Arc;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

pub fn create_router(state: Arc<api::DeployState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/deploys/anonymous", post(api::create_anonymous_deploy))
        .fallback(get(api::serve_static_file))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
}

async fn health_check() -> &'static str {
    "OK"
}
