# Task 16: Custom Domain Support

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to map custom domains to their projects with file-based verification.

**Architecture:** Add domains table with verification workflow. Users add domains (pending status), verify ownership by uploading a verification file, then StaticHub serves content for verified domains. Static serving checks custom domains before subdomain lookup.

**Tech Stack:** SQLite (domains table), file-based verification (statichub-verify.txt)

---

## File Structure

**Database:**
- `server/migrations/002_domains.sql` - domains table schema

**Models:**
- `server/src/models/domain.rs` - Domain model with CRUD operations
- `server/src/models/mod.rs` - export Domain

**API:**
- `server/src/api/domains.rs` - domain management endpoints
- `server/src/api/mod.rs` - export domain endpoints
- `server/src/lib.rs` - wire up domain routes
- `server/src/api/serve.rs` - update to handle custom domains

**CLI:**
- `cli/src/main.rs` - add domain commands
- `cli/src/client.rs` - add domain API methods

**Tests:**
- `server/tests/domain_tests.rs` - integration tests

---

## Task 1: Database Schema for Domains

**Files:**
- Create: `server/migrations/002_domains.sql`

- [ ] **Step 1: Write the migration**

Create `server/migrations/002_domains.sql`:

```sql
-- Domains table
CREATE TABLE domains (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    domain TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending_verification', -- 'pending_verification', 'verified', 'failed'
    verification_token TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    verified_at TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX idx_domains_project_id ON domains(project_id);
CREATE INDEX idx_domains_domain ON domains(domain);
CREATE INDEX idx_domains_status ON domains(status);
```

- [ ] **Step 2: Run the migration**

Run: `sqlx migrate run --database-url sqlite:statichub.db`
Expected: Migration 002 applied successfully

- [ ] **Step 3: Commit**

