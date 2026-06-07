# StaticHub Service Device Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace CLI login's direct Google OAuth URL flow with a StaticHub-owned device flow that shows a short user code, persists pending sessions, and returns the StaticHub JWT exactly once.

**Architecture:** Add a DB-backed `DeviceLoginSession` model for device-code polling state, expose `/auth/device`, `/auth/device/verify`, and `/auth/device/token` routes, and adapt the existing Google OAuth callback so it can approve device sessions by OAuth state. Keep the old Google login endpoints during rollout and update the CLI `login` command to use the new device endpoints by default.

**Tech Stack:** Rust, Axum, SQLx/SQLite migrations, oauth2 crate, reqwest CLI client, existing JWT and credential storage helpers.

---

## File Structure

- Create: `server/migrations/003_device_login_sessions.sql`
  - Adds the persistent table for device login sessions.
- Create: `server/src/models/device_login_session.rs`
  - Owns SQL operations for creating, looking up, approving, polling, consuming, and expiring device sessions.
- Modify: `server/src/models/mod.rs`
  - Exports `DeviceLoginSession` and `DeviceLoginStatus`.
- Modify: `server/src/api/auth.rs`
  - Adds device flow request/response types and handlers.
  - Reuses existing Google user lookup and JWT generation.
  - Preserves old `/auth/login/google` and `/auth/status/:session_id` handlers during rollout.
- Modify: `server/src/api/mod.rs`
  - Re-exports the new handlers.
- Modify: `server/src/lib.rs`
  - Registers enabled and disabled auth routes.
- Modify: `server/src/api/auth_disabled.rs`
  - Existing generic disabled handler is enough; route registration changes use it.
- Modify: `server/tests/auth_tests.rs`
  - Adds integration tests for device session creation, polling, verification, and auth-disabled behavior.
- Modify: `cli/src/auth.rs`
  - Replaces old login wire types with device flow request/response types.
- Modify: `cli/src/client.rs`
  - Adds `create_device_session` and `poll_device_token`.
  - Leaves old login methods in place for compatibility until cleanup.
- Modify: `cli/src/main.rs`
  - Changes `Commands::Login` to show verification URL and user code, poll with device code, handle slow-down and terminal statuses, and save credentials.

---

### Task 1: Add Device Login Session Migration

**Files:**
- Create: `server/migrations/003_device_login_sessions.sql`

- [ ] **Step 1: Write the migration**

Create `server/migrations/003_device_login_sessions.sql` with:

```sql
CREATE TABLE device_login_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_code_hash TEXT NOT NULL UNIQUE,
    user_code TEXT NOT NULL UNIQUE,
    oauth_state TEXT UNIQUE,
    status TEXT NOT NULL CHECK(status IN ('pending', 'verified', 'approved', 'denied', 'expired', 'consumed')),
    token TEXT,
    poll_interval_seconds INTEGER NOT NULL DEFAULT 5,
    last_polled_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP NOT NULL,
    consumed_at TIMESTAMP
);

CREATE INDEX idx_device_login_sessions_user_code ON device_login_sessions(user_code);
CREATE INDEX idx_device_login_sessions_oauth_state ON device_login_sessions(oauth_state);
CREATE INDEX idx_device_login_sessions_expires_at ON device_login_sessions(expires_at);
```

- [ ] **Step 2: Run targeted migration-backed tests**

Run:

```bash
cargo test -p statichub-server auth_tests --no-run
```

Expected: compile succeeds. If SQLx migration discovery reports a migration error, fix the SQL before continuing.

- [ ] **Step 3: Commit**

```bash
git add server/migrations/003_device_login_sessions.sql
git commit -m "Add device login session migration"
```

---

### Task 2: Add Device Login Session Model

**Files:**
- Create: `server/src/models/device_login_session.rs`
- Modify: `server/src/models/mod.rs`

- [ ] **Step 1: Add the model file**

Create `server/src/models/device_login_session.rs` with this structure:

