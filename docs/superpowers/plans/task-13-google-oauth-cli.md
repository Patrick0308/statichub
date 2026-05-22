# Task 13: Google OAuth (CLI)

## Goal

Implement the CLI side of Google OAuth authentication. This completes the login flow by adding the `statichub login` command that opens the browser, polls the server for the JWT token, and saves credentials locally.

## Files

- Modify: `cli/src/main.rs`
- Modify: `cli/src/auth.rs`
- Modify: `cli/src/client.rs`
- Modify: `cli/Cargo.toml`

## Implementation Steps

### Step 1: Add dependencies

Add to `cli/Cargo.toml`:

```toml
uuid = { version = "1", features = ["v4"] }
```

(tokio and serde_json should already be present)

### Step 2: Update auth module for login flow

Modify `cli/src/auth.rs`:

Add response types and login function:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    pub auth_url: String,
}

#[derive(Debug, Deserialize)]
pub struct StatusResponse {
    pub token: Option<String>,
}

// Existing Credentials struct and functions remain...

pub fn generate_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
```

### Step 3: Add OAuth methods to Client

Modify `cli/src/client.rs`:

```rust
use crate::auth::{LoginRequest, LoginResponse, StatusResponse};

impl Client {
    // Existing deploy_anonymous method...

    pub async fn initiate_login(&self, session_id: &str) -> Result<LoginResponse> {
        let url = format!("{}/auth/login/google", self.base_url);

        let response = self.client
            .post(&url)
            .json(&LoginRequest {
                session_id: session_id.to_string(),
            })
            .send()
            .await
            .context("Failed to initiate login")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Login initiation failed with status {}: {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse login response")
    }

    pub async fn poll_auth_status(&self, session_id: &str) -> Result<StatusResponse> {
        let url = format!("{}/auth/status/{}", self.base_url, session_id);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to poll auth status")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Auth status check failed with status {}: {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse status response")
    }
}
```

### Step 4: Implement login command

Modify `cli/src/main.rs`:

In the `Commands::Login` match arm:

```rust
Commands::Login => {
    let server_url = std::env::var("STATICHUB_SERVER")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    println!("🔐 Logging in to StaticHub...");

    // Generate session ID
    let session_id = auth::generate_session_id();

    // Initiate login
    let client = client::Client::new(server_url.clone());
    let login_response = client.initiate_login(&session_id).await?;

    // Open browser
    println!("📱 Opening browser for authentication...");
    println!("   If the browser doesn't open, visit: {}", login_response.auth_url);

    if let Err(e) = open::that(&login_response.auth_url) {
        println!("   ⚠️  Could not open browser automatically: {}", e);
        println!("   Please open the URL manually in your browser.");
    }

    // Poll for token
    println!("⏳ Waiting for authentication...");
    let mut attempts = 0;
    let max_attempts = 60; // 5 minutes (60 * 5 seconds)

    let token = loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        attempts += 1;

        let status = client.poll_auth_status(&session_id).await?;

        if let Some(token) = status.token {
            break token;
        }

        if attempts >= max_attempts {
            anyhow::bail!("Authentication timed out. Please try again.");
        }

        // Show progress
        if attempts % 6 == 0 {
            println!("   Still waiting... ({}/5 minutes)", attempts / 12);
        }
    };

    // Save credentials
    auth::save_credentials(&token)?;

    println!("✅ Login successful!");
    println!("   Credentials saved to ~/.statichub/credentials.json");
}
```

### Step 5: Add open dependency

Add to `cli/Cargo.toml`:

```toml
open = "5"
```

### Step 6: Implement logout command

Modify `cli/src/main.rs`:

In the `Commands::Logout` match arm:

```rust
Commands::Logout => {
    match auth::load_credentials() {
        Ok(_) => {
            auth::clear_credentials()?;
            println!("✅ Logged out successfully");
            println!("   Credentials removed from ~/.statichub/credentials.json");
        }
        Err(_) => {
            println!("ℹ️  Not logged in");
        }
    }
}
```

Update `auth.rs` to add `clear_credentials`:

```rust
pub fn clear_credentials() -> Result<()> {
    let creds_path = credentials_path()?;

    if creds_path.exists() {
        std::fs::remove_file(&creds_path)
            .with_context(|| format!("Failed to remove credentials at {}", creds_path.display()))?;
    }

    Ok(())
}
```

### Step 7: Write integration test

This is tricky because it requires a running server. Instead, write a unit test for the client methods.

Add to `cli/src/client.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = Client::new("http://localhost:3000".to_string());
        assert_eq!(client.base_url, "http://localhost:3000");
    }

    #[test]
    fn test_session_id_generation() {
        let session_id = crate::auth::generate_session_id();
        assert_eq!(session_id.len(), 36); // UUID v4 format
    }
}
```

### Step 8: Test credentials functions

Add to `cli/src/auth.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_credentials_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");

        // Mock credentials path
        std::env::set_var("HOME", temp_dir.path());

        let token = "test.jwt.token";
        save_credentials(token).unwrap();

        let loaded = load_credentials().unwrap();
        assert_eq!(loaded.access_token, token);

        clear_credentials().unwrap();
        assert!(load_credentials().is_err());
    }

    #[test]
    fn test_session_id_format() {
        let session_id = generate_session_id();
        // Should be a valid UUID v4
        assert!(uuid::Uuid::parse_str(&session_id).is_ok());
    }
}
```

### Step 9: Run tests

Run: `cargo test -p statichub`
Expected: All tests pass

### Step 10: Manual end-to-end test

**Prerequisites:**
- Server running with Google OAuth configured (from Task 12)
- Valid Google OAuth credentials in server's .env

**Test:**

Terminal 1 - Start server:
```bash
cd server
cargo run
```

Terminal 2 - Test login:
```bash
cargo run -- login
```

Expected:
1. Browser opens to Google OAuth page
2. User authorizes
3. CLI polls and receives token
4. Credentials saved to ~/.statichub/credentials.json
5. Success message printed

Verify credentials file:
```bash
cat ~/.statichub/credentials.json
```

Test logout:
```bash
cargo run -- logout
```

Expected:
- Success message
- credentials.json removed

### Step 11: Test authenticated deploy (prep for Task 14)

This won't work yet since Task 14 isn't implemented, but we can verify the credentials are loaded:

```bash
cargo run -- login
cargo run -- deploy /tmp/test-site --name my-app
```

Expected: Error about authenticated deploys not implemented yet (OK)

### Step 12: Commit

```bash
git add cli/src/main.rs cli/src/auth.rs cli/src/client.rs cli/Cargo.toml
git commit -m "feat: implement CLI Google OAuth login and logout commands

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

## Success Criteria

- `statichub login` command works end-to-end
- Browser opens automatically to Google OAuth page
- CLI polls server for token with progress indicators
- Token is saved to ~/.statichub/credentials.json
- Timeout after 5 minutes with clear error message
- `statichub logout` removes credentials file
- Progress messages are clear and helpful
- All tests pass
- Credentials file has correct permissions (readable only by user)
- Session ID is properly formatted as UUID v4
