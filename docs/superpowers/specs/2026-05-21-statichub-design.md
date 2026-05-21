# StaticHub: Static Web Publishing Platform for Frontend Developers

**Design Specification**
**Date:** 2026-05-21
**Status:** Draft

## Overview

StaticHub is a static web publishing platform inspired by Surge and GitHub Pages, designed to give frontend developers a friction-free deployment experience. It supports both anonymous quick deploys and authenticated project management with custom domains.

### Core Principles

- **CLI-first**: Primary interface is a command-line tool
- **Zero friction for getting started**: Deploy without creating an account
- **Progressive enhancement**: Login unlocks advanced features
- **Developer-friendly**: Simple config, familiar workflows
- **Rust-powered**: Fast, safe, single-binary distribution

## Architecture

### System Components

The system consists of two main components:

1. **CLI Client (`statichub`)** - Rust binary installed on developer machines
   - Handles authentication flows
   - Prepares and uploads deployments
   - Manages project configuration
   - Stores credentials locally

2. **Server (`statichub-server`)** - Rust binary deployed on infrastructure
   - REST API for CLI communication
   - OAuth integration (Google for MVP)
   - Static file HTTP server
   - Storage management (filesystem initially, S3-ready)
   - Database for metadata and relationships

### Architecture Pattern

**Monolithic server + thin CLI client** approach provides:
- Clean separation of concerns
- Easy future web dashboard addition
- Flexible deployment options
- Standard patterns for contributors

### Storage Abstraction

Storage layer uses trait-based design for easy migration from filesystem to object storage:

```rust
trait Storage: Send + Sync {
    async fn store_deploy(&self, deploy_id: &str, tarball: &[u8]) -> Result<DeployMetadata>;
    async fn get_file(&self, deploy_id: &str, path: &str) -> Result<Vec<u8>>;
    async fn delete_deploy(&self, deploy_id: &str) -> Result<()>;
    async fn list_files(&self, deploy_id: &str) -> Result<Vec<FileInfo>>;
}
```

**MVP Implementation:** `FilesystemStorage` stores files in `/var/statichub/deploys/`
**Future:** `S3Storage` for scalability and CDN integration

## Data Model

### Users Table
```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    oauth_provider TEXT NOT NULL,  -- 'google' (github later)
    oauth_id TEXT NOT NULL,        -- Provider's user ID
    email TEXT NOT NULL,
    username TEXT NOT NULL,         -- For subdomain generation
    created_at TIMESTAMP NOT NULL,
    UNIQUE(oauth_provider, oauth_id)
);
```

### Projects Table
```sql
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    owner_id INTEGER,              -- NULL for anonymous projects
    name TEXT NOT NULL UNIQUE,     -- Subdomain slug
    subdomain TEXT NOT NULL,       -- Full subdomain (name.statichub.io)
    is_anonymous BOOLEAN NOT NULL DEFAULT 0,
    current_deploy_id INTEGER,     -- Active deployment
    config TEXT,                   -- JSON serialized project config
    last_deployed_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES users(id)
);
```

### Deploys Table
```sql
CREATE TABLE deploys (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    version INTEGER NOT NULL,      -- Auto-increment per project
    storage_path TEXT NOT NULL,    -- Where files are stored
    status TEXT NOT NULL,          -- 'uploading', 'ready', 'failed'
    file_count INTEGER NOT NULL,
    total_size_bytes INTEGER NOT NULL,
    deployed_at TIMESTAMP NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id),
    UNIQUE(project_id, version)
);
```

### Custom Domains Table
```sql
CREATE TABLE custom_domains (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    domain TEXT NOT NULL UNIQUE,   -- e.g., 'www.example.com'
    verified BOOLEAN NOT NULL DEFAULT 0,
    verification_token TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    verified_at TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id)
);
```

### Deploy Tokens Table
```sql
CREATE TABLE deploy_tokens (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    token_hash TEXT NOT NULL,      -- bcrypt hashed
    name TEXT NOT NULL,            -- User-friendly label
    last_used_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id)
);
```

### Data Retention

- **Logged-in projects**: Keep last 10 deploys per project (configurable)
- **Anonymous projects**: Delete after 30 days of inactivity
- **Old deploys**: Auto-cleanup when exceeding version limit

