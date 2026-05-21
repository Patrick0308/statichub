# StaticHub Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a static web publishing platform with CLI-first deployment, anonymous quick deploys, and authenticated project management.

**Architecture:** Rust workspace with two binaries (CLI client + server), SQLite database, filesystem storage with trait abstraction for future S3 migration, Google OAuth, and Let's Encrypt SSL support.

**Tech Stack:** Rust, Axum, SQLx (SQLite), OAuth2, JWT, Clap, Reqwest, Serde

---

## File Structure Overview

```
statichub/
├── Cargo.toml                          # Workspace manifest
├── cli/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs                     # CLI entry point
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── deploy.rs               # Deploy command
│   │   │   ├── login.rs                # Login command
│   │   │   ├── list.rs                 # List projects
│   │   │   ├── rollback.rs             # Rollback command
│   │   │   ├── domain.rs               # Domain management
│   │   │   └── token.rs                # Token management
│   │   ├── config.rs                   # Config file parsing
│   │   ├── auth.rs                     # Auth token storage
│   │   ├── api_client.rs               # HTTP client for server API
│   │   └── upload.rs                   # Tarball creation/upload
│   └── tests/
│       ├── integration_tests.rs
│       └── fixtures/
├── server/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs                     # Server entry point
│   │   ├── api/
│   │   │   ├── mod.rs
│   │   │   ├── auth.rs                 # Auth endpoints
│   │   │   ├── projects.rs             # Project endpoints
│   │   │   ├── deploys.rs              # Deploy endpoints
│   │   │   ├── domains.rs              # Domain endpoints
│   │   │   └── tokens.rs               # Token endpoints
│   │   ├── models/
│   │   │   ├── mod.rs
│   │   │   ├── user.rs
│   │   │   ├── project.rs
│   │   │   ├── deploy.rs
│   │   │   ├── domain.rs
│   │   │   └── token.rs
│   │   ├── storage/
│   │   │   ├── mod.rs
│   │   │   ├── trait.rs                # Storage trait
│   │   │   └── filesystem.rs           # Filesystem implementation
│   │   ├── auth/
│   │   │   ├── mod.rs
│   │   │   ├── oauth.rs                # Google OAuth
│   │   │   └── jwt.rs                  # JWT token handling
│   │   ├── serve.rs                    # Static file serving
│   │   ├── db.rs                       # Database connection
│   │   └── error.rs                    # Error types
│   ├── migrations/
│   │   └── 001_initial_schema.sql
│   └── tests/
│       ├── api_tests.rs
│       └── storage_tests.rs
├── shared/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── types.rs                    # Shared types (config, etc.)
└── docs/
    └── superpowers/
        ├── specs/
        │   └── 2026-05-21-statichub-design.md
        └── plans/
            └── 2026-05-21-statichub-implementation.md
```

---

## Task 1: Project Setup and Workspace Configuration

**Files:**
- Create: `Cargo.toml`
- Create: `cli/Cargo.toml`
- Create: `server/Cargo.toml`
- Create: `shared/Cargo.toml`
- Create: `.gitignore`

- [ ] **Step 1: Create workspace Cargo.toml**

```toml
[workspace]
members = ["cli", "server", "shared"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["StaticHub Contributors"]
license = "MIT"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "1"
```

- [ ] **Step 2: Create CLI Cargo.toml**

```toml
[package]
name = "statichub"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[[bin]]
name = "statichub"
path = "src/main.rs"

[dependencies]
# Workspace dependencies
serde.workspace = true
serde_json.workspace = true
serde_yaml.workspace = true
tokio.workspace = true
anyhow.workspace = true
thiserror.workspace = true

# CLI specific
clap = { version = "4", features = ["derive"] }
reqwest = { version = "0.11", features = ["json", "stream"] }
flate2 = "1"
tar = "0.4"
indicatif = "0.17"
colored = "2"
dirs = "5"

# Shared crate
statichub-shared = { path = "../shared" }

[dev-dependencies]
mockito = "1"
tempfile = "3"
```

- [ ] **Step 3: Create Server Cargo.toml**

```toml
[package]
name = "statichub-server"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[[bin]]
name = "statichub-server"
path = "src/main.rs"

[dependencies]
# Workspace dependencies
serde.workspace = true
serde_json.workspace = true
serde_yaml.workspace = true
tokio.workspace = true
anyhow.workspace = true
thiserror.workspace = true

# Web framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "compression", "fs", "trace"] }

# Database
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls", "migrate"] }

# Auth
oauth2 = "4"
jsonwebtoken = "9"
bcrypt = "0.15"

# HTTP utilities
mime_guess = "2"
reqwest = "0.11"

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
regex = "1"
async-trait = "0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Shared crate
statichub-shared = { path = "../shared" }

[dev-dependencies]
wiremock = "0.6"
tempfile = "3"
```

- [ ] **Step 4: Create Shared Cargo.toml**

```toml
[package]
name = "statichub-shared"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
serde_yaml.workspace = true
thiserror.workspace = true
```

