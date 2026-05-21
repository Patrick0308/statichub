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
