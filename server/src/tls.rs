use anyhow::{bail, Result};
use std::path::PathBuf;

#[derive(Clone)]
pub struct TlsConfig {
    port: u16,
    email: String,
    cert_dir: PathBuf,
    dns_provider: DnsProvider,
    dns_api_token: String,
    acme_directory: AcmeDirectory,
    domains: Vec<String>,
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

    /// Get the TLS port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the email address for ACME registration
    pub fn email(&self) -> &str {
        &self.email
    }

    /// Get the certificate directory path
    pub fn cert_dir(&self) -> &PathBuf {
        &self.cert_dir
    }

    /// Get the DNS provider
    pub fn dns_provider(&self) -> &DnsProvider {
        &self.dns_provider
    }

    /// Get the DNS API token
    pub fn dns_api_token(&self) -> &str {
        &self.dns_api_token
    }

    /// Get the ACME directory
    pub fn acme_directory(&self) -> &AcmeDirectory {
        &self.acme_directory
    }

    /// Get the domains
    pub fn domains(&self) -> &[String] {
        &self.domains
    }
}

impl std::fmt::Debug for TlsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsConfig")
            .field("port", &self.port)
            .field("email", &self.email)
            .field("cert_dir", &self.cert_dir)
            .field("dns_provider", &self.dns_provider)
            .field("dns_api_token", &"***REDACTED***")
            .field("acme_directory", &self.acme_directory)
            .field("domains", &self.domains)
            .finish()
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
