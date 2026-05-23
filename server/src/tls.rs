use anyhow::{bail, Result};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct TlsConfig {
    pub enabled: bool,
    pub port: u16,
    pub email: String,
    pub cert_dir: PathBuf,
    pub dns_provider: DnsProvider,
    pub dns_api_token: String,
    pub acme_directory: AcmeDirectory,
    pub domains: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DnsProvider {
    Cloudflare,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AcmeDirectory {
    Staging,
    Production,
}

impl TlsConfig {
    /// Parse TLS configuration from environment variables
    /// Returns None if TLS is disabled
    pub fn from_env(_allowed_domains: &[String]) -> Result<Option<Self>> {
        let enabled = std::env::var("STATICHUB_TLS_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        if !enabled {
            return Ok(None);
        }

        // Will implement full parsing in next steps
        bail!("TLS configuration parsing not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_tls_disabled_by_default() {
        std::env::remove_var("STATICHUB_TLS_ENABLED");

        let allowed_domains = vec!["statichub.dev".to_string()];
        let result = TlsConfig::from_env(&allowed_domains).unwrap();

        assert!(result.is_none());

        std::env::remove_var("STATICHUB_TLS_ENABLED");
    }
}
