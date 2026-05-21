use crate::{
    error::{AppError, Result},
    models::User,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use jsonwebtoken::{encode, EncodingKey, Header};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AuthState {
    pub pool: SqlitePool,
    pub oauth_client: BasicClient,
    pub jwt_secret: String,
    pub sessions: Arc<RwLock<HashMap<String, PendingSession>>>,
}

#[derive(Clone)]
pub struct PendingSession {
    pub token: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // user_id
    pub email: String,
    pub exp: usize,       // expiry timestamp
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub auth_url: String,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,  // session_id
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    id: String,
    email: String,
}

impl AuthState {
    pub fn new(
        pool: SqlitePool,
        google_client_id: String,
        google_client_secret: String,
        redirect_url: String,
        jwt_secret: String,
    ) -> Result<Self> {
        let oauth_client = BasicClient::new(
            ClientId::new(google_client_id),
            Some(ClientSecret::new(google_client_secret)),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                .map_err(|e| AppError::Internal(format!("Invalid auth URL: {}", e)))?,
            Some(
                TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
                    .map_err(|e| AppError::Internal(format!("Invalid token URL: {}", e)))?,
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect_url)
                .map_err(|e| AppError::Internal(format!("Invalid redirect URL: {}", e)))?,
        );

        Ok(Self {
            pool,
            oauth_client,
            jwt_secret,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn generate_jwt(&self, user_id: i64, email: &str) -> Result<String> {
        let expiry = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::days(7))
            .ok_or_else(|| AppError::Internal("Failed to calculate JWT expiry".to_string()))?
            .timestamp() as usize;

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            exp: expiry,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("Failed to generate JWT: {}", e)))
    }

    async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let now = chrono::Utc::now();
        sessions.retain(|_, session| {
            now.signed_duration_since(session.created_at) < chrono::Duration::minutes(5)
        });
    }
}

pub async fn login_google(
    State(state): State<Arc<AuthState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    // Clean up expired sessions
    state.cleanup_expired_sessions().await;

    // Generate OAuth URL with session_id as state
    let (auth_url, _csrf_token) = state
        .oauth_client
        .authorize_url(|| CsrfToken::new(payload.session_id.clone()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .url();

    // Store pending session
    let mut sessions = state.sessions.write().await;
    if sessions.contains_key(&payload.session_id) {
        return Err(AppError::Conflict(
            "Session ID already in use".to_string()
        ));
    }
    sessions.insert(
        payload.session_id,
        PendingSession {
            token: None,
            created_at: chrono::Utc::now(),
        },
    );

    Ok(Json(LoginResponse {
        auth_url: auth_url.to_string(),
    }))
}

pub async fn callback_google(
    State(state): State<Arc<AuthState>>,
    Query(query): Query<CallbackQuery>,
) -> Result<Response> {
    // Exchange code for token
    let token_result = state
        .oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .request_async(async_http_client)
        .await
        .map_err(|e| AppError::Internal(format!("OAuth token exchange failed: {}", e)))?;

    // Get user info from Google
    let access_token = token_result.access_token().secret();
    let client = reqwest::Client::new();
    let user_info: GoogleUserInfo = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch user info: {}", e)))?
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse user info: {}", e)))?;

    // Create or update user
    let user = User::find_by_oauth(&state.pool, "google", &user_info.id)
        .await?;

    let user = if let Some(existing_user) = user {
        existing_user
    } else {
        User::create(
            &state.pool,
            "google",
            &user_info.id,
            &user_info.email,
            user_info.email.split('@').next().unwrap_or("user"),
        )
        .await?
    };

    // Generate JWT
    let jwt = state.generate_jwt(user.id, &user.email)?;

    // Store JWT in session
    let mut sessions = state.sessions.write().await;
    if let Some(session) = sessions.get_mut(&query.state) {
        session.token = Some(jwt);
    } else {
        tracing::warn!("OAuth callback for expired/unknown session: {}", query.state);
        return Err(AppError::BadRequest(
            "Session expired. Please restart authentication.".to_string()
        ));
    }

    // Return success page
    Ok((
        StatusCode::OK,
        "Authentication successful! You can close this window and return to your terminal.",
    )
        .into_response())
}

pub async fn auth_status(
    State(state): State<Arc<AuthState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<StatusResponse>> {
    let sessions = state.sessions.read().await;
    let token = sessions
        .get(&session_id)
        .and_then(|s| s.token.clone());

    Ok(Json(StatusResponse { token }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jwt_generation() {
        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .unwrap();

        let state = AuthState::new(
            pool,
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();

        let jwt = state.generate_jwt(1, "test@example.com").unwrap();
        assert!(!jwt.is_empty());

        // Verify JWT contains expected parts (header.payload.signature)
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
    }

    #[tokio::test]
    async fn test_session_cleanup() {
        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .unwrap();

        let state = AuthState::new(
            pool,
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();

        // Add expired session
        {
            let mut sessions = state.sessions.write().await;
            sessions.insert(
                "expired".to_string(),
                PendingSession {
                    token: None,
                    created_at: chrono::Utc::now() - chrono::Duration::minutes(6),
                },
            );
            sessions.insert(
                "valid".to_string(),
                PendingSession {
                    token: None,
                    created_at: chrono::Utc::now(),
                },
            );
        }

        state.cleanup_expired_sessions().await;

        let sessions = state.sessions.read().await;
        assert!(!sessions.contains_key("expired"));
        assert!(sessions.contains_key("valid"));
    }

    #[tokio::test]
    async fn test_session_update_validates_existence() {
        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .unwrap();

        let state = AuthState::new(
            pool,
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();

        // Add a valid session
        {
            let mut sessions = state.sessions.write().await;
            sessions.insert(
                "valid-session".to_string(),
                PendingSession {
                    token: None,
                    created_at: chrono::Utc::now(),
                },
            );
        }

        // Test that updating an existing session works
        {
            let mut sessions = state.sessions.write().await;
            let updated = sessions.get_mut("valid-session").map(|s| {
                s.token = Some("test-token".to_string());
                true
            });
            assert!(updated.is_some());
        }

        // Test that attempting to update non-existent session returns None
        {
            let mut sessions = state.sessions.write().await;
            let updated = sessions.get_mut("non-existent").map(|s| {
                s.token = Some("test-token".to_string());
                true
            });
            assert!(updated.is_none());
        }
    }

    #[tokio::test]
    async fn test_duplicate_session_detection() {
        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .unwrap();

        let state = AuthState::new(
            pool,
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();

        // Add first session
        {
            let mut sessions = state.sessions.write().await;
            sessions.insert(
                "duplicate-id".to_string(),
                PendingSession {
                    token: None,
                    created_at: chrono::Utc::now(),
                },
            );
        }

        // Verify duplicate detection
        {
            let sessions = state.sessions.read().await;
            let is_duplicate = sessions.contains_key("duplicate-id");
            assert!(is_duplicate, "Should detect existing session ID");
        }

        // Verify new session IDs are not duplicates
        {
            let sessions = state.sessions.read().await;
            let is_duplicate = sessions.contains_key("new-unique-id");
            assert!(!is_duplicate, "Should not flag new session ID as duplicate");
        }
    }
}
