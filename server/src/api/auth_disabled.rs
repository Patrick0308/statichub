use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Serialize)]
struct DisabledError {
    error: &'static str,
}

fn disabled_response() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(DisabledError {
            error: "authentication is disabled in local mode",
        }),
    )
}

pub async fn auth_disabled() -> impl IntoResponse {
    disabled_response()
}

pub async fn protected_disabled() -> impl IntoResponse {
    disabled_response()
}
