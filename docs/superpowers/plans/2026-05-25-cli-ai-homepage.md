# Statichub CLI AI Homepage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a new `/` homepage on statichub server that markets AI-first CLI/Skill usage while preserving existing subdomain static-site serving behavior.

**Architecture:** Keep the existing Axum router and fallback flow, but add a base-domain-aware fast path in serving logic. Requests to bare domain (`statichub.io` style host) render a built-in homepage and dedicated static assets; subdomain traffic continues to use deployed project files. Homepage assets are versioned under a reserved prefix to avoid conflicts with user content.

**Tech Stack:** Rust, Axum, tower middleware, embedded/static file serving, SQLx integration tests.

---

## File Structure

- Modify: `server/src/api/serve.rs`
  - Add base-domain homepage branch before project lookup.
  - Add helper to serve built-in homepage and reserved asset paths.
- Modify: `server/src/api/mod.rs`
  - Export any new helpers only if needed by tests.
- Create: `server/src/web/homepage.rs`
  - Centralize homepage HTML/CSS/JS responses and content-types.
- Create: `server/src/web/mod.rs`
  - Module wiring for `web::homepage`.
- Modify: `server/src/lib.rs`
  - Register `web` module if required by compiler structure.
- Create: `server/static/home/index.html`
  - Quickstart-first, no-hero homepage markup.
- Create: `server/static/home/home.css`
  - Light, technical visual style and responsive rules.
- Create: `server/static/home/home.js`
  - OS tab switch, Skill/CLI path switch, copy-to-clipboard interactions.
- Modify: `server/tests/serve_tests.rs`
  - Add coverage for base-domain homepage and reserved asset paths.
  - Add regression test ensuring subdomain `/` still serves deploy `index.html`.

### Task 1: Add failing tests for base-domain homepage behavior

**Files:**
- Modify: `server/tests/serve_tests.rs`
- Test: `server/tests/serve_tests.rs`

- [ ] **Step 1: Write failing tests for homepage and asset route**

```rust
#[sqlx::test]
async fn test_base_domain_root_serves_homepage(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let state = Arc::new(DeployState { pool: pool.clone(), storage });
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
    assert_eq!(response.headers().get("content-type").unwrap(), "text/html; charset=utf-8");
}

#[sqlx::test]
async fn test_base_domain_home_asset_serves_css(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let state = Arc::new(DeployState { pool: pool.clone(), storage });
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
    assert_eq!(response.headers().get("content-type").unwrap(), "text/css; charset=utf-8");
}
```

- [ ] **Step 2: Run targeted tests to confirm failure**

Run: `cargo test -p statichub-server test_base_domain_root_serves_homepage test_base_domain_home_asset_serves_css -- --nocapture`
Expected: FAIL with 400/404 because current flow requires project subdomain lookup.

- [ ] **Step 3: Add regression test for subdomain behavior**

```rust
#[sqlx::test]
async fn test_subdomain_root_still_serves_project_index(pool: SqlitePool) {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(FilesystemStorage::new(temp_dir.path().to_path_buf()));
    let project = Project::create_anonymous(&pool, None).await.unwrap();
    let storage_path = format!("{}/deploy-1", project.subdomain);
    let deploy = Deploy::create(&pool, project.id, &storage_path).await.unwrap();

    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .unwrap();

    storage.store_file(&storage_path, "index.html", b"<h1>Project Home</h1>").await.unwrap();

    let state = Arc::new(DeployState { pool: pool.clone(), storage: storage.clone() });
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
}
```

- [ ] **Step 4: Run regression test and keep all failures documented**

Run: `cargo test -p statichub-server test_subdomain_root_still_serves_project_index -- --nocapture`
Expected: PASS on current code.

- [ ] **Step 5: Commit test-only changes**

```bash
git add server/tests/serve_tests.rs
git commit -m "test: add failing coverage for base-domain homepage routing"
```

### Task 2: Implement base-domain homepage routing and embedded assets

**Files:**
- Create: `server/src/web/mod.rs`
- Create: `server/src/web/homepage.rs`
- Modify: `server/src/lib.rs`
- Modify: `server/src/api/serve.rs`
- Create: `server/static/home/index.html`
- Create: `server/static/home/home.css`
- Create: `server/static/home/home.js`

- [ ] **Step 1: Add minimal web module for homepage assets**

```rust
// server/src/web/mod.rs
pub mod homepage;
```

```rust
// server/src/web/homepage.rs
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;

const HOME_HTML: &str = include_str!("../../static/home/index.html");
const HOME_CSS: &str = include_str!("../../static/home/home.css");
const HOME_JS: &str = include_str!("../../static/home/home.js");

pub fn serve_home(path: &str) -> Option<Response> {
    let (content_type, body) = match path {
        "/" => ("text/html; charset=utf-8", HOME_HTML),
        "/__home/home.css" => ("text/css; charset=utf-8", HOME_CSS),
        "/__home/home.js" => ("application/javascript; charset=utf-8", HOME_JS),
        _ => return None,
    };

    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static(content_type));
    Some((StatusCode::OK, headers, body).into_response())
}
```

