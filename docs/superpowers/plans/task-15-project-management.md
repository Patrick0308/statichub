# Task 15: Project Management Commands

## Goal

Implement project management commands that allow users to list their projects, view project details with deploy history, and rollback to previous versions.

## Files

- Create: `server/src/api/management.rs`
- Modify: `server/src/api/mod.rs`
- Modify: `server/src/lib.rs`
- Modify: `cli/src/main.rs`
- Modify: `cli/src/client.rs`
- Create: `server/tests/management_tests.rs`

## Implementation Steps

### Step 1: Create management API endpoints

Create: `server/src/api/management.rs`

```rust
use crate::{
    api::DeployState,
    error::{AppError, Result},
    middleware::AuthUser,
    models::{Deploy, Project},
};
use axum::{
    extract::{Extension, Path, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ProjectListItem {
    pub id: i64,
    pub name: String,
    pub subdomain: String,
    pub url: String,
    pub current_version: Option<i64>,
    pub last_deployed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectDetail {
    pub id: i64,
    pub name: String,
    pub subdomain: String,
    pub url: String,
    pub current_version: Option<i64>,
    pub deploys: Vec<DeployInfo>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct DeployInfo {
    pub version: i64,
    pub deploy_id: i64,
    pub status: String,
    pub file_count: i64,
    pub total_size_bytes: i64,
    pub deployed_at: String,
    pub is_current: bool,
}

#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    pub version: i64,
}

pub async fn list_projects(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<ProjectListItem>>> {
    let projects = Project::list_by_owner(&state.pool, auth_user.user_id).await?;

    let items: Vec<ProjectListItem> = projects
        .into_iter()
        .map(|p| {
            let base_domain = state
                .base_url
                .replace("http://", "")
                .replace("https://", "");

            let current_version = if let Some(deploy_id) = p.current_deploy_id {
                // Get version from current deploy
                // For now, we'll fetch it. Consider caching or denormalizing.
                None // TODO: Query deploy.version
            } else {
                None
            };

            ProjectListItem {
                id: p.id,
                name: p.name.clone(),
                subdomain: format!("{}.{}", p.subdomain, base_domain),
                url: format!("https://{}.{}", p.subdomain, base_domain),
                current_version,
                last_deployed_at: p.last_deployed_at.map(|dt| dt.to_string()),
                created_at: p.created_at.to_string(),
            }
        })
        .collect();

    Ok(Json(items))
}

pub async fn get_project_info(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(project_name): Path<String>,
) -> Result<Json<ProjectDetail>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Get all deploys for this project
    let deploys = Deploy::list_by_project(&state.pool, project.id).await?;

    let deploy_infos: Vec<DeployInfo> = deploys
        .into_iter()
        .map(|d| DeployInfo {
            version: d.version,
            deploy_id: d.id,
            status: d.status,
            file_count: d.file_count,
            total_size_bytes: d.total_size_bytes,
            deployed_at: d.deployed_at.to_string(),
            is_current: Some(d.id) == project.current_deploy_id,
        })
        .collect();

    let base_domain = state
        .base_url
        .replace("http://", "")
        .replace("https://", "");

    Ok(Json(ProjectDetail {
        id: project.id,
        name: project.name.clone(),
        subdomain: format!("{}.{}", project.subdomain, base_domain),
        url: format!("https://{}.{}", project.subdomain, base_domain),
        current_version: deploy_infos
            .iter()
            .find(|d| d.is_current)
            .map(|d| d.version),
        deploys: deploy_infos,
        created_at: project.created_at.to_string(),
    }))
}

pub async fn rollback_project(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(project_name): Path<String>,
    Json(payload): Json<RollbackRequest>,
) -> Result<Json<ProjectDetail>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Find the target deploy by version
    let target_deploy = Deploy::find_by_version(&state.pool, project.id, payload.version)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Version {} not found for project {}",
                payload.version, project_name
            ))
        })?;

    // Update project's current_deploy_id
    sqlx::query(
        "UPDATE projects SET current_deploy_id = ?, last_deployed_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(target_deploy.id)
    .bind(project.id)
    .execute(&state.pool)
    .await?;

    // Return updated project info
    get_project_info(State(state), Extension(auth_user), Path(project_name)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deploy_info_serialization() {
        let info = DeployInfo {
            version: 1,
            deploy_id: 123,
            status: "active".to_string(),
            file_count: 10,
            total_size_bytes: 1024,
            deployed_at: "2024-01-01T00:00:00".to_string(),
            is_current: true,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"is_current\":true"));
    }
}
```

### Step 2: Export management module

Modify `server/src/api/mod.rs`:

```rust
mod deploys;
mod serve;
mod auth;
mod projects;
mod management;

pub use deploys::{create_anonymous_deploy, DeployState};
pub use serve::serve_static_file;
pub use auth::{login_google, callback_google, auth_status, AuthState};
pub use projects::create_project_deploy;
pub use management::{list_projects, get_project_info, rollback_project};
```

### Step 3: Wire up management routes

Modify `server/src/lib.rs`, in `create_router`:

```rust
let authenticated_routes = Router::new()
    .route("/api/projects/:name/deploys", post(api::create_project_deploy))
    .route("/api/projects", get(api::list_projects))
    .route("/api/projects/:name", get(api::get_project_info))
    .route("/api/projects/:name/rollback", post(api::rollback_project))
    .layer(from_fn_with_state(auth_state.clone(), auth_middleware))
    .with_state(deploy_state.clone());
```

### Step 4: Add CLI commands

Modify `cli/src/main.rs`:

