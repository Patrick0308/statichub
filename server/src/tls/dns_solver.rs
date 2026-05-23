use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait DnsSolver: Send + Sync {
    async fn set_txt_record(&self, domain: &str, value: &str) -> Result<()>;
    async fn delete_txt_record(&self, domain: &str, value: &str) -> Result<()>;
}

pub struct CloudflareSolver {
    pub api_token: String,
    client: reqwest::Client,
}

impl CloudflareSolver {
    pub fn new(api_token: String) -> Self {
        Self {
            api_token,
            client: reqwest::Client::new(),
        }
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
}
