use axum::{
    extract::Request,
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use crate::{config::{parse_host, ServerConfig}, error::{AppError, Result}};

#[derive(Clone, Debug)]
pub struct RequestHost {
    pub domain: String,
    pub port: Option<u16>,
}

impl RequestHost {
    pub fn to_string(&self) -> String {
        match self.port {
            Some(port) => format!("{}:{}", self.domain, port),
            None => self.domain.clone(),
        }
    }
}

pub async fn host_validation_middleware(
    config: axum::extract::State<ServerConfig>,
    mut req: Request,
    next: Next,
) -> Result<Response> {
    let headers: &HeaderMap = req.headers();

    // Extract Host header
    let host_header = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| AppError::InvalidHost("Host header is required".to_string()))?;

    // Parse domain and port
    let (domain, port) = parse_host(host_header)
        .map_err(|e| AppError::InvalidHost(format!("Invalid host header: {}", e)))?;

    // Validate domain
    if !config.is_allowed(&domain) {
        return Err(AppError::DomainNotAllowed(format!(
            "Domain '{}' is not configured for this server",
            domain
        )));
    }

    // Attach to request extensions
    let request_host = RequestHost { domain, port };
    req.extensions_mut().insert(request_host);

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_host_to_string_with_port() {
        let host = RequestHost {
            domain: "localhost".to_string(),
            port: Some(3000),
        };
        assert_eq!(host.to_string(), "localhost:3000");
    }

    #[test]
    fn test_request_host_to_string_without_port() {
        let host = RequestHost {
            domain: "statichub.dev".to_string(),
            port: None,
        };
        assert_eq!(host.to_string(), "statichub.dev");
    }
}
