# Auth-Disabled Local Start Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable `statichub-server` to start locally without Google OAuth env vars while explicitly returning `503` for auth/protected endpoints when auth is disabled.

**Architecture:** Add an explicit runtime auth mode (`Enabled` or `Disabled`) resolved from environment variables, then compose router behavior based on that mode. In disabled mode, keep endpoint shapes stable by mounting explicit `503` handlers rather than removing routes. Preserve existing behavior when auth env vars are present.

**Tech Stack:** Rust, Axum, SQLx, Tokio, existing `statichub-server` module layout and test suite.

---

## File Structure (Planned Changes)

- Modify: `server/src/config.rs`
- Modify: `server/src/main.rs`
- Modify: `server/src/lib.rs`
- Modify: `server/src/api/mod.rs`
- Create: `server/src/api/auth_disabled.rs`
- Modify: `server/tests/auth_tests.rs`
- Modify: `README.md`
- Modify: `.env.example`

Responsibilities:
- `config.rs`: auth mode enum + env resolution.
- `main.rs`: startup wiring according to mode.
- `lib.rs`: router composition by mode.
- `api/auth_disabled.rs`: reusable `503` handlers.
- `auth_tests.rs`: mode behavior regression coverage.
- docs: local startup semantics and env variable grouping.

### Task 1: Add Auth Mode in Configuration

**Files:**
- Modify: `server/src/config.rs`
- Test: `server/src/config.rs` (unit tests in existing test module)

- [ ] **Step 1: Write failing tests for auth mode resolution**

Add these tests in `#[cfg(test)] mod config_tests`:

```rust
#[test]
#[serial]
fn test_auth_mode_enabled_when_google_env_present() {
    std::env::set_var("STATICHUB_GOOGLE_CLIENT_ID", "id");
    std::env::set_var("STATICHUB_GOOGLE_CLIENT_SECRET", "secret");

    let mode = resolve_auth_mode_from_env();
    assert_eq!(mode, AuthMode::Enabled);

    std::env::remove_var("STATICHUB_GOOGLE_CLIENT_ID");
    std::env::remove_var("STATICHUB_GOOGLE_CLIENT_SECRET");
}

#[test]
#[serial]
fn test_auth_mode_disabled_when_google_env_missing() {
    std::env::remove_var("STATICHUB_GOOGLE_CLIENT_ID");
    std::env::remove_var("STATICHUB_GOOGLE_CLIENT_SECRET");

    let mode = resolve_auth_mode_from_env();
    assert_eq!(mode, AuthMode::Disabled);
}

#[test]
#[serial]
fn test_auth_mode_disabled_when_google_env_empty() {
    std::env::set_var("STATICHUB_GOOGLE_CLIENT_ID", "");
    std::env::set_var("STATICHUB_GOOGLE_CLIENT_SECRET", "secret");

    let mode = resolve_auth_mode_from_env();
    assert_eq!(mode, AuthMode::Disabled);

    std::env::remove_var("STATICHUB_GOOGLE_CLIENT_ID");
    std::env::remove_var("STATICHUB_GOOGLE_CLIENT_SECRET");
}
```

- [ ] **Step 2: Run targeted test to verify failure**

Run:

```bash
cargo test -p statichub-server config_tests::test_auth_mode_enabled_when_google_env_present
```

Expected: FAIL because `AuthMode` / `resolve_auth_mode_from_env` do not exist yet.

- [ ] **Step 3: Implement auth mode enum + resolver**

Add to `server/src/config.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Enabled,
    Disabled,
}

pub fn resolve_auth_mode_from_env() -> AuthMode {
    let client_id = std::env::var("STATICHUB_GOOGLE_CLIENT_ID").ok();
    let client_secret = std::env::var("STATICHUB_GOOGLE_CLIENT_SECRET").ok();

    match (client_id, client_secret) {
        (Some(id), Some(secret)) if !id.trim().is_empty() && !secret.trim().is_empty() => {
            AuthMode::Enabled
        }
        _ => AuthMode::Disabled,
    }
}
```

- [ ] **Step 4: Run tests to verify pass**

Run:

```bash
cargo test -p statichub-server config_tests::test_auth_mode_enabled_when_google_env_present config_tests::test_auth_mode_disabled_when_google_env_missing config_tests::test_auth_mode_disabled_when_google_env_empty
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/config.rs
git commit -m "Add auth mode resolution from environment"
```

### Task 2: Add Disabled Auth Handlers

**Files:**
- Create: `server/src/api/auth_disabled.rs`
- Modify: `server/src/api/mod.rs`
- Test: `server/tests/auth_tests.rs`

- [ ] **Step 1: Write failing integration test for disabled auth endpoint**

Add this test in `server/tests/auth_tests.rs`:

```rust
#[sqlx::test]
async fn auth_login_returns_503_when_auth_disabled(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });

    let app = create_router(
        deploy_state,
        statichub_server::config::AuthMode::Disabled,
        None,
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/login/google")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"session_id":"test-session-123"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "authentication is disabled in local mode");
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test -p statichub-server auth_login_returns_503_when_auth_disabled -- --nocapture
```

Expected: FAIL because no disabled handler exists.

- [ ] **Step 3: Implement disabled handler module**

Create `server/src/api/auth_disabled.rs`:

```rust
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
```

Export in `server/src/api/mod.rs`:

```rust
mod auth_disabled;
pub use auth_disabled::{auth_disabled, protected_disabled};
```

- [ ] **Step 4: Re-run the test**

Run:

```bash
cargo test -p statichub-server auth_login_returns_503_when_auth_disabled -- --nocapture
```

