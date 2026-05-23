use anyhow::{Result, Context, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
struct CreateDnsRecord {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    ttl: u32,
}

#[derive(Deserialize)]
struct DnsRecord {
    id: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    record_type: String,
    #[allow(dead_code)]
    name: String,
    content: String,
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

#[async_trait]
impl DnsSolver for CloudflareSolver {
    async fn set_txt_record(&self, domain: &str, value: &str) -> Result<()> {
        let zone_id = self.get_zone_id(domain).await?;
        let record_name = format!("_acme-challenge.{}", domain);

        let url = format!("{}/client/v4/zones/{}/dns_records", self.base_url, zone_id);

        let record = CreateDnsRecord {
            record_type: "TXT".to_string(),
            name: record_name,
            content: value.to_string(),
            ttl: 120, // 2 minutes
        };

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&record)
            .send()
            .await
            .context("Failed to create TXT record")?;

        if !response.status().is_success() {
            bail!("Cloudflare API returned status: {}", response.status());
        }

        let api_response: CloudflareResponse<serde_json::Value> = response
            .json()
            .await
            .context("Failed to parse Cloudflare response")?;

        if !api_response.success {
            if let Some(errors) = api_response.errors {
                let error_msgs: Vec<String> = errors.iter()
                    .map(|e| e.message.clone())
                    .collect();
                bail!("Failed to create TXT record: {}", error_msgs.join(", "));
            }
            bail!("Failed to create TXT record");
        }

        tracing::info!("✓ Set DNS TXT record: {} = {}", record.name, value);
        Ok(())
    }

    async fn delete_txt_record(&self, domain: &str, value: &str) -> Result<()> {
        let zone_id = self.get_zone_id(domain).await?;
        let record_name = format!("_acme-challenge.{}", domain);

        // First, find the record ID
        let list_url = format!(
            "{}/client/v4/zones/{}/dns_records?type=TXT&name={}",
            self.base_url, zone_id, record_name
        );

        let response = self.client
            .get(&list_url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to list TXT records")?;

        if !response.status().is_success() {
            bail!("Cloudflare API returned status: {}", response.status());
        }

        let api_response: CloudflareResponse<Vec<DnsRecord>> = response
            .json()
            .await
            .context("Failed to parse Cloudflare response")?;

        if !api_response.success {
            if let Some(errors) = api_response.errors {
                let error_msgs: Vec<String> = errors.iter()
                    .map(|e| e.message.clone())
                    .collect();
                bail!("Failed to list TXT records: {}", error_msgs.join(", "));
            }
            bail!("Failed to list TXT records");
        }

        // Find matching record
        let record = api_response.result
            .into_iter()
            .find(|r| r.content == value)
            .context(format!("TXT record not found: {}", record_name))?;

        // Delete the record
        let delete_url = format!(
            "{}/client/v4/zones/{}/dns_records/{}",
            self.base_url, zone_id, record.id
        );

        let response = self.client
            .delete(&delete_url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to delete TXT record")?;

        if !response.status().is_success() {
            bail!("Cloudflare API returned status: {}", response.status());
        }

        let api_response: CloudflareResponse<serde_json::Value> = response
            .json()
            .await
            .context("Failed to parse Cloudflare response")?;

        if !api_response.success {
            if let Some(errors) = api_response.errors {
                let error_msgs: Vec<String> = errors.iter()
                    .map(|e| e.message.clone())
                    .collect();
                bail!("Failed to delete TXT record: {}", error_msgs.join(", "));
            }
            bail!("Failed to delete TXT record");
        }

        tracing::info!("✓ Deleted DNS TXT record: {}", record_name);
        Ok(())
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

    #[tokio::test]
    async fn test_set_txt_record_success() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, header, body_json};

        let mock_server = MockServer::start().await;

        // Mock zone lookup
        Mock::given(method("GET"))
            .and(path("/client/v4/zones"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "result": [{"id": "zone_123", "name": "statichub.dev"}]
            })))
            .mount(&mock_server)
            .await;

        // Mock TXT record creation
        Mock::given(method("POST"))
            .and(path("/client/v4/zones/zone_123/dns_records"))
            .and(header("Authorization", "Bearer test_token"))
            .and(body_json(serde_json::json!({
                "type": "TXT",
                "name": "_acme-challenge.app.statichub.dev",
                "content": "test_value_123",
                "ttl": 120
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "result": {"id": "record_456"}
            })))
            .mount(&mock_server)
            .await;

        let solver = CloudflareSolver::new_with_base_url(
            "test_token".to_string(),
            mock_server.uri(),
        );

        solver.set_txt_record("app.statichub.dev", "test_value_123")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_delete_txt_record_success() {
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, path_regex, header};

        let mock_server = MockServer::start().await;

        // Mock zone lookup
        Mock::given(method("GET"))
            .and(path("/client/v4/zones"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "result": [{"id": "zone_123", "name": "statichub.dev"}]
            })))
            .mount(&mock_server)
            .await;

        // Mock DNS record list
        Mock::given(method("GET"))
            .and(path("/client/v4/zones/zone_123/dns_records"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "result": [{
                    "id": "record_789",
                    "type": "TXT",
                    "name": "_acme-challenge.app.statichub.dev",
                    "content": "test_value_123"
                }]
            })))
            .mount(&mock_server)
            .await;

        // Mock record deletion
        Mock::given(method("DELETE"))
            .and(path_regex("/client/v4/zones/zone_123/dns_records/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "result": {"id": "record_789"}
            })))
            .mount(&mock_server)
            .await;

        let solver = CloudflareSolver::new_with_base_url(
            "test_token".to_string(),
            mock_server.uri(),
        );

        solver.delete_txt_record("app.statichub.dev", "test_value_123")
            .await
            .unwrap();
    }
}
