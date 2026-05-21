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