- [ ] **Step 5: Create .gitignore**

```
# Rust
target/
Cargo.lock
**/*.rs.bk
*.pdb

# IDE
.vscode/
.idea/
*.swp
*.swo
*~

# OS
.DS_Store
Thumbs.db

# StaticHub specific
/var/statichub/
*.db
*.db-shm
*.db-wal
.statichub/

# Test artifacts
/test-deploys/
```

- [ ] **Step 6: Commit workspace setup**

```bash
git add Cargo.toml cli/Cargo.toml server/Cargo.toml shared/Cargo.toml .gitignore
git commit -m "chore: initialize Rust workspace with CLI, server, and shared crates

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 2: Shared Types and Config Structures

**Files:**
- Create: `shared/src/lib.rs`
- Create: `shared/src/types.rs`

- [ ] **Step 1: Write test for config parsing**

Create: `shared/src/types.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config_yaml() {
        let yaml = r#"
name: my-project
clean_urls: true
spa: false
headers:
  "/*.js":
    cache-control: public, max-age=31536000
redirects:
  - from: /old
    to: /new
    status: 301
"#;

        let config: ProjectConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, Some("my-project".to_string()));
        assert_eq!(config.clean_urls, Some(true));
        assert_eq!(config.spa, Some(false));
        assert_eq!(config.headers.as_ref().unwrap().len(), 1);
        assert_eq!(config.redirects.as_ref().unwrap().len(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p statichub-shared`
Expected: Compilation error - types not defined

- [ ] **Step 3: Implement shared types**

Add to `shared/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Project configuration from statichub.yaml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub clean_urls: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub spa: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, HashMap<String, String>>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirects: Option<Vec<Redirect>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redirect {
    pub from: String,
    pub to: String,
    pub status: u16,
}

/// API response types
#[derive(Debug, Serialize, Deserialize)]
pub struct DeployResponse {
    pub url: String,
    pub subdomain: String,
    pub version: Option<i64>,
    pub deploy_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub code: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub subdomain: String,
    pub is_anonymous: bool,
    pub current_version: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeployInfo {
    pub version: i64,
    pub file_count: i64,
    pub total_size_bytes: i64,
    pub deployed_at: String,
    pub status: String,
}
```

- [ ] **Step 4: Create lib.rs to export types**

Create: `shared/src/lib.rs`

```rust
pub mod types;

pub use types::*;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p statichub-shared`
Expected: All tests pass

- [ ] **Step 6: Commit shared types**

```bash
git add shared/
git commit -m "feat: add shared types and config structures

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 3: Database Schema and Migrations

**Files:**
- Create: `server/migrations/001_initial_schema.sql`
- Create: `server/src/db.rs`

- [ ] **Step 1: Create initial database migration**

Create: `server/migrations/001_initial_schema.sql`

```sql
-- Users table
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    oauth_provider TEXT NOT NULL,
    oauth_id TEXT NOT NULL,
    email TEXT NOT NULL,
    username TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(oauth_provider, oauth_id)
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_username ON users(username);

-- Projects table
CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    name TEXT NOT NULL UNIQUE,
    subdomain TEXT NOT NULL UNIQUE,
    is_anonymous BOOLEAN NOT NULL DEFAULT 0,
    current_deploy_id INTEGER,
    config TEXT,
    last_deployed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_projects_owner ON projects(owner_id);
CREATE INDEX idx_projects_subdomain ON projects(subdomain);

-- Deploys table
CREATE TABLE deploys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    version INTEGER NOT NULL,
    storage_path TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('uploading', 'ready', 'failed')),
    file_count INTEGER NOT NULL DEFAULT 0,
    total_size_bytes INTEGER NOT NULL DEFAULT 0,
    deployed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    UNIQUE(project_id, version)
);

CREATE INDEX idx_deploys_project ON deploys(project_id);

-- Custom domains table
CREATE TABLE custom_domains (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    domain TEXT NOT NULL UNIQUE,
    verified BOOLEAN NOT NULL DEFAULT 0,
    verification_token TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    verified_at TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX idx_domains_project ON custom_domains(project_id);
CREATE INDEX idx_domains_domain ON custom_domains(domain);

-- Deploy tokens table
CREATE TABLE deploy_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    token_hash TEXT NOT NULL,
    name TEXT NOT NULL,
    last_used_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX idx_tokens_project ON deploy_tokens(project_id);

-- OAuth sessions table (for CLI login flow)
CREATE TABLE oauth_sessions (
    session_id TEXT PRIMARY KEY,
    token TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_sessions_expires ON oauth_sessions(expires_at);
```

- [ ] **Step 2: Write database connection module**

Create: `server/src/db.rs`

```rust
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::time::Duration;

pub async fn create_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_in_memory_db() {
        let pool = create_pool(":memory:").await.unwrap();

        // Verify tables exist
        let result = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&pool)
            .await
            .unwrap();

        assert!(result.len() >= 5); // At least 5 tables
    }
}
```

- [ ] **Step 3: Run test to verify migrations work**

Run: `cargo test -p statichub-server test_create_in_memory_db`
Expected: Test passes, tables created

- [ ] **Step 4: Commit database schema**

```bash
git add server/migrations/ server/src/db.rs
git commit -m "feat: add database schema and migrations

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 4: Storage Trait and Filesystem Implementation

**Files:**
- Create: `server/src/storage/mod.rs`
- Create: `server/src/storage/trait.rs`
- Create: `server/src/storage/filesystem.rs`
- Create: `server/tests/storage_tests.rs`

- [ ] **Step 1: Write test for storage trait**

Create: `server/tests/storage_tests.rs`

```rust
use statichub_server::storage::{Storage, FilesystemStorage};
use tempfile::TempDir;

#[tokio::test]
async fn test_store_and_retrieve_deploy() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let deploy_id = "test-project/deploy-1";
    let content = b"hello world";

    // Store file
    storage.store_file(deploy_id, "index.html", content).await.unwrap();

    // Retrieve file
    let retrieved = storage.get_file(deploy_id, "index.html").await.unwrap();
    assert_eq!(retrieved, content);
}

#[tokio::test]
async fn test_list_files_in_deploy() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let deploy_id = "test-project/deploy-1";

    storage.store_file(deploy_id, "index.html", b"<html>").await.unwrap();
    storage.store_file(deploy_id, "app.js", b"console.log()").await.unwrap();

    let files = storage.list_files(deploy_id).await.unwrap();
    assert_eq!(files.len(), 2);
}

#[tokio::test]
async fn test_delete_deploy() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let deploy_id = "test-project/deploy-1";
    storage.store_file(deploy_id, "index.html", b"<html>").await.unwrap();

    storage.delete_deploy(deploy_id).await.unwrap();

    let result = storage.get_file(deploy_id, "index.html").await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p statichub-server --test storage_tests`
Expected: Compilation errors - types not defined

- [ ] **Step 3: Define storage trait**

Create: `server/src/storage/trait.rs`

```rust
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
}

