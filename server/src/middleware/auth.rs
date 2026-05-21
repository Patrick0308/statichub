use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{api::AuthState, error::AppError};

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    pub sub: String,  // user_id as string
    pub email: String,
    pub exp: usize,
}

pub async fn auth_middleware(
    State(state): State<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Extract Authorization header
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    // Extract Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    // Validate JWT
    let decoding_key = DecodingKey::from_secret(state.jwt_secret.as_bytes());
    let validation = Validation::default();

    let token_data = decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|e| {
            tracing::warn!("JWT validation failed: {}", e);
            AppError::Unauthorized
        })?;

    // Parse user_id from sub claim
    let user_id = token_data.claims.sub.parse::<i64>()
        .map_err(|e| {
            tracing::error!("Failed to parse user_id from JWT sub claim: {}", e);
            AppError::Unauthorized
        })?;

    // Add AuthUser to request extensions
    let auth_user = AuthUser {
        user_id,
        email: token_data.claims.email,
    };
    req.extensions_mut().insert(auth_user);

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::AuthState;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn test_handler(req: Request<Body>) -> StatusCode {
        // Extract AuthUser from extensions
        let auth_user = req.extensions().get::<AuthUser>();
        if auth_user.is_some() {
            StatusCode::OK
        } else {
            StatusCode::UNAUTHORIZED
        }
    }

    #[tokio::test]
    async fn test_auth_middleware_valid_token() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        let auth_state = Arc::new(
            AuthState::new(
                pool,
                "test_client_id".to_string(),
                "test_client_secret".to_string(),
                "http://localhost:3000/auth/callback/google".to_string(),
                "test_secret".to_string(),
            )
            .unwrap(),
        );

        // Generate a valid JWT
        let jwt = auth_state.generate_jwt(123, "test@example.com").unwrap();

        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn_with_state(
                auth_state.clone(),
                auth_middleware,
            ))
            .with_state(auth_state);

        let request = Request::builder()
            .uri("/test")
            .header("authorization", format!("Bearer {}", jwt))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_middleware_missing_header() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        let auth_state = Arc::new(
            AuthState::new(
                pool,
                "test_client_id".to_string(),
                "test_client_secret".to_string(),
                "http://localhost:3000/auth/callback/google".to_string(),
                "test_secret".to_string(),
            )
            .unwrap(),
        );

        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn_with_state(
                auth_state.clone(),
                auth_middleware,
            ))
            .with_state(auth_state);

        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_middleware_invalid_token() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        let auth_state = Arc::new(
            AuthState::new(
                pool,
                "test_client_id".to_string(),
                "test_client_secret".to_string(),
                "http://localhost:3000/auth/callback/google".to_string(),
                "test_secret".to_string(),
            )
            .unwrap(),
        );

        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn_with_state(
                auth_state.clone(),
                auth_middleware,
            ))
            .with_state(auth_state);

        let request = Request::builder()
            .uri("/test")
            .header("authorization", "Bearer invalid.token.here")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
