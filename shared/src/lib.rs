// Shared types and utilities for StaticHub

pub mod types;

pub use types::*;

/// Build full project URL from subdomain and host
///
/// # Examples
///
/// ```
/// use statichub_shared::build_project_url;
///
/// // With port
/// let url = build_project_url("my-app", "localhost:3000");
/// assert_eq!(url, "http://my-app.localhost:3000");
///
/// // Without port
/// let url = build_project_url("my-app", "statichub.dev");
/// assert_eq!(url, "http://my-app.statichub.dev");
/// ```
pub fn build_project_url(subdomain: &str, host: &str) -> String {
    format!("http://{}.{}", subdomain, host)
}

#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn test_build_project_url_localhost_with_port() {
        assert_eq!(
            build_project_url("test-app", "localhost:3000"),
            "http://test-app.localhost:3000"
        );
    }

    #[test]
    fn test_build_project_url_without_port() {
        assert_eq!(
            build_project_url("my-project", "statichub.dev"),
            "http://my-project.statichub.dev"
        );
    }

    #[test]
    fn test_build_project_url_custom_domain_with_port() {
        assert_eq!(
            build_project_url("app", "example.com:8080"),
            "http://app.example.com:8080"
        );
    }
}
