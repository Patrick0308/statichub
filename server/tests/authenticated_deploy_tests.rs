use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use statichub_server::{api, create_router, db, models, storage};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn test_authenticated_deploy_creates_new_project() {
    let pool = db::create_pool(":memory:").await.unwrap();

    // Create a test user
    let user = models::User::create(&pool, "google", "test123", "test@example.com", "testuser")
        .await
        .unwrap();

    // Setup states
    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(storage::FilesystemStorage::new(temp_storage_dir.path().to_path_buf())) as Arc<dyn storage::Storage>;

    let deploy_state = Arc::new(api::DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://localhost:3000".to_string(),
    });

    let auth_state = Arc::new(
        api::AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let app = create_router(deploy_state, auth_state.clone());

    // Generate JWT
    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    // Create multipart request
    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.html\"\r\n\r\n<html>Test</html>\r\n--{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/projects/my-test-app/deploys")
        .header("authorization", format!("Bearer {}", jwt))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Debug: print response body on error
    let status = response.status();
    if status != StatusCode::OK {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        eprintln!("Error response: {}", String::from_utf8_lossy(&body));
        panic!("Expected 200, got {}", status);
    }

    assert_eq!(status, StatusCode::OK);

    // Verify project was created
    let project = models::Project::find_by_name(&pool, "my-test-app")
        .await
        .unwrap();
    assert!(project.is_some());
    let project = project.unwrap();
    assert_eq!(project.owner_id, Some(user.id));
    assert!(!project.is_anonymous);
}

#[tokio::test]
async fn test_authenticated_deploy_requires_jwt() {
    let pool = db::create_pool(":memory:").await.unwrap();

    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(storage::FilesystemStorage::new(temp_storage_dir.path().to_path_buf())) as Arc<dyn storage::Storage>;

    let deploy_state = Arc::new(api::DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://localhost:3000".to_string(),
    });

    let auth_state = Arc::new(
        api::AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let app = create_router(deploy_state, auth_state);

    // Request without JWT
    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.html\"\r\n\r\n<html>Test</html>\r\n--{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/projects/my-test-app/deploys")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_authenticated_deploy_validates_project_name() {
    let pool = db::create_pool(":memory:").await.unwrap();

    let user = models::User::create(&pool, "google", "test123", "test@example.com", "testuser")
        .await
        .unwrap();

    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(storage::FilesystemStorage::new(temp_storage_dir.path().to_path_buf())) as Arc<dyn storage::Storage>;

    let deploy_state = Arc::new(api::DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://localhost:3000".to_string(),
    });

    let auth_state = Arc::new(
        api::AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let app = create_router(deploy_state, auth_state.clone());

    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.html\"\r\n\r\n<html>Test</html>\r\n--{}--\r\n",
        boundary, boundary
    );

    // Test invalid project name (uppercase)
    let request = Request::builder()
        .method("POST")
        .uri("/api/projects/MyApp/deploys")
        .header("authorization", format!("Bearer {}", jwt))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_authenticated_deploy_enforces_ownership() {
    let pool = db::create_pool(":memory:").await.unwrap();

    // Create two users
    let user1 = models::User::create(&pool, "google", "user1", "user1@example.com", "user1")
        .await
        .unwrap();
    let user2 = models::User::create(&pool, "google", "user2", "user2@example.com", "user2")
        .await
        .unwrap();

    // Create a project owned by user1
    let _project = models::Project::create_owned(&pool, user1.id, "user1-project", None)
        .await
        .unwrap();

    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(storage::FilesystemStorage::new(temp_storage_dir.path().to_path_buf())) as Arc<dyn storage::Storage>;

    let deploy_state = Arc::new(api::DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://localhost:3000".to_string(),
    });

    let auth_state = Arc::new(
        api::AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let app = create_router(deploy_state, auth_state.clone());

    // Try to deploy to user1's project as user2
    let jwt = auth_state.generate_jwt(user2.id, &user2.email).unwrap();

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.html\"\r\n\r\n<html>Test</html>\r\n--{}--\r\n",
        boundary, boundary
    );

    let request = Request::builder()
        .method("POST")
        .uri("/api/projects/user1-project/deploys")
        .header("authorization", format!("Bearer {}", jwt))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_authenticated_deploy_increments_version() {
    let pool = db::create_pool(":memory:").await.unwrap();

    let user = models::User::create(&pool, "google", "test123", "test@example.com", "testuser")
        .await
        .unwrap();

    let temp_storage_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(storage::FilesystemStorage::new(temp_storage_dir.path().to_path_buf())) as Arc<dyn storage::Storage>;

    let deploy_state = Arc::new(api::DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://localhost:3000".to_string(),
    });

    let auth_state = Arc::new(
        api::AuthState::new(
            pool.clone(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.html\"\r\n\r\n<html>Test</html>\r\n--{}--\r\n",
        boundary, boundary
    );

    // First deploy
    let app1 = create_router(
        deploy_state.clone(),
        auth_state.clone(),
    );
    let request1 = Request::builder()
        .method("POST")
        .uri("/api/projects/versioned-app/deploys")
        .header("authorization", format!("Bearer {}", jwt))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body.clone()))
        .unwrap();

    let response1 = app1.oneshot(request1).await.unwrap();
    assert_eq!(response1.status(), StatusCode::OK);
    let body1 = axum::body::to_bytes(response1.into_body(), usize::MAX)
        .await
        .unwrap();
    let deploy1: statichub_shared::DeployResponse = serde_json::from_slice(&body1).unwrap();
    assert_eq!(deploy1.version, Some(1));

    // Second deploy
    let app2 = create_router(deploy_state, auth_state.clone());
    let request2 = Request::builder()
        .method("POST")
        .uri("/api/projects/versioned-app/deploys")
        .header("authorization", format!("Bearer {}", jwt))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap();

    let response2 = app2.oneshot(request2).await.unwrap();
    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = axum::body::to_bytes(response2.into_body(), usize::MAX)
        .await
        .unwrap();
    let deploy2: statichub_shared::DeployResponse = serde_json::from_slice(&body2).unwrap();
    assert_eq!(deploy2.version, Some(2));
}
