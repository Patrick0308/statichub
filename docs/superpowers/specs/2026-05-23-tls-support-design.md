# TLS Support Design

**Date:** 2026-05-23
**Status:** Approved

## Goal

Add automatic HTTPS support to StaticHub server using Let's Encrypt with DNS-01 challenge. TLS will be optional (disabled by default) and configurable via environment variables. Supports wildcard certificates.

## Architecture

### Core Approach

- Use `axum-server` to provide TLS support for Axum applications
- Use `rustls-acme` to automatically handle Let's Encrypt certificate acquisition and renewal
- Use DNS-01 challenge for domain validation (supports wildcard certificates)
- DNS provider abstraction layer for different DNS APIs (starting with Cloudflare)

### Startup Flow

1. **Load Configuration**
   - Check `STATICHUB_TLS_ENABLED` environment variable
   - If disabled: use existing `axum::serve` with HTTP mode
   - If enabled: proceed with TLS initialization

2. **TLS Initialization** (when enabled)
   - Validate required configuration (email, DNS provider, API token)
   - Initialize DNS provider (based on `STATICHUB_DNS_PROVIDER`)
   - Extract domains from `STATICHUB_ALLOWED_DOMAINS`:
     - `statichub.dev` → request `*.statichub.dev` wildcard certificate
     - `api.example.com` → request `api.example.com` specific certificate
     - Filter out localhost and local addresses
   - Initialize `rustls-acme` state (certificate directory, contact email, domain list, DNS solver)
   - Use `axum-server` with TLS acceptor to listen on configured port (default 443)
   - Background task automatically checks certificate expiration and renews

3. **Certificate Acquisition**
   - If certificates don't exist: acquire from Let's Encrypt
   - If acquisition fails: server startup fails with clear error message
   - Background renewal task runs every 12 hours

### Certificate Storage

- **Directory:** `{STATICHUB_CERT_DIR}` (default: `./var/statichub/certs`)
- `rustls-acme` automatically manages certificate files
- Certificates cached on disk, survive restarts

### DNS Provider Support

**Phase 1:** Cloudflare only

**Future:** Can extend to support:
- Aliyun DNS
- DNSPod
- Route53
- etc.

## Components

### 1. TLS Configuration Module (`server/src/tls.rs`)

**Responsibilities:**
- Parse TLS-related environment variables
- Provide `TlsConfig` struct
- Validate configuration completeness

**Key Types:**
```rust
pub struct TlsConfig {
    pub enabled: bool,
    pub port: u16,
    pub email: String,
    pub cert_dir: PathBuf,
    pub dns_provider: DnsProvider,
    pub dns_api_token: String,
    pub acme_directory: AcmeDirectory,
    pub domains: Vec<String>,
}

pub enum DnsProvider {
    Cloudflare,
}

pub enum AcmeDirectory {
    Staging,
    Production,
}

impl TlsConfig {
    pub fn from_env(allowed_domains: &[String]) -> Result<Option<Self>>;
    fn extract_certificate_domains(allowed_domains: &[String]) -> Vec<String>;
}
```

**Domain Extraction Logic:**
- Input: `["statichub.dev", "api.example.com", "localhost"]`
- Output: `["*.statichub.dev", "api.example.com"]`
- Filter out: localhost, 127.0.0.1, local addresses

### 2. DNS Solver Module (`server/src/tls/dns_solver.rs`)

**Responsibilities:**
- Abstract DNS API operations
- Implement DNS-01 challenge record management

**Key Types:**
```rust
#[async_trait]
pub trait DnsSolver: Send + Sync {
    async fn set_txt_record(&self, domain: &str, value: &str) -> Result<()>;
    async fn delete_txt_record(&self, domain: &str, value: &str) -> Result<()>;
}

pub struct CloudflareSolver {
    api_token: String,
    client: reqwest::Client,
}

impl CloudflareSolver {
    pub fn new(api_token: String) -> Self;
    async fn get_zone_id(&self, domain: &str) -> Result<String>;
}
```

**Cloudflare API Operations:**
- Get zone ID from domain name
- Create TXT record: `_acme-challenge.{domain}` with token value
- Delete TXT record after validation

### 3. Certificate Manager (`server/src/tls/manager.rs`)

**Responsibilities:**
- Integrate `rustls-acme`
- Manage certificate acquisition and renewal
- Provide TLS acceptor for axum-server

**Key Types:**
```rust
pub struct CertificateManager {
    state: AcmeState,
}

impl CertificateManager {
    pub async fn new(config: TlsConfig, dns_solver: Arc<dyn DnsSolver>) -> Result<Self>;
    pub fn acceptor(&self) -> AxumAcceptor;
}
```

