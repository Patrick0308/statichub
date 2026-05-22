# Task 10: CLI Anonymous Deploy Command

## Goal

Wire up the deploy command to actually upload files to the server. This completes the end-to-end anonymous deployment flow.

## Files

- Create: `cli/src/client.rs`
- Modify: `cli/src/main.rs`
- Modify: `cli/Cargo.toml` (ensure reqwest with multipart feature)

## Implementation Steps

### Step 1: Ensure reqwest dependency

Verify `cli/Cargo.toml` has:

```toml
reqwest = { version = "0.11", features = ["multipart", "json"] }
```

### Step 2: Create HTTP client module

Create: `cli/src/client.rs`

```rust
use anyhow::{Context, Result};
use reqwest::multipart::{Form, Part};
use statichub_shared::DeployResponse;

pub struct Client {
    base_url: String,
    client: reqwest::Client,
}

impl Client {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn deploy_anonymous(&self, files: &[crate::upload::UploadFile]) -> Result<DeployResponse> {
        let url = format!("{}/api/deploys/anonymous", self.base_url);

        let mut form = Form::new();

        for file in files {
            let part = Part::bytes(file.content.clone())
                .file_name(file.path.clone());
            form = form.part("files", part);
        }

        let response = self.client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .context("Failed to send deploy request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Deploy failed with status {}: {}", status, body);
        }

        let deploy_response: DeployResponse = response
            .json()
            .await
            .context("Failed to parse deploy response")?;

        Ok(deploy_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = Client::new("http://localhost:3000".to_string());
        assert_eq!(client.base_url, "http://localhost:3000");
    }
}
```

### Step 3: Wire up deploy command

Modify `cli/src/main.rs`:

Add at the top:
```rust
mod client;
```

Replace the `Deploy` match arm:

```rust
Commands::Deploy { directory, name } => {
    if name.is_some() {
        anyhow::bail!("Named projects require login. Use 'statichub login' first.");
    }

    let dir = directory
        .as_ref()
        .map(|d| std::path::PathBuf::from(d))
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    println!("📦 Collecting files from {}...", dir.display());
    let files = upload::collect_files(&dir)?;
    println!("   Found {} files", files.len());

    let server_url = std::env::var("STATICHUB_SERVER")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    println!("🚀 Deploying to {}...", server_url);
    let client = client::Client::new(server_url);
    let response = client.deploy_anonymous(&files).await?;

    println!("✅ Deploy successful!");
    println!("   URL: {}", response.url);
    println!("   Subdomain: {}", response.subdomain);
}
```

### Step 4: Test end-to-end (manual)

This requires the server to be running.

Terminal 1 - Start server:
```bash
cargo run -p statichub-server
```

Terminal 2 - Create test site and deploy:
```bash
mkdir -p /tmp/test-site
echo "<h1>Hello StaticHub</h1>" > /tmp/test-site/index.html
echo "body { color: blue; }" > /tmp/test-site/style.css

cargo run -p statichub -- deploy /tmp/test-site
```

Expected output:
```
📦 Collecting files from /tmp/test-site...
   Found 2 files
🚀 Deploying to http://localhost:3000...
✅ Deploy successful!
   URL: https://abc123.statichub.io
   Subdomain: abc123.statichub.io
```

### Step 5: Run unit tests

```bash
cargo test -p statichub
```

Expected: All tests pass

### Step 6: Commit

```bash
git add cli/src/client.rs cli/src/main.rs cli/Cargo.toml
git commit -m "feat: implement CLI anonymous deploy command

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

## Success Criteria

- Client module properly sends multipart requests
- Deploy command collects files using upload module
- Files are uploaded to server API
- Server response is parsed and displayed
- End-to-end deploy works from CLI to server
- Error messages are clear and helpful