## User Flows

### Anonymous Deploy (No Login)

**Use Case:** Quick throwaway deployments, testing, sharing prototypes

**Flow:**
1. User runs `statichub deploy ./dist`
2. CLI creates gzipped tarball of files
3. CLI uploads to server (no auth required)
4. Server generates random subdomain (e.g., `x7k2m9.statichub.io`)
5. Server stores files and creates anonymous project
6. CLI displays:
   ```
   ✓ Deployed to https://x7k2m9.statichub.io

   This is a temporary deployment with a random URL.
   Login to get custom names, domains, and rollback: statichub login
   ```

**Limitations:**
- Random subdomain only
- No rollback capability
- No custom domains
- No deploy history
- Auto-deleted after 30 days of inactivity

### Logged-In Deploy

**Use Case:** Production sites, projects needing management features

**Flow:**
1. User runs `statichub login` (first time only)
2. OAuth flow with Google (opens browser)
3. CLI stores JWT token locally
4. User runs `statichub deploy ./dist --name my-app`
5. Server creates/updates owned project
6. CLI displays:
   ```
   ✓ Deployed to https://my-app.statichub.io
   Version: 3
   Previous versions: 1, 2
   ```

**Features Unlocked:**
- Named subdomains (`my-app.statichub.io`)
- Deploy history (last 10 versions)
- Rollback support
- Custom domains
- Deploy tokens for CI/CD
- Permanent storage (not auto-deleted)

### Authentication Flow (Google OAuth)

**CLI Login Process:**

1. User runs `statichub login`
2. CLI generates random `session_id` (UUID)
3. CLI calls `POST /auth/login/google` with `session_id`
4. Server returns Google OAuth URL
5. CLI opens browser to OAuth URL
6. User authorizes with Google account
7. Google redirects to `/auth/callback/google?code=...&state={session_id}`
8. Server exchanges code for user info (email, name, Google ID)
9. Server creates/updates user record
10. Server generates JWT token (7-day expiry)
11. Server stores token mapped to `session_id` temporarily (5 min TTL)
12. CLI polls `GET /auth/status/{session_id}` until token ready
13. CLI saves token to `~/.statichub/credentials.json`
14. Browser shows: "Authentication successful! Return to your terminal."

**Token Storage:**
```json
{
  "access_token": "eyJhbGc...",
  "expires_at": "2026-05-28T10:30:00Z"
}
```

### Custom Domain Setup

**Flow:**

1. **Add Domain:**
   ```bash
   statichub domain add www.example.com
   ```
   Server generates verification token and returns:
   ```
   Add this TXT record to verify ownership:

   Host: _statichub-challenge.www.example.com
   Type: TXT
   Value: statichub-verify=abc123xyz

   Then run: statichub domain verify www.example.com
   ```

2. **Verify Domain:**
   ```bash
   statichub domain verify www.example.com
   ```
   - Server performs DNS TXT lookup
   - If token matches, marks domain as verified
   - Returns CNAME instructions:
   ```
   ✓ Domain verified!

   Add this record to point your domain:

   Host: www.example.com
   Type: CNAME
   Value: domains.statichub.io
   ```

3. **Automatic SSL:**
   - Server auto-provisions Let's Encrypt certificate when verified
   - Auto-renews before expiration
   - HTTPS enforced (HTTP redirects to HTTPS)

**Limits:**
- Anonymous projects: 0 custom domains
- Logged-in users: 5 domains per project

## CLI Commands

### Core Commands

```bash
# Authentication
statichub login                    # Google OAuth via browser
statichub logout                   # Clear local credentials

# Deployment
statichub deploy [directory]       # Deploy (anonymous if not logged in)
statichub deploy --name my-app     # Named deploy (requires login)
statichub deploy --message "msg"   # Add deploy message

# Project Management (requires login)
statichub list                     # List owned projects
statichub info [project]           # Show project details & deploy history
statichub rollback [project] [ver] # Rollback to specific version
statichub delete [project]         # Delete project (with confirmation)

# Custom Domains (requires login)
statichub domain add <domain>      # Add custom domain
statichub domain verify <domain>   # Verify domain ownership
statichub domain list              # List domains for current project
statichub domain remove <domain>   # Remove custom domain

# Deploy Tokens (requires login)
statichub token create [name]      # Create deploy token for CI/CD
statichub token list               # List tokens for current project
statichub token revoke <id>        # Revoke token
```