**Integration with rustls-acme:**
- Configure ACME directory URL (staging or production)
- Set contact email
- Register DNS solver for challenge handling
- Configure certificate cache directory
- Return TLS acceptor for axum-server

### 4. Server Launcher (`server/src/main.rs`)

**Modified Serve Function:**
```rust
async fn serve() -> anyhow::Result<()> {
    // ... existing setup ...

    let config = ServerConfig::from_env()?;

    // Check if TLS is enabled
    if let Some(tls_config) = TlsConfig::from_env(&config.allowed_domains)? {
        // TLS mode
        let dns_solver = create_dns_solver(&tls_config)?;
        let cert_manager = CertificateManager::new(tls_config, dns_solver).await?;

        let addr = SocketAddr::from(([0, 0, 0, 0], tls_config.port));
        tracing::info!("🚀 Server listening on {} (HTTPS)", addr);

        axum_server::bind_rustls(addr, tls_config.acceptor())
            .serve(app.into_make_service())
            .await?;
    } else {
        // HTTP mode (existing code)
        let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
        tracing::info!("🚀 Server listening on {} (HTTP)", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
    }

    Ok(())
}
```

### 5. CLI Commands (`server/src/cli.rs`)

**New TLS Subcommands:**
```rust
pub enum Commands {
    Serve { port: Option<u16> },
    Db { command: DbCommands },
    Tls { command: TlsCommands },  // NEW
}

pub enum TlsCommands {
    Renew,   // Manually trigger certificate renewal
    Status,  // Show certificate status
}
```

**Command Behavior:**
- `statichub-server tls renew`: Force renewal of all certificates
- `statichub-server tls status`: Display certificate info (domains, expiration dates)

## Configuration

### Environment Variables

```bash
# TLS Toggle
STATICHUB_TLS_ENABLED=true          # Default: false

# TLS Port
STATICHUB_TLS_PORT=443              # Default: 443 (overrides STATICHUB_PORT when TLS enabled)

# Let's Encrypt Configuration
STATICHUB_TLS_EMAIL=admin@example.com   # Required when TLS enabled
STATICHUB_ACME_DIRECTORY=staging        # Default: staging (options: staging, production)

# Certificate Storage
STATICHUB_CERT_DIR=./var/statichub/certs  # Default: ./var/statichub/certs

# DNS Provider
STATICHUB_DNS_PROVIDER=cloudflare   # Required when TLS enabled (options: cloudflare)
STATICHUB_DNS_API_TOKEN=xxx         # Required when TLS enabled
```

### Configuration Validation

**Required when TLS enabled:**
- `STATICHUB_TLS_EMAIL` must be set
- `STATICHUB_DNS_PROVIDER` must be set
- `STATICHUB_DNS_API_TOKEN` must be set

**Domain Validation:**
- `STATICHUB_ALLOWED_DOMAINS` cannot include localhost when TLS enabled (warning, not error)
- Must have at least one valid domain for certificate

**ACME Directory:**
- Default: `staging` (safe for testing, avoids production rate limits)
- Staging certificates are not trusted by browsers (test-only)
- Production should only be used after testing with staging

**Let's Encrypt Rate Limits:**
- Production: 50 certificates per domain per week
- Staging: Much more lenient
- Always test with staging first

## Error Handling

### Startup Errors (Fail Fast)

1. **Missing Required Configuration**
   ```
   ❌ TLS configuration error: STATICHUB_TLS_EMAIL is required when TLS is enabled
   💡 Set STATICHUB_TLS_EMAIL=your@email.com
   ```
   - Exit code: 1

2. **DNS API Authentication Failed**
   ```
   ❌ DNS API authentication failed (Cloudflare)
   💡 Check your STATICHUB_DNS_API_TOKEN is valid
   💡 Test with: curl -H "Authorization: Bearer $TOKEN" https://api.cloudflare.com/client/v4/user/tokens/verify
   ```
   - Exit code: 1

3. **Certificate Acquisition Failed**
   ```
   ❌ Failed to acquire TLS certificate for *.statichub.dev
   💡 Check DNS records can be updated: _acme-challenge.statichub.dev
   💡 Verify DNS API token has zone edit permissions
   💡 Check logs for detailed error
   ```
   - Exit code: 1
   - Server does not start (TLS enabled means HTTPS required)

### Runtime Errors (Graceful Degradation)

1. **Certificate Renewal Failed**
   - Log error with details
   - Continue using existing certificate (30-day renewal window)
   - Retry every hour
   - Alert via logs if certificate will expire in < 7 days

