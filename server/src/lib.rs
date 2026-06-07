pub mod api;
pub mod cli;
pub mod config;
pub mod db;
pub mod error;
pub mod markdown;
pub mod middleware;
pub mod models;
pub mod storage;
pub mod tls;
pub mod web;

// Test utilities available for integration tests
pub mod test_utils;

use axum::{
    middleware as axum_middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

pub fn create_router(
    deploy_state: Arc<api::DeployState>,
    auth_mode: config::AuthMode,
    auth_state: Option<Arc<api::AuthState>>,
) -> Router {
    let deploy_routes = Router::new()
        .route("/api/deploys/anonymous", post(api::create_anonymous_deploy))
        .with_state(deploy_state.clone());

    let (auth_routes, authenticated_routes) = match auth_mode {
        config::AuthMode::Enabled => {
            let auth_state = auth_state.expect("auth_state must be present when auth is enabled");
            let auth_routes = Router::new()
                .route("/auth/login/google", post(api::login_google))
                .route("/auth/callback/google", get(api::callback_google))
                .route("/auth/status/:session_id", get(api::auth_status))
                .route(
                    "/auth/device",
                    post(api::device_start).get(api::device_page),
                )
                .route("/auth/device/verify", post(api::device_verify))
                .route("/auth/device/token", post(api::device_token))
                .with_state(auth_state.clone());

            let authenticated_routes = Router::new()
                .route(
                    "/api/projects/:name/deploys",
                    post(api::create_project_deploy),
                )
                .route("/api/apikeys", post(api::create_api_key).get(api::list_api_keys))
                .route("/api/apikeys/:id/revoke", post(api::revoke_api_key))
                .route("/api/projects", get(api::list_projects))
                .route("/api/projects/:name", get(api::get_project_info))
                .route("/api/projects/:name/rollback", post(api::rollback_project))
                .layer(axum_middleware::from_fn_with_state(
                    auth_state,
                    middleware::auth_middleware,
                ))
                .with_state(deploy_state.clone());
            (auth_routes, authenticated_routes)
        }
        config::AuthMode::Disabled => {
            let auth_routes = Router::new()
                .route("/auth/login/google", post(api::auth_disabled))
                .route("/auth/callback/google", get(api::auth_disabled))
                .route("/auth/status/:session_id", get(api::auth_disabled))
                .route(
                    "/auth/device",
                    post(api::auth_disabled).get(api::auth_disabled),
                )
                .route("/auth/device/verify", post(api::auth_disabled))
                .route("/auth/device/token", post(api::auth_disabled));

            let authenticated_routes = Router::new()
                .route("/api/projects/:name/deploys", post(api::protected_disabled))
                .route(
                    "/api/apikeys",
                    post(api::protected_disabled).get(api::protected_disabled),
                )
                .route("/api/apikeys/:id/revoke", post(api::protected_disabled))
                .route("/api/projects", get(api::protected_disabled))
                .route("/api/projects/:name", get(api::protected_disabled))
                .route("/api/projects/:name/rollback", post(api::protected_disabled));
            (auth_routes, authenticated_routes)
        }
    };

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