```bash
git add server/migrations/002_domains.sql
git commit -m "feat: add domains table schema

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 2: Domain Model

**Files:**
- Create: `server/src/models/domain.rs`
- Modify: `server/src/models/mod.rs`

- [ ] **Step 1: Write the Domain struct**

Create `server/src/models/domain.rs`:

```rust
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Domain {
    pub id: i64,
    pub project_id: i64,
    pub domain: String,
    pub status: String,
    pub verification_token: String,
    pub created_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

impl Domain {
    pub fn new(project_id: i64, domain: String, verification_token: String) -> Self {
        Self {
            id: 0,
            project_id,
            domain,
            status: "pending_verification".to_string(),
            verification_token,
            created_at: Utc::now(),
            verified_at: None,
        }
    }
}
```

- [ ] **Step 2: Add create method**

Add to `domain.rs`:

```rust
impl Domain {
    // ... existing new method ...

    pub async fn create(
        pool: &SqlitePool,
        project_id: i64,
        domain: &str,
        verification_token: &str,
    ) -> Result<Self, sqlx::Error> {
        let domain = sqlx::query_as::<_, Domain>(
            "INSERT INTO domains (project_id, domain, verification_token, status)
             VALUES (?, ?, ?, 'pending_verification')
             RETURNING *"
        )
        .bind(project_id)
        .bind(domain)
        .bind(verification_token)
        .fetch_one(pool)
        .await?;

        Ok(domain)
    }
}
```

- [ ] **Step 3: Add find methods**

Add to `domain.rs`:

```rust
impl Domain {
    // ... existing methods ...

    pub async fn find_by_domain(
        pool: &SqlitePool,
        domain: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        let domain = sqlx::query_as::<_, Domain>(
            "SELECT * FROM domains WHERE domain = ?"
        )
        .bind(domain)
        .fetch_optional(pool)
        .await?;

        Ok(domain)
    }

    pub async fn list_by_project(
        pool: &SqlitePool,
        project_id: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let domains = sqlx::query_as::<_, Domain>(
            "SELECT * FROM domains WHERE project_id = ? ORDER BY created_at DESC"
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;

        Ok(domains)
    }
}
```

- [ ] **Step 4: Add verify and delete methods**

Add to `domain.rs`:

```rust
impl Domain {
    // ... existing methods ...

    pub async fn mark_verified(
        pool: &SqlitePool,
        domain_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE domains SET status = 'verified', verified_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(domain_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn mark_failed(
        pool: &SqlitePool,
        domain_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE domains SET status = 'failed' WHERE id = ?"
        )
        .bind(domain_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn delete(
        pool: &SqlitePool,
        project_id: i64,
        domain: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM domains WHERE project_id = ? AND domain = ?"
        )
        .bind(project_id)
        .bind(domain)
        .execute(pool)
        .await?;

        Ok(())
    }
}
```

- [ ] **Step 5: Add tests**

Add to `domain.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_new() {
        let domain = Domain::new(1, "example.com".to_string(), "token123".to_string());
        assert_eq!(domain.project_id, 1);
        assert_eq!(domain.domain, "example.com");
        assert_eq!(domain.status, "pending_verification");
        assert_eq!(domain.verification_token, "token123");
    }
}
```

- [ ] **Step 6: Export Domain**

Modify `server/src/models/mod.rs`:

```rust
mod user;
mod project;
mod deploy;
mod domain;

pub use user::User;
pub use project::Project;
pub use deploy::Deploy;
pub use domain::Domain;
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p statichub-server`
Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add server/src/models/domain.rs server/src/models/mod.rs
git commit -m "feat: add Domain model with CRUD operations

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 3: Domain API Endpoints

**Files:**
- Create: `server/src/api/domains.rs`
- Modify: `server/src/api/mod.rs`
- Modify: `server/src/lib.rs`

- [ ] **Step 1: Create domains API file**

Create `server/src/api/domains.rs`:

```rust
use crate::{
    api::DeployState,
    error::{AppError, Result},
    middleware::AuthUser,
    models::{Deploy, Domain, Project},
    storage::Storage,
};
use axum::{
    extract::{Extension, Path, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct AddDomainRequest {
    pub domain: String,
}

#[derive(Debug, Serialize)]
pub struct DomainResponse {
    pub id: i64,
    pub domain: String,
    pub status: String,
    pub verification_token: String,
    pub verification_instructions: String,
    pub created_at: String,
    pub verified_at: Option<String>,
}

impl From<Domain> for DomainResponse {
    fn from(d: Domain) -> Self {
        let instructions = format!(
            "Upload a file named 'statichub-verify.txt' to your domain root containing: {}",
            d.verification_token
        );

        Self {
            id: d.id,
            domain: d.domain,
            status: d.status,
            verification_token: d.verification_token,
            verification_instructions: instructions,
            created_at: d.created_at.to_string(),
            verified_at: d.verified_at.map(|dt| dt.to_string()),
        }
    }
}
```

- [ ] **Step 2: Implement add_domain endpoint**

Add to `domains.rs`:

```rust
pub async fn add_domain(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(project_name): Path<String>,
    Json(payload): Json<AddDomainRequest>,
) -> Result<Json<DomainResponse>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Validate domain format
    let domain = payload.domain.trim().to_lowercase();
    if domain.is_empty() || !domain.contains('.') {
        return Err(AppError::BadRequest("Invalid domain format".to_string()));
    }

    // Check if domain already exists
    if let Some(_) = Domain::find_by_domain(&state.pool, &domain).await? {
        return Err(AppError::Conflict("Domain already in use".to_string()));
    }

    // Generate verification token
    let verification_token = uuid::Uuid::new_v4().to_string();

    // Create domain
    let domain = Domain::create(&state.pool, project.id, &domain, &verification_token).await?;

    Ok(Json(domain.into()))
}
```

- [ ] **Step 3: Implement list_domains endpoint**

Add to `domains.rs`:

```rust
pub async fn list_domains(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(project_name): Path<String>,
) -> Result<Json<Vec<DomainResponse>>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Get domains
    let domains = Domain::list_by_project(&state.pool, project.id).await?;

    Ok(Json(domains.into_iter().map(|d| d.into()).collect()))
}
```

- [ ] **Step 4: Implement verify_domain endpoint**

Add to `domains.rs`:

```rust
pub async fn verify_domain(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((project_name, domain_name)): Path<(String, String)>,
) -> Result<Json<DomainResponse>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Find domain
    let domain = Domain::find_by_domain(&state.pool, &domain_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Domain not found: {}", domain_name)))?;

    // Verify domain belongs to project
    if domain.project_id != project.id {
        return Err(AppError::Forbidden("Domain does not belong to this project".to_string()));
    }

    // Already verified?
    if domain.status == "verified" {
        return Ok(Json(domain.into()));
    }

    // Get current deploy
    let deploy_id = project.current_deploy_id.ok_or_else(|| {
        AppError::BadRequest("Project has no deployment yet".to_string())
    })?;

    let deploy = Deploy::find_by_id(&state.pool, deploy_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Deploy not found: {}", deploy_id)))?;

    // Try to fetch verification file
    let verification_path = "statichub-verify.txt";
    match state.storage.get_file(&deploy.storage_path, verification_path).await {
        Ok(content) => {
            let content_str = String::from_utf8_lossy(&content).trim().to_string();

            if content_str == domain.verification_token {
                // Verification successful
                Domain::mark_verified(&state.pool, domain.id).await?;

                let updated_domain = Domain::find_by_domain(&state.pool, &domain_name)
                    .await?
                    .unwrap();

                Ok(Json(updated_domain.into()))
            } else {
                // Token mismatch
                Domain::mark_failed(&state.pool, domain.id).await?;
                Err(AppError::BadRequest("Verification token mismatch".to_string()))
            }
        }
        Err(_) => {
            // File not found
            Domain::mark_failed(&state.pool, domain.id).await?;
            Err(AppError::BadRequest("Verification file not found".to_string()))
        }
    }
}
```

- [ ] **Step 5: Implement remove_domain endpoint**

Add to `domains.rs`:

```rust
pub async fn remove_domain(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((project_name, domain_name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Delete domain
    Domain::delete(&state.pool, project.id, &domain_name).await?;

    Ok(Json(serde_json::json!({ "success": true })))
}
```

- [ ] **Step 6: Add uuid dependency**

Modify `server/Cargo.toml`, add to dependencies:

```toml
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 7: Export domains module**

Modify `server/src/api/mod.rs`:

```rust
mod deploys;
mod serve;
mod auth;
mod projects;
mod management;
mod domains;

pub use deploys::{create_anonymous_deploy, DeployState};
pub use serve::serve_static_file;
pub use auth::{login_google, callback_google, auth_status, AuthState};
pub use projects::create_project_deploy;
pub use management::{list_projects, get_project_info, rollback_project};
pub use domains::{add_domain, list_domains, verify_domain, remove_domain};
```

- [ ] **Step 8: Wire up domain routes**

Modify `server/src/lib.rs`, in the `authenticated_routes` section:

```rust
let authenticated_routes = Router::new()
    .route("/api/projects/:name/deploys", post(api::create_project_deploy))
    .route("/api/projects", get(api::list_projects))
    .route("/api/projects/:name", get(api::get_project_info))
    .route("/api/projects/:name/rollback", post(api::rollback_project))
    .route("/api/projects/:name/domains", post(api::add_domain))
    .route("/api/projects/:name/domains", get(api::list_domains))
    .route("/api/projects/:name/domains/:domain/verify", post(api::verify_domain))
    .route("/api/projects/:name/domains/:domain", delete(api::remove_domain))
    .layer(from_fn_with_state(auth_state.clone(), auth_middleware))
    .with_state(deploy_state.clone());
```

- [ ] **Step 9: Compile and check**

Run: `cargo check -p statichub-server`
Expected: No errors

- [ ] **Step 10: Commit**

```bash
git add server/src/api/domains.rs server/src/api/mod.rs server/src/lib.rs server/Cargo.toml
git commit -m "feat: add domain management API endpoints

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 4: Update Static File Serving for Custom Domains

**Files:**
- Modify: `server/src/api/serve.rs`

- [ ] **Step 1: Read current serve logic**

Read `server/src/api/serve.rs` to understand current subdomain lookup

- [ ] **Step 2: Add custom domain lookup**

Modify `serve_static_file` in `server/src/api/serve.rs`:

Replace the subdomain lookup section (around lines 20-26) with:

```rust
pub async fn serve_static_file(
    Host(hostname): Host,
    State(state): State<Arc<DeployState>>,
    request: Request,
) -> Result<Response> {
    // Try custom domain first
    let project = if let Some(domain_project) = try_custom_domain(&state, &hostname).await? {
        domain_project
    } else {
        // Fall back to subdomain
        let subdomain = extract_subdomain(&hostname, &state.base_url)?;
        Project::find_by_subdomain(&state.pool, &subdomain)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", subdomain)))?
    };

    // ... rest of the function remains the same ...
```

- [ ] **Step 3: Implement try_custom_domain helper**

Add before `serve_static_file` in `serve.rs`:

```rust
use crate::models::Domain;

async fn try_custom_domain(
    state: &Arc<DeployState>,
    hostname: &str,
) -> Result<Option<Project>> {
    // Check if this hostname is a verified custom domain
    if let Some(domain) = Domain::find_by_domain(&state.pool, hostname).await? {
        if domain.status == "verified" {
            // Find the project this domain belongs to
            let project = sqlx::query_as::<_, Project>(
                "SELECT * FROM projects WHERE id = ?"
            )
            .bind(domain.project_id)
            .fetch_optional(&state.pool)
            .await?;

            return Ok(project);
        }
    }

    Ok(None)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p statichub-server`
Expected: All tests pass (existing serve tests should still work)

- [ ] **Step 5: Commit**

```bash
git add server/src/api/serve.rs
git commit -m "feat: add custom domain support to static file serving

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 5: CLI Domain Commands

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/src/client.rs`

- [ ] **Step 1: Add domain response types to client**

Modify `cli/src/client.rs`, add:

```rust
#[derive(Debug, Deserialize)]
pub struct DomainResponse {
    pub id: i64,
    pub domain: String,
    pub status: String,
    pub verification_token: String,
    pub verification_instructions: String,
    pub created_at: String,
    pub verified_at: Option<String>,
}
```

- [ ] **Step 2: Add client methods**

Add to `impl Client` in `cli/src/client.rs`:

```rust
pub async fn add_domain(
    &self,
    project: &str,
    domain: &str,
    token: &str,
) -> Result<DomainResponse> {
    let url = format!("{}/api/projects/{}/domains", self.base_url, project);

    let response = self
        .client
        .post(&url)
        .header("authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({ "domain": domain }))
        .send()
        .await
        .context("Failed to add domain")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to add domain: {} - {}", status, body);
    }

    response
        .json()
        .await
        .context("Failed to parse domain response")
}

pub async fn list_domains(
    &self,
    project: &str,
    token: &str,
) -> Result<Vec<DomainResponse>> {
    let url = format!("{}/api/projects/{}/domains", self.base_url, project);

    let response = self
        .client
        .get(&url)
        .header("authorization", format!("Bearer {}", token))
        .send()
        .await
        .context("Failed to list domains")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to list domains: {} - {}", status, body);
    }

    response
        .json()
        .await
        .context("Failed to parse domains list")
}

pub async fn verify_domain(
    &self,
    project: &str,
    domain: &str,
    token: &str,
) -> Result<DomainResponse> {
    let url = format!("{}/api/projects/{}/domains/{}/verify", self.base_url, project, domain);

    let response = self
        .client
        .post(&url)
        .header("authorization", format!("Bearer {}", token))
        .send()
        .await
        .context("Failed to verify domain")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to verify domain: {} - {}", status, body);
    }

    response
        .json()
        .await
        .context("Failed to parse domain response")
}

pub async fn remove_domain(
    &self,
    project: &str,
    domain: &str,
    token: &str,
) -> Result<()> {
    let url = format!("{}/api/projects/{}/domains/{}", self.base_url, project, domain);

    let response = self
        .client
        .delete(&url)
        .header("authorization", format!("Bearer {}", token))
        .send()
        .await
        .context("Failed to remove domain")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to remove domain: {} - {}", status, body);
    }

    Ok(())
}
```

- [ ] **Step 3: Add domain subcommand to CLI**

Modify `cli/src/main.rs`, add to `Commands` enum:

```rust
/// Domain management
#[command(subcommand)]
Domain(DomainCommands),
```

- [ ] **Step 4: Define DomainCommands enum**

Add after `Commands` enum in `cli/src/main.rs`:

```rust
#[derive(Subcommand)]
enum DomainCommands {
    /// Add a custom domain to a project
    Add {
        /// Project name
        project: String,
        /// Domain name (e.g., example.com)
        domain: String,
    },

    /// List domains for a project
    List {
        /// Project name
        project: String,
    },

    /// Verify domain ownership
    Verify {
        /// Project name
        project: String,
        /// Domain name
        domain: String,
    },

    /// Remove a domain from a project
    Remove {
        /// Project name
        project: String,
        /// Domain name
        domain: String,
    },
}
```

- [ ] **Step 5: Implement domain command handlers**

Add to main function's match in `cli/src/main.rs`:

```rust
Commands::Domain(domain_cmd) => {
    let credentials = auth::load_credentials()
        .context("Not logged in. Run 'statichub login' first.")?;

    let server_url = std::env::var("STATICHUB_SERVER")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let client = client::Client::new(server_url);

    match domain_cmd {
        DomainCommands::Add { project, domain } => {
            println!("🌐 Adding domain {} to {}...", domain, project);

            let response = client
                .add_domain(&project, &domain, &credentials.access_token)
                .await?;

            println!("✅ Domain added!");
            println!("   Status: {}", response.status);
            println!("\n📝 Verification instructions:");
            println!("   {}", response.verification_instructions);
            println!("\n   After uploading the file, run:");
            println!("   statichub domain verify {} {}", project, domain);
        }

        DomainCommands::List { project } => {
            let domains = client
                .list_domains(&project, &credentials.access_token)
                .await?;

            if domains.is_empty() {
                println!("📭 No domains configured");
                println!("   Add one with: statichub domain add {} <domain>", project);
            } else {
                println!("🌐 Domains for {}:\n", project);
                for domain in domains {
                    let status_icon = match domain.status.as_str() {
                        "verified" => "✅",
                        "pending_verification" => "⏳",
                        "failed" => "❌",
                        _ => "❓",
                    };

                    println!("  {} {} - {}", status_icon, domain.domain, domain.status);

                    if domain.status == "pending_verification" {
                        println!("     Upload: statichub-verify.txt with content: {}", domain.verification_token);
                    }

                    if let Some(verified_at) = domain.verified_at {
                        println!("     Verified: {}", verified_at);
                    }

                    println!();
                }
            }
        }

        DomainCommands::Verify { project, domain } => {
            println!("🔍 Verifying domain {}...", domain);

            let response = client
                .verify_domain(&project, &domain, &credentials.access_token)
                .await?;

            if response.status == "verified" {
                println!("✅ Domain verified successfully!");
                println!("   {} is now live", domain);
            } else {
                println!("❌ Verification failed");
                println!("   Status: {}", response.status);
            }
        }

        DomainCommands::Remove { project, domain } => {
            println!("🗑️  Removing domain {}...", domain);

            client
                .remove_domain(&project, &domain, &credentials.access_token)
                .await?;

            println!("✅ Domain removed");
        }
    }
}
```

- [ ] **Step 6: Build CLI**

Run: `cargo build -p statichub`
Expected: Builds successfully

- [ ] **Step 7: Commit**

```bash
git add cli/src/main.rs cli/src/client.rs
git commit -m "feat: add CLI domain management commands

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 6: Integration Tests

**Files:**
- Create: `server/tests/domain_tests.rs`

- [ ] **Step 1: Write test setup**

Create `server/tests/domain_tests.rs`:

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
```

- [ ] **Step 2: Test add domain**

Add to `domain_tests.rs`:

```rust
#[sqlx::test]
async fn test_add_domain(pool: SqlitePool) {
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
```

- [ ] **Step 3: Test list domains**

Add to `domain_tests.rs`:

```rust
#[sqlx::test]
async fn test_list_domains(pool: SqlitePool) {
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
```

- [ ] **Step 4: Test domain verification**

Add to `domain_tests.rs`:

```rust
#[sqlx::test]
async fn test_verify_domain_success(pool: SqlitePool) {
    let storage = Arc::new(FilesystemStorage::new("./test_storage".into()));

    let deploy_state = Arc::new(DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
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
    let domain = Domain::create(&pool, project.id, "example.com", "test-token-123")
        .await
        .unwrap();

    // Create verification file in deploy
    let files = vec![(
        "statichub-verify.txt".to_string(),
        "test-token-123".as_bytes().to_vec(),
    )];
    storage.store_deploy(&deploy.storage_path, files).await.unwrap();

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
```

- [ ] **Step 5: Test remove domain**

Add to `domain_tests.rs`:

```rust
#[sqlx::test]
async fn test_remove_domain(pool: SqlitePool) {
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
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p statichub-server`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add server/tests/domain_tests.rs
git commit -m "test: add integration tests for domain management

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 7: Manual End-to-End Testing

**Files:** None (manual testing)

- [ ] **Step 1: Start server**

Terminal 1:
```bash
cd server
cargo run
```

Expected: Server starts on port 3000

- [ ] **Step 2: Login**

Terminal 2:
```bash
cargo run -p statichub -- login
```

Expected: Browser opens, authenticate, credentials saved

- [ ] **Step 3: Deploy a test site**

```bash
mkdir -p /tmp/domain-test
echo "<h1>Hello from custom domain</h1>" > /tmp/domain-test/index.html
cargo run -p statichub -- deploy /tmp/domain-test --name domain-test
```

Expected: Deploy successful, URL printed

- [ ] **Step 4: Add a domain**

```bash
cargo run -p statichub -- domain add domain-test example.test
```

Expected:
- Domain added
- Verification instructions printed
- Shows verification token

- [ ] **Step 5: List domains**

```bash
cargo run -p statichub -- domain list domain-test
```

Expected:
- Shows example.test with status "pending_verification"
- Shows verification token

- [ ] **Step 6: Create verification file**

```bash
# Get the token from step 4 output
echo "YOUR_VERIFICATION_TOKEN" > /tmp/domain-test/statichub-verify.txt
cargo run -p statichub -- deploy /tmp/domain-test --name domain-test
```

Expected: New version deployed with verification file

- [ ] **Step 7: Verify domain**

```bash
cargo run -p statichub -- domain verify domain-test example.test
```

Expected: Domain verified successfully

- [ ] **Step 8: Check domain status**

```bash
cargo run -p statichub -- domain list domain-test
```

Expected: Status changed to "verified"

- [ ] **Step 9: Test custom domain serving (requires /etc/hosts)**

Add to `/etc/hosts`:
```
127.0.0.1 example.test
```

Then:
```bash
curl http://example.test:3000/
```

Expected: Returns the deployed HTML

- [ ] **Step 10: Remove domain**

```bash
cargo run -p statichub -- domain remove domain-test example.test
```

Expected: Domain removed

- [ ] **Step 11: Verify removal**

```bash
cargo run -p statichub -- domain list domain-test
```

Expected: No domains listed

---

## Task 8: Final Commit and Documentation

**Files:**
- None (final commit)

- [ ] **Step 1: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 2: Check compilation**

Run: `cargo build --workspace --release`
Expected: Builds successfully

- [ ] **Step 3: Create final summary commit**

```bash
git add -A
git commit -m "feat: complete custom domain support with verification

- Add domains table and model
- Implement domain API endpoints (add, list, verify, remove)
- Update static file serving to handle custom domains
- Add CLI domain management commands
- Add integration tests for domain functionality
- File-based domain verification

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Success Criteria

- Domains table exists with proper schema and indexes
- Domain model has CRUD operations
- API endpoints work: add, list, verify, remove
- Static file serving checks custom domains before subdomains
- Verification works via file upload
- CLI commands provide nice UX with clear instructions
- All tests pass (70+ tests total)
- Manual E2E flow works end-to-end
- Verified domains serve content correctly
- Multiple domains can be added to same project
- Domain ownership is properly validated
- Verification tokens are unique and secure (UUIDs)

---

## Self-Review Checklist

**Spec Coverage:**
- ✅ Domain table schema
- ✅ Domain model with CRUD
- ✅ API endpoints (add, list, verify, remove)
- ✅ Custom domain serving
- ✅ File-based verification
- ✅ CLI commands
- ✅ Integration tests

**No Placeholders:**
- ✅ All code blocks are complete
- ✅ All SQL queries are provided
- ✅ All function signatures match
- ✅ All test cases are written out

**Type Consistency:**
- ✅ Domain struct consistent throughout
- ✅ DomainResponse struct matches API and CLI
- ✅ Method signatures match between definition and usage
- ✅ Database types match Rust types
