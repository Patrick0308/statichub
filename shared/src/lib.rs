// Shared types and utilities for StaticHub

pub mod types;

pub use types::*;

/// Build full project URL from subdomain identifier and base URL
///
/// # Examples
///
/// ```
/// use statichub_shared::build_project_url;
///
/// // Development
/// let url = build_project_url("my-app", "http://localhost:3000");
/// assert_eq!(url, "https://my-app.localhost:3000");
///
/// // Production
/// let url = build_project_url("my-app", "https://statichub.io");
/// assert_eq!(url, "https://my-app.statichub.io");
/// ```
pub fn build_project_url(subdomain: &str, base_url: &str) -> String {
    let domain = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    format!("https://{}.{}", subdomain, domain)
}

#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn test_build_project_url_localhost() {
        assert_eq!(
            build_project_url("test-app", "http://localhost:3000"),
            "https://test-app.localhost:3000"
        );
    }

    #[test]
    fn test_build_project_url_production() {
        assert_eq!(
            build_project_url("my-project", "https://statichub.io"),
            "https://my-project.statichub.io"
        );
    }

    #[test]
    fn test_build_project_url_custom_domain() {
        assert_eq!(
            build_project_url("app", "https://custom.com"),
            "https://app.custom.com"
        );
    }

    #[test]
    fn test_build_project_url_with_http_prefix() {
        assert_eq!(
            build_project_url("app", "http://example.org"),
            "https://app.example.org"
        );
    }
}
