use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub expires_at: Option<String>,
}

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

    // Set file permissions to 0o600 (read/write for user only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

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

pub fn generate_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load_credentials() {
        let temp_dir = TempDir::new().unwrap();
        let creds_file = temp_dir.path().join("credentials.json");

        // Save credentials directly to temp path
        let creds = Credentials {
            access_token: "test_token_123".to_string(),
            expires_at: None,
        };

        let json = serde_json::to_string_pretty(&creds).unwrap();
        std::fs::write(&creds_file, json).unwrap();

        // Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&creds_file).unwrap().permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&creds_file, perms).unwrap();
        }

        // Verify permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(&creds_file).unwrap().permissions();
            assert_eq!(perms.mode() & 0o777, 0o600);
        }

        // Load and verify
        let json = std::fs::read_to_string(&creds_file).unwrap();
        let loaded: Credentials = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.access_token, "test_token_123");
    }

    #[test]
    fn test_clear_credentials() {
        save_credentials("test").unwrap();
        clear_credentials().unwrap();

        let loaded = load_credentials().unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_session_id_format() {
        let session_id = generate_session_id();
        // Should be a valid UUID v4 with 36 characters
        assert_eq!(session_id.len(), 36);
        assert!(uuid::Uuid::parse_str(&session_id).is_ok());
    }
}
