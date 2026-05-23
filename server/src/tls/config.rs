use anyhow::{bail, Context, Result};
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
    pub fn from_env(allowed_domains: &[String]) -> Result<Option<Self>> {
        let enabled = std::env::var("STATICHUB_TLS_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        if !enabled {
            return Ok(None);
        }

        // Required fields
        let email = std::env::var("STATICHUB_TLS_EMAIL")
            .context("STATICHUB_TLS_EMAIL is required when TLS is enabled")?;

        let dns_provider_str = std::env::var("STATICHUB_DNS_PROVIDER")
            .context("STATICHUB_DNS_PROVIDER is required when TLS is enabled")?;

        let dns_provider = match dns_provider_str.to_lowercase().as_str() {
            "cloudflare" => DnsProvider::Cloudflare,
            _ => bail!("Unsupported DNS provider: {}. Supported: cloudflare", dns_provider_str),
        };

        let dns_api_token = std::env::var("STATICHUB_DNS_API_TOKEN")
            .context("STATICHUB_DNS_API_TOKEN is required when TLS is enabled")?;

        // Optional fields with defaults
        let port = std::env::var("STATICHUB_TLS_PORT")
            .unwrap_or_else(|_| "443".to_string())
            .parse()
            .context("Invalid STATICHUB_TLS_PORT value")?;

        let cert_dir = std::env::var("STATICHUB_CERT_DIR")
            .unwrap_or_else(|_| "./var/statichub/certs".to_string())
            .into();

        let acme_directory_str = std::env::var("STATICHUB_ACME_DIRECTORY")
            .unwrap_or_else(|_| "staging".to_string());

        let acme_directory = match acme_directory_str.to_lowercase().as_str() {
            "staging" => AcmeDirectory::Staging,
            "production" => AcmeDirectory::Production,
            _ => bail!("Invalid STATICHUB_ACME_DIRECTORY: {}. Use 'staging' or 'production'", acme_directory_str),
        };

        // Extract domains for certificates
        let domains = Self::extract_certificate_domains(allowed_domains);

        if domains.is_empty() {
            bail!("No valid domains for TLS certificates. Check STATICHUB_ALLOWED_DOMAINS");
        }

        Ok(Some(Self {
            port,
            email,
            cert_dir,
            dns_provider,
            dns_api_token,
            acme_directory,
            domains,
        }))
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
    /// - Base domains (statichub.dev) -> both root (statichub.dev) and wildcard (*.statichub.dev)
    /// - Subdomains (app.statichub.dev) -> keep as-is
    /// - Filter out localhost, 127.0.0.1, and local addresses
    fn extract_certificate_domains(allowed_domains: &[String]) -> Vec<String> {
        let mut result = Vec::new();

        for domain in allowed_domains {
            // Filter out localhost and local addresses
            if domain.contains("localhost")
                || domain.starts_with("127.")
                || domain.starts_with("192.168.")
                || domain.starts_with("10.")
                || domain.starts_with("172.16.")
                || domain.starts_with("172.17.")
                || domain.starts_with("172.18.")
                || domain.starts_with("172.19.")
                || domain.starts_with("172.20.")
                || domain.starts_with("172.21.")
                || domain.starts_with("172.22.")
                || domain.starts_with("172.23.")
                || domain.starts_with("172.24.")
                || domain.starts_with("172.25.")
                || domain.starts_with("172.26.")
                || domain.starts_with("172.27.")
                || domain.starts_with("172.28.")
                || domain.starts_with("172.29.")
                || domain.starts_with("172.30.")
                || domain.starts_with("172.31.")
            {
                continue;
            }

            let parts: Vec<&str> = domain.split('.').collect();
            if parts.len() > 2 {
                // Subdomain like app.statichub.dev -> keep as-is
                result.push(domain.clone());
            } else {
                // Base domain like statichub.dev -> add both root and wildcard
                result.push(domain.clone());
                result.push(format!("*.{}", domain));
            }
        }

        result
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

        assert_eq!(output, vec!["statichub.dev".to_string(), "*.statichub.dev".to_string()]);
    }

    #[test]
    fn test_extract_certificate_domains_specific() {
        let input = vec!["app.statichub.dev".to_string()];
        let output = TlsConfig::extract_certificate_domains(&input);

        assert_eq!(output, vec!["app.statichub.dev".to_string()]);
    }

    #[test]
    fn test_extract_certificate_domains_filter_localhost() {
        let input = vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "192.168.1.1".to_string(),
            "statichub.dev".to_string(),
        ];
        let output = TlsConfig::extract_certificate_domains(&input);

        assert_eq!(output, vec!["statichub.dev".to_string(), "*.statichub.dev".to_string()]);
    }

    #[test]
    fn test_extract_certificate_domains_subdomain() {
        let input = vec![
            "test.sub.statichub.dev".to_string(),
            "example.com".to_string(),
        ];
        let output = TlsConfig::extract_certificate_domains(&input);

        // Subdomains stay as-is (specific certificates)
        // Base domains get both root and wildcard
        assert_eq!(output, vec![
            "test.sub.statichub.dev".to_string(),
            "example.com".to_string(),
            "*.example.com".to_string(),
        ]);
    }

    #[test]
    #[serial]
    fn test_tls_enabled_requires_email() {
        std::env::set_var("STATICHUB_TLS_ENABLED", "true");
        std::env::remove_var("STATICHUB_TLS_EMAIL");
        std::env::set_var("STATICHUB_DNS_PROVIDER", "cloudflare");
        std::env::set_var("STATICHUB_DNS_API_TOKEN", "test_token");

        let allowed_domains = vec!["statichub.dev".to_string()];
        let result = TlsConfig::from_env(&allowed_domains);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("STATICHUB_TLS_EMAIL"));

        std::env::remove_var("STATICHUB_TLS_ENABLED");
        std::env::remove_var("STATICHUB_DNS_PROVIDER");
        std::env::remove_var("STATICHUB_DNS_API_TOKEN");
    }

    #[test]
    #[serial]
    fn test_tls_enabled_requires_dns_provider() {
        std::env::set_var("STATICHUB_TLS_ENABLED", "true");
        std::env::set_var("STATICHUB_TLS_EMAIL", "test@example.com");
        std::env::remove_var("STATICHUB_DNS_PROVIDER");
        std::env::set_var("STATICHUB_DNS_API_TOKEN", "test_token");

        let allowed_domains = vec!["statichub.dev".to_string()];
        let result = TlsConfig::from_env(&allowed_domains);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("STATICHUB_DNS_PROVIDER"));

        std::env::remove_var("STATICHUB_TLS_ENABLED");
        std::env::remove_var("STATICHUB_TLS_EMAIL");
        std::env::remove_var("STATICHUB_DNS_API_TOKEN");
    }

    #[test]
    #[serial]
    fn test_tls_full_configuration() {
        std::env::set_var("STATICHUB_TLS_ENABLED", "true");
        std::env::set_var("STATICHUB_TLS_EMAIL", "admin@example.com");
        std::env::set_var("STATICHUB_TLS_PORT", "8443");
        std::env::set_var("STATICHUB_CERT_DIR", "/tmp/certs");
        std::env::set_var("STATICHUB_DNS_PROVIDER", "cloudflare");
        std::env::set_var("STATICHUB_DNS_API_TOKEN", "test_token_123");
        std::env::set_var("STATICHUB_ACME_DIRECTORY", "production");

        let allowed_domains = vec!["statichub.dev".to_string()];
        let result = TlsConfig::from_env(&allowed_domains).unwrap();

        assert!(result.is_some());
        let config = result.unwrap();
        assert_eq!(config.port, 8443);
        assert_eq!(config.email, "admin@example.com");
        assert_eq!(config.cert_dir, PathBuf::from("/tmp/certs"));
        assert_eq!(config.dns_provider, DnsProvider::Cloudflare);
        assert_eq!(config.dns_api_token, "test_token_123");
        assert_eq!(config.acme_directory, AcmeDirectory::Production);
        assert_eq!(config.domains, vec!["statichub.dev", "*.statichub.dev"]);

        std::env::remove_var("STATICHUB_TLS_ENABLED");
        std::env::remove_var("STATICHUB_TLS_EMAIL");
        std::env::remove_var("STATICHUB_TLS_PORT");
        std::env::remove_var("STATICHUB_CERT_DIR");
        std::env::remove_var("STATICHUB_DNS_PROVIDER");
        std::env::remove_var("STATICHUB_DNS_API_TOKEN");
        std::env::remove_var("STATICHUB_ACME_DIRECTORY");
    }

    #[test]
    #[serial]
    fn test_tls_enabled_requires_dns_api_token() {
        std::env::set_var("STATICHUB_TLS_ENABLED", "true");
        std::env::set_var("STATICHUB_TLS_EMAIL", "test@example.com");
        std::env::set_var("STATICHUB_DNS_PROVIDER", "cloudflare");
        std::env::remove_var("STATICHUB_DNS_API_TOKEN");

        let allowed_domains = vec!["statichub.dev".to_string()];
        let result = TlsConfig::from_env(&allowed_domains);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("STATICHUB_DNS_API_TOKEN"));

        std::env::remove_var("STATICHUB_TLS_ENABLED");
        std::env::remove_var("STATICHUB_TLS_EMAIL");
        std::env::remove_var("STATICHUB_DNS_PROVIDER");
    }

    #[test]
    #[serial]
    fn test_tls_invalid_dns_provider() {
        std::env::set_var("STATICHUB_TLS_ENABLED", "true");
        std::env::set_var("STATICHUB_TLS_EMAIL", "test@example.com");
        std::env::set_var("STATICHUB_DNS_PROVIDER", "invalid_provider");
        std::env::set_var("STATICHUB_DNS_API_TOKEN", "test_token");

        let allowed_domains = vec!["statichub.dev".to_string()];
        let result = TlsConfig::from_env(&allowed_domains);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported DNS provider"));

        std::env::remove_var("STATICHUB_TLS_ENABLED");
        std::env::remove_var("STATICHUB_TLS_EMAIL");
        std::env::remove_var("STATICHUB_DNS_PROVIDER");
        std::env::remove_var("STATICHUB_DNS_API_TOKEN");
    }

    #[test]
    #[serial]
    fn test_tls_invalid_acme_directory() {
        std::env::set_var("STATICHUB_TLS_ENABLED", "true");
        std::env::set_var("STATICHUB_TLS_EMAIL", "test@example.com");
        std::env::set_var("STATICHUB_DNS_PROVIDER", "cloudflare");
        std::env::set_var("STATICHUB_DNS_API_TOKEN", "test_token");
        std::env::set_var("STATICHUB_ACME_DIRECTORY", "invalid");

        let allowed_domains = vec!["statichub.dev".to_string()];
        let result = TlsConfig::from_env(&allowed_domains);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid STATICHUB_ACME_DIRECTORY"));

        std::env::remove_var("STATICHUB_TLS_ENABLED");
        std::env::remove_var("STATICHUB_TLS_EMAIL");
        std::env::remove_var("STATICHUB_DNS_PROVIDER");
        std::env::remove_var("STATICHUB_DNS_API_TOKEN");
        std::env::remove_var("STATICHUB_ACME_DIRECTORY");
    }

    #[test]
    #[serial]
    fn test_tls_invalid_port() {
        std::env::set_var("STATICHUB_TLS_ENABLED", "true");
        std::env::set_var("STATICHUB_TLS_EMAIL", "test@example.com");
        std::env::set_var("STATICHUB_TLS_PORT", "not_a_number");
        std::env::set_var("STATICHUB_DNS_PROVIDER", "cloudflare");
        std::env::set_var("STATICHUB_DNS_API_TOKEN", "test_token");

        let allowed_domains = vec!["statichub.dev".to_string()];
        let result = TlsConfig::from_env(&allowed_domains);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid STATICHUB_TLS_PORT"));

        std::env::remove_var("STATICHUB_TLS_ENABLED");
        std::env::remove_var("STATICHUB_TLS_EMAIL");
        std::env::remove_var("STATICHUB_TLS_PORT");
        std::env::remove_var("STATICHUB_DNS_PROVIDER");
        std::env::remove_var("STATICHUB_DNS_API_TOKEN");
    }

    #[test]
    #[serial]
    fn test_tls_no_valid_domains() {
        std::env::set_var("STATICHUB_TLS_ENABLED", "true");
        std::env::set_var("STATICHUB_TLS_EMAIL", "test@example.com");
        std::env::set_var("STATICHUB_DNS_PROVIDER", "cloudflare");
        std::env::set_var("STATICHUB_DNS_API_TOKEN", "test_token");

        let allowed_domains = vec!["localhost".to_string(), "127.0.0.1".to_string()];
        let result = TlsConfig::from_env(&allowed_domains);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No valid domains for TLS certificates"));

        std::env::remove_var("STATICHUB_TLS_ENABLED");
        std::env::remove_var("STATICHUB_TLS_EMAIL");
        std::env::remove_var("STATICHUB_DNS_PROVIDER");
        std::env::remove_var("STATICHUB_DNS_API_TOKEN");
    }
}