```rust
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceLoginStatus {
    Pending,
    Verified,
    Approved,
    Denied,
    Expired,
    Consumed,
}

impl DeviceLoginStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceLoginStatus::Pending => "pending",
            DeviceLoginStatus::Verified => "verified",
            DeviceLoginStatus::Approved => "approved",
            DeviceLoginStatus::Denied => "denied",
            DeviceLoginStatus::Expired => "expired",
            DeviceLoginStatus::Consumed => "consumed",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "verified" => DeviceLoginStatus::Verified,
            "approved" => DeviceLoginStatus::Approved,
            "denied" => DeviceLoginStatus::Denied,
            "expired" => DeviceLoginStatus::Expired,
            "consumed" => DeviceLoginStatus::Consumed,
            _ => DeviceLoginStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeviceLoginSession {
    pub id: i64,
    pub device_code_hash: String,
    pub user_code: String,
    pub oauth_state: Option<String>,
    pub status: String,
    pub token: Option<String>,
    pub poll_interval_seconds: i64,
    pub last_polled_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
}

impl DeviceLoginSession {
    pub async fn create(
        pool: &SqlitePool,
        device_code_hash: &str,
        user_code: &str,
        expires_at: chrono::NaiveDateTime,
        poll_interval_seconds: i64,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            r#"
            INSERT INTO device_login_sessions (
                device_code_hash,
                user_code,
                status,
                expires_at,
                poll_interval_seconds
            )
            VALUES (?, ?, 'pending', ?, ?)
            RETURNING *
            "#,
        )
        .bind(device_code_hash)
        .bind(user_code)
        .bind(expires_at)
        .bind(poll_interval_seconds)
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_device_code_hash(
        pool: &SqlitePool,
        device_code_hash: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            "SELECT * FROM device_login_sessions WHERE device_code_hash = ? LIMIT 1",
        )
        .bind(device_code_hash)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_user_code(
        pool: &SqlitePool,
        user_code: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            "SELECT * FROM device_login_sessions WHERE user_code = ? LIMIT 1",
        )
        .bind(user_code)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_oauth_state(
        pool: &SqlitePool,
        oauth_state: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            "SELECT * FROM device_login_sessions WHERE oauth_state = ? LIMIT 1",
        )
        .bind(oauth_state)
        .fetch_optional(pool)
        .await
    }

    pub async fn attach_oauth_state(
        pool: &SqlitePool,
        id: i64,
        oauth_state: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            r#"
            UPDATE device_login_sessions
            SET oauth_state = ?, status = 'verified'
            WHERE id = ? AND status = 'pending'
            RETURNING *
            "#,
        )
        .bind(oauth_state)
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn approve(
        pool: &SqlitePool,
        id: i64,
        token: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            r#"
            UPDATE device_login_sessions
            SET token = ?, status = 'approved'
            WHERE id = ? AND status = 'verified'
            RETURNING *
            "#,
        )
        .bind(token)
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn mark_polled(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE device_login_sessions SET last_polled_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn consume_token(pool: &SqlitePool, id: i64) -> Result<Option<String>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let session = sqlx::query_as::<_, DeviceLoginSession>(
            "SELECT * FROM device_login_sessions WHERE id = ? AND status = 'approved' LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(session) = session else {
            tx.commit().await?;
            return Ok(None);
        };

        sqlx::query(
            r#"
            UPDATE device_login_sessions
            SET token = NULL, status = 'consumed', consumed_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(session.token)
    }

    pub async fn expire_old(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE device_login_sessions
            SET status = 'expired', token = NULL
            WHERE expires_at <= CURRENT_TIMESTAMP
              AND status IN ('pending', 'verified', 'approved')
            "#,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub fn status(&self) -> DeviceLoginStatus {
        DeviceLoginStatus::from_str(&self.status)
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at <= chrono::Utc::now().naive_utc()
    }
}
```

- [ ] **Step 2: Export the model**

Modify `server/src/models/mod.rs`:

```rust
mod api_key;
mod deploy;
mod device_login_session;
mod project;
mod user;

pub use api_key::ApiKey;
pub use deploy::Deploy;
pub use device_login_session::{DeviceLoginSession, DeviceLoginStatus};
pub use project::Project;
pub use user::User;
```

- [ ] **Step 3: Add focused model tests**

