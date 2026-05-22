use anyhow::{Context, Result};

/// Parse host header into domain and optional port
/// Examples:
///   "localhost:3000" -> ("localhost", Some(3000))
///   "statichub.dev" -> ("statichub.dev", None)
///   "statichub.dev:80" -> ("statichub.dev", Some(80))
pub fn parse_host(host: &str) -> Result<(String, Option<u16>)> {
    if let Some((domain, port_str)) = host.rsplit_once(':') {
        let port = port_str.parse::<u16>()
            .context("Invalid port in host header")?;
        Ok((domain.to_string(), Some(port)))
    } else {
        Ok((host.to_string(), None))
    }
}

/// Build host string from domain and optional port
pub fn build_host(domain: &str, port: Option<u16>) -> String {
    match port {
        Some(p) => format!("{}:{}", domain, p),
        None => domain.to_string(),
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub allowed_domains: Vec<String>,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .context("Invalid PORT value")?;

        let allowed_domains = std::env::var("ALLOWED_DOMAINS")
            .unwrap_or_else(|_| "localhost,statichub.dev".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Self { port, allowed_domains })
    }

    pub fn is_allowed(&self, domain: &str) -> bool {
        self.allowed_domains.iter().any(|d| {
            if d.starts_with("*.") {
                // Wildcard match: *.example.com matches foo.example.com
                // Must have at least one character before the base domain
                let base = &d[1..]; // Remove "*" to get ".example.com"
                domain.ends_with(base) && domain.len() > base.len()
            } else {
                d == domain
            }
        })
    }

    /// Extract the base domain from a hostname based on allowed domains
    /// For example, given "app.statichub.io" and allowed_domains ["*.statichub.io"],
    /// returns "statichub.io"
    pub fn extract_base_domain(&self, hostname: &str) -> Option<String> {
        for allowed in &self.allowed_domains {
            if allowed.starts_with("*.") {
                let base = &allowed[2..]; // Remove "*." to get "example.com"
                if hostname.ends_with(&format!(".{}", base)) {
                    return Some(base.to_string());
                }
            } else if allowed == hostname {
                return Some(hostname.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_host_with_port() {
        let (domain, port) = parse_host("localhost:3000").unwrap();
        assert_eq!(domain, "localhost");
        assert_eq!(port, Some(3000));
    }

    #[test]
    fn test_parse_host_without_port() {
        let (domain, port) = parse_host("statichub.dev").unwrap();
        assert_eq!(domain, "statichub.dev");
        assert_eq!(port, None);
    }

    #[test]
    fn test_parse_host_with_port_80() {
        let (domain, port) = parse_host("example.com:80").unwrap();
        assert_eq!(domain, "example.com");
        assert_eq!(port, Some(80));
    }

    #[test]
    fn test_parse_host_invalid_port() {
        let result = parse_host("example.com:invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_build_host_with_port() {
        assert_eq!(build_host("localhost", Some(3000)), "localhost:3000");
    }

    #[test]
    fn test_build_host_without_port() {
        assert_eq!(build_host("statichub.dev", None), "statichub.dev");
    }

    #[test]
    fn test_build_host_with_port_80() {
        assert_eq!(build_host("example.com", Some(80)), "example.com:80");
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_default_port() {
        std::env::remove_var("PORT");
        let config = ServerConfig::from_env().unwrap();
        assert_eq!(config.port, 3000);
        std::env::remove_var("PORT");
    }

    #[test]
    #[serial]
    fn test_custom_port() {
        std::env::remove_var("PORT");
        std::env::set_var("PORT", "8080");
        let config = ServerConfig::from_env().unwrap();
        assert_eq!(config.port, 8080);
        std::env::remove_var("PORT");
    }

    #[test]
    #[serial]
    fn test_invalid_port() {
        std::env::remove_var("PORT");
        std::env::set_var("PORT", "not_a_number");
        let result = ServerConfig::from_env();
        assert!(result.is_err());
        std::env::remove_var("PORT");
    }

    #[test]
    #[serial]
    fn test_default_allowed_domains() {
        std::env::remove_var("ALLOWED_DOMAINS");
        let config = ServerConfig::from_env().unwrap();
        assert_eq!(config.allowed_domains.len(), 2);
        assert!(config.allowed_domains.contains(&"localhost".to_string()));
        assert!(config.allowed_domains.contains(&"statichub.dev".to_string()));
        std::env::remove_var("ALLOWED_DOMAINS");
    }

    #[test]
    #[serial]
    fn test_custom_allowed_domains() {
        std::env::remove_var("ALLOWED_DOMAINS");
        std::env::set_var("ALLOWED_DOMAINS", "example.com,test.dev,localhost");
        let config = ServerConfig::from_env().unwrap();
        assert_eq!(config.allowed_domains.len(), 3);
        assert!(config.allowed_domains.contains(&"example.com".to_string()));
        assert!(config.allowed_domains.contains(&"test.dev".to_string()));
        assert!(config.allowed_domains.contains(&"localhost".to_string()));
        std::env::remove_var("ALLOWED_DOMAINS");
    }

    #[test]
    #[serial]
    fn test_allowed_domains_with_spaces() {
        std::env::remove_var("ALLOWED_DOMAINS");
        std::env::set_var("ALLOWED_DOMAINS", " example.com , test.dev , localhost ");
        let config = ServerConfig::from_env().unwrap();
        assert_eq!(config.allowed_domains.len(), 3);
        assert!(config.allowed_domains.contains(&"example.com".to_string()));
        std::env::remove_var("ALLOWED_DOMAINS");
    }

    #[test]
    #[serial]
    fn test_allowed_domains_filters_empty() {
        std::env::remove_var("ALLOWED_DOMAINS");
        std::env::set_var("ALLOWED_DOMAINS", "example.com,,test.dev");
        let config = ServerConfig::from_env().unwrap();
        assert_eq!(config.allowed_domains.len(), 2);
        std::env::remove_var("ALLOWED_DOMAINS");
    }

    #[test]
    fn test_is_allowed_positive() {
        let config = ServerConfig {
            port: 3000,
            allowed_domains: vec!["localhost".to_string(), "example.com".to_string()],
        };
        assert!(config.is_allowed("localhost"));
        assert!(config.is_allowed("example.com"));
    }

    #[test]
    fn test_is_allowed_negative() {
        let config = ServerConfig {
            port: 3000,
            allowed_domains: vec!["localhost".to_string(), "example.com".to_string()],
        };
        assert!(!config.is_allowed("malicious.com"));
        assert!(!config.is_allowed("statichub.dev"));
    }

    #[test]
    fn test_wildcard_subdomain_positive() {
        let config = ServerConfig {
            port: 3000,
            allowed_domains: vec!["*.example.com".to_string()],
        };
        assert!(config.is_allowed("foo.example.com"));
        assert!(config.is_allowed("bar.example.com"));
        assert!(config.is_allowed("test-123.example.com"));
    }

    #[test]
    fn test_wildcard_subdomain_negative() {
        let config = ServerConfig {
            port: 3000,
            allowed_domains: vec!["*.example.com".to_string()],
        };
        // Base domain should not match wildcard
        assert!(!config.is_allowed("example.com"));
        // Different domain should not match
        assert!(!config.is_allowed("example.org"));
        assert!(!config.is_allowed("foo.example.org"));
    }
}
