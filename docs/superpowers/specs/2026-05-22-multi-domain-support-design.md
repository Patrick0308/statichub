# Multi-Domain Support for StaticHub

**Design Specification**
**Date:** 2026-05-22
**Status:** Draft

## Overview

Add multi-domain support to StaticHub, allowing a single server instance to be accessed through multiple domains while maintaining a single listening port. The server will dynamically generate deployment URLs based on the client's access domain.

## Problem Statement

Currently, StaticHub has the following limitations:

1. **Fixed BASE_URL**: Server uses a single `BASE_URL` environment variable, making it hard to serve multiple domains
2. **Hardcoded defaults**: Client defaults to `http://statichub.dev:3000`, which doesn't match production port 80
3. **Inflexible port configuration**: Changing the server port requires changing multiple configuration points

## Goals

1. Server supports multiple domains on a single port
2. Deployment URLs reflect the domain the client used to access the server
3. Simplified port configuration with automatic propagation
4. Client defaults updated for production use
5. Maintain backward compatibility where possible

## Design

### Configuration Model

#### Server Configuration

**Environment Variables:**

```bash
# Server listening port (default: 3000)
PORT=80

# Comma-separated list of allowed domains (default: localhost,statichub.dev)
ALLOWED_DOMAINS=statichub.dev,localhost,mycompany.com

# OAuth callback URL (existing, unchanged)
GOOGLE_REDIRECT_URL=http://statichub.dev/auth/callback/google
```

**Removed:**
- `BASE_URL` - No longer needed, replaced by dynamic host extraction

**New Configuration Structure:**

```rust
// server/src/config.rs
pub struct ServerConfig {
    pub port: u16,
    pub allowed_domains: Vec<String>,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .context("Invalid PORT value")?;

        let allowed_domains = std::env::var("ALLOWED_DOMAINS")
            .unwrap_or_else(|_| "localhost,statichub.dev".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Self { port, allowed_domains })
    }

    pub fn is_allowed(&self, domain: &str) -> bool {
        self.allowed_domains.iter().any(|d| d == domain)
    }
}
```

#### Client Configuration

**Default Server URL:**

Change from `http://statichub.dev:3000` to `http://statichub.dev`:

```rust
// cli/src/main.rs and cli/src/client.rs
let server_url = std::env::var("STATICHUB_SERVER")
    .unwrap_or_else(|_| "http://statichub.dev".to_string());
```

Users can still override with `STATICHUB_SERVER` environment variable for local development.

### Host Validation Middleware

A new middleware validates the `Host` header on incoming requests.

**File:** `server/src/middleware/host.rs`

**Functionality:**
1. Extract `Host` header from request
2. Parse domain and port from host string
3. Validate domain against `ALLOWED_DOMAINS`
4. Attach validated host info to request extensions for downstream use
5. Return 400 Bad Request if host is invalid or missing

**Request Extension:**

```rust
#[derive(Clone)]
pub struct RequestHost {
    pub domain: String,
    pub port: Option<u16>,
}

impl RequestHost {
    pub fn to_string(&self) -> String {
        match self.port {
            Some(port) => format!("{}:{}", self.domain, port),
            None => self.domain.clone(),
        }
    }
}
```

### Host Parsing Utilities

**File:** `server/src/config.rs`

```rust
/// Parse host header into domain and optional port
/// Examples:
///   "localhost:3000" -> ("localhost", Some(3000))
///   "statichub.dev" -> ("statichub.dev", None)
///   "statichub.dev:80" -> ("statichub.dev", Some(80))
pub fn parse_host(host: &str) -> Result<(String, Option<u16>)> {
    if let Some((domain, port_str)) = host.rsplit_once(':') {
        let port = port_str.parse::<u16>()
            .context("Invalid port in host header")?;
        Ok((domain.to_string(), Some(port)))
    } else {
        Ok((host.to_string(), None))
    }
}

/// Build host string from domain and optional port
pub fn build_host(domain: &str, port: Option<u16>) -> String {
    match port {
        Some(p) => format!("{}:{}", domain, p),
        None => domain.to_string(),
    }
}
```

### Dynamic URL Generation

#### Current Approach (Single Domain)

```rust
// Uses fixed BASE_URL
let url = format!("{}/{}", state.base_url, subdomain);
// Result: http://localhost:3000/my-app
```

#### New Approach (Multi-Domain)

```rust
// Extract host from request
let request_host = req.extensions()
    .get::<RequestHost>()
    .ok_or(Error::MissingHost)?;

// Build deployment URL using request's domain
let scheme = "http";  // TODO: Use "https" based on configuration
let url = format!("{}://{}.{}", scheme, subdomain, request_host.to_string());
// Result: http://my-app.statichub.dev:3000 (if accessed via statichub.dev:3000)
// Result: http://my-app.localhost:3000 (if accessed via localhost:3000)
```

