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
