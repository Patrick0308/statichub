use anyhow::{Context, Result};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use statichub_shared::{DeployResponse, ProjectConfig};
use crate::auth::{LoginRequest, LoginResponse, StatusResponse};

#[derive(Debug, Deserialize)]
pub struct ProjectListItem {
    pub name: String,
    pub url: String,
    pub current_version: Option<i64>,
    pub last_deployed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectDetail {
    pub name: String,
    pub url: String,
    pub current_version: Option<i64>,
    pub deploys: Vec<DeployInfo>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct DeployInfo {
    pub version: i64,
    pub file_count: i64,
    pub total_size_bytes: i64,
    pub deployed_at: String,
    pub is_current: bool,
}

#[derive(Debug, Deserialize)]
pub struct ApiKeyItem {
    pub id: i64,
    pub name: String,
    pub prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyResponse {
    pub id: i64,
    pub name: String,
    pub prefix: String,
    pub api_key: String,
}

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

    pub async fn deploy_anonymous(
        &self,
        files: &[crate::upload::UploadFile],
        config: Option<&ProjectConfig>,
    ) -> Result<DeployResponse> {
        let url = format!("{}/api/deploys/anonymous", self.base_url);

        let mut form = Form::new();

        for file in files {
            let part = Part::bytes(file.content.clone())
                .file_name(file.path.clone());
            form = form.part("files", part);
        }

        // Add config if provided
        if let Some(cfg) = config {
            let config_json = serde_json::to_string(cfg)
                .context("Failed to serialize config")?;
            form = form.text("config", config_json);
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

    pub async fn deploy_authenticated(
        &self,
        project_name: &str,
        files: &[crate::upload::UploadFile],
        token: &str,
        config: Option<&ProjectConfig>,
    ) -> Result<DeployResponse> {
        let url = format!("{}/api/projects/{}/deploys", self.base_url, project_name);

        let mut form = Form::new();

        for file in files {
            let part = Part::bytes(file.content.clone())
                .file_name(file.path.clone());
            form = form.part("files", part);
        }

        // Add config if provided
        if let Some(cfg) = config {
            let config_json = serde_json::to_string(cfg)
                .context("Failed to serialize config")?;
            form = form.text("config", config_json);
        }

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
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

    pub async fn list_projects(&self, token: &str) -> Result<Vec<ProjectListItem>> {
        let url = format!("{}/api/projects", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to list projects")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to list projects: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse projects list")
    }

    pub async fn get_project_info(&self, project: &str, token: &str) -> Result<ProjectDetail> {
        let url = format!("{}/api/projects/{}", self.base_url, project);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to get project info")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to get project info: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse project info")
    }

    pub async fn rollback_project(
        &self,
        project: &str,
        version: i64,
        token: &str,
    ) -> Result<ProjectDetail> {
        let url = format!("{}/api/projects/{}/rollback", self.base_url, project);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({ "version": version }))
            .send()
            .await
            .context("Failed to rollback project")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Rollback failed: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse rollback response")
    }

    pub async fn create_api_key(&self, jwt: &str, name: &str) -> Result<CreateApiKeyResponse> {
        let url = format!("{}/api/apikeys", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", jwt))
            .json(&serde_json::json!({ "name": name }))
            .send()
            .await
            .context("Failed to create api key")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to create api key: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse api key create response")
    }

    pub async fn list_api_keys(&self, jwt: &str) -> Result<Vec<ApiKeyItem>> {
        let url = format!("{}/api/apikeys", self.base_url);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", jwt))
            .send()
            .await
            .context("Failed to list api keys")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to list api keys: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse api keys list")
    }

    pub async fn revoke_api_key(&self, jwt: &str, id: i64) -> Result<()> {
        let url = format!("{}/api/apikeys/{}/revoke", self.base_url, id);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", jwt))
            .send()
            .await
            .context("Failed to revoke api key")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to revoke api key: {} - {}", status, body);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = Client::new("https://statichub.dev".to_string());
        assert_eq!(client.base_url, "https://statichub.dev");
    }
}