#[async_trait]
pub trait Storage: Send + Sync {
    /// Store a single file in a deploy
    async fn store_file(
        &self,
        deploy_id: &str,
        path: &str,
        content: &[u8],
    ) -> Result<(), StorageError>;

    /// Get a file from a deploy
    async fn get_file(
        &self,
        deploy_id: &str,
        path: &str,
    ) -> Result<Vec<u8>, StorageError>;

    /// List all files in a deploy
    async fn list_files(
        &self,
        deploy_id: &str,
    ) -> Result<Vec<FileInfo>, StorageError>;

    /// Delete an entire deploy
    async fn delete_deploy(
        &self,
        deploy_id: &str,
    ) -> Result<(), StorageError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}
```

- [ ] **Step 4: Implement filesystem storage**

Create: `server/src/storage/filesystem.rs`

```rust
use super::trait_::{FileInfo, Storage, StorageError};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct FilesystemStorage {
    base_path: PathBuf,
}

impl FilesystemStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn validate_path(&self, path: &str) -> Result<(), StorageError> {
        if path.contains("..") || path.starts_with('/') {
            return Err(StorageError::InvalidPath(path.to_string()));
        }
        Ok(())
    }

    fn deploy_path(&self, deploy_id: &str) -> PathBuf {
        self.base_path.join(deploy_id)
    }
}

#[async_trait]
impl Storage for FilesystemStorage {
    async fn store_file(
        &self,
        deploy_id: &str,
        path: &str,
        content: &[u8],
    ) -> Result<(), StorageError> {
        self.validate_path(path)?;

        let file_path = self.deploy_path(deploy_id).join(path);

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&file_path, content).await?;
        Ok(())
    }

    async fn get_file(
        &self,
        deploy_id: &str,
        path: &str,
    ) -> Result<Vec<u8>, StorageError> {
        self.validate_path(path)?;

        let file_path = self.deploy_path(deploy_id).join(path);

        if !file_path.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }

        let content = fs::read(&file_path).await?;
        Ok(content)
    }

    async fn list_files(
        &self,
        deploy_id: &str,
    ) -> Result<Vec<FileInfo>, StorageError> {
        let deploy_path = self.deploy_path(deploy_id);

        if !deploy_path.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        collect_files(&deploy_path, &deploy_path, &mut files).await?;
        Ok(files)
    }

    async fn delete_deploy(
        &self,
        deploy_id: &str,
    ) -> Result<(), StorageError> {
        let deploy_path = self.deploy_path(deploy_id);

        if deploy_path.exists() {
            fs::remove_dir_all(&deploy_path).await?;
        }

        Ok(())
    }
}

async fn collect_files(
    base: &Path,
    current: &Path,
    files: &mut Vec<FileInfo>,
) -> Result<(), std::io::Error> {
    let mut entries = fs::read_dir(current).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let metadata = entry.metadata().await?;

        if metadata.is_file() {
            let relative = path.strip_prefix(base).unwrap();
            files.push(FileInfo {
                path: relative.to_string_lossy().to_string(),
                size: metadata.len(),
            });
        } else if metadata.is_dir() {
            collect_files(base, &path, files).await?;
        }
    }

    Ok(())
}
```

- [ ] **Step 5: Create storage module exports**

Create: `server/src/storage/mod.rs`

```rust
mod trait_;
mod filesystem;