### Configuration File

**Location:** `statichub.yaml` or `statichub.yml` in deploy directory

**Example:**
```yaml
name: my-project
clean_urls: true
spa: false

# Cache static assets aggressively
headers:
  "/*.js":
    cache-control: public, max-age=31536000, immutable
  "/*.css":
    cache-control: public, max-age=31536000, immutable
  "/*.woff2":
    cache-control: public, max-age=31536000, immutable

# Redirects with wildcard support
redirects:
  - from: /old-path
    to: /new-path
    status: 301
  - from: /blog/*
    to: /articles/:splat
    status: 301
```

**CLI Flag Overrides:**
```bash
statichub deploy --spa --name my-app  # Overrides spa: false in config
```

## Server API

### REST Endpoints

**Authentication:**
```
POST   /auth/login/google              # Initiate OAuth flow
GET    /auth/callback/google           # OAuth callback
GET    /auth/status/{session_id}       # Poll for token (CLI)
POST   /auth/logout                    # Invalidate token
```

**Projects:**
```
GET    /api/projects                   # List user's projects
POST   /api/projects                   # Create new project
GET    /api/projects/{name}            # Get project details
DELETE /api/projects/{name}            # Delete project
```

**Deploys:**
```
GET    /api/projects/{name}/deploys                    # List deploys
POST   /api/projects/{name}/deploys                    # Create deploy
POST   /api/projects/{name}/deploys/{version}/rollback # Rollback
DELETE /api/projects/{name}/deploys/{version}          # Delete deploy
POST   /api/deploys/anonymous                          # Anonymous deploy
```

**Custom Domains:**
```
GET    /api/projects/{name}/domains          # List domains
POST   /api/projects/{name}/domains          # Add domain
GET    /api/projects/{name}/domains/{domain}/verify  # Verify domain
DELETE /api/projects/{name}/domains/{domain} # Remove domain
```

**Deploy Tokens:**
```
GET    /api/projects/{name}/tokens     # List tokens
POST   /api/projects/{name}/tokens     # Create token
DELETE /api/projects/{name}/tokens/{id} # Revoke token
```

### Authentication Methods

**User Auth:** Bearer token in `Authorization: Bearer {jwt}` header
**Deploy Token Auth:** `X-Deploy-Token: {token}` header (for CI/CD)

## Static File Serving

### Hostname-Based Routing

```rust
match request.host() {
    "api.statichub.io" => handle_api_request(),
    subdomain if subdomain.ends_with(".statichub.io") => serve_static_files(),
    custom_domain => lookup_and_serve_custom_domain(),
}
```

### File Resolution Logic

For each request:
1. Extract subdomain/domain → lookup project
2. Get current active deploy (or specific version for rollback preview)
3. Resolve file path based on project config:
   - **Exact match**: serve file directly
   - **Clean URLs**: `/about` → try `/about.html`, then `/about/index.html`
   - **SPA mode**: 404s → serve `/index.html` with 200 status
4. Apply custom headers from config
5. Apply redirects if path matches rules

### Response Features

- Content-Type detection via `mime_guess`
- Gzip/Brotli compression (on-the-fly via tower-http)
- ETag generation for caching
- Range request support for large files
- Custom headers from `statichub.yaml`

### Default Headers

```
X-Served-By: StaticHub
Cache-Control: public, max-age=0, must-revalidate
```

User config overrides defaults.

### Redirect Rules

Supports wildcards and placeholders:

```yaml
redirects:
  - from: /old-path
    to: /new-path
    status: 301
  - from: /blog/*
    to: /articles/:splat  # :splat captures remaining path
    status: 302
```

## Deploy Process

### CLI Upload Flow

1. **Preparation:**
   - Read `statichub.yaml` if exists
   - Merge with CLI flags
   - Validate files (size limits, path safety)
   - Calculate file hashes for integrity

