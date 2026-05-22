pub mod config;
pub mod db;
pub mod storage;
pub mod models;
pub mod error;
pub mod api;
pub mod middleware;
pub mod cli;

// Test utilities available for integration tests
pub mod test_utils;

use axum::{middleware as axum_middleware, routing::{delete, get, post}, Router};
use std::sync::Arc;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

pub fn create_router(
    deploy_state: Arc<api::DeployState>,
    auth_state: Arc<api::AuthState>,
) -> Router {
    let auth_routes = Router::new()
        .route("/auth/login/google", post(api::login_google))
        .route("/auth/callback/google", get(api::callback_google))
        .route("/auth/status/:session_id", get(api::auth_status))
        .with_state(auth_state.clone());

    let deploy_routes = Router::new()
        .route("/api/deploys/anonymous", post(api::create_anonymous_deploy))
        .with_state(deploy_state.clone());

    // Authenticated routes with JWT middleware
    let authenticated_routes = Router::new()
        .route("/api/projects/:name/deploys", post(api::create_project_deploy))
        .route("/api/projects", get(api::list_projects))
        .route("/api/projects/:name", get(api::get_project_info))
        .route("/api/projects/:name/rollback", post(api::rollback_project))
        .route("/api/projects/:name/domains", post(api::add_domain))
        .route("/api/projects/:name/domains", get(api::list_domains))
        .route("/api/projects/:name/domains/:domain/verify", post(api::verify_domain))
        .route("/api/projects/:name/domains/:domain", delete(api::remove_domain))
        .layer(axum_middleware::from_fn_with_state(
            auth_state.clone(),
            middleware::auth_middleware,
        ))
        .with_state(deploy_state.clone());

    Router::new()
        .route("/health", get(health_check))
        .merge(auth_routes)
        .merge(deploy_routes)
        .merge(authenticated_routes)
        .fallback(get(api::serve_static_file))
        .with_state(deploy_state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
}

async fn health_check() -> &'static str {
    "OK"
}
