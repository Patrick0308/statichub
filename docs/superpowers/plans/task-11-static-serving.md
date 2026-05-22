# Task 11: Static File Serving

## Goal

Implement HTTP static file serving with hostname-based routing, clean URLs, SPA mode, custom headers, and redirects. This makes deployed sites actually viewable in browsers.

## Files

- Create: `server/src/api/serve.rs`
- Modify: `server/src/main.rs`
- Modify: `server/src/api/mod.rs`
- Create: `server/tests/serve_tests.rs`

## Implementation Steps

### Step 1: Add mime_guess dependency

Add to `server/Cargo.toml`:

```toml
mime_guess = "2"
```

### Step 2: Create file serving handler with tests

Create: `server/tests/serve_tests.rs`

```rust
use axum::{
    body::Body,
    http::{Request, StatusCode, HeaderValue},
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
    let storage = Arc::new(FilesystemStorage::new("./test_storage".into()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

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
    assert_eq!(body, b"<h1>Hello</h1>");
}

#[sqlx::test]
async fn test_serve_nested_file(pool: SqlitePool) {
    let storage = Arc::new(FilesystemStorage::new("./test_storage".into()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

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
    let storage = Arc::new(FilesystemStorage::new("./test_storage".into()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Update project config to enable clean URLs
    sqlx::query("UPDATE projects SET config = ? WHERE id = ?")
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
    assert_eq!(body, b"<h1>About</h1>");
}

#[sqlx::test]
async fn test_spa_mode(pool: SqlitePool) {
    let storage = Arc::new(FilesystemStorage::new("./test_storage".into()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Update project config to enable SPA mode
    sqlx::query("UPDATE projects SET config = ? WHERE id = ?")
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
    assert_eq!(body, b"<h1>SPA</h1>");
}

#[sqlx::test]
async fn test_custom_headers(pool: SqlitePool) {
    let storage = Arc::new(FilesystemStorage::new("./test_storage".into()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

    // Update project config with custom headers
    sqlx::query("UPDATE projects SET config = ? WHERE id = ?")
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
    let storage = Arc::new(FilesystemStorage::new("./test_storage".into()));
    let project = Project::create_anonymous(&pool, "testproject").await.unwrap();
    let deploy = Deploy::create(&pool, project.id, "testproject/deploy-1").await.unwrap();

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
    let storage = Arc::new(FilesystemStorage::new("./test_storage".into()));

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
```

### Step 3: Run tests to verify they fail

Run: `cargo test -p statichub-server --test serve_tests`
Expected: Tests fail (serve handler not yet implemented)

### Step 4: Implement serve handler

Create: `server/src/api/serve.rs`