pub use trait_::{FileInfo, Storage, StorageError};
pub use filesystem::FilesystemStorage;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p statichub-server --test storage_tests`
Expected: All tests pass

- [ ] **Step 7: Commit storage implementation**

```bash
git add server/src/storage/ server/tests/storage_tests.rs
git commit -m "feat: implement storage trait and filesystem backend

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 5: Database Models

**Files:**
- Create: `server/src/models/mod.rs`
- Create: `server/src/models/user.rs`
- Create: `server/src/models/project.rs`
- Create: `server/src/models/deploy.rs`

- [ ] **Step 1: Write test for user model**

Create: `server/src/models/user.rs`

```rust
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub oauth_provider: String,
    pub oauth_id: String,
    pub email: String,
    pub username: String,
    pub created_at: chrono::NaiveDateTime,
}

impl User {
    pub async fn create(
        pool: &SqlitePool,
        oauth_provider: &str,
        oauth_id: &str,
        email: &str,
        username: &str,
    ) -> Result<User, sqlx::Error> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (oauth_provider, oauth_id, email, username)
            VALUES (?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(oauth_provider)
        .bind(oauth_id)
        .bind(email)
        .bind(username)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_oauth(
        pool: &SqlitePool,
        oauth_provider: &str,
        oauth_id: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE oauth_provider = ? AND oauth_id = ?"
        )
        .bind(oauth_provider)
        .bind(oauth_id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        id: i64,
    ) -> Result<Option<User>, sqlx::Error> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_pool;

    #[tokio::test]
    async fn test_create_and_find_user() {
        let pool = create_pool(":memory:").await.unwrap();

        let user = User::create(
            &pool,
            "google",
            "123456",
            "test@example.com",
            "testuser",
        ).await.unwrap();

        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.username, "testuser");

        let found = User::find_by_oauth(&pool, "google", "123456")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(found.id, user.id);
    }
}
```

- [ ] **Step 2: Write project model**

Create: `server/src/models/project.rs`

```rust
use sqlx::SqlitePool;
use statichub_shared::ProjectConfig;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Project {
    pub id: i64,
    pub owner_id: Option<i64>,
    pub name: String,
    pub subdomain: String,
    pub is_anonymous: bool,
    pub current_deploy_id: Option<i64>,
    pub config: Option<String>,
    pub last_deployed_at: chrono::NaiveDateTime,
    pub created_at: chrono::NaiveDateTime,
}

impl Project {
    pub async fn create_anonymous(
        pool: &SqlitePool,
        subdomain: &str,
    ) -> Result<Project, sqlx::Error> {
        let project = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (name, subdomain, is_anonymous)
            VALUES (?, ?, 1)
            RETURNING *
            "#,
        )
        .bind(subdomain)
        .bind(subdomain)
        .fetch_one(pool)
        .await?;

        Ok(project)
    }

    pub async fn create_owned(
        pool: &SqlitePool,
        owner_id: i64,
        name: &str,
        config: Option<&ProjectConfig>,
    ) -> Result<Project, sqlx::Error> {
        let subdomain = format!("{}.statichub.io", name);
        let config_json = config.map(|c| serde_json::to_string(c).ok()).flatten();

        let project = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (owner_id, name, subdomain, is_anonymous, config)
            VALUES (?, ?, ?, 0, ?)
            RETURNING *
            "#,
        )
        .bind(owner_id)
        .bind(name)
        .bind(&subdomain)
        .bind(config_json)
        .fetch_one(pool)
        .await?;

        Ok(project)
    }

    pub async fn find_by_name(
        pool: &SqlitePool,
        name: &str,
    ) -> Result<Option<Project>, sqlx::Error> {
        let project = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE name = ?"
        )
        .bind(name)
        .fetch_optional(pool)
        .await?;

        Ok(project)
    }

    pub async fn find_by_subdomain(
        pool: &SqlitePool,
        subdomain: &str,
    ) -> Result<Option<Project>, sqlx::Error> {
        let project = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE subdomain = ?"
        )
        .bind(subdomain)
        .fetch_optional(pool)
        .await?;

        Ok(project)
    }

    pub async fn list_by_owner(
        pool: &SqlitePool,
        owner_id: i64,
    ) -> Result<Vec<Project>, sqlx::Error> {
        let projects = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE owner_id = ? ORDER BY created_at DESC"
        )
        .bind(owner_id)
        .fetch_all(pool)
        .await?;

        Ok(projects)
    }

    pub fn get_config(&self) -> Option<ProjectConfig> {
        self.config.as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_pool;
    use crate::models::User;

    #[tokio::test]
    async fn test_create_anonymous_project() {
        let pool = create_pool(":memory:").await.unwrap();

        let project = Project::create_anonymous(&pool, "x7k2m9").await.unwrap();
        assert!(project.is_anonymous);
        assert_eq!(project.name, "x7k2m9");
    }

    #[tokio::test]
    async fn test_create_owned_project() {
        let pool = create_pool(":memory:").await.unwrap();

        let user = User::create(&pool, "google", "123", "test@example.com", "testuser")
            .await.unwrap();

        let project = Project::create_owned(&pool, user.id, "my-app", None)
            .await.unwrap();

        assert!(!project.is_anonymous);
        assert_eq!(project.name, "my-app");
        assert_eq!(project.owner_id, Some(user.id));
    }
}
```

