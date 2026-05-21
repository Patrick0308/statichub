use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::ServiceExt;
use statichub_server::{
    create_router,
    models::{Project, Deploy},
    storage::{FilesystemStorage, Storage},
    api::DeployState,
};

#[sqlx::test]
async fn test_serve_index_html(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Set current deploy
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    // Store files
    storage.store_file("testproject/deploy-1", "index.html", b"<h1>Hello</h1>").await.unwrap();

    // Create app
    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

    // Request
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/html"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"<h1>Hello</h1>");
}

#[sqlx::test]
async fn test_serve_nested_file(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Set current deploy
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage.store_file("testproject/deploy-1", "css/style.css", b"body {}").await.unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/css/style.css")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/css"
    );
}

#[sqlx::test]
async fn test_clean_urls(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Set current deploy and update project config to enable clean URLs
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"clean_urls": true}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage.store_file("testproject/deploy-1", "about.html", b"<h1>About</h1>").await.unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

    // Request /about (without .html)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/about")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"<h1>About</h1>");
}

#[sqlx::test]
async fn test_spa_mode(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Set current deploy and update project config to enable SPA mode
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"spa": true}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage.store_file("testproject/deploy-1", "index.html", b"<h1>SPA</h1>").await.unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

    // Request non-existent path
    let response = app
        .oneshot(
            Request::builder()
                .uri("/app/dashboard/settings")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should serve index.html
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"<h1>SPA</h1>");
}

#[sqlx::test]
async fn test_custom_headers(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Set current deploy and update project config with custom headers
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"headers": {"/": {"X-Custom": "value"}}}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage.store_file("testproject/deploy-1", "index.html", b"<h1>Hello</h1>").await.unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("x-custom").unwrap(),
        "value"
    );
}

#[sqlx::test]
async fn test_not_found(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

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
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nonexistent.html")
                .header("Host", "testproject.statichub.io")
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
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

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
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Set current deploy
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    // Store directory index file
    storage.store_file("testproject/deploy-1", "subdir/index.html", b"<h1>Subdir</h1>").await.unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

    // Request /subdir/ (with trailing slash)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/subdir/")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"<h1>Subdir</h1>");
}

#[sqlx::test]
async fn test_redirect_exact_path(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Set current deploy and update project config with redirects
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"redirects": [{"from": "/old", "to": "/new", "status": 301}]}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage.store_file("testproject/deploy-1", "index.html", b"<h1>Home</h1>").await.unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

    // Request /old which should redirect to /new
    let response = app
        .oneshot(
            Request::builder()
                .uri("/old")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get("location").unwrap(),
        "/new"
    );
}

#[sqlx::test]
async fn test_redirect_custom_status(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Set current deploy and update project config with 302 redirect
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"redirects": [{"from": "/temp", "to": "/target", "status": 302}]}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage.store_file("testproject/deploy-1", "index.html", b"<h1>Home</h1>").await.unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state);

    // Request /temp which should redirect to /target with 302
    let response = app
        .oneshot(
            Request::builder()
                .uri("/temp")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(
        response.headers().get("location").unwrap(),
        "/target"
    );
}

#[sqlx::test]
async fn test_redirect_not_matching_similar_paths(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Set current deploy and update project config with redirect rule
    sqlx::query("UPDATE projects SET current_deploy_id = ?, config = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(r#"{"redirects": [{"from": "/old", "to": "/new", "status": 301}]}"#)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    // Store files that should NOT trigger redirect
    storage.store_file("testproject/deploy-1", "old-page.html", b"<h1>Old Page</h1>").await.unwrap();
    storage.store_file("testproject/deploy-1", "index.html", b"<h1>Home</h1>").await.unwrap();

    let state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
        base_url: "http://statichub.io".to_string(),
    });
    let app = create_router(state.clone());

    // Request /old-page should NOT redirect (doesn't match /old exactly or with trailing /)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/old-page")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should serve the file, not redirect
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Request /old/ SHOULD redirect (prefix match with /)
    let app2 = create_router(state.clone());
    let response2 = app2
        .oneshot(
            Request::builder()
                .uri("/old/")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response2.headers().get("location").unwrap(),
        "/new"
    );

    // Request /old/page SHOULD redirect (prefix match with /)
    let app3 = create_router(state);
    let response3 = app3
        .oneshot(
            Request::builder()
                .uri("/old/page")
                .header("Host", "testproject.statichub.io")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response3.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response3.headers().get("location").unwrap(),
        "/new"
    );
}