```rust
use crate::{
    api::DeployState,
    error::{AppError, Result},
    models::Project,
    storage::Storage,
};
use axum::{
    body::Body,
    extract::{Host, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use statichub_shared::ProjectConfig;

pub async fn serve_static_file(
    Host(hostname): Host,
    State(state): State<Arc<DeployState>>,
    request: Request,
) -> Result<Response> {
    // Extract subdomain
    let subdomain = extract_subdomain(&hostname, &state.base_url)?;

    // Find project by subdomain
    let project = Project::find_by_subdomain(&state.pool, &subdomain)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", subdomain)))?;

    // Get project config
    let config = project.get_config().unwrap_or_default();

    // Get current deploy
    let deploy_id = project.current_deploy_id.ok_or_else(|| {
        AppError::NotFound(format!("No deployment found for project: {}", subdomain))
    })?;

    let deploy = crate::models::Deploy::find_by_id(&state.pool, deploy_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Deploy not found: {}", deploy_id)))?;

    // Resolve file path
    let request_path = request.uri().path();
    let file_path = resolve_file_path(request_path, &config, &state.storage, &deploy.storage_path).await?;

    // Get file content
    let content = state
        .storage
        .get_file(&deploy.storage_path, &file_path)
        .await
        .map_err(|e| match e {
            crate::storage::StorageError::NotFound(_) => {
                AppError::NotFound(format!("File not found: {}", request_path))
            }
            _ => AppError::Storage(e.to_string()),
        })?;

    // Detect content type
    let content_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    // Build response with custom headers
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_str(&content_type).unwrap());

    // Apply custom headers from config
    if let Some(custom_headers) = &config.headers {
        for (pattern, header_map) in custom_headers {
            if request_path.starts_with(pattern) {
                for (key, value) in header_map {
                    if let Ok(header_value) = HeaderValue::from_str(value) {
                        headers.insert(
                            key.parse::<axum::http::HeaderName>().unwrap(),
                            header_value,
                        );
                    }
                }
            }
        }
    }

    Ok((StatusCode::OK, headers, content).into_response())
}

fn extract_subdomain(hostname: &str, base_url: &str) -> Result<String> {
    // Remove protocol from base_url
    let base_domain = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    // Extract subdomain
    if let Some(subdomain) = hostname.strip_suffix(&format!(".{}", base_domain)) {
        Ok(subdomain.to_string())
    } else {
        Err(AppError::BadRequest(format!(
            "Invalid hostname: {}",
            hostname
        )))
    }
}

async fn resolve_file_path(
    request_path: &str,
    config: &ProjectConfig,
    storage: &Arc<dyn Storage>,
    deploy_path: &str,
) -> Result<String> {
    let mut path = request_path.trim_start_matches('/').to_string();

    // If path is empty, try index.html
    if path.is_empty() {
        path = "index.html".to_string();
    }

    // Try exact path first
    if file_exists(storage, deploy_path, &path).await {
        return Ok(path);
    }

    // Clean URLs: try adding .html
    if config.clean_urls.unwrap_or(false) {
        let html_path = format!("{}.html", path);
        if file_exists(storage, deploy_path, &html_path).await {
            return Ok(html_path);
        }
    }

    // Directory index: try path/index.html
    let index_path = format!("{}/index.html", path);
    if file_exists(storage, deploy_path, &index_path).await {
        return Ok(index_path);
    }

    // SPA mode: fallback to index.html for non-existent paths
    if config.spa.unwrap_or(false) {
        if file_exists(storage, deploy_path, "index.html").await {
            return Ok("index.html".to_string());
        }
    }

    // File not found
    Err(AppError::NotFound(format!("File not found: {}", request_path)))
}

async fn file_exists(storage: &Arc<dyn Storage>, deploy_path: &str, file_path: &str) -> bool {
    storage.get_file(deploy_path, file_path).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_subdomain() {
        assert_eq!(
            extract_subdomain("abc123.statichub.io", "http://statichub.io").unwrap(),
            "abc123"
        );
        assert_eq!(
            extract_subdomain("test.statichub.io", "https://statichub.io").unwrap(),
            "test"
        );
    }

    #[test]
    fn test_extract_subdomain_invalid() {
        assert!(extract_subdomain("example.com", "http://statichub.io").is_err());
    }
}
```

### Step 5: Wire up serve handler

Modify `server/src/api/mod.rs`:

```rust
mod deploys;
mod serve;

pub use deploys::{create_anonymous_deploy, DeployState};
pub use serve::serve_static_file;
```

### Step 6: Add catch-all route to router

Modify `server/src/main.rs`, update the `create_router` function:

```rust
pub fn create_router(state: Arc<DeployState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/deploys/anonymous", post(api::create_anonymous_deploy))
        .fallback(get(api::serve_static_file))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
}
```

### Step 7: Run tests to verify they pass

Run: `cargo test -p statichub-server --test serve_tests`
Expected: All tests pass

### Step 8: Run all tests

Run: `cargo test`
Expected: All tests pass

### Step 9: Manual test end-to-end

Terminal 1 - Start server:
```bash
cargo run -p statichub-server
```

Terminal 2 - Deploy test site:
```bash
mkdir -p /tmp/test-serve
cat > /tmp/test-serve/index.html << 'EOF'
<!DOCTYPE html>
<html>
<head><title>Test Site</title></head>
<body>
  <h1>Hello from StaticHub!</h1>
  <a href="/about">About</a>
</body>
</html>
EOF

cat > /tmp/test-serve/about.html << 'EOF'
<!DOCTYPE html>
<html>
<head><title>About</title></head>
<body><h1>About Page</h1></body>
</html>
EOF

cargo run -p statichub -- deploy /tmp/test-serve
```

Note the subdomain from output (e.g., `abc123.statichub.io`).

Terminal 3 - Test with curl:
```bash
# Replace abc123 with actual subdomain
curl -H "Host: abc123.statichub.io" http://localhost:3000/
curl -H "Host: abc123.statichub.io" http://localhost:3000/about.html
```

Expected: HTML content returned

### Step 10: Commit

```bash
git add server/src/api/serve.rs server/src/api/mod.rs server/src/main.rs server/tests/serve_tests.rs server/Cargo.toml
git commit -m "feat: implement static file serving with clean URLs and SPA mode

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

## Success Criteria

- Hostname-based routing resolves subdomain to project
- Files served from storage with correct Content-Type
- Clean URLs work (e.g., `/about` serves `about.html`)
- SPA mode falls back to `index.html` for non-existent paths
- Custom headers applied from configuration
- Directory indexes work (e.g., `/foo/` serves `/foo/index.html`)
- 404 for truly non-existent files
- All tests pass
- Manual end-to-end deploy → serve workflow works
