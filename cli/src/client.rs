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
pub struct DomainResponse {
    pub domain: String,
    pub project_name: String,
    pub verified: bool,
    pub dns_target: String,
    pub created_at: String,
    pub verified_at: Option<String>,
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

    pub async fn add_domain(
        &self,
        project: &str,
        domain: &str,
        token: &str,
    ) -> Result<DomainResponse> {
        let url = format!("{}/api/projects/{}/domains", self.base_url, project);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({ "domain": domain }))
            .send()
            .await
            .context("Failed to add domain")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to add domain: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse domain response")
    }

    pub async fn list_domains(
        &self,
        project: &str,
        token: &str,
    ) -> Result<Vec<DomainResponse>> {
        let url = format!("{}/api/projects/{}/domains", self.base_url, project);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to list domains")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to list domains: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse domains list")
    }

    pub async fn verify_domain(
        &self,
        project: &str,
        domain: &str,
        token: &str,
    ) -> Result<DomainResponse> {
        let url = format!(
            "{}/api/projects/{}/domains/{}/verify",
            self.base_url, project, domain
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to verify domain")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to verify domain: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse domain response")
    }

    pub async fn remove_domain(
        &self,
        project: &str,
        domain: &str,
        token: &str,
    ) -> Result<()> {
        let url = format!("{}/api/projects/{}/domains/{}", self.base_url, project, domain);

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to remove domain")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to remove domain: {} - {}", status, body);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = Client::new("http://statichub.dev".to_string());
        assert_eq!(client.base_url, "http://statichub.dev");
    }
}
