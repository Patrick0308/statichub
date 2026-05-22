use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::ServiceExt;
use statichub_server::{
    api::{AuthState, DeployState},
    create_router,
    models::{Deploy, Project, User},
    storage::{FilesystemStorage, Storage},
};

#[sqlx::test]
async fn test_add_domain(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });

    let auth_state = Arc::new(
        AuthState::new(
            pool.clone(),
            "test".to_string(),
            "test".to_string(),
            "http://localhost:3000/callback".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let user = User::create(&pool, "google", "user1", "test@example.com", "testuser")
        .await
        .unwrap();

    let _project = Project::create_owned(&pool, user.id, "myapp", None)
        .await
        .unwrap();

    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let app = create_router(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects/myapp/domains")
                .method("POST")
                .header("authorization", format!("Bearer {}", jwt))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"domain": "example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["domain"], "example.com");
    assert_eq!(json["status"], "pending_verification");
    assert!(json["verification_token"].is_string());
}

#[sqlx::test]
async fn test_list_domains(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });

    let auth_state = Arc::new(
        AuthState::new(
            pool.clone(),
            "test".to_string(),
            "test".to_string(),
            "http://localhost:3000/callback".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let user = User::create(&pool, "google", "user1", "test@example.com", "testuser")
        .await
        .unwrap();

    let project = Project::create_owned(&pool, user.id, "myapp", None)
        .await
        .unwrap();

    // Add two domains
    use statichub_server::models::Domain;
    Domain::create(&pool, project.id, "example.com", "token1")
        .await
        .unwrap();
    Domain::create(&pool, project.id, "example.org", "token2")
        .await
        .unwrap();

    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let app = create_router(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects/myapp/domains")
                .method("GET")
                .header("authorization", format!("Bearer {}", jwt))
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

    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[sqlx::test]
async fn test_verify_domain_success(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));

    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });

    let auth_state = Arc::new(
        AuthState::new(
            pool.clone(),
            "test".to_string(),
            "test".to_string(),
            "http://localhost:3000/callback".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let user = User::create(&pool, "google", "user1", "test@example.com", "testuser")
        .await
        .unwrap();

    let project = Project::create_owned(&pool, user.id, "myapp", None)
        .await
        .unwrap();

    // Create deploy
    let deploy = Deploy::create(&pool, project.id, "myapp/deploy-1")
        .await
        .unwrap();

    // Set as current
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    // Add domain
    use statichub_server::models::Domain;
    let _domain = Domain::create(&pool, project.id, "example.com", "test-token-123")
        .await
        .unwrap();

    // Create verification file in deploy
    storage
        .store_file(
            &deploy.storage_path,
            "statichub-verify.txt",
            "test-token-123".as_bytes(),
        )
        .await
        .unwrap();

    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let app = create_router(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects/myapp/domains/example.com/verify")
                .method("POST")
                .header("authorization", format!("Bearer {}", jwt))
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

    assert_eq!(json["status"], "verified");
}

#[sqlx::test]
async fn test_remove_domain(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
    });

    let auth_state = Arc::new(
        AuthState::new(
            pool.clone(),
            "test".to_string(),
            "test".to_string(),
            "http://localhost:3000/callback".to_string(),
            "test_secret".to_string(),
        )
        .unwrap(),
    );

    let user = User::create(&pool, "google", "user1", "test@example.com", "testuser")
        .await
        .unwrap();

    let project = Project::create_owned(&pool, user.id, "myapp", None)
        .await
        .unwrap();

    // Add domain
    use statichub_server::models::Domain;
    Domain::create(&pool, project.id, "example.com", "token1")
        .await
        .unwrap();

    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let app = create_router(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects/myapp/domains/example.com")
                .method("DELETE")
                .header("authorization", format!("Bearer {}", jwt))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify domain is gone
    let domain = Domain::find_by_domain(&pool, "example.com")
        .await
        .unwrap();
    assert!(domain.is_none());
}