Append to `server/src/models/device_login_session.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_find_approve_and_consume_session() {
        let pool = crate::test_utils::create_test_pool().await.unwrap();
        let expires_at = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::minutes(10))
            .unwrap()
            .naive_utc();

        let created = DeviceLoginSession::create(&pool, "hash1", "ABCD-EFGH", expires_at, 5)
            .await
            .unwrap();
        assert_eq!(created.status(), DeviceLoginStatus::Pending);

        let found = DeviceLoginSession::find_by_user_code(&pool, "ABCD-EFGH")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, created.id);

        let verified = DeviceLoginSession::attach_oauth_state(&pool, created.id, "state1")
            .await
            .unwrap();
        assert_eq!(verified.status(), DeviceLoginStatus::Verified);

        let approved = DeviceLoginSession::approve(&pool, created.id, "jwt123")
            .await
            .unwrap();
        assert_eq!(approved.status(), DeviceLoginStatus::Approved);

        let token = DeviceLoginSession::consume_token(&pool, created.id)
            .await
            .unwrap();
        assert_eq!(token.as_deref(), Some("jwt123"));

        let token_again = DeviceLoginSession::consume_token(&pool, created.id)
            .await
            .unwrap();
        assert!(token_again.is_none());
    }
}
```

- [ ] **Step 4: Run model test**

Run:

```bash
cargo test -p statichub-server device_login_session
```

Expected: all `device_login_session` tests pass.

- [ ] **Step 5: Commit**

```bash
git add server/src/models/device_login_session.rs server/src/models/mod.rs
git commit -m "Add device login session model"
```

---

### Task 3: Add Device Flow Server Endpoints

**Files:**
- Modify: `server/src/api/auth.rs`
- Modify: `server/src/api/mod.rs`
- Modify: `server/src/lib.rs`

- [ ] **Step 1: Add imports and wire types**

In `server/src/api/auth.rs`, update imports:

```rust
use crate::{
    error::{AppError, Result},
    models::{DeviceLoginSession, DeviceLoginStatus, User},
};
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng, RngCore};
use sha2::{Digest, Sha256};
```

Add the new types near the existing auth wire types:

```rust
const DEVICE_EXPIRES_IN_SECONDS: i64 = 600;
const DEVICE_POLL_INTERVAL_SECONDS: i64 = 5;

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
```

- [ ] **Step 2: Add generation helpers**

Add helper functions in `server/src/api/auth.rs`:

```rust
fn generate_device_code() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("sdc_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn hash_device_code(device_code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(device_code.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn generate_user_code() -> String {
    let raw: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| !matches!(c, '0' | 'O' | '1' | 'I' | 'L'))
        .take(8)
        .collect();
    format!("{}-{}", &raw[0..4], &raw[4..8])
}

fn normalize_user_code(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect::<String>()
        .chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

fn verification_base_url() -> String {
    std::env::var("STATICHUB_PUBLIC_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string())
        .trim_end_matches('/')
        .to_string()
}
```

- [ ] **Step 3: Add create and page handlers**

Add handlers in `server/src/api/auth.rs`:

```rust
pub async fn device_start(
    State(state): State<Arc<AuthState>>,
) -> Result<Json<DeviceStartResponse>> {
    DeviceLoginSession::expire_old(&state.pool).await?;

    let device_code = generate_device_code();
    let device_code_hash = hash_device_code(&device_code);
    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(DEVICE_EXPIRES_IN_SECONDS))
        .ok_or_else(|| AppError::Internal("Failed to calculate device login expiry".to_string()))?
        .naive_utc();

    let mut user_code = generate_user_code();
    for _ in 0..5 {
        let created = DeviceLoginSession::create(
            &state.pool,
            &device_code_hash,
            &user_code,
            expires_at,
            DEVICE_POLL_INTERVAL_SECONDS,
        )
        .await;

        match created {
            Ok(_) => {
                let base = verification_base_url();
                let verification_uri = format!("{}/auth/device", base);
                let verification_uri_complete = format!("{}?code={}", verification_uri, user_code);
                return Ok(Json(DeviceStartResponse {
                    device_code,
                    user_code,
                    verification_uri,
                    verification_uri_complete,
                    expires_in: DEVICE_EXPIRES_IN_SECONDS,
                    interval: DEVICE_POLL_INTERVAL_SECONDS,
                }));
            }
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                user_code = generate_user_code();
            }
            Err(err) => return Err(AppError::Database(err)),
        }
    }

    Err(AppError::Internal(
        "Failed to create unique device login code".to_string(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct DevicePageQuery {
    pub code: Option<String>,
}

pub async fn device_page(Query(query): Query<DevicePageQuery>) -> Html<String> {
    let code = query.code.unwrap_or_default();
    Html(format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>StaticHub Login</title>
</head>
<body>
<main>
<h1>StaticHub Login</h1>
<form method="post" action="/auth/device/verify">
<label for="user_code">Enter code</label>
<input id="user_code" name="user_code" value="{code}" autocomplete="one-time-code" autofocus>
<button type="submit">Continue</button>
</form>
</main>
</body>
</html>"#
    ))
}
```