- [ ] **Step 3: Write deploy model**

Create: `server/src/models/deploy.rs`

```rust
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Deploy {
    pub id: i64,
    pub project_id: i64,
    pub version: i64,
    pub storage_path: String,
    pub status: String,
    pub file_count: i64,
    pub total_size_bytes: i64,
    pub deployed_at: chrono::NaiveDateTime,
}

impl Deploy {
    pub async fn create(
        pool: &SqlitePool,
        project_id: i64,
        storage_path: &str,
    ) -> Result<Deploy, sqlx::Error> {
        // Get next version number
        let next_version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM deploys WHERE project_id = ?"
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;

        let deploy = sqlx::query_as::<_, Deploy>(
            r#"
            INSERT INTO deploys (project_id, version, storage_path, status)
            VALUES (?, ?, ?, 'uploading')
            RETURNING *
            "#,
        )
        .bind(project_id)
        .bind(next_version)
        .bind(storage_path)
        .fetch_one(pool)
        .await?;

        Ok(deploy)
    }

    pub async fn update_status(
        pool: &SqlitePool,
        deploy_id: i64,
        status: &str,
        file_count: i64,
        total_size_bytes: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE deploys
            SET status = ?, file_count = ?, total_size_bytes = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(file_count)
        .bind(total_size_bytes)
        .bind(deploy_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn list_by_project(
        pool: &SqlitePool,
        project_id: i64,
        limit: i64,
    ) -> Result<Vec<Deploy>, sqlx::Error> {
        let deploys = sqlx::query_as::<_, Deploy>(
            "SELECT * FROM deploys WHERE project_id = ? ORDER BY version DESC LIMIT ?"
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(deploys)
    }

    pub async fn find_by_version(
        pool: &SqlitePool,
        project_id: i64,
        version: i64,
    ) -> Result<Option<Deploy>, sqlx::Error> {
        let deploy = sqlx::query_as::<_, Deploy>(
            "SELECT * FROM deploys WHERE project_id = ? AND version = ?"
        )
        .bind(project_id)
        .bind(version)
        .fetch_optional(pool)
        .await?;

        Ok(deploy)
    }

    pub async fn delete_old_deploys(
        pool: &SqlitePool,
        project_id: i64,
        keep_count: i64,
    ) -> Result<Vec<String>, sqlx::Error> {
        // Get storage paths of deploys to delete
        let storage_paths: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT storage_path FROM deploys
            WHERE project_id = ?
            ORDER BY version DESC
            LIMIT -1 OFFSET ?
            "#,
        )
        .bind(project_id)
        .bind(keep_count)
        .fetch_all(pool)
        .await?;

        // Delete them
        if !storage_paths.is_empty() {
            sqlx::query(
                r#"
                DELETE FROM deploys
                WHERE project_id = ?
                AND id NOT IN (
                    SELECT id FROM deploys
                    WHERE project_id = ?
                    ORDER BY version DESC
                    LIMIT ?
                )
                "#,
            )
            .bind(project_id)
            .bind(project_id)
            .bind(keep_count)
            .execute(pool)
            .await?;
        }

        Ok(storage_paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_pool;
    use crate::models::{User, Project};

    #[tokio::test]
    async fn test_create_deploy_increments_version() {
        let pool = create_pool(":memory:").await.unwrap();

        let user = User::create(&pool, "google", "123", "test@example.com", "testuser")
            .await.unwrap();
        let project = Project::create_owned(&pool, user.id, "test", None)
            .await.unwrap();

        let deploy1 = Deploy::create(&pool, project.id, "test/deploy-1").await.unwrap();
        assert_eq!(deploy1.version, 1);

        let deploy2 = Deploy::create(&pool, project.id, "test/deploy-2").await.unwrap();
        assert_eq!(deploy2.version, 2);
    }

    #[tokio::test]
    async fn test_delete_old_deploys() {
        let pool = create_pool(":memory:").await.unwrap();

        let user = User::create(&pool, "google", "123", "test@example.com", "testuser")
            .await.unwrap();
        let project = Project::create_owned(&pool, user.id, "test", None)
            .await.unwrap();

        // Create 5 deploys
        for i in 1..=5 {
            Deploy::create(&pool, project.id, &format!("test/deploy-{}", i))
                .await.unwrap();
        }

        // Keep only 3 most recent
        let deleted = Deploy::delete_old_deploys(&pool, project.id, 3)
            .await.unwrap();

        assert_eq!(deleted.len(), 2);

        let remaining = Deploy::list_by_project(&pool, project.id, 10)
            .await.unwrap();
        assert_eq!(remaining.len(), 3);
    }
}
```