Expected: still FAIL until router wiring is added in Task 3.

- [ ] **Step 5: Commit**

```bash
git add server/src/api/auth_disabled.rs server/src/api/mod.rs server/tests/auth_tests.rs
git commit -m "Add disabled auth response handlers"
```

### Task 3: Wire Startup and Router by Auth Mode

**Files:**
- Modify: `server/src/main.rs`
- Modify: `server/src/lib.rs`
- Test: `server/tests/auth_tests.rs`

- [ ] **Step 1: Write failing tests for protected endpoint behavior in disabled mode**

Add in `server/tests/auth_tests.rs`:

```rust
#[sqlx::test]
async fn protected_route_returns_503_when_auth_disabled(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });

    let app = create_router(
        deploy_state,
        statichub_server::config::AuthMode::Disabled,
        None,
    );

    let response = app
        .oneshot(Request::builder().uri("/api/projects").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "authentication is disabled in local mode");
}

#[sqlx::test]
async fn anonymous_route_still_available_when_auth_disabled(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });

    let app = create_router(
        deploy_state,
        statichub_server::config::AuthMode::Disabled,
        None,
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/deploys/anonymous")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"files":[{"path":"index.html","content":"<h1>ok</h1>"}]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p statichub-server protected_route_returns_503_when_auth_disabled anonymous_route_still_available_when_auth_disabled -- --nocapture
```

Expected: FAIL before router mode-aware wiring.

- [ ] **Step 3: Update router signature and composition**

In `server/src/lib.rs`, change signature to accept auth mode and optional auth state:

```rust
pub fn create_router(
    deploy_state: Arc<api::DeployState>,
    auth_mode: crate::config::AuthMode,
    auth_state: Option<Arc<api::AuthState>>,
) -> Router {
    // Always mount health + anonymous + static fallback.
    // If Enabled: mount real auth/protected routes.
    // If Disabled: mount same paths with api::auth_disabled / api::protected_disabled.
}
```

In `server/src/main.rs`, wire it:

```rust
let auth_mode = statichub_server::config::resolve_auth_mode_from_env();
let auth_state = match auth_mode {
    statichub_server::config::AuthMode::Enabled => Some(Arc::new(api::AuthState::new(
        pool.clone(),
        std::env::var("STATICHUB_GOOGLE_CLIENT_ID").expect("STATICHUB_GOOGLE_CLIENT_ID must be set"),
        std::env::var("STATICHUB_GOOGLE_CLIENT_SECRET").expect("STATICHUB_GOOGLE_CLIENT_SECRET must be set"),
        std::env::var("STATICHUB_GOOGLE_REDIRECT_URL").unwrap_or_else(|_| "http://localhost:3000/auth/callback/google".to_string()),
        std::env::var("STATICHUB_JWT_SECRET").expect("STATICHUB_JWT_SECRET must be set in production"),
    )?)),
    statichub_server::config::AuthMode::Disabled => {
        tracing::warn!("Auth disabled: missing Google OAuth env vars; /auth and protected APIs will return 503");
        None
    }
};

let app = create_router(deploy_state, auth_mode, auth_state)
    .layer(axum::middleware::from_fn_with_state(
        config.clone(),
        statichub_server::middleware::host_validation_middleware,
    ));
```

- [ ] **Step 4: Run targeted server tests**

Run:

```bash
cargo test -p statichub-server auth_ -- --nocapture
cargo test -p statichub-server middleware::auth -- --nocapture
```

Expected: PASS, including new disabled-mode tests.

- [ ] **Step 5: Commit**

```bash
git add server/src/main.rs server/src/lib.rs server/tests/auth_tests.rs
git commit -m "Wire auth-enabled and auth-disabled router modes"
```

### Task 4: Update Documentation for Local Startup

**Files:**
- Modify: `README.md`
- Modify: `.env.example`

- [ ] **Step 1: Add README local-mode behavior docs**

Update `README.md` “Run Server Locally” section with:

```markdown
For quick local startup, Google OAuth variables are optional.
If `STATICHUB_GOOGLE_CLIENT_ID` / `STATICHUB_GOOGLE_CLIENT_SECRET` are missing,
server starts in auth-disabled mode:
- `/auth/*` returns `503`
- authenticated management APIs return `503`
- anonymous deploy + static serving remain available
```

- [ ] **Step 2: Group env vars in `.env.example`**

Update `.env.example` comments to separate:
- Base startup env vars
- Optional auth-enabled env vars (`STATICHUB_GOOGLE_*`, `STATICHUB_JWT_SECRET`)

- [ ] **Step 3: Verify formatting and references**

Run:

```bash
rg -n "auth-disabled mode|STATICHUB_GOOGLE_CLIENT_ID|STATICHUB_JWT_SECRET" README.md .env.example
```

Expected: new guidance visible in both files.

- [ ] **Step 4: Commit**

```bash
git add README.md .env.example
git commit -m "Document auth-disabled local startup mode"
```

### Task 5: Final Validation

**Files:**
- No code changes expected (verification only)

- [ ] **Step 1: Run targeted package tests**

```bash
cargo test -p statichub-server
```

Expected: PASS.

- [ ] **Step 2: Run workspace tests for regression confidence**

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 3: Run workspace check**

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 4: Summarize validation evidence in PR/body**

Include exact commands and pass/fail outcomes.

- [ ] **Step 5: Final integration commit (if needed)**

```bash
git status
# If any final plan-aligned updates remain:
git add <files>
git commit -m "Finalize auth-disabled local startup behavior"
```