- [ ] **Step 2: Wire module into crate**

```rust
// server/src/lib.rs
pub mod web;
```

- [ ] **Step 3: Branch in `serve_static_file` for base domain requests**

```rust
// server/src/api/serve.rs (near start of serve_static_file)
let request_host = extensions
    .get::<RequestHost>()
    .ok_or(AppError::MissingHost)?;

let base_domain = &request_host.base_domain;
let request_path = request.uri().path();

if hostname == *base_domain || hostname == format!("www.{}", base_domain) {
    if let Some(response) = crate::web::homepage::serve_home(request_path) {
        return Ok(response);
    }
    return Err(AppError::NotFound(format!("Homepage asset not found: {}", request_path)));
}
```

- [ ] **Step 4: Add initial homepage static files matching approved design**

```html
<!-- server/static/home/index.html -->
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Statichub - From AI prompt to production URL</title>
    <link rel="stylesheet" href="/__home/home.css" />
  </head>
  <body>
    <header class="top-nav">...</header>
    <main>
      <section id="intro">...</section>
      <section id="quickstart">...</section>
      <section id="install">...</section>
      <section id="ai-builders">...</section>
      <section id="capabilities">...</section>
      <section id="faq">...</section>
    </main>
    <script src="/__home/home.js" defer></script>
  </body>
</html>
```

```css
/* server/static/home/home.css */
:root { --bg: #f6fbff; --ink: #0f172a; --accent: #00b8ff; }
/* responsive, quickstart-first, no hero */
```

```js
// server/static/home/home.js
// tab switching + copy button behavior
```

- [ ] **Step 5: Run focused tests**

Run: `cargo test -p statichub-server test_base_domain_root_serves_homepage test_base_domain_home_asset_serves_css test_subdomain_root_still_serves_project_index -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Commit implementation**

```bash
git add server/src/lib.rs server/src/api/serve.rs server/src/web/mod.rs server/src/web/homepage.rs server/static/home/index.html server/static/home/home.css server/static/home/home.js
git commit -m "feat: serve AI-focused statichub homepage on base domain"
```

### Task 3: Add interaction coverage and content assertions

**Files:**
- Modify: `server/tests/serve_tests.rs`

- [ ] **Step 1: Add assertions for key homepage content markers**

```rust
let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
let html = String::from_utf8(body.to_vec()).unwrap();
assert!(html.contains("From AI prompt to production URL."));
assert!(html.contains("Skill-first"));
assert!(html.contains("CLI-first"));
assert!(html.contains("Install CLI"));
```

- [ ] **Step 2: Add negative test for missing reserved asset**

```rust
#[sqlx::test]
async fn test_base_domain_unknown_home_asset_returns_404(pool: SqlitePool) {
    // request /__home/unknown.js on statichub.io host
    // expect StatusCode::NOT_FOUND
}
```

- [ ] **Step 3: Run full serve test suite**

Run: `cargo test -p statichub-server --test serve_tests -- --nocapture`
Expected: PASS with existing serve behaviors unchanged.

- [ ] **Step 4: Commit expanded tests**

```bash
git add server/tests/serve_tests.rs
git commit -m "test: validate homepage content and reserved asset errors"
```

### Task 4: Final verification and documentation update

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Document homepage behavior for base domain vs subdomain**

```markdown
## Server Routing Notes

- Base domain (`statichub.io`) serves the built-in product homepage.
- Project subdomains (`<project>.statichub.io`) continue to serve deployed static files.
- Built-in homepage assets are reserved under `/__home/*`.
```

- [ ] **Step 2: Run project-level checks**

Run: `cargo test -p statichub-server`
Expected: PASS.

Run: `cargo fmt --all --check`
Expected: PASS.

- [ ] **Step 3: Commit docs + final polish**

```bash
git add README.md
git commit -m "docs: document homepage routing and reserved home assets"
```

- [ ] **Step 4: Prepare handoff summary**

```text
- Added base-domain homepage path and reserved assets.
- Preserved existing subdomain static serving.
- Added integration tests for route split and homepage content markers.
```

## Self-Review Checklist

- Spec coverage: homepage at `/`, no-hero quickstart-first structure, Skill/CLI dual path, install tabs, fallback behavior, and mobile-aware implementation are covered by Tasks 2-3.
- Placeholder scan: no TBD/TODO placeholders remain; every task includes concrete files and commands.
- Type consistency: `serve_home`, reserved `/__home/*` prefix, and base-domain branch naming are consistent across tasks.

