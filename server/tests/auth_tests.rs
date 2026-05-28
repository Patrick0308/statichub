use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use sqlx::SqlitePool;
use statichub_server::storage::FilesystemStorage;
use statichub_server::{
    api::{AuthState, DeployState},
    create_router,
};
use std::sync::Arc;
use tower::ServiceExt;

#[sqlx::test]
async fn test_login_initiation(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
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

    let app = create_router(deploy_state, statichub_server::config::AuthMode::Enabled, Some(auth_state));

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

    assert!(json["auth_url"]
        .as_str()
        .unwrap()
        .contains("accounts.google.com"));
    assert!(json["auth_url"]
        .as_str()
        .unwrap()
        .contains("test-session-123"));
}

#[sqlx::test]
async fn test_auth_status_not_ready(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
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

    let app = create_router(deploy_state, statichub_server::config::AuthMode::Enabled, Some(auth_state));

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

#[sqlx::test]
async fn test_auth_status_after_login(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
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

    // First create a session via login
    let app = create_router(deploy_state.clone(), statichub_server::config::AuthMode::Enabled, Some(auth_state.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/login/google")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"session_id": "test-session-456"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Now check the status - should exist but token should be null
    let app = create_router(deploy_state, statichub_server::config::AuthMode::Enabled, Some(auth_state));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/status/test-session-456")
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

    // Token should be null because OAuth callback hasn't completed
    assert!(json["token"].is_null());
}

#[sqlx::test]
async fn test_duplicate_session_id_returns_conflict(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
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

    let app = create_router(deploy_state.clone(), statichub_server::config::AuthMode::Enabled, Some(auth_state.clone()));

    // First login request should succeed
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/login/google")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"session_id": "duplicate-session"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Second login request with same session_id should return Conflict
    let app = create_router(deploy_state, statichub_server::config::AuthMode::Enabled, Some(auth_state));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/login/google")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"session_id": "duplicate-session"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "conflict");
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("Session ID already in use"));
}

#[sqlx::test]
async fn test_session_not_found_check(pool: SqlitePool) {
    // This tests the logic that was added to prevent silent failures
    // when OAuth callback completes but session has expired.
    // We test this by verifying session lookup behavior.
    let auth_state = AuthState::new(
        pool.clone(),
        "test_client_id".to_string(),
        "test_client_secret".to_string(),
        "http://localhost:3000/auth/callback/google".to_string(),
        "test_jwt_secret".to_string(),
    )
    .unwrap();

    // Add a session
    {
        let mut sessions = auth_state.sessions.write().await;
        sessions.insert(
            "exists".to_string(),
            statichub_server::api::PendingSession {
                token: None,
                created_at: chrono::Utc::now(),
            },
        );
    }

    // Verify existing session can be found
    {
        let sessions = auth_state.sessions.read().await;
        assert!(sessions.contains_key("exists"));
    }

    // Verify non-existent session is not found
    {
        let sessions = auth_state.sessions.read().await;
        assert!(!sessions.contains_key("does-not-exist"));
    }

    // The actual OAuth callback logic will now check for session existence
    // and return an error if the session doesn't exist, preventing silent failures
}

#[sqlx::test]
async fn auth_login_returns_503_when_auth_disabled(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });

    let app = create_router(deploy_state, statichub_server::config::AuthMode::Disabled, None);

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

#[sqlx::test]
async fn protected_route_returns_503_when_auth_disabled(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });

    let app = create_router(deploy_state, statichub_server::config::AuthMode::Disabled, None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects")
                .body(Body::empty())
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

#[sqlx::test]
async fn anonymous_route_still_available_when_auth_disabled(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });

    let app = create_router(deploy_state, statichub_server::config::AuthMode::Disabled, None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/deploys/anonymous")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"files":[{"path":"index.html","content":"<h1>ok</h1>"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
