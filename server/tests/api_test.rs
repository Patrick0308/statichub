use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use serial_test::serial;
use statichub_server::{
    api::{AuthState, DeployState},
    config::ServerConfig,
    create_router,
    models::Deploy,
    storage::{FilesystemStorage, Storage},
};
use statichub_shared::DeployResponse;
use std::sync::Arc;
use tower::ServiceExt;

fn multipart_body(boundary: &str, parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (filename, content) in parts {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"files\"; filename=\"{}\"\r\n\r\n",
                filename
            )
            .as_bytes(),
        );
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    body
}

#[tokio::test]
#[serial]
async fn test_deploy_with_different_hosts() {
    std::env::set_var(
        "STATICHUB_ALLOWED_DOMAINS",
        "localhost,statichub.dev,example.com",
    );

    let pool = statichub_server::test_utils::create_test_pool()
        .await
        .unwrap();

    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(
        temp_storage_dir.path().to_path_buf(),
    ));

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
    let app = create_router(
        deploy_state.clone(),
        statichub_server::config::AuthMode::Enabled,
        Some(auth_state.clone()),
    )
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
    let app = create_router(
        deploy_state.clone(),
        statichub_server::config::AuthMode::Enabled,
        Some(auth_state.clone()),
    )
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
    let storage = Arc::new(FilesystemStorage::new(
        temp_storage_dir.path().to_path_buf(),
    ));

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

    let app = create_router(
        deploy_state,
        statichub_server::config::AuthMode::Enabled,
        Some(auth_state),
    )
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
    let storage = Arc::new(FilesystemStorage::new(
        temp_storage_dir.path().to_path_buf(),
    ));

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

    let app = create_router(
        deploy_state,
        statichub_server::config::AuthMode::Enabled,
        Some(auth_state),
    )
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

#[tokio::test]
#[serial]
async fn test_anonymous_markdown_deploy_renders_and_stores_source() {
    std::env::set_var("STATICHUB_ALLOWED_DOMAINS", "localhost");

    let pool = statichub_server::test_utils::create_test_pool()
        .await
        .unwrap();
    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(
        temp_storage_dir.path().to_path_buf(),
    ));
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
    let app = create_router(
        deploy_state,
        statichub_server::config::AuthMode::Enabled,
        Some(auth_state),
    )
    .layer(axum::middleware::from_fn_with_state(
        config,
        statichub_server::middleware::host_validation_middleware,
    ));

    let boundary = "----MarkdownBoundary";
    let markdown = b"# Markdown Page\n\nHello **StaticHub**.";
    let request = Request::builder()
        .method("POST")
        .uri("/api/deploys/anonymous")
        .header("host", "localhost:3000")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(multipart_body(
            boundary,
            &[("index.md", markdown)],
        )))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let deploy_response: DeployResponse = serde_json::from_slice(&body_bytes).unwrap();
    let deploy = Deploy::find_by_id(&pool, deploy_response.deploy_id)
        .await
        .unwrap()
        .unwrap();

    let source = storage
        .get_file(&deploy.storage_path, "index.md")
        .await
        .unwrap();
    let html = storage
        .get_file(&deploy.storage_path, "index.html")
        .await
        .unwrap();
    let html = String::from_utf8(html).unwrap();

    assert_eq!(source, markdown);
    assert!(html.contains("<h1>Markdown Page</h1>"));
    assert!(html.contains("<strong>StaticHub</strong>"));
    assert_eq!(deploy.file_count, 2);
    assert_eq!(
        deploy.total_size_bytes,
        markdown.len() as i64 + html.as_bytes().len() as i64
    );

    std::env::remove_var("STATICHUB_ALLOWED_DOMAINS");
}

#[tokio::test]
#[serial]
async fn test_markdown_deploy_rejects_non_utf8_source() {
    std::env::set_var("STATICHUB_ALLOWED_DOMAINS", "localhost");

    let pool = statichub_server::test_utils::create_test_pool()
        .await
        .unwrap();
    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(
        temp_storage_dir.path().to_path_buf(),
    ));
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage,
    });
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
    let config = ServerConfig::from_env().unwrap();
    let app = create_router(
        deploy_state,
        statichub_server::config::AuthMode::Enabled,
        Some(auth_state),
    )
    .layer(axum::middleware::from_fn_with_state(
        config,
        statichub_server::middleware::host_validation_middleware,
    ));

    let boundary = "----InvalidMarkdownBoundary";
    let request = Request::builder()
        .method("POST")
        .uri("/api/deploys/anonymous")
        .header("host", "localhost:3000")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(multipart_body(
            boundary,
            &[("index.md", &[0xff, 0xfe])],
        )))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    std::env::remove_var("STATICHUB_ALLOWED_DOMAINS");
}

#[tokio::test]
#[serial]
async fn test_markdown_in_multi_file_deploy_is_not_rendered() {
    std::env::set_var("STATICHUB_ALLOWED_DOMAINS", "localhost");

    let pool = statichub_server::test_utils::create_test_pool()
        .await
        .unwrap();
    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(
        temp_storage_dir.path().to_path_buf(),
    ));
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
    let app = create_router(
        deploy_state,
        statichub_server::config::AuthMode::Enabled,
        Some(auth_state),
    )
    .layer(axum::middleware::from_fn_with_state(
        config,
        statichub_server::middleware::host_validation_middleware,
    ));

    let boundary = "----MultiMarkdownBoundary";
    let request = Request::builder()
        .method("POST")
        .uri("/api/deploys/anonymous")
        .header("host", "localhost:3000")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(multipart_body(
            boundary,
            &[
                ("index.md", b"# Not Rendered"),
                ("style.css", b"body { color: red; }"),
            ],
        )))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let deploy_response: DeployResponse = serde_json::from_slice(&body_bytes).unwrap();
    let deploy = Deploy::find_by_id(&pool, deploy_response.deploy_id)
        .await
        .unwrap()
        .unwrap();

    storage
        .get_file(&deploy.storage_path, "index.md")
        .await
        .unwrap();
    storage
        .get_file(&deploy.storage_path, "style.css")
        .await
        .unwrap();
    let generated = storage.get_file(&deploy.storage_path, "index.html").await;

    assert!(generated.is_err());
    assert_eq!(deploy.file_count, 2);

    std::env::remove_var("STATICHUB_ALLOWED_DOMAINS");
}
