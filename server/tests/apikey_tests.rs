use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
};
use serde_json::Value;
use sqlx::SqlitePool;
use statichub_server::{
    api::{AuthState, DeployState},
    config::ServerConfig,
    create_router,
    middleware::hash_api_key,
    models::{ApiKey, User},
    storage::FilesystemStorage,
};
use std::sync::Arc;
use tower::ServiceExt;

fn create_test_router_with_middleware(
    deploy_state: Arc<DeployState>,
    auth_state: Arc<AuthState>,
) -> axum::Router {
    let config = ServerConfig {
        port: 3000,
        allowed_domains: vec!["localhost".to_string()],
    };
    create_router(deploy_state, statichub_server::config::AuthMode::Enabled, Some(auth_state)).layer(middleware::from_fn_with_state(
        config,
        statichub_server::middleware::host_validation_middleware,
    ))
}

#[sqlx::test]
async fn test_apikey_create_list_revoke_with_jwt(pool: SqlitePool) {
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
    let jwt = auth_state.generate_jwt(user.id, &user.email).unwrap();

    let app = create_test_router_with_middleware(deploy_state.clone(), auth_state.clone());

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/apikeys")
                .method("POST")
                .header("host", "localhost:3000")
                .header("authorization", format!("Bearer {}", jwt))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"ci"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: Value = serde_json::from_slice(&create_body).unwrap();
    let key_id = create_json["id"].as_i64().unwrap();
    let plaintext = create_json["api_key"].as_str().unwrap().to_string();
    assert!(plaintext.starts_with("shk_"));

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/apikeys")
                .header("host", "localhost:3000")
                .header("authorization", format!("Bearer {}", jwt))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);

    let revoke_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/apikeys/{}/revoke", key_id))
                .method("POST")
                .header("host", "localhost:3000")
                .header("authorization", format!("Bearer {}", jwt))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(revoke_response.status(), StatusCode::OK);
}

#[sqlx::test]
async fn test_apikey_cannot_manage_apikeys(pool: SqlitePool) {
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

    let user = User::create(&pool, "google", "user2", "u2@example.com", "u2")
        .await
        .unwrap();
    let api_key_plain = "shk_manage_test_123";
    ApiKey::create(
        &pool,
        user.id,
        "ci",
        "shk_manage",
        &hash_api_key(api_key_plain),
    )
    .await
    .unwrap();

    let app = create_test_router_with_middleware(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/apikeys")
                .header("host", "localhost:3000")
                .header("authorization", format!("Bearer {}", api_key_plain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn test_apikey_can_access_project_endpoints(pool: SqlitePool) {
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

    let user = User::create(&pool, "google", "user3", "u3@example.com", "u3")
        .await
        .unwrap();
    let api_key_plain = "shk_project_test_123";
    ApiKey::create(
        &pool,
        user.id,
        "ci",
        "shk_project",
        &hash_api_key(api_key_plain),
    )
    .await
    .unwrap();

    let app = create_test_router_with_middleware(deploy_state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects")
                .header("host", "localhost:3000")
                .header("authorization", format!("Bearer {}", api_key_plain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
