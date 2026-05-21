use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use statichub_shared::ErrorResponse;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match self {
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
            }
            AppError::Storage(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "storage_error", e)
            }
            AppError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, "not_found", msg)
            }
            AppError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "unauthorized", "Unauthorized".to_string())
            }
            AppError::Forbidden(msg) => {
                (StatusCode::FORBIDDEN, "forbidden", msg)
            }
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "bad_request", msg)
            }
            AppError::Conflict(msg) => {
                (StatusCode::CONFLICT, "conflict", msg)
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", msg)
            }
        };

        let body = Json(ErrorResponse {
            error: error_code.to_string(),
            message,
            code: status.as_u16(),
        });

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