- [ ] **Step 4: Create models module**

Create: `server/src/models/mod.rs`

```rust
mod user;
mod project;
mod deploy;

pub use user::User;
pub use project::Project;
pub use deploy::Deploy;
```

- [ ] **Step 5: Run model tests**

Run: `cargo test -p statichub-server models::`
Expected: All tests pass

- [ ] **Step 6: Commit database models**

```bash
git add server/src/models/
git commit -m "feat: add database models for users, projects, and deploys

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 6: Error Types and Server Basics

**Files:**
- Create: `server/src/error.rs`
- Create: `server/src/main.rs`

- [ ] **Step 1: Create error types**

Create: `server/src/error.rs`

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use statichub_shared::ErrorResponse;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match self {
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
            }
            AppError::Storage(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "storage_error", e)
            }
            AppError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, "not_found", msg)
            }
            AppError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "unauthorized", "Unauthorized".to_string())
            }
            AppError::Forbidden(msg) => {
                (StatusCode::FORBIDDEN, "forbidden", msg)
            }
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "bad_request", msg)
            }
            AppError::Conflict(msg) => {
                (StatusCode::CONFLICT, "conflict", msg)
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", msg)
            }
        };

        let body = Json(ErrorResponse {
            error: error_code.to_string(),
            message,
            code: status.as_u16(),
        });

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
```

- [ ] **Step 2: Create minimal server main**

Create: `server/src/main.rs`

```rust
mod db;
mod models;
mod storage;
mod error;

use axum::{Router, routing::get};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "statichub_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Database setup
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:statichub.db".to_string());

    let pool = db::create_pool(&database_url).await?;

    tracing::info!("Database connected and migrations run");

    // Storage setup
    let storage_path = std::env::var("STORAGE_PATH")
        .unwrap_or_else(|_| "./var/statichub/deploys".to_string());

    let storage = storage::FilesystemStorage::new(storage_path.into());

    // Build router
    let app = Router::new()
        .route("/health", get(health_check));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}
```

- [ ] **Step 3: Test server compilation**

Run: `cargo build -p statichub-server`
Expected: Compiles successfully

- [ ] **Step 4: Test health check endpoint**

Run in terminal 1:
```bash
cargo run -p statichub-server
```

Run in terminal 2:
```bash
curl http://localhost:3000/health
```

Expected: Response "OK"

Stop the server (Ctrl+C in terminal 1)

- [ ] **Step 5: Commit error types and server skeleton**

```bash
git add server/src/error.rs server/src/main.rs
git commit -m "feat: add error types and basic server skeleton

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 7: Anonymous Deploy API (Server)

**Files:**
- Create: `server/src/api/mod.rs`
- Create: `server/src/api/deploys.rs`
- Modify: `server/src/main.rs`

- [ ] **Step 1: Write test for anonymous deploy**

Create: `server/src/api/deploys.rs`

```rust
use axum::{
    extract::{State, Multipart},
    Json,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use statichub_shared::DeployResponse;
use crate::{error::Result, storage::Storage, models::{Project, Deploy}};

pub struct DeployState {
    pub pool: SqlitePool,
    pub storage: Arc<dyn Storage>,
}

pub async fn create_anonymous_deploy(
    State(state): State<Arc<DeployState>>,
    mut multipart: Multipart,
) -> Result<Json<DeployResponse>> {
    // Generate random subdomain
    let subdomain = generate_random_subdomain();

    // Create anonymous project
    let project = Project::create_anonymous(&state.pool, &subdomain).await?;

    // Create deploy record
    let storage_path = format!("{}/deploy-1", subdomain);
    let deploy = Deploy::create(&state.pool, project.id, &storage_path).await?;

    // Extract and store files from multipart
    let mut file_count = 0;
    let mut total_size = 0u64;

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.file_name().unwrap_or("file").to_string();
        let data = field.bytes().await.unwrap();

        total_size += data.len() as u64;
        file_count += 1;

        state.storage.store_file(&storage_path, &name, &data).await
            .map_err(|e| crate::error::AppError::Storage(e.to_string()))?;
    }

    // Update deploy status
    Deploy::update_status(&state.pool, deploy.id, "ready", file_count, total_size as i64).await?;

    // Update project current_deploy_id
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&state.pool)
        .await?;

    Ok(Json(DeployResponse {
        url: format!("https://{}.statichub.io", subdomain),
        subdomain: format!("{}.statichub.io", subdomain),
        version: None,
        deploy_id: deploy.id.to_string(),
    }))
}