### Impact on API Endpoints

The following endpoints generate URLs and need modification:

**Anonymous Deploy:**
- `POST /api/deploys/anonymous`
- Returns deployment URL based on request host

**Authenticated Deploy:**
- `POST /api/projects/{name}/deploys`
- Returns deployment URL based on request host

**Project List:**
- `GET /api/projects`
- Each project's URL reflects request host

**Project Details:**
- `GET /api/projects/{name}`
- Project URL reflects request host

### State Changes

**Before:**

```rust
pub struct DeployState {
    pub pool: SqlitePool,
    pub storage: Arc<dyn Storage>,
    pub base_url: String,  // Fixed URL
}
```

**After:**

```rust
pub struct DeployState {
    pub pool: SqlitePool,
    pub storage: Arc<dyn Storage>,
    // base_url removed - now derived from request
}
```

### Server Startup Changes

**File:** `server/src/main.rs`

**Before:**

```rust
let base_url = std::env::var("BASE_URL")
    .unwrap_or_else(|_| "http://localhost:3000".to_string());

let deploy_state = Arc::new(DeployState {
    pool: pool.clone(),
    storage: storage.clone(),
    base_url,
});

let addr = SocketAddr::from(([0, 0, 0, 0], 3000));  // Hardcoded port
```

**After:**

```rust
let config = ServerConfig::from_env()?;

let deploy_state = Arc::new(DeployState {
    pool: pool.clone(),
    storage: storage.clone(),
    // base_url removed
});

let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

// Add host validation middleware to router
let app = create_router(deploy_state, auth_state)
    .layer(HostValidationLayer::new(config));
```

## Implementation Details

### File Changes

#### New Files

1. **`server/src/config.rs`**
   - `ServerConfig` struct
   - Environment variable parsing
   - Host parsing utilities
   - Domain validation

2. **`server/src/middleware/host.rs`**
   - `HostValidationLayer` middleware
   - `RequestHost` extension type
   - Host extraction and validation logic

#### Modified Files

1. **`server/src/main.rs`**
   - Remove `BASE_URL` environment variable usage
   - Add `ServerConfig` initialization
   - Use `config.port` for server binding
   - Add host validation middleware to router

2. **`server/src/api/mod.rs`**
   - Update `DeployState` - remove `base_url` field
   - Export host middleware

3. **`server/src/middleware/mod.rs`**
   - Add `pub mod host;`

4. **`server/src/api/deploys.rs`**
   - Update `deploy_anonymous` handler
   - Update `deploy_authenticated` handler
   - Extract `RequestHost` from extensions
   - Generate URLs dynamically using request host

5. **`server/src/api/projects.rs`**
   - Update `list_projects` handler
   - Update `get_project` handler
   - Generate URLs dynamically using request host

6. **`server/src/error.rs`**
   - Add `InvalidHost` error variant
   - Add `DomainNotAllowed` error variant
   - Add `MissingHost` error variant

7. **`cli/src/main.rs`**
   - Update default `STATICHUB_SERVER` value
   - Change from `http://statichub.dev:3000` to `http://statichub.dev`

8. **`cli/src/client.rs`**
   - Update default server URL in `Client::new` usage
   - Ensure consistency across all server URL references

## Error Handling

### New Error Types

```rust
// server/src/error.rs
pub enum Error {
    // ... existing errors

    /// Host header is missing or malformed
    InvalidHost(String),

    /// Domain is not in the allowed list
    DomainNotAllowed(String),

    /// Host information not found in request (middleware issue)
    MissingHost,
}
```

### Error Responses

**Invalid or Missing Host:**
```http
HTTP/1.1 400 Bad Request
Content-Type: application/json

{
  "error": "invalid_host",
  "message": "Host header is required"
}
```

**Domain Not Allowed:**
```http
HTTP/1.1 403 Forbidden
Content-Type: application/json

{
  "error": "domain_not_allowed",
  "message": "Domain 'unknown.com' is not configured for this server"
}
```

## Testing Strategy

### Unit Tests

**Config Parsing:**
```rust
#[test]
fn test_parse_allowed_domains() {
    std::env::set_var("ALLOWED_DOMAINS", "localhost,example.com,test.dev");
    let config = ServerConfig::from_env().unwrap();
    assert_eq!(config.allowed_domains.len(), 3);
    assert!(config.is_allowed("localhost"));
    assert!(config.is_allowed("example.com"));
    assert!(!config.is_allowed("unknown.com"));
}
```