- [ ] **Step 4: Add verify and poll handlers**

Because the verification page submits form-encoded data, add the `Form` extractor to the `axum::extract` import and use this handler:

```rust
pub async fn device_verify(
    State(state): State<Arc<AuthState>>,
    axum::extract::Form(payload): axum::extract::Form<DeviceVerifyRequest>,
) -> Result<Redirect> {
    DeviceLoginSession::expire_old(&state.pool).await?;

    let user_code = normalize_user_code(&payload.user_code);
    if user_code.len() != 9 {
        return Err(AppError::BadRequest("Invalid device login code".to_string()));
    }

    let session = DeviceLoginSession::find_by_user_code(&state.pool, &user_code)
        .await?
        .ok_or_else(|| AppError::NotFound("Device login code not found".to_string()))?;

    if session.is_expired() || session.status() == DeviceLoginStatus::Expired {
        return Err(AppError::BadRequest("Device login code expired".to_string()));
    }

    if session.status() != DeviceLoginStatus::Pending {
        return Err(AppError::Conflict(
            "Device login code has already been used".to_string(),
        ));
    }

    let oauth_state = uuid::Uuid::new_v4().to_string();
    DeviceLoginSession::attach_oauth_state(&state.pool, session.id, &oauth_state).await?;

    let (auth_url, _csrf_token) = state
        .oauth_client
        .authorize_url(|| CsrfToken::new(oauth_state))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .url();

    Ok(Redirect::to(auth_url.as_str()))
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

    if session.is_expired() || session.status() == DeviceLoginStatus::Expired {
        return Ok(Json(DeviceTokenResponse {
            status: "expired_token".to_string(),
            token: None,
            interval: None,
        }));
    }

    if let Some(last_polled_at) = session.last_polled_at {
        let elapsed = chrono::Utc::now()
            .naive_utc()
            .signed_duration_since(last_polled_at)
            .num_seconds();
        if elapsed < session.poll_interval_seconds {
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
        DeviceLoginStatus::Consumed => Ok(Json(DeviceTokenResponse {
            status: "expired_token".to_string(),
            token: None,
            interval: None,
        })),
        _ => {
            DeviceLoginSession::mark_polled(&state.pool, session.id).await?;
            Ok(Json(DeviceTokenResponse {
                status: "authorization_pending".to_string(),
                token: None,
                interval: Some(session.poll_interval_seconds),
            }))
        }
    }
}
```

- [ ] **Step 5: Update callback to approve device sessions**

In `callback_google`, after generating `jwt`, first try device-session approval:

```rust
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
```

Leave the existing in-memory `sessions.get_mut(&query.state)` block after this device-session branch so old login endpoints still work during rollout.

- [ ] **Step 6: Export and route the handlers**

Modify `server/src/api/mod.rs` export:

```rust
pub use auth::{
    auth_status, callback_google, device_page, device_start, device_token, device_verify,
    login_google, AuthState, PendingSession,
};
```

Modify enabled auth routes in `server/src/lib.rs`:

```rust
let auth_routes = Router::new()
    .route("/auth/device", post(api::device_start).get(api::device_page))
    .route("/auth/device/verify", post(api::device_verify))
    .route("/auth/device/token", post(api::device_token))
    .route("/auth/login/google", post(api::login_google))
    .route("/auth/callback/google", get(api::callback_google))
    .route("/auth/status/:session_id", get(api::auth_status))
    .with_state(auth_state.clone());
```

Modify disabled auth routes in `server/src/lib.rs`:

```rust
let auth_routes = Router::new()
    .route("/auth/device", post(api::auth_disabled).get(api::auth_disabled))
    .route("/auth/device/verify", post(api::auth_disabled))
    .route("/auth/device/token", post(api::auth_disabled))
    .route("/auth/login/google", post(api::auth_disabled))
    .route("/auth/callback/google", get(api::auth_disabled))
    .route("/auth/status/:session_id", get(api::auth_disabled));
```

- [ ] **Step 7: Run server compile check**

Run:

```bash
cargo test -p statichub-server auth::tests::test_jwt_generation
```

Expected: the JWT unit test passes and the new handlers compile.

- [ ] **Step 8: Commit**

```bash
git add server/src/api/auth.rs server/src/api/mod.rs server/src/lib.rs
git commit -m "Add device login auth endpoints"
```

---

### Task 4: Add Server Device Flow Integration Tests

**Files:**
- Modify: `server/tests/auth_tests.rs`

- [ ] **Step 1: Add helper imports**

At the top of `server/tests/auth_tests.rs`, add `header`:

```rust
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
```

- [ ] **Step 2: Add device start test**

Append:

```rust
#[sqlx::test]
async fn device_start_returns_codes_and_urls(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });
    let auth_state = Arc::new(
        AuthState::new(
            pool,
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_jwt_secret".to_string(),
        )
        .unwrap(),
    );
    let app = create_router(deploy_state, statichub_server::config::AuthMode::Enabled, Some(auth_state));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/device")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(json["device_code"].as_str().unwrap().starts_with("sdc_"));
    assert_eq!(json["user_code"].as_str().unwrap().len(), 9);
    assert!(json["verification_uri"].as_str().unwrap().ends_with("/auth/device"));
    assert!(json["verification_uri_complete"]
        .as_str()
        .unwrap()
        .contains(json["user_code"].as_str().unwrap()));
    assert_eq!(json["expires_in"], 600);
    assert_eq!(json["interval"], 5);
}
```

- [ ] **Step 3: Add polling pending and slow-down test**

Append:

```rust
#[sqlx::test]
async fn device_token_pending_then_slow_down(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });
    let auth_state = Arc::new(
        AuthState::new(
            pool,
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_jwt_secret".to_string(),
        )
        .unwrap(),
    );
    let app = create_router(deploy_state.clone(), statichub_server::config::AuthMode::Enabled, Some(auth_state.clone()));

    let start_response = app
        .oneshot(
            Request::builder()
                .uri("/auth/device")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(start_response.into_body(), usize::MAX).await.unwrap();
    let start_json: Value = serde_json::from_slice(&body).unwrap();
    let device_code = start_json["device_code"].as_str().unwrap();

    let app = create_router(deploy_state.clone(), statichub_server::config::AuthMode::Enabled, Some(auth_state.clone()));
    let pending_response = app
        .oneshot(
            Request::builder()
                .uri("/auth/device/token")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"device_code":"{}"}}"#, device_code)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pending_response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(pending_response.into_body(), usize::MAX).await.unwrap();
    let pending_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(pending_json["status"], "authorization_pending");

    let app = create_router(deploy_state, statichub_server::config::AuthMode::Enabled, Some(auth_state));
    let slow_response = app
        .oneshot(
            Request::builder()
                .uri("/auth/device/token")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"device_code":"{}"}}"#, device_code)))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(slow_response.into_body(), usize::MAX).await.unwrap();
    let slow_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(slow_json["status"], "slow_down");
}
```

- [ ] **Step 4: Add verification redirect test**

Append:

```rust
#[sqlx::test]
async fn device_verify_redirects_to_google(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });
    let auth_state = Arc::new(
        AuthState::new(
            pool,
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_jwt_secret".to_string(),
        )
        .unwrap(),
    );
    let app = create_router(deploy_state.clone(), statichub_server::config::AuthMode::Enabled, Some(auth_state.clone()));

    let start_response = app
        .oneshot(
            Request::builder()
                .uri("/auth/device")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(start_response.into_body(), usize::MAX).await.unwrap();
    let start_json: Value = serde_json::from_slice(&body).unwrap();
    let user_code = start_json["user_code"].as_str().unwrap();

    let app = create_router(deploy_state, statichub_server::config::AuthMode::Enabled, Some(auth_state));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/device/verify")
                .method("POST")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("user_code={}", user_code)))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers().get(header::LOCATION).unwrap().to_str().unwrap();
    assert!(location.contains("accounts.google.com"));
}
```

- [ ] **Step 5: Add disabled auth route test**

Append:

```rust
#[sqlx::test]
async fn device_start_returns_503_when_auth_disabled(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool,
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });
    let app = create_router(deploy_state, statichub_server::config::AuthMode::Disabled, None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/device")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
```

- [ ] **Step 6: Run auth tests**

Run:

```bash
cargo test -p statichub-server --test auth_tests
```

Expected: all auth integration tests pass.

- [ ] **Step 7: Commit**

```bash
git add server/tests/auth_tests.rs
git commit -m "Test device login auth flow"
```

---

### Task 5: Update CLI Device Flow Client Types

**Files:**
- Modify: `cli/src/auth.rs`
- Modify: `cli/src/client.rs`

- [ ] **Step 1: Add CLI wire types**

In `cli/src/auth.rs`, keep `Credentials` and credential storage functions. Replace old login-specific types with:

```rust
#[derive(Debug, Deserialize)]
pub struct DeviceStartResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i64,
    pub interval: i64,
}

#[derive(Debug, Deserialize)]
pub struct DeviceTokenResponse {
    pub status: String,
    pub token: Option<String>,
    pub interval: Option<i64>,
}
```

Leave `generate_session_id` in place until the old server endpoints are removed.

- [ ] **Step 2: Update client imports**

In `cli/src/client.rs`, replace:

```rust
use crate::auth::{LoginRequest, LoginResponse, StatusResponse};
```

with:

```rust
use crate::auth::{DeviceStartResponse, DeviceTokenResponse};
```

- [ ] **Step 3: Add device client methods**

In `impl Client` in `cli/src/client.rs`, add:

```rust
pub async fn create_device_session(&self) -> Result<DeviceStartResponse> {
    let url = format!("{}/auth/device", self.base_url);
    let response = self
        .client
        .post(&url)
        .send()
        .await
        .context("Failed to start device login")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Device login failed with status {}: {}", status, body);
    }

    response
        .json()
        .await
        .context("Failed to parse device login response")
}

pub async fn poll_device_token(&self, device_code: &str) -> Result<DeviceTokenResponse> {
    let url = format!("{}/auth/device/token", self.base_url);
    let response = self
        .client
        .post(&url)
        .json(&serde_json::json!({ "device_code": device_code }))
        .send()
        .await
        .context("Failed to poll device login")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Device login poll failed with status {}: {}", status, body);
    }

    response
        .json()
        .await
        .context("Failed to parse device login poll response")
}
```

- [ ] **Step 4: Keep old methods temporarily compiling**

If `initiate_login` and `poll_auth_status` remain, keep the old `LoginRequest`, `LoginResponse`, and `StatusResponse` types in `cli/src/auth.rs` until a cleanup task removes the old flow. If they are removed now, delete those two methods from `cli/src/client.rs` in the same step.

Preferred rollout for this plan: keep the old types and methods to avoid removing compatibility endpoints in the same feature.

- [ ] **Step 5: Run CLI compile test**

Run:

```bash
cargo test -p statichub test_client_creation
```

Expected: CLI client tests pass.

- [ ] **Step 6: Commit**

```bash
git add cli/src/auth.rs cli/src/client.rs
git commit -m "Add CLI device login client"
```

---

### Task 6: Change CLI Login Command to Device Flow

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Replace `Commands::Login` body**

Replace the current `Commands::Login` match arm with:

```rust
Commands::Login => {
    let server_url = resolve_server_url(cli.local);
    let client = client::Client::new(server_url);

    println!("Logging in to StaticHub...");

    let login = client.create_device_session().await?;

    println!();
    println!("Open this URL:");
    println!("  {}", login.verification_uri);
    println!();
    println!("Enter this code:");
    println!("  {}", login.user_code);
    println!();
    println!("Or open:");
    println!("  {}", login.verification_uri_complete);
    println!();

    if let Err(e) = open::that(&login.verification_uri_complete) {
        println!("Could not open browser automatically: {}", e);
        println!("Please open the URL manually in your browser.");
        println!();
    }

    println!("Waiting for authentication...");

    let started_at = std::time::Instant::now();
    let expires_after = std::time::Duration::from_secs(login.expires_in.max(1) as u64);
    let mut interval = std::time::Duration::from_secs(login.interval.max(1) as u64);

    let token = loop {
        if started_at.elapsed() >= expires_after {
            anyhow::bail!("Authentication timed out. Please run 'statichub login' again.");
        }

        tokio::time::sleep(interval).await;
        let status = client.poll_device_token(&login.device_code).await?;

        match status.status.as_str() {
            "approved" => {
                let token = status
                    .token
                    .ok_or_else(|| anyhow::anyhow!("Login was approved but no token was returned"))?;
                break token;
            }
            "authorization_pending" => {
                if let Some(next_interval) = status.interval {
                    interval = std::time::Duration::from_secs(next_interval.max(1) as u64);
                }
            }
            "slow_down" => {
                let next_interval = status
                    .interval
                    .unwrap_or_else(|| interval.as_secs() as i64 + 5);
                interval = std::time::Duration::from_secs(next_interval.max(1) as u64);
            }
            "expired_token" => {
                anyhow::bail!("Authentication code expired. Please run 'statichub login' again.");
            }
            "access_denied" => {
                anyhow::bail!("Authentication was denied.");
            }
            other => {
                anyhow::bail!("Unexpected login status from server: {}", other);
            }
        }
    };

    auth::save_credentials(&token)?;

    println!("Login successful!");
    println!("Credentials saved to ~/.statichub/credentials.json");
}
```

- [ ] **Step 2: Run CLI build test**

Run:

```bash
cargo test -p statichub
```

Expected: CLI tests pass.

- [ ] **Step 3: Commit**

```bash
git add cli/src/main.rs
git commit -m "Use device flow for CLI login"
```

---

### Task 7: End-to-End Verification and Regression Checks

**Files:**
- No source changes expected unless verification exposes defects.

- [ ] **Step 1: Run server auth tests**

Run:

```bash
cargo test -p statichub-server --test auth_tests
```

Expected: all auth tests pass.

- [ ] **Step 2: Run API key regression tests**

Run:

```bash
cargo test -p statichub-server --test apikey_tests
```

Expected: API key tests pass, confirming API key behavior remains unchanged.

- [ ] **Step 3: Run CLI tests**

Run:

```bash
cargo test -p statichub
```

Expected: CLI tests pass.

- [ ] **Step 4: Run full workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: all workspace tests pass.

- [ ] **Step 5: Capture final status**

Run:

```bash
git status --short
```

Expected: only intentional tracked changes remain. The pre-existing `server/static/home/index.html` modification may still appear and must not be included in device-flow commits unless the user explicitly asks.

---

## Self-Review

Spec coverage:
- CLI device session creation: Task 5 and Task 6.
- Verification URL and short user code: Task 3 and Task 6.
- Browser verification page and Google redirect: Task 3 and Task 4.
- Google callback approval: Task 3.
- Persistent sessions: Task 1 and Task 2.
- Polling statuses and slow-down: Task 3, Task 4, and Task 6.
- Token single-consumption: Task 2 and Task 3.
- API key compatibility: Task 7.

Placeholder scan:
- No task uses deferred placeholder wording. Code snippets define concrete file contents, functions, status strings, and commands.

Type consistency:
- Server response fields use `device_code`, `user_code`, `verification_uri`, `verification_uri_complete`, `expires_in`, and `interval`.
- CLI response structs use the same field names.
- Poll responses use `status`, optional `token`, and optional `interval`.