fn generate_random_subdomain() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();

    (0..6)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_pool;
    use crate::storage::FilesystemStorage;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_random_subdomain() {
        let sub1 = generate_random_subdomain();
        let sub2 = generate_random_subdomain();

        assert_eq!(sub1.len(), 6);
        assert_ne!(sub1, sub2); // Likely different
        assert!(sub1.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
```

- [ ] **Step 2: Add rand dependency to server**

Modify: `server/Cargo.toml`

Add to `[dependencies]`:
```toml
rand = "0.8"
```

- [ ] **Step 3: Create API module**

Create: `server/src/api/mod.rs`

```rust
pub mod deploys;

pub use deploys::*;
```

- [ ] **Step 4: Wire up anonymous deploy endpoint**

Modify: `server/src/main.rs`

Update imports and router:

```rust
mod db;
mod models;
mod storage;
mod error;
mod api;

use axum::{Router, routing::{get, post}};
use std::{net::SocketAddr, sync::Arc};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "statichub_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Database setup
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:statichub.db".to_string());

    let pool = db::create_pool(&database_url).await?;

    tracing::info!("Database connected and migrations run");

    // Storage setup
    let storage_path = std::env::var("STORAGE_PATH")
        .unwrap_or_else(|_| "./var/statichub/deploys".to_string());

    let storage = Arc::new(storage::FilesystemStorage::new(storage_path.into())) as Arc<dyn storage::Storage>;

    // Shared state
    let deploy_state = Arc::new(api::DeployState {
        pool: pool.clone(),
        storage: storage.clone(),
    });

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/deploys/anonymous", post(api::create_anonymous_deploy))
        .with_state(deploy_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}
```

- [ ] **Step 5: Test compilation**

Run: `cargo build -p statichub-server`
Expected: Compiles successfully

- [ ] **Step 6: Commit anonymous deploy API**

```bash
git add server/src/api/ server/src/main.rs server/Cargo.toml
git commit -m "feat: add anonymous deploy API endpoint

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 8: CLI Config and Auth Token Storage

**Files:**
- Create: `cli/src/main.rs`
- Create: `cli/src/config.rs`
- Create: `cli/src/auth.rs`

- [ ] **Step 1: Write test for config parsing**

Create: `cli/src/config.rs`

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use statichub_shared::ProjectConfig;
use std::path::{Path, PathBuf};

pub fn find_config_file(dir: &Path) -> Option<PathBuf> {
    let yaml_path = dir.join("statichub.yaml");
    if yaml_path.exists() {
        return Some(yaml_path);
    }

    let yml_path = dir.join("statichub.yml");
    if yml_path.exists() {
        return Some(yml_path);
    }

    None
}

pub fn load_config(path: &Path) -> Result<ProjectConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let config: ProjectConfig = serde_yaml::from_str(&content)
        .with_context(|| "Failed to parse config file")?;

    Ok(config)
}

pub fn merge_config(base: ProjectConfig, overrides: ProjectConfig) -> ProjectConfig {
    ProjectConfig {
        name: overrides.name.or(base.name),
        clean_urls: overrides.clean_urls.or(base.clean_urls),
        spa: overrides.spa.or(base.spa),
        headers: overrides.headers.or(base.headers),
        redirects: overrides.redirects.or(base.redirects),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_config_file_yaml() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("statichub.yaml");
        fs::write(&config_path, "name: test").unwrap();

        let found = find_config_file(temp.path());
        assert!(found.is_some());
        assert_eq!(found.unwrap(), config_path);
    }

    #[test]
    fn test_load_config() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("statichub.yaml");
        fs::write(&config_path, "name: my-project\nclean_urls: true").unwrap();

        let config = load_config(&config_path).unwrap();
        assert_eq!(config.name, Some("my-project".to_string()));
        assert_eq!(config.clean_urls, Some(true));
    }

    #[test]
    fn test_merge_config() {
        let base = ProjectConfig {
            name: Some("base".to_string()),
            clean_urls: Some(false),
            spa: None,
            headers: None,
            redirects: None,
        };

        let overrides = ProjectConfig {
            name: None,
            clean_urls: Some(true),
            spa: Some(true),
            headers: None,
            redirects: None,
        };

        let merged = merge_config(base, overrides);
        assert_eq!(merged.name, Some("base".to_string()));
        assert_eq!(merged.clean_urls, Some(true));
        assert_eq!(merged.spa, Some(true));
    }
}
```

- [ ] **Step 2: Write auth token storage**

Create: `cli/src/auth.rs`

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub expires_at: Option<String>,
}

fn credentials_path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .context("Could not find home directory")?;

    let config_dir = home.join(".statichub");
    std::fs::create_dir_all(&config_dir)?;

    Ok(config_dir.join("credentials.json"))
}

pub fn save_credentials(token: &str) -> Result<()> {
    let creds = Credentials {
        access_token: token.to_string(),
        expires_at: None,
    };

    let path = credentials_path()?;
    let json = serde_json::to_string_pretty(&creds)?;
    std::fs::write(&path, json)?;

    Ok(())
}

pub fn load_credentials() -> Result<Option<Credentials>> {
    let path = credentials_path()?;

    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let creds: Credentials = serde_json::from_str(&content)?;

    Ok(Some(creds))
}

pub fn clear_credentials() -> Result<()> {
    let path = credentials_path()?;

    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load_credentials() {
        let token = "test_token_123";
        save_credentials(token).unwrap();

        let loaded = load_credentials().unwrap().unwrap();
        assert_eq!(loaded.access_token, token);

        // Cleanup
        clear_credentials().unwrap();
    }

    #[test]
    fn test_clear_credentials() {
        save_credentials("test").unwrap();
        clear_credentials().unwrap();

        let loaded = load_credentials().unwrap();
        assert!(loaded.is_none());
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p statichub`
Expected: All tests pass

