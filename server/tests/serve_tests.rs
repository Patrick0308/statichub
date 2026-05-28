use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
};
use sqlx::SqlitePool;
use statichub_server::{
    api::{AuthState, DeployState},
    config::ServerConfig,
    create_router,
    models::{Deploy, Project},
    storage::{FilesystemStorage, Storage},
};
use std::sync::Arc;
use tower::ServiceExt;

fn create_test_auth_state(pool: SqlitePool) -> Arc<AuthState> {
    Arc::new(
        AuthState::new(
            pool,
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            "http://localhost:3000/auth/callback/google".to_string(),
            "test_jwt_secret".to_string(),
        )
        .unwrap(),
    )
}

fn create_test_router_with_middleware(
    deploy_state: Arc<DeployState>,
    auth_state: Arc<AuthState>,
) -> axum::Router {
    let config = ServerConfig {
        port: 3000,
        allowed_domains: vec![
            "localhost".to_string(),
            "statichub.io".to_string(),
            "*.statichub.io".to_string(),
        ],
    };
    create_router(deploy_state, statichub_server::config::AuthMode::Enabled, Some(auth_state)).layer(middleware::from_fn_with_state(
        config,
        statichub_server::middleware::host_validation_middleware,
    ))
}

#[sqlx::test]
async fn test_serve_index_html(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    // Store files
    storage
        .store_file(&storage_path, "index.html", b"<h1>Hello</h1>")
        .await
        .unwrap();

    // Create app
    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    // Request
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    if status != StatusCode::OK {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        eprintln!(
            "Error response status: {}, body: {}",
            status,
            String::from_utf8_lossy(&body)
        );
        panic!("Expected 200, got {}", status);
    }
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.headers().get("content-type").unwrap(), "text/html");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"<h1>Hello</h1>");
}

#[sqlx::test]
async fn test_serve_nested_file(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage
        .store_file(&storage_path, "css/style.css", b"body {}")
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/css/style.css")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("content-type").unwrap(), "text/css");
}

#[sqlx::test]
async fn test_clean_urls(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy and update project config to enable clean URLs
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"clean_urls": true}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage
        .store_file(&storage_path, "about.html", b"<h1>About</h1>")
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    // Request /about (without .html)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/about")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"<h1>About</h1>");
}

#[sqlx::test]
async fn test_spa_mode(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy and update project config to enable SPA mode
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"spa": true}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage
        .store_file(&storage_path, "index.html", b"<h1>SPA</h1>")
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    // Request non-existent path
    let response = app
        .oneshot(
            Request::builder()
                .uri("/app/dashboard/settings")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should serve index.html
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"<h1>SPA</h1>");
}

#[sqlx::test]
async fn test_custom_headers(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy and update project config with custom headers
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"headers": {"/": {"X-Custom": "value"}}}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage
        .store_file(&storage_path, "index.html", b"<h1>Hello</h1>")
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("x-custom").unwrap(), "value");
}

#[sqlx::test]
async fn test_not_found(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nonexistent.html")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_subdomain_not_found(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("Host", "nonexistent.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_directory_index(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    // Store directory index file
    storage
        .store_file(&storage_path, "subdir/index.html", b"<h1>Subdir</h1>")
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    // Request /subdir/ (with trailing slash)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/subdir/")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"<h1>Subdir</h1>");
}

#[sqlx::test]
async fn test_redirect_exact_path(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy and update project config with redirects
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"redirects": [{"from": "/old", "to": "/new", "status": 301}]}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage
        .store_file(&storage_path, "index.html", b"<h1>Home</h1>")
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    // Request /old which should redirect to /new
    let response = app
        .oneshot(
            Request::builder()
                .uri("/old")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(response.headers().get("location").unwrap(), "/new");
}

#[sqlx::test]
async fn test_redirect_custom_status(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy and update project config with 302 redirect
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"redirects": [{"from": "/temp", "to": "/target", "status": 302}]}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage
        .store_file(&storage_path, "index.html", b"<h1>Home</h1>")
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    // Request /temp which should redirect to /target with 302
    let response = app
        .oneshot(
            Request::builder()
                .uri("/temp")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(response.headers().get("location").unwrap(), "/target");
}

#[sqlx::test]
async fn test_redirect_not_matching_similar_paths(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    // Set current deploy and update project config with redirect rule
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"redirects": [{"from": "/old", "to": "/new", "status": 301}]}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    // Store files that should NOT trigger redirect
    storage
        .store_file(&storage_path, "old-page.html", b"<h1>Old Page</h1>")
        .await
        .unwrap();
    storage
        .store_file(&storage_path, "index.html", b"<h1>Home</h1>")
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state.clone(), auth_state);

    // Request /old-page should NOT redirect (doesn't match /old exactly or with trailing /)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/old-page")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should serve the file, not redirect
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Request /old/ SHOULD redirect (prefix match with /)
    let auth_state2 = create_test_auth_state(pool.clone());
    let app2 = create_test_router_with_middleware(state.clone(), auth_state2);
    let response2 = app2
        .oneshot(
            Request::builder()
                .uri("/old/")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(response2.headers().get("location").unwrap(), "/new");

    // Request /old/page SHOULD redirect (prefix match with /)
    let auth_state3 = create_test_auth_state(pool.clone());
    let app3 = create_test_router_with_middleware(state, auth_state3);
    let response3 = app3
        .oneshot(
            Request::builder()
                .uri("/old/page")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response3.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(response3.headers().get("location").unwrap(), "/new");
}

#[sqlx::test]
async fn test_base_domain_root_serves_homepage(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage,
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("Host", "statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("AI-generated content"));
    assert!(html.contains("auto-published."));
    assert!(html.contains("id=\"hero\""));
    assert!(html.contains("/__home/home.css"));
    assert!(html.contains("/__home/home.js"));
    assert!(html.contains("id=\"quickstart\""));
    assert!(html.contains("id=\"install\""));
}

#[sqlx::test]
async fn test_base_domain_root_with_port_serves_homepage(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage,
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("Host", "statichub.io:3000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
}

#[sqlx::test]
async fn test_base_domain_home_asset_serves_css(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage,
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/__home/home.css")
                .header("Host", "statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/css; charset=utf-8"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let css = String::from_utf8(body.to_vec()).unwrap();
    assert!(!css.is_empty());
    assert!(css.contains("--accent: #2f80ed;"));
    assert!(css.contains(".hero"));
    assert!(css.contains(".tab-btn.is-active"));
}

#[sqlx::test]
async fn test_base_domain_unknown_home_asset_returns_404(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage,
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/__home/unknown.js")
                .header("Host", "statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_subdomain_root_still_serves_project_index(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path)
        .await
        .unwrap();

    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage
        .store_file(&storage_path, "index.html", b"<h1>Project Home</h1>")
        .await
        .unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });
    let auth_state = create_test_auth_state(pool.clone());
    let app = create_test_router_with_middleware(state, auth_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("Host", &format!("{}.statichub.io", project.subdomain))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"<h1>Project Home</h1>");
}