Add to Commands enum:

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// List your projects
    List,

    /// Show project details and deploy history
    Info {
        /// Project name
        project: String,
    },

    /// Rollback project to a previous version
    Rollback {
        /// Project name
        project: String,
        /// Version to rollback to
        version: i64,
    },
}
```

Add command handlers:

```rust
Commands::List => {
    let credentials = auth::load_credentials()
        .context("Not logged in. Run 'statichub login' first.")?;

    let server_url = std::env::var("STATICHUB_SERVER")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let client = client::Client::new(server_url);
    let projects = client.list_projects(&credentials.access_token).await?;

    if projects.is_empty() {
        println!("📭 No projects yet");
        println!("   Deploy with a name: statichub deploy --name my-app");
    } else {
        println!("📋 Your projects:\n");
        for project in projects {
            println!("  {} - {}", project.name, project.url);
            if let Some(version) = project.current_version {
                println!("    Version: {}", version);
            }
            if let Some(deployed_at) = project.last_deployed_at {
                println!("    Last deployed: {}", deployed_at);
            }
            println!();
        }
    }
}

Commands::Info { project } => {
    let credentials = auth::load_credentials()
        .context("Not logged in. Run 'statichub login' first.")?;

    let server_url = std::env::var("STATICHUB_SERVER")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let client = client::Client::new(server_url);
    let info = client.get_project_info(project, &credentials.access_token).await?;

    println!("📦 Project: {}", info.name);
    println!("   URL: {}", info.url);
    println!("   Created: {}", info.created_at);
    if let Some(version) = info.current_version {
        println!("   Current version: {}", version);
    }
    println!("\n📜 Deploy history:");

    for deploy in info.deploys {
        let current_marker = if deploy.is_current { " (current)" } else { "" };
        println!(
            "  v{} - {} files, {} bytes, {}{}",
            deploy.version,
            deploy.file_count,
            deploy.total_size_bytes,
            deploy.deployed_at,
            current_marker
        );
    }
}

Commands::Rollback { project, version } => {
    let credentials = auth::load_credentials()
        .context("Not logged in. Run 'statichub login' first.")?;

    let server_url = std::env::var("STATICHUB_SERVER")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    println!("🔄 Rolling back {} to version {}...", project, version);

    let client = client::Client::new(server_url);
    let info = client
        .rollback_project(project, *version, &credentials.access_token)
        .await?;

    println!("✅ Rollback successful!");
    println!("   {} is now at version {}", info.name, info.current_version.unwrap_or(0));
    println!("   URL: {}", info.url);
}
```

### Step 5: Add client methods

Modify `cli/src/client.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ProjectListItem {
    pub name: String,
    pub url: String,
    pub current_version: Option<i64>,
    pub last_deployed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectDetail {
    pub name: String,
    pub url: String,
    pub current_version: Option<i64>,
    pub deploys: Vec<DeployInfo>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct DeployInfo {
    pub version: i64,
    pub file_count: i64,
    pub total_size_bytes: i64,
    pub deployed_at: String,
    pub is_current: bool,
}

impl Client {
    // ... existing methods ...

    pub async fn list_projects(&self, token: &str) -> Result<Vec<ProjectListItem>> {
        let url = format!("{}/api/projects", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to list projects")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to list projects: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse projects list")
    }

    pub async fn get_project_info(&self, project: &str, token: &str) -> Result<ProjectDetail> {
        let url = format!("{}/api/projects/{}", self.base_url, project);

        let response = self
            .client
            .get(&url)
            .header("authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to get project info")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to get project info: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse project info")
    }

    pub async fn rollback_project(
        &self,
        project: &str,
        version: i64,
        token: &str,
    ) -> Result<ProjectDetail> {
        let url = format!("{}/api/projects/{}/rollback", self.base_url, project);

        let response = self
            .client
            .post(&url)
            .header("authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({ "version": version }))
            .send()
            .await
            .context("Failed to rollback project")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Rollback failed: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse rollback response")
    }
}
```

### Step 6: Write integration tests

Create: `server/tests/management_tests.rs`

```rust
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
    let deploy = Deploy::create(&pool, project.id, "myapp/deploy-1")
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
    let deploy1 = Deploy::create(&pool, project.id, "myapp/deploy-1")
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
```

### Step 7: Run tests

Run: `cargo test`
Expected: All tests pass

### Step 8: Manual end-to-end test

Terminal 1 - Start server:
```bash
cargo run -p statichub-server
```

Terminal 2 - Test commands:
```bash
# Login
cargo run -p statichub -- login

# Deploy a project
mkdir -p /tmp/myapp
echo "<h1>Version 1</h1>" > /tmp/myapp/index.html
cargo run -p statichub -- deploy /tmp/myapp --name myapp

# List projects
cargo run -p statichub -- list

# Get project info
cargo run -p statichub -- info myapp

# Deploy version 2
echo "<h1>Version 2</h1>" > /tmp/myapp/index.html
cargo run -p statichub -- deploy /tmp/myapp --name myapp

# Check info shows version 2
cargo run -p statichub -- info myapp

# Rollback to version 1
cargo run -p statichub -- rollback myapp 1

# Verify rollback
cargo run -p statichub -- info myapp
```

### Step 9: Commit

```bash
git add server/src/api/management.rs server/src/api/mod.rs server/src/lib.rs cli/src/main.rs cli/src/client.rs server/tests/management_tests.rs
git commit -m "feat: implement project management commands (list, info, rollback)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

## Success Criteria

- GET /api/projects lists user's owned projects
- GET /api/projects/:name returns project details with deploy history
- POST /api/projects/:name/rollback switches to previous version
- Ownership validation prevents access to other users' projects
- CLI `list` command displays projects nicely
- CLI `info` command shows deploy history
- CLI `rollback` command updates current version
- All commands require authentication
- All tests pass
- Clear error messages for missing projects or versions
- Rollback is instantaneous (just updates pointer, no file operations)
