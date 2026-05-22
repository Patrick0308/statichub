use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;
use statichub_server::{
    api::{AuthState, DeployState},
    config::ServerConfig,
    create_router,
    storage::FilesystemStorage,
};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_deploy_with_different_hosts() {
    std::env::set_var("STATICHUB_ALLOWED_DOMAINS", "localhost,statichub.dev,example.com");

    let pool = statichub_server::test_utils::create_test_pool()
        .await
        .unwrap();

    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_storage_dir.path().to_path_buf()));

    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });

    let auth_state = Arc::new(
        AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let config = ServerConfig::from_env().unwrap();

    // Create router with host validation middleware
    let app = create_router(deploy_state.clone(), auth_state.clone())
        .layer(axum::middleware::from_fn_with_state(
            config.clone(),
            statichub_server::middleware::host_validation_middleware,
        ));

    // Test 1: Deploy via localhost:3000
    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.html\"\r\n\r\n<html>Test localhost</html>\r\n--{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/deploys/anonymous")
        .header("host", "localhost:3000")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Deploy via localhost:3000 should succeed"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let deploy: Value = serde_json::from_slice(&body_bytes).unwrap();
    let url = deploy["url"].as_str().unwrap();
    assert!(
        url.contains("localhost:3000"),
        "URL should contain localhost:3000, got {}",
        url
    );

    // Test 2: Deploy via statichub.dev
    let app = create_router(deploy_state.clone(), auth_state.clone())
        .layer(axum::middleware::from_fn_with_state(
            config.clone(),
            statichub_server::middleware::host_validation_middleware,
        ));

    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.html\"\r\n\r\n<html>Test statichub.dev</html>\r\n--{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/deploys/anonymous")
        .header("host", "statichub.dev")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Deploy via statichub.dev should succeed"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let deploy: Value = serde_json::from_slice(&body_bytes).unwrap();
    let url = deploy["url"].as_str().unwrap();
    assert!(
        url.contains("statichub.dev"),
        "URL should contain statichub.dev, got {}",
        url
    );

    std::env::remove_var("STATICHUB_ALLOWED_DOMAINS");
}

#[tokio::test]
#[serial]
async fn test_reject_unallowed_domain() {
    std::env::set_var("STATICHUB_ALLOWED_DOMAINS", "localhost,statichub.dev");

    let pool = statichub_server::test_utils::create_test_pool()
        .await
        .unwrap();

    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_storage_dir.path().to_path_buf()));

    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });

    let auth_state = Arc::new(
        AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let config = ServerConfig::from_env().unwrap();

    let app = create_router(deploy_state, auth_state)
        .layer(axum::middleware::from_fn_with_state(
            config,
            statichub_server::middleware::host_validation_middleware,
        ));

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.html\"\r\n\r\n<html>Test malicious</html>\r\n--{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/deploys/anonymous")
        .header("host", "malicious.com")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Request with unauthorized domain should be rejected with 403"
    );

    std::env::remove_var("STATICHUB_ALLOWED_DOMAINS");
}

#[tokio::test]
#[serial]
async fn test_missing_host_header() {
    std::env::set_var("STATICHUB_ALLOWED_DOMAINS", "localhost");

    let pool = statichub_server::test_utils::create_test_pool()
        .await
        .unwrap();

    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_storage_dir.path().to_path_buf()));

    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });

    let auth_state = Arc::new(
        AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let config = ServerConfig::from_env().unwrap();

    let app = create_router(deploy_state, auth_state)
        .layer(axum::middleware::from_fn_with_state(
            config,
            statichub_server::middleware::host_validation_middleware,
        ));

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.html\"\r\n\r\n<html>Test</html>\r\n--{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/deploys/anonymous")
        // No Host header
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should fail with BAD_REQUEST or similar
    assert!(
        !response.status().is_success(),
        "Request without Host header should fail"
    );

    std::env::remove_var("STATICHUB_ALLOWED_DOMAINS");
}