**Host Parsing:**
```rust
#[test]
fn test_parse_host() {
    assert_eq!(
        parse_host("localhost:3000").unwrap(),
        ("localhost".to_string(), Some(3000))
    );
    assert_eq!(
        parse_host("statichub.dev").unwrap(),
        ("statichub.dev".to_string(), None)
    );
}
```

### Integration Tests

**Multi-Domain Deploy:**
```rust
#[tokio::test]
async fn test_deploy_returns_correct_domain() {
    // Deploy via localhost:3000
    let response = client
        .post("http://localhost:3000/api/deploys/anonymous")
        .header("Host", "localhost:3000")
        .send()
        .await?;

    let deploy: DeployResponse = response.json().await?;
    assert!(deploy.url.contains("localhost:3000"));

    // Deploy via statichub.dev
    let response = client
        .post("http://localhost:3000/api/deploys/anonymous")
        .header("Host", "statichub.dev")
        .send()
        .await?;

    let deploy: DeployResponse = response.json().await?;
    assert!(deploy.url.contains("statichub.dev"));
}
```

**Domain Validation:**
```rust
#[tokio::test]
async fn test_reject_unallowed_domain() {
    let response = client
        .post("http://localhost:3000/api/deploys/anonymous")
        .header("Host", "malicious.com")
        .send()
        .await?;

    assert_eq!(response.status(), 403);
}
```

## Migration Guide

### For Existing Deployments

**Before:**
```bash
BASE_URL=http://example.com:8080
GOOGLE_REDIRECT_URL=http://example.com:8080/auth/callback/google
```

**After:**
```bash
PORT=8080
ALLOWED_DOMAINS=example.com
GOOGLE_REDIRECT_URL=http://example.com:8080/auth/callback/google
```

### For Local Development

**Before:**
```bash
# Server
BASE_URL=http://localhost:3000

# Client
STATICHUB_SERVER=http://localhost:3000
```

**After:**
```bash
# Server (defaults work fine)
# PORT=3000 (default)
# ALLOWED_DOMAINS=localhost,statichub.dev (default)

# Client
STATICHUB_SERVER=http://localhost:3000  # Still need this for local dev
```

## Default Behavior

If no environment variables are set:

**Server:**
- Listens on `0.0.0.0:3000`
- Allows domains: `localhost`, `statichub.dev`

**Client:**
- Connects to `http://statichub.dev` by default
- Override with `STATICHUB_SERVER=http://localhost:3000` for local development

## Security Considerations

### Domain Validation

- All incoming requests must have a valid `Host` header
- Only domains in `ALLOWED_DOMAINS` are accepted
- Prevents host header injection attacks
- Prevents unauthorized domain usage

### OAuth Callback

- OAuth callback URL remains fixed via `GOOGLE_REDIRECT_URL`
- Only one callback needs to be configured in Google Console
- No dynamic callback URLs (security risk)

### Port Exposure

- Server binds to `0.0.0.0` (all interfaces) but port is configurable
- Use firewall rules to restrict access as needed
- Consider using reverse proxy (nginx/caddy) in production

## Future Enhancements

### HTTPS Detection

Automatically use `https://` scheme when:
- Request came through TLS termination proxy
- Checking `X-Forwarded-Proto` header
- Or explicit configuration

### Domain-Specific Configuration

Support different configurations per domain:
```yaml
domains:
  statichub.dev:
    analytics: enabled
  localhost:
    analytics: disabled
```

### Wildcard Domain Support

Support patterns in `ALLOWED_DOMAINS`:
```bash
ALLOWED_DOMAINS=localhost,*.statichub.dev,*.example.com
```

## Success Criteria

1. Server accepts requests from multiple configured domains
2. Deployment URLs match the domain used to access the server
3. Client works with updated default URL
4. Port configuration works correctly
5. Unauthorized domains are rejected
6. OAuth flow continues to work
7. All existing tests pass
8. New tests cover multi-domain scenarios

## Implementation Checklist

- [ ] Create `server/src/config.rs` with `ServerConfig`
- [ ] Create `server/src/middleware/host.rs` with validation
- [ ] Update `server/src/main.rs` for new configuration
- [ ] Update `server/src/api/mod.rs` to use middleware
- [ ] Update `server/src/api/deploys.rs` for dynamic URLs
- [ ] Update `server/src/api/projects.rs` for dynamic URLs
- [ ] Add error types to `server/src/error.rs`
- [ ] Update `cli/src/main.rs` default URL
- [ ] Update `cli/src/client.rs` default URL
- [ ] Write unit tests for config and parsing
- [ ] Write integration tests for multi-domain deploys
- [ ] Update documentation
- [ ] Update `.env.example` files