2. **DNS API Temporary Failure**
   - Log error
   - Retry with exponential backoff
   - If renewal continues to fail, alert in logs

### User-Friendly Error Messages

All errors should include:
- ❌ What went wrong
- 💡 How to fix it
- Example commands or verification steps

## Testing

### Unit Tests

**1. TLS Configuration (`tls.rs`)**
- ✅ Parse environment variables correctly
- ✅ Validate required fields (fail when missing)
- ✅ Domain extraction logic (filter localhost, convert to wildcard)
- ✅ ACME directory parsing (staging/production)

**2. DNS Solver (`dns_solver.rs`)**
- ✅ Mock Cloudflare API responses
- ✅ Test TXT record creation/deletion
- ✅ Test error handling (authentication failure, API errors)
- ✅ Test zone ID lookup

### Integration Tests

**1. Staging Environment Test**
- Requires real domain and Cloudflare token
- Tests complete certificate acquisition flow
- Skip in CI (needs external credentials)
- Document in README for manual testing

**2. HTTP Mode Test**
- ✅ Verify TLS disabled mode works unchanged
- ✅ Ensure no regression in existing functionality

### Manual Testing Checklist

```bash
# 1. Test with staging (safe, won't hit rate limits)
STATICHUB_TLS_ENABLED=true \
STATICHUB_ACME_DIRECTORY=staging \
STATICHUB_TLS_EMAIL=test@example.com \
STATICHUB_DNS_PROVIDER=cloudflare \
STATICHUB_DNS_API_TOKEN=xxx \
STATICHUB_ALLOWED_DOMAINS=statichub.dev \
./target/release/statichub-server

# 2. Verify DNS challenge (check logs)
# Should see: Setting DNS TXT record _acme-challenge.statichub.dev

# 3. Verify certificate acquisition
# Should see: ✓ Certificate acquired for *.statichub.dev

# 4. Test HTTPS connection
curl -v https://test.statichub.dev:443

# 5. Verify certificate details
openssl s_client -connect statichub.dev:443 -servername statichub.dev

# 6. Test production (after staging works)
STATICHUB_ACME_DIRECTORY=production ...

# 7. Test manual renewal
./target/release/statichub-server tls renew

# 8. Test certificate status
./target/release/statichub-server tls status
```

## Implementation Notes

### Dependencies to Add

```toml
[dependencies]
# TLS support
axum-server = { version = "0.6", features = ["tls-rustls"] }
rustls-acme = "0.10"
rustls = "0.21"

# DNS provider
# (Cloudflare API client will use existing reqwest)
```

### Phase 1 Scope

**In Scope:**
- ✅ TLS configuration and validation
- ✅ Cloudflare DNS-01 solver
- ✅ Certificate acquisition and storage
- ✅ Automatic renewal (background task)
- ✅ Manual renewal command
- ✅ Certificate status command
- ✅ Staging/production environment support

**Out of Scope:**
- ❌ Other DNS providers (future)
- ❌ HTTP to HTTPS redirect (TLS enabled = HTTPS only)
- ❌ Custom certificate upload (use Let's Encrypt only)
- ❌ Certificate monitoring/alerting (log warnings only)

### Security Considerations

1. **DNS API Token Protection**
   - Never log API tokens
   - Store in environment variable only
   - Validate token on startup

2. **Certificate File Permissions**
   - Ensure cert directory has restricted permissions
   - Only server process should read certificates

3. **ACME Account Key**
   - `rustls-acme` manages account key automatically
   - Stored in certificate directory
   - Should persist across restarts

### Deployment Considerations

1. **First Deployment**
   - Start with staging environment
   - Verify DNS API works correctly
   - Check certificate acquisition succeeds
   - Switch to production

2. **Certificate Renewal Window**
   - Let's Encrypt certificates valid for 90 days
   - Renewal starts at 60 days (30-day buffer)
   - If renewal fails, 30 days to fix before expiration

3. **DNS Provider Downtime**
   - Renewal will fail if DNS API is down
   - Retry mechanism ensures eventual success
   - Monitor logs for renewal failures

4. **Rate Limiting**
   - Staging: test freely
   - Production: 50 certs/domain/week limit
   - Be cautious with production testing

## Success Criteria

1. ✅ TLS can be enabled via environment variable
2. ✅ Wildcard certificates work (`*.statichub.dev`)
3. ✅ Certificate automatically acquired on first start
4. ✅ Certificate automatically renewed before expiration
5. ✅ Clear error messages for configuration issues
6. ✅ Manual renewal command works
7. ✅ HTTP mode continues to work when TLS disabled
8. ✅ Staging environment protects from production rate limits
