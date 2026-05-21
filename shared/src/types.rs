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
