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
    storage::FilesystemStorage,
};

#[sqlx::test]
async fn test_list_projects(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
        base_url: "http://localhost:3000".to_string(),
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

    // Create user
    let user = User::create(&pool, "google", "user1", "test@example.com", "testuser")
        .await
        .unwrap();

    // Create projects
    Project::create_owned(&pool, user.id, "project1", None)
        .await
        .unwrap();
    Project::create_owned(&pool, user.id, "project2", None)
        .await
        .unwrap();

    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let app = create_router(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects")
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
async fn test_get_project_info(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
        base_url: "http://localhost:3000".to_string(),
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

    // Create a deploy
    let _deploy = Deploy::create(&pool, project.id, "myapp/deploy-1")
        .await
        .unwrap();

    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let app = create_router(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects/myapp")
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

    assert_eq!(json["name"], "myapp");
    assert!(json["deploys"].is_array());
    assert_eq!(json["deploys"].as_array().unwrap().len(), 1);
}

#[sqlx::test]
async fn test_rollback_project(pool: SqlitePool) {
    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: Arc::new(FilesystemStorage::new("./test_storage".into())),
        base_url: "http://localhost:3000".to_string(),
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

    // Create two deploys
    let _deploy1 = Deploy::create(&pool, project.id, "myapp/deploy-1")
        .await
        .unwrap();
    let deploy2 = Deploy::create(&pool, project.id, "myapp/deploy-2")
        .await
        .unwrap();

    // Set current to deploy2
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy2.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let app = create_router(deploy_state, auth_state);

    // Rollback to version 1
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects/myapp/rollback")
                .method("POST")
                .header("authorization", format!("Bearer {}", jwt))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"version": 1}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["current_version"], 1);
}
