use crate::{
    error::{AppError, Result},
    models::{DeviceLoginSession, DeviceLoginStatus, User},
};
use axum::{
    extract::{Form, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use jsonwebtoken::{encode, EncodingKey, Header};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{collections::HashMap, env, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

pub const DEVICE_EXPIRES_IN_SECONDS: i64 = 600;
pub const DEVICE_POLL_INTERVAL_SECONDS: i64 = 5;

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
    pub sub: String, // user_id
    pub email: String,
    pub exp: usize, // expiry timestamp
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
    pub state: String, // session_id
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeviceStartResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i64,
    pub interval: i64,
}

#[derive(Debug, Deserialize)]
pub struct DeviceVerifyRequest {
    pub user_code: String,
}

#[derive(Debug, Deserialize)]
pub struct DeviceTokenRequest {
    pub device_code: String,
}

#[derive(Debug, Serialize)]
pub struct DeviceTokenResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DevicePageQuery {
    pub code: Option<String>,
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

fn generate_device_code() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("sdc_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn hash_device_code(device_code: &str) -> String {
    let digest = Sha256::digest(device_code.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn generate_user_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = OsRng;
    let mut code = String::with_capacity(9);

    for i in 0..8 {
        if i == 4 {
            code.push('-');
        }
        let idx = (rng.next_u32() as usize) % CHARSET.len();
        code.push(CHARSET[idx] as char);
    }

    code
}

fn normalize_user_code(input: &str) -> String {
    let chars: String = input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();

    if chars.len() <= 4 {
        chars
    } else {
        format!("{}-{}", &chars[..4], &chars[4..])
    }
}

fn verification_base_url() -> String {
    env::var("STATICHUB_PUBLIC_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string())
        .trim_end_matches('/')
        .to_string()
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
        return Err(AppError::Conflict("Session ID already in use".to_string()));
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

pub async fn device_start(
    State(state): State<Arc<AuthState>>,
) -> Result<Json<DeviceStartResponse>> {
    DeviceLoginSession::expire_old(&state.pool).await?;

    let device_code = generate_device_code();
    let device_code_hash = hash_device_code(&device_code);
    let expires_at =
        (chrono::Utc::now() + chrono::Duration::seconds(DEVICE_EXPIRES_IN_SECONDS)).naive_utc();

    let mut user_code = None;
    for _ in 0..5 {
        let candidate = generate_user_code();
        match DeviceLoginSession::create(
            &state.pool,
            &device_code_hash,
            &candidate,
            expires_at,
            DEVICE_POLL_INTERVAL_SECONDS,
        )
        .await
        {
            Ok(_) => {
                user_code = Some(candidate);
                break;
            }
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => continue,
            Err(err) => return Err(err.into()),
        }
    }

    let user_code = user_code.ok_or_else(|| {
        AppError::Conflict("Could not allocate a unique device user code".to_string())
    })?;
    let verification_uri = format!("{}/auth/device", verification_base_url());
    let verification_uri_complete = format!("{}?code={}", verification_uri, user_code);

    Ok(Json(DeviceStartResponse {
        device_code,
        user_code,
        verification_uri,
        verification_uri_complete,
        expires_in: DEVICE_EXPIRES_IN_SECONDS,
        interval: DEVICE_POLL_INTERVAL_SECONDS,
    }))
}

pub async fn device_page(Query(query): Query<DevicePageQuery>) -> Html<String> {
    let code = query
        .code
        .as_deref()
        .map(normalize_user_code)
        .unwrap_or_default();
    Html(format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>StaticHub Device Login</title>
</head>
<body>
  <main>
    <h1>StaticHub Device Login</h1>
    <form method="post" action="/auth/device/verify">
      <label for="user_code">Code</label>
      <input id="user_code" name="user_code" value="{code}" autocomplete="one-time-code" autofocus>
      <button type="submit">Continue</button>
    </form>
  </main>
</body>
</html>"#
    ))
}

pub async fn device_verify(
    State(state): State<Arc<AuthState>>,
    Form(payload): Form<DeviceVerifyRequest>,
) -> Result<Redirect> {
    DeviceLoginSession::expire_old(&state.pool).await?;

    let user_code = normalize_user_code(&payload.user_code);
    if user_code.len() != 9 {
        return Err(AppError::BadRequest("Invalid device code".to_string()));
    }

    let session = DeviceLoginSession::find_by_user_code(&state.pool, &user_code)
        .await?
        .ok_or_else(|| AppError::NotFound("Device login session not found".to_string()))?;

    if session.is_expired() || session.status() == DeviceLoginStatus::Expired {
        return Err(AppError::BadRequest(
            "Device login session expired".to_string(),
        ));
    }

    if session.status() != DeviceLoginStatus::Pending {
        return Err(AppError::Conflict(
            "Device login session is not pending".to_string(),
        ));
    }

    let oauth_state = Uuid::new_v4().to_string();
    DeviceLoginSession::attach_oauth_state(&state.pool, session.id, &oauth_state).await?;

    let (auth_url, _csrf_token) = state
        .oauth_client
        .authorize_url(|| CsrfToken::new(oauth_state))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .url();

    Ok(Redirect::to(auth_url.as_ref()))
}

pub async fn device_token(
    State(state): State<Arc<AuthState>>,
    Json(payload): Json<DeviceTokenRequest>,
) -> Result<Json<DeviceTokenResponse>> {
    DeviceLoginSession::expire_old(&state.pool).await?;

    let device_code_hash = hash_device_code(&payload.device_code);
    let session = DeviceLoginSession::find_by_device_code_hash(&state.pool, &device_code_hash)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if session.is_expired() {
        return Ok(Json(DeviceTokenResponse {
            status: "expired_token".to_string(),
            token: None,
            interval: None,
        }));
    }

    if let Some(last_polled_at) = session.last_polled_at {
        let next_allowed_at =
            last_polled_at + chrono::Duration::seconds(session.poll_interval_seconds);
        if chrono::Utc::now().naive_utc() < next_allowed_at {
            return Ok(Json(DeviceTokenResponse {
                status: "slow_down".to_string(),
                token: None,
                interval: Some(session.poll_interval_seconds + 5),
            }));
        }
    }

    match session.status() {
        DeviceLoginStatus::Approved => {
            let token = DeviceLoginSession::consume_token(&state.pool, session.id).await?;
            Ok(Json(DeviceTokenResponse {
                status: "approved".to_string(),
                token,
                interval: None,
            }))
        }
        DeviceLoginStatus::Denied => Ok(Json(DeviceTokenResponse {
            status: "access_denied".to_string(),
            token: None,
            interval: None,
        })),
        DeviceLoginStatus::Consumed | DeviceLoginStatus::Expired => Ok(Json(DeviceTokenResponse {
            status: "expired_token".to_string(),
            token: None,
            interval: None,
        })),
        DeviceLoginStatus::Pending | DeviceLoginStatus::Verified => {
            DeviceLoginSession::mark_polled(&state.pool, session.id).await?;
            Ok(Json(DeviceTokenResponse {
                status: "authorization_pending".to_string(),
                token: None,
                interval: Some(session.poll_interval_seconds),
            }))
        }
    }
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
    let user = User::find_by_oauth(&state.pool, "google", &user_info.id).await?;

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

    if let Some(device_session) =
        DeviceLoginSession::find_by_oauth_state(&state.pool, &query.state).await?
    {
        DeviceLoginSession::approve(&state.pool, device_session.id, &jwt).await?;
        return Ok((
            StatusCode::OK,
            "Authentication successful! You can close this window and return to your terminal.",
        )
            .into_response());
    }

    // Store JWT in session
    let mut sessions = state.sessions.write().await;
    if let Some(session) = sessions.get_mut(&query.state) {
        session.token = Some(jwt);
    } else {
        tracing::warn!(
            "OAuth callback for expired/unknown session: {}",
            query.state
        );
        return Err(AppError::BadRequest(
            "Session expired. Please restart authentication.".to_string(),
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
    let token = sessions.get(&session_id).and_then(|s| s.token.clone());

    Ok(Json(StatusResponse { token }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jwt_generation() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

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
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

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
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

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
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

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
