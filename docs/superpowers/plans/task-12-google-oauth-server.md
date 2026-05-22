# Task 12: Google OAuth (Server)

## Goal

Implement Google OAuth authentication on the server side. This enables users to log in and access authenticated features like named projects, rollback, and custom domains.

## Files

- Create: `server/src/api/auth.rs`
- Modify: `server/src/api/mod.rs`
- Modify: `server/src/main.rs`
- Modify: `server/Cargo.toml`
- Create: `server/tests/auth_tests.rs`
- Create: `.env.example`

## Implementation Steps

### Step 1: Add dependencies

Add to `server/Cargo.toml`:

```toml
oauth2 = "4"
jsonwebtoken = "9"
```

### Step 2: Create AuthState and auth module skeleton

Create: `server/src/api/auth.rs`

```rust
use crate::{
    error::{AppError, Result},
    models::User,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Redirect, Response},
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

    fn generate_jwt(&self, user_id: i64, email: &str) -> Result<String> {
        let expiry = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::days(7))
            .expect("Valid timestamp")
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
        .await?
        .unwrap_or_else(|| {
            // User doesn't exist, will create in next step
            User {
                id: 0,
                oauth_provider: "google".to_string(),
                oauth_id: user_info.id.clone(),
                email: user_info.email.clone(),
                username: user_info.email.split('@').next().unwrap_or("user").to_string(),
                created_at: chrono::Utc::now().naive_utc(),
            }
        });

    let user = if user.id == 0 {
        User::create(
            &state.pool,
            "google",
            &user_info.id,
            &user_info.email,
            user_info.email.split('@').next().unwrap_or("user"),
        )
        .await?
    } else {
        user
    };

    // Generate JWT
    let jwt = state.generate_jwt(user.id, &user.email)?;

    // Store JWT in session
    let mut sessions = state.sessions.write().await;
    if let Some(session) = sessions.get_mut(&query.state) {
        session.token = Some(jwt);
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

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    id: String,
    email: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_generation() {
        let state = AuthState::new(
            sqlx::SqlitePool::connect("sqlite::memory:")
                .await
                .unwrap(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();

        let jwt = state.generate_jwt(1, "test@example.com").unwrap();
        assert!(!jwt.is_empty());
    }
}
```

### Step 3: Export auth module

Modify `server/src/api/mod.rs`:

```rust
mod deploys;
mod serve;
mod auth;

pub use deploys::{create_anonymous_deploy, DeployState};
pub use serve::serve_static_file;
pub use auth::{login_google, callback_google, auth_status, AuthState};
```

### Step 4: Wire up auth routes

Modify `server/src/main.rs`:

1. Add environment variable loading at the top of `main()`:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // ... existing tracing setup ...
}
```

2. Update `create_router` function:

```rust
pub fn create_router(deploy_state: Arc<DeployState>, auth_state: Arc<AuthState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/deploys/anonymous", post(api::create_anonymous_deploy))
        .route("/auth/login/google", post(api::login_google))
        .route("/auth/callback/google", get(api::callback_google))
        .route("/auth/status/:session_id", get(api::auth_status))
        .fallback(get(api::serve_static_file))
        .with_state(deploy_state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
}
```

Wait, this won't work because we have two different state types. We need to use a shared state wrapper.

Actually, let me reconsider. Axum allows us to use multiple State extractors if we layer them. Let me revise:

```rust
use axum::extract::State;

pub fn create_router(deploy_state: Arc<DeployState>, auth_state: Arc<AuthState>) -> Router {
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
```

3. In `main()`, create AuthState and pass both states:

```rust
let auth_state = Arc::new(api::AuthState::new(
    pool.clone(),
    std::env::var("GOOGLE_CLIENT_ID")
        .expect("GOOGLE_CLIENT_ID must be set"),
    std::env::var("GOOGLE_CLIENT_SECRET")
        .expect("GOOGLE_CLIENT_SECRET must be set"),
    std::env::var("GOOGLE_REDIRECT_URL")
        .unwrap_or_else(|_| "http://localhost:3000/auth/callback/google".to_string()),
    std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-change-in-production".to_string()),
)?);

let app = create_router(state, auth_state);
```

### Step 5: Add dotenv dependency

Add to `server/Cargo.toml`:

```toml
dotenv = "0.15"
reqwest = { version = "0.11", features = ["json"] }  # Update existing reqwest
```

### Step 6: Create .env.example

Create: `.env.example` in project root:

```bash
# Google OAuth Configuration
GOOGLE_CLIENT_ID=your-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-client-secret
GOOGLE_REDIRECT_URL=http://localhost:3000/auth/callback/google

# JWT Configuration
JWT_SECRET=change-this-in-production

# Database
DATABASE_URL=sqlite:./statichub.db

# Server
STATICHUB_PORT=3000
STORAGE_PATH=./storage
```

### Step 7: Write integration tests

Create: `server/tests/auth_tests.rs`

```rust
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::ServiceExt;
use statichub_server::{create_router, api::{DeployState, AuthState}};
use statichub_server::storage::FilesystemStorage;

#[sqlx::test]
async fn test_login_initiation(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
        base_url: "http://localhost:3000".to_string(),
    });

    let auth_state = Arc::new(
        AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_jwt_secret".to_string(),
        )
        .unwrap(),
    );

    let app = create_router(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/login/google")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"session_id": "test-session-123"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(json["auth_url"].as_str().unwrap().contains("accounts.google.com"));
    assert!(json["auth_url"].as_str().unwrap().contains("test-session-123"));
}

#[sqlx::test]
async fn test_auth_status_not_ready(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
        base_url: "http://localhost:3000".to_string(),
    });

    let auth_state = Arc::new(
        AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_jwt_secret".to_string(),
        )
        .unwrap(),
    );

    let app = create_router(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/status/nonexistent-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(json["token"].is_null());
}
```

### Step 8: Run tests

Run: `cargo test -p statichub-server --test auth_tests`
Expected: Tests pass

### Step 9: Run all tests

Run: `cargo test`
Expected: All tests pass (note: callback test requires mocking Google OAuth, which is complex - we test the flow manually)

### Step 10: Manual test (with real Google OAuth)

**Prerequisites:**
1. Create Google OAuth credentials at https://console.cloud.google.com/
2. Add authorized redirect URI: `http://localhost:3000/auth/callback/google`
3. Copy Client ID and Client Secret
4. Create `.env` file based on `.env.example` with real credentials

**Test:**

Terminal 1 - Start server:
```bash
cargo run -p statichub-server
```

Terminal 2 - Test login flow:
```bash
# Generate session ID
SESSION_ID=$(uuidgen)

# Initiate login
curl -X POST http://localhost:3000/auth/login/google \
  -H "Content-Type: application/json" \
  -d "{\"session_id\": \"$SESSION_ID\"}" | jq .

# Copy the auth_url and open in browser
# Complete OAuth flow

# Poll for token
curl http://localhost:3000/auth/status/$SESSION_ID | jq .
```

Expected: After OAuth completion, token appears in status response

### Step 11: Commit

```bash
git add server/src/api/auth.rs server/src/api/mod.rs server/src/main.rs server/tests/auth_tests.rs server/Cargo.toml .env.example
git commit -m "feat: implement Google OAuth authentication server-side

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

## Success Criteria

- POST /auth/login/google returns valid Google OAuth URL
- Session ID is properly stored and tracked
- GET /auth/status/:session_id returns null token before OAuth completion
- OAuth callback creates/updates user in database
- JWT token is generated with 7-day expiry
- JWT token is stored in session for CLI polling
- Session cleanup removes expired sessions (5 min TTL)
- All tests pass
- Manual OAuth flow works end-to-end
- .env.example documents required environment variables
