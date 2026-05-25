use crate::{
    api::DeployState,
    config::parse_host,
    error::{AppError, Result},
    middleware::RequestHost,
    models::Project,
    storage::Storage,
};
use axum::{
    extract::{Host, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use statichub_shared::ProjectConfig;
use std::sync::Arc;

pub async fn serve_static_file(
    Host(hostname): Host,
    State(state): State<Arc<DeployState>>,
    axum::http::request::Parts { extensions, .. }: axum::http::request::Parts,
    request: Request,
) -> Result<Response> {
    let (hostname, _) =
        parse_host(&hostname).map_err(|e| AppError::BadRequest(format!("Invalid hostname: {}", e)))?;

    let request_host = extensions
        .get::<RequestHost>()
        .ok_or(AppError::MissingHost)?;

    let base_domain = &request_host.base_domain;
    let request_path = request.uri().path();

    // Serve built-in homepage for base domain and www alias.
    if hostname == *base_domain || hostname == format!("www.{}", base_domain) {
        if let Some(response) = crate::web::homepage::serve_home(request_path) {
            return Ok(response);
        }
        return Err(AppError::NotFound(format!(
            "Homepage asset not found: {}",
            request_path
        )));
    }

    // Subdomain lookup
    let subdomain = extract_subdomain(&hostname, base_domain)?;
    let project = Project::find_by_subdomain(&state.pool, &subdomain)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", subdomain)))?;

    // Get project config
    let config = project.get_config().unwrap_or_default();

    // Get current deploy
    let deploy_id = project.current_deploy_id.ok_or_else(|| {
        AppError::NotFound(format!("No deployment found for project: {}", project.name))
    })?;

    let deploy = crate::models::Deploy::find_by_id(&state.pool, deploy_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Deploy not found: {}", deploy_id)))?;

    // Check for redirects BEFORE resolving files
    if let Some(redirects) = &config.redirects {
        for redirect in redirects {
            // Match exact path or path prefix
            if request_path == redirect.from
                || request_path.starts_with(&format!("{}/", redirect.from))
            {
                let mut headers = HeaderMap::new();
                headers.insert(
                    header::LOCATION,
                    HeaderValue::from_str(&redirect.to)
                        .map_err(|_| AppError::BadRequest("Invalid redirect URL".to_string()))?,
                );
                return Ok((
                    StatusCode::from_u16(redirect.status).map_err(|_| {
                        AppError::BadRequest("Invalid redirect status code".to_string())
                    })?,
                    headers,
                )
                    .into_response());
            }
        }
    }

    // Resolve file path
    let file_path =
        resolve_file_path(request_path, &config, &state.storage, &deploy.storage_path).await?;

    // Get file content
    let content = state
        .storage
        .get_file(&deploy.storage_path, &file_path)
        .await
        .map_err(|e| match e {
            crate::storage::StorageError::NotFound(_) => {
                AppError::NotFound(format!("File not found: {}", request_path))
            }
            _ => AppError::Storage(e.to_string()),
        })?;

    // Detect content type
    let content_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    // Build response with custom headers
    let mut headers = HeaderMap::new();
    headers.insert(
        "content-type",
        HeaderValue::from_str(&content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );

    // Apply custom headers from config
    if let Some(custom_headers) = &config.headers {
        for (pattern, header_map) in custom_headers {
            if request_path.starts_with(pattern) {
                for (key, value) in header_map {
                    if let Ok(header_value) = HeaderValue::from_str(value) {
                        if let Ok(header_name) = key.parse::<axum::http::HeaderName>() {
                            headers.insert(header_name, header_value);
                        } else {
                            tracing::warn!("Invalid custom header name: {}", key);
                        }
                    }
                }
            }
        }
    }

    Ok((StatusCode::OK, headers, content).into_response())
}

fn extract_subdomain(hostname: &str, base_url: &str) -> Result<String> {
    // Remove protocol from base_url
    let base_domain = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    // Extract subdomain
    if let Some(subdomain) = hostname.strip_suffix(&format!(".{}", base_domain)) {
        Ok(subdomain.to_string())
    } else {
        Err(AppError::BadRequest(format!(
            "Invalid hostname: {}",
            hostname
        )))
    }
}

async fn resolve_file_path(
    request_path: &str,
    config: &ProjectConfig,
    storage: &Arc<dyn Storage>,
    deploy_path: &str,
) -> Result<String> {
    let mut path = request_path.trim_start_matches('/').to_string();

    // If path is empty, try index.html
    if path.is_empty() {
        path = "index.html".to_string();
    }

    // Try exact path first
    if file_exists(storage, deploy_path, &path).await {
        return Ok(path);
    }

    // Clean URLs: try adding .html
    if config.clean_urls.unwrap_or(false) {
        let html_path = format!("{}.html", path);
        if file_exists(storage, deploy_path, &html_path).await {
            return Ok(html_path);
        }
    }

    // Directory index: try path/index.html
    let index_path = format!("{}/index.html", path);
    if file_exists(storage, deploy_path, &index_path).await {
        return Ok(index_path);
    }

    // SPA mode: fallback to index.html for non-existent paths
    if config.spa.unwrap_or(false) {
        if file_exists(storage, deploy_path, "index.html").await {
            return Ok("index.html".to_string());
        }
    }

    // File not found
    Err(AppError::NotFound(format!(
        "File not found: {}",
        request_path
    )))
}

async fn file_exists(storage: &Arc<dyn Storage>, deploy_path: &str, file_path: &str) -> bool {
    storage.get_file(deploy_path, file_path).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_subdomain() {
        assert_eq!(
            extract_subdomain("abc123.statichub.io", "http://statichub.io").unwrap(),
            "abc123"
        );
        assert_eq!(
            extract_subdomain("test.statichub.io", "https://statichub.io").unwrap(),
            "test"
        );
    }

    #[test]
    fn test_extract_subdomain_invalid() {
        assert!(extract_subdomain("example.com", "http://statichub.io").is_err());
    }
}