2. **Compression:**
   - Create tarball in memory
   - Gzip compress
   - Stream to server with progress bar

3. **Upload:**
   - POST to `/api/projects/{name}/deploys` or `/api/deploys/anonymous`
   - Include auth token (if logged in)
   - Stream tarball as request body

### Server Processing

1. **Receive:**
   - Stream tarball to disk (don't load in memory)
   - Validate size limits (500MB max)

2. **Extract:**
   - Extract to temporary directory `/tmp/deploy-{uuid}`
   - Security checks:
     - No path traversal (`..` in paths)
     - No hidden files (`.env`, `.git`)
     - No executables or server scripts
   - Limit file count (10,000 max)

3. **Store:**
   - Atomically move to `/var/statichub/deploys/{project}/deploy-{version}`
   - Update database:
     - Create deploy record
     - Increment version number
     - Update project's `current_deploy_id`
   - Delete old deploys if exceeding retention limit

4. **Response:**
   - Return deploy metadata (URL, version, file count)

### Storage Layout (Filesystem)

```
/var/statichub/deploys/
  ├── my-app/
  │   ├── deploy-1/
  │   │   ├── index.html
  │   │   ├── assets/
  │   │   │   ├── app.js
  │   │   │   └── styles.css
  │   │   └── favicon.ico
  │   ├── deploy-2/
  │   └── deploy-3/
  ├── x7k2m9/  # anonymous project
  │   └── deploy-1/
```

### Rollback Process

- Update `projects.current_deploy_id` to previous deploy
- No file movement needed
- Instant operation

## Security

### Authentication Security

- JWT tokens with 7-day expiration
- Tokens refresh on activity
- Deploy tokens: bcrypt hashed, never returned after creation
- HTTPS enforced everywhere (HTTP → HTTPS redirect)

### Upload Security

**Size Limits:**
- Max file size: 100MB per file
- Max total size: 500MB per deploy
- Max file count: 10,000 files

**Content Validation:**
- Reject executables (`.exe`, `.sh`, `.bin`)
- Reject server-side scripts (`.php`, `.py`, `.rb`)
- Path traversal prevention
- Tarball bomb protection

### Static Serving Security

- No directory listing
- Block hidden files (`.env`, `.git`, `.htaccess`)
- Configurable CORS headers
- CSP header support via config
- XSS protection headers

### Rate Limiting

**Per-IP Limits:**
- Anonymous deploys: 10/hour
- API requests: 100/hour

**Per-User Limits:**
- Deploys: 1,000/day
- Domain additions: 10/day

Implementation: `tower-governor` middleware

### Domain Security

- DNS TXT verification required
- Auto-remove unverified domains after 7 days
- Let's Encrypt SSL auto-provisioning
- Certificate auto-renewal

## Error Handling

### CLI Errors

**Network Failures:**
- Retry with exponential backoff (3 attempts)
- Clear error messages with recovery guidance

**Auth Errors:**
- Detect expired tokens
- Prompt to run `statichub login`

**File Errors:**
- Validate before upload
- Report specific issues (file too large, invalid path)

### Server Errors

**Standard JSON Format:**
```json
{
  "error": "project_name_taken",
  "message": "Project name 'my-app' is already taken",
  "code": 409
}
```

**HTTP Status Codes:**
- 400: Bad request (validation errors)
- 401: Unauthorized (missing/invalid token)
- 403: Forbidden (insufficient permissions)
- 404: Not found
- 409: Conflict (name taken, etc.)
- 429: Rate limited
- 500: Internal server error

### Input Validation

- Project names: lowercase alphanumeric + hyphens only, 3-63 chars
- Domains: valid domain format
- Paths: no `..`, no absolute paths
- Sanitize all user input

## Technology Stack

### CLI (`statichub`)

```toml
[dependencies]
# CLI framework
clap = { version = "4", features = ["derive"] }

# HTTP client
reqwest = { version = "0.11", features = ["json", "stream"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# Compression
flate2 = "1"  # gzip
tar = "0.4"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Terminal UI
indicatif = "0.17"  # progress bars
colored = "2"        # colored output

# Error handling
anyhow = "1"
thiserror = "1"
```

### Server (`statichub-server`)

```toml
[dependencies]
# Web framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "compression", "fs"] }

# Async runtime
tokio = { version = "1", features = ["full"] }

# Database
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls"] }

# Auth
oauth2 = "4"       # OAuth flows
jsonwebtoken = "9" # JWT tokens
bcrypt = "0.15"    # token hashing

# Storage abstraction
async-trait = "0.1"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# HTTP utilities
mime_guess = "2"   # content-type detection
reqwest = "0.11"   # DNS verification, OAuth

# SSL/TLS
rustls = "0.21"
acme2 = "0.5"      # Let's Encrypt

# Rate limiting
tower-governor = "0.1"

# Error handling
anyhow = "1"
thiserror = "1"

# Utilities
uuid = { version = "1", features = ["v4"] }
chrono = "0.4"
regex = "1"
```

### Database

**MVP:** SQLite
- Embedded, zero configuration
- Perfect for getting started
- Single-file database

**Production:** PostgreSQL migration path
- Better concurrency
- Proven scalability
- Use `sqlx` migrations for schema versioning

### Build & Distribution

- Single binary compilation (no runtime dependencies)
- Cross-compile for Linux, macOS, Windows
- Release via GitHub Releases with pre-built binaries
- Optional: Homebrew formula for macOS (`brew install statichub`)

## Testing Strategy

### CLI Testing

**Unit Tests:**
- Config parsing (YAML, flag merging)
- Tarball creation and compression
- Token storage/retrieval
- Path validation

**Integration Tests:**
- Mock server for API interactions
- Auth flow with mock OAuth
- Deploy workflow end-to-end
- Error handling scenarios

### Server Testing

**Unit Tests:**
- Storage trait implementations
- File path resolution (clean URLs, SPA)
- Redirect rule matching
- Header merging
- Input validation

**Integration Tests:**
- Full API endpoints with test database
- OAuth callback handling (mock Google)
- Deploy upload and extraction
- Rollback functionality
- Custom domain verification (mock DNS)

**End-to-End Tests:**
- Real CLI → Real server (local)
- Anonymous deploy flow
- Login → named deploy flow
- Custom domain setup
- Rollback and deletion

### Test Infrastructure

- `cargo test` for all Rust tests
- Mock HTTP with `wiremock` or `mockito`
- Test database: in-memory SQLite
- Temporary directories for storage tests
- CI/CD: GitHub Actions running tests on every PR

## MVP Scope

### Must-Have Features

**For Users:**
- Anonymous deploy (random subdomain)
- Login with Google OAuth
- Named deploys (custom subdomain)
- Deploy history (last 10 versions)
- Rollback to previous versions
- Custom domain support with SSL
- Deploy tokens for CI/CD

**For Operations:**
- Filesystem storage
- SQLite database
- Basic rate limiting
- Auto-cleanup of old deploys
- Error logging

### Explicitly Out of Scope (MVP)

- GitHub OAuth (Google only for MVP)
- S3 storage (filesystem only initially)
- Analytics/usage stats
- Team/organization support
- Preview URLs for PRs
- Deploy hooks/notifications
- Web dashboard (CLI only)
- Database migration from anonymous to owned projects

### Future Enhancements

**Phase 2:**
- GitHub OAuth provider
- S3/R2 storage backend
- Preview deployments
- Deploy webhooks

**Phase 3:**
- Team/organization support
- Usage analytics
- Web dashboard
- Database caching layer
- Multi-region support

## Open Questions

None - design is complete and approved.

## Success Criteria

**MVP is successful if:**
1. Users can deploy anonymously in < 30 seconds
2. Named deploys work reliably for logged-in users
3. Custom domains can be added and verified
4. Rollback works without downtime
5. Deploy tokens enable CI/CD workflows
6. Zero security incidents in first 3 months
7. 95%+ uptime for static file serving

## Next Steps

1. Create implementation plan (via writing-plans skill)
2. Set up project structure (workspace with CLI + server)
3. Implement core data models and database schema
4. Build CLI with deploy command (anonymous flow first)
5. Build server API and static file serving
6. Add Google OAuth
7. Add custom domain support
8. Testing and security hardening
9. Documentation and examples
10. Beta release