- [ ] **Step 4: Create CLI main skeleton**

Create: `cli/src/main.rs`

```rust
mod config;
mod auth;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "statichub")]
#[command(about = "Static web publishing for frontend developers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Deploy static files
    Deploy {
        /// Directory to deploy (default: current directory)
        directory: Option<String>,

        /// Project name (requires login)
        #[arg(long)]
        name: Option<String>,
    },

    /// Login with Google OAuth
    Login,

    /// Logout and clear credentials
    Logout,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Deploy { directory, name } => {
            println!("Deploy command - not yet implemented");
            println!("  Directory: {:?}", directory);
            println!("  Name: {:?}", name);
        }
        Commands::Login => {
            println!("Login command - not yet implemented");
        }
        Commands::Logout => {
            auth::clear_credentials()?;
            println!("✓ Logged out successfully");
        }
    }

    Ok(())
}
```

- [ ] **Step 5: Test CLI compilation**

Run: `cargo build -p statichub`
Expected: Compiles successfully

- [ ] **Step 6: Test CLI commands**

Run: `cargo run -p statichub -- --help`
Expected: Shows help text

Run: `cargo run -p statichub -- logout`
Expected: "✓ Logged out successfully"

- [ ] **Step 7: Commit CLI config and auth**

```bash
git add cli/src/
git commit -m "feat: add CLI config parsing and auth token storage

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Remaining Tasks (9-19) - To Be Completed

The first 8 tasks provide the foundation (workspace setup, database, storage, models, basic server, CLI skeleton, anonymous deploy API). The remaining tasks build on this foundation:

**Task 9: CLI Upload and Tarball Creation**
- Implement file collection and filtering
- Create gzipped tarball
- Add progress tracking with indicatif

**Task 10: CLI Anonymous Deploy Command**
- Create HTTP client for server API
- Wire up deploy command
- Handle multipart upload

**Task 11: Static File Serving**
- Implement hostname-based routing
- File resolution (clean URLs, SPA mode)
- Custom headers and redirects
- Content-Type detection

**Task 12: Google OAuth (Server)**
- Implement `/auth/login/google` and `/auth/callback/google`
- OAuth2 flow with google-oauth crate
- JWT token generation
- Session management for CLI polling

**Task 13: Google OAuth (CLI)**
- Implement login command
- Open browser for OAuth
- Poll server for token
- Save credentials

**Task 14: Authenticated Deploy API**
- Add JWT middleware
- Implement `/api/projects/{name}/deploys` for owned projects
- Project ownership validation

**Task 15: Project Management Commands**
- `statichub list` - list user's projects
- `statichub info [project]` - project details
- `statichub rollback [project] [version]` - rollback deploy

**Task 16: Custom Domain Support**
- Add domain model (already in schema)
- Implement `/api/projects/{name}/domains` endpoints
- DNS TXT verification
- CNAME serving

**Task 17: Deploy Tokens**
- Add token model (already in schema)
- Implement `/api/projects/{name}/tokens` endpoints
- Token-based deploy auth
- CLI token commands

**Task 18: Integration Testing**
- End-to-end test: anonymous deploy
- End-to-end test: authenticated deploy
- End-to-end test: rollback
- End-to-end test: custom domains

**Task 19: Documentation and Polish**
- Write README.md
- Add deployment guide
- Environment variable documentation
- CLI help text improvements

---

## Implementation Notes

**Tasks 1-8 are complete and detailed above.** Tasks 9-19 follow the same pattern:
1. Write failing tests first
2. Implement minimal code to pass
3. Run tests to verify
4. Commit with descriptive message

**Key Implementation Order:**
- Tasks 1-8: Foundation (can be executed immediately)
- Tasks 9-10: CLI deploy works end-to-end anonymously
- Task 11: Static file serving (makes deployed sites accessible)
- Tasks 12-13: Auth enables logged-in features
- Tasks 14-17: Full feature set
- Tasks 18-19: Polish and testing

**Tech Stack Reminders:**
- Database: SQLite with sqlx migrations
- Storage: Filesystem (S3-ready via trait)
- Server: Axum + Tower
- CLI: Clap + Reqwest
- All Rust, TDD throughout

---

## Execution Handoff

**Plan Status:** Foundation tasks (1-8) are fully specified with TDD steps. Remaining tasks (9-19) are outlined and ready for detailed planning during execution.

**Two execution options:**

**1. Subagent-Driven (recommended)** - Fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach would you like?
