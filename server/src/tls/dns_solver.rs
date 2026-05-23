use anyhow::{Result, Context, bail};
use async_trait::async_trait;
use serde::Deserialize;

#[async_trait]
pub trait DnsSolver: Send + Sync {
    async fn set_txt_record(&self, domain: &str, value: &str) -> Result<()>;
    async fn delete_txt_record(&self, domain: &str, value: &str) -> Result<()>;
}

#[derive(Deserialize)]
struct CloudflareResponse<T> {
    success: bool,
    result: T,
    errors: Option<Vec<CloudflareError>>,
}

#[derive(Deserialize)]
struct CloudflareError {
    message: String,
}

#[derive(Deserialize)]
struct Zone {
    id: String,
    name: String,
}

pub struct CloudflareSolver {
    pub api_token: String,
    client: reqwest::Client,
    base_url: String,
}

impl CloudflareSolver {
    pub fn new(api_token: String) -> Self {
        Self {
            api_token,
            client: reqwest::Client::new(),
            base_url: "https://api.cloudflare.com".to_string(),
        }
    }

    #[cfg(test)]
    fn new_with_base_url(api_token: String, base_url: String) -> Self {
        Self {
            api_token,
            client: reqwest::Client::new(),
            base_url,
        }
    }

    async fn get_zone_id(&self, domain: &str) -> Result<String> {
        // Extract base domain (last two parts)
        let parts: Vec<&str> = domain.split('.').collect();
        let base_domain = if parts.len() >= 2 {
            format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
        } else {
            domain.to_string()
        };

        let url = format!("{}/client/v4/zones?name={}", self.base_url, base_domain);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to query Cloudflare zones")?;

        let api_response: CloudflareResponse<Vec<Zone>> = response
            .json()
            .await
            .context("Failed to parse Cloudflare response")?;

        if !api_response.success {
            if let Some(errors) = api_response.errors {
                let error_msgs: Vec<String> = errors.iter()
                    .map(|e| e.message.clone())
                    .collect();
                bail!("Cloudflare API error: {}", error_msgs.join(", "));
            }
            bail!("Cloudflare API request failed");
        }

        api_response.result
            .into_iter()
            .find(|z| z.name == base_domain)
            .map(|z| z.id)
            .context(format!("Zone not found for domain: {}", base_domain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloudflare_solver_creation() {
        let solver = CloudflareSolver::new("test_token".to_string());
        assert_eq!(solver.api_token, "test_token");
    }

    #[tokio::test]
    async fn test_get_zone_id_success() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, header};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/client/v4/zones"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "result": [
                    {
                        "id": "zone_123",
                        "name": "statichub.dev"
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let solver = CloudflareSolver::new_with_base_url(
            "test_token".to_string(),
            mock_server.uri(),
        );

        let zone_id = solver.get_zone_id("statichub.dev").await.unwrap();
        assert_eq!(zone_id, "zone_123");
    }

    #[tokio::test]
    async fn test_get_zone_id_not_found() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, header};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/client/v4/zones"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "result": []
            })))
            .mount(&mock_server)
            .await;

        let solver = CloudflareSolver::new_with_base_url(
            "test_token".to_string(),
            mock_server.uri(),
        );

        let result = solver.get_zone_id("notfound.dev").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Zone not found"));
    }
}
