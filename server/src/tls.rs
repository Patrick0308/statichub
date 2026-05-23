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

    /// Extract certificate domains from allowed domains
    /// - Base domains (statichub.dev) -> wildcard (*.statichub.dev)
    /// - Subdomains (app.statichub.dev) -> keep as-is
    /// - Filter out localhost, 127.0.0.1, and local addresses
    fn extract_certificate_domains(allowed_domains: &[String]) -> Vec<String> {
        allowed_domains
            .iter()
            .filter(|domain| {
                // Filter out localhost and local addresses
                !domain.contains("localhost")
                    && !domain.starts_with("127.")
                    && !domain.starts_with("192.168.")
                    && !domain.starts_with("10.")
                    && !domain.starts_with("172.")
            })
            .map(|domain| {
                // If domain has subdomain (more than 2 parts), keep as-is
                // Otherwise, convert to wildcard
                let parts: Vec<&str> = domain.split('.').collect();
                if parts.len() > 2 {
                    // Subdomain like app.statichub.dev -> keep as-is
                    domain.clone()
                } else {
                    // Base domain like statichub.dev -> *.statichub.dev
                    format!("*.{}", domain)
                }
            })
            .collect()
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

    #[test]
    fn test_extract_certificate_domains_wildcard() {
        let input = vec!["statichub.dev".to_string()];
        let output = TlsConfig::extract_certificate_domains(&input);

        assert_eq!(output, vec!["*.statichub.dev".to_string()]);
    }

    #[test]
    fn test_extract_certificate_domains_specific() {
        let input = vec!["api.example.com".to_string()];
        let output = TlsConfig::extract_certificate_domains(&input);

        assert_eq!(output, vec!["api.example.com".to_string()]);
    }

    #[test]
    fn test_extract_certificate_domains_filter_localhost() {
        let input = vec![
            "statichub.dev".to_string(),
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "api.example.com".to_string(),
        ];
        let output = TlsConfig::extract_certificate_domains(&input);

        assert_eq!(output, vec![
            "*.statichub.dev".to_string(),
            "api.example.com".to_string(),
        ]);
    }

    #[test]
    fn test_extract_certificate_domains_subdomain() {
        let input = vec![
            "app.statichub.dev".to_string(),
            "api.statichub.dev".to_string(),
        ];
        let output = TlsConfig::extract_certificate_domains(&input);

        // Subdomains stay as-is (specific certificates)
        assert_eq!(output, vec![
            "app.statichub.dev".to_string(),
            "api.statichub.dev".to_string(),
        ]);
    }
}
