use axum::{
    extract::{Multipart, Path, State},
    Extension, Json,
};
use std::sync::Arc;
use statichub_shared::{build_project_url, DeployResponse};

use crate::{
    error::{AppError, Result},
    middleware::{AuthUser, RequestHost},
    models::{Deploy, Project},
    storage::Storage,
};

// Re-export DeployState from deploys module instead of duplicating
pub use super::DeployState;

pub async fn create_project_deploy(
    State(state): State<Arc<DeployState>>,
    Path(project_name): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
    axum::http::request::Parts { extensions, .. }: axum::http::request::Parts,
    mut multipart: Multipart,
) -> Result<Json<DeployResponse>> {
    // Extract host from request
    let request_host = extensions
        .get::<RequestHost>()
        .ok_or(AppError::MissingHost)?;

    // Validate project name before starting transaction
    validate_project_name(&project_name)?;

    // Begin transaction for all database operations
    let mut tx = state.pool.begin().await?;

    // Find existing project or create new one (within transaction)
    let project = match Project::find_by_name_tx(&mut tx, &project_name).await? {
        Some(existing) => {
            // Verify ownership
            if existing.owner_id != Some(auth_user.user_id) {
                return Err(AppError::Forbidden(
                    "You do not own this project".to_string(),
                ));
            }
            existing
        }
        None => {
            // Create new owned project
            Project::create_owned_tx(&mut tx, auth_user.user_id, &project_name, None).await?
        }
    };

    // Create deploy record with temporary storage_path
    // Deploy::create_tx will automatically calculate the correct version
    let temp_storage_path = format!("{}/deploy-temp", project_name);
    let deploy = Deploy::create_tx(&mut tx, project.id, &temp_storage_path).await?;

    // Now update storage_path with the actual version
    let actual_storage_path = format!("{}/deploy-{}", project_name, deploy.version);
    sqlx::query("UPDATE deploys SET storage_path = ? WHERE id = ?")
        .bind(&actual_storage_path)
        .bind(deploy.id)
        .execute(&mut *tx)
        .await?;

    // Update project's current_deploy_id (within transaction)
    sqlx::query("UPDATE projects SET current_deploy_id = ?, last_deployed_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&mut *tx)
        .await?;

    // Commit transaction before file upload
    tx.commit().await?;

    // Process multipart upload (outside transaction since it's not DB)
    let mut file_count = 0;
    let mut total_size = 0u64;

    let upload_result = process_multipart_files(
        &mut multipart,
        &state.storage,
        &actual_storage_path,
        &mut file_count,
        &mut total_size,
    )
    .await;

    // If storage fails, mark deploy as failed
    if let Err(e) = upload_result {
        let _ = Deploy::update_status(&state.pool, deploy.id, "failed", 0, 0).await;
        return Err(e);
    }

    // Update deploy status
    Deploy::update_status(&state.pool, deploy.id, "ready", file_count, total_size as i64).await?;

    Ok(Json(DeployResponse {
        url: build_project_url(&project.subdomain, &request_host.to_string()),
        subdomain: project.subdomain.clone(),
        version: Some(deploy.version),
        deploy_id: deploy.id,
        project_id: Some(project.id),
    }))
}

fn validate_project_name(name: &str) -> Result<()> {
    // Check length (1-63 characters)
    if name.is_empty() || name.len() > 63 {
        return Err(AppError::BadRequest(
            "Project name must be between 1 and 63 characters".to_string(),
        ));
    }

    // Check format: lowercase letters, numbers, and hyphens only
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AppError::BadRequest(
            "Project name must contain only lowercase letters, numbers, and hyphens".to_string(),
        ));
    }

    // Cannot start or end with hyphen
    if name.starts_with('-') || name.ends_with('-') {
        return Err(AppError::BadRequest(
            "Project name cannot start or end with a hyphen".to_string(),
        ));
    }

    Ok(())
}

async fn process_multipart_files(
    multipart: &mut Multipart,
    storage: &Arc<dyn Storage>,
    storage_path: &str,
    file_count: &mut i64,
    total_size: &mut u64,
) -> Result<()> {
    const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100MB per file
    const MAX_TOTAL_SIZE: u64 = 500 * 1024 * 1024; // 500MB total
    const MAX_FILE_COUNT: i64 = 1000; // Max 1000 files

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {}", e)))?
    {
        // Skip fields without filename (e.g., "config" text field)
        let filename = match field.file_name() {
            Some(name) => name.to_string(),
            None => continue,
        };

        let sanitized_filename = sanitize_filename(&filename)?;

        // Read file data
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to read file data: {}", e)))?;

        // Check file size limits
        if data.len() as u64 > MAX_FILE_SIZE {
            return Err(AppError::BadRequest(format!(
                "File '{}' exceeds maximum size of 100MB",
                sanitized_filename
            )));
        }

        *total_size += data.len() as u64;
        if *total_size > MAX_TOTAL_SIZE {
            return Err(AppError::BadRequest(
                "Total upload size exceeds maximum of 500MB".to_string(),
            ));
        }

        *file_count += 1;
        if *file_count > MAX_FILE_COUNT {
            return Err(AppError::BadRequest(
                "Too many files (maximum 1000)".to_string(),
            ));
        }

        // Store the file
        storage
            .store_file(storage_path, &sanitized_filename, &data)
            .await
            .map_err(|e| AppError::Storage(e.to_string()))?;
    }

    Ok(())
}

fn sanitize_filename(filename: &str) -> Result<String> {
    // Reject empty filenames
    if filename.trim().is_empty() {
        return Err(AppError::BadRequest("Filename cannot be empty".to_string()));
    }

    // Reject paths with directory traversal attempts
    if filename.contains("..") {
        return Err(AppError::BadRequest(format!(
            "Invalid filename: '{}' contains directory traversal",
            filename
        )));
    }

    // Reject absolute paths (starting with / or \)
    if filename.starts_with('/') || filename.starts_with('\\') {
        return Err(AppError::BadRequest(format!(
            "Invalid filename: '{}' cannot be an absolute path",
            filename
        )));
    }

    // Normalize path separators to forward slashes
    let normalized = filename.replace('\\', "/");

    // Reject any path component starting with a dot (hidden files/directories)
    for component in normalized.split('/') {
        if component.starts_with('.') {
            return Err(AppError::BadRequest(format!(
                "Invalid filename: '{}' contains hidden file or directory",
                filename
            )));
        }
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_project_name_valid() {
        assert!(validate_project_name("my-app").is_ok());
        assert!(validate_project_name("app123").is_ok());
        assert!(validate_project_name("my-cool-app-2024").is_ok());
        assert!(validate_project_name("a").is_ok());
    }

    #[test]
    fn test_validate_project_name_invalid() {
        // Empty
        assert!(validate_project_name("").is_err());

        // Too long
        let long_name = "a".repeat(64);
        assert!(validate_project_name(&long_name).is_err());

        // Uppercase
        assert!(validate_project_name("MyApp").is_err());

        // Special characters
        assert!(validate_project_name("my_app").is_err());
        assert!(validate_project_name("my.app").is_err());
        assert!(validate_project_name("my app").is_err());

        // Starts/ends with hyphen
        assert!(validate_project_name("-myapp").is_err());
        assert!(validate_project_name("myapp-").is_err());
    }

    #[test]
    fn test_sanitize_filename_valid() {
        assert!(sanitize_filename("index.html").is_ok());
        assert!(sanitize_filename("styles.css").is_ok());
        assert!(sanitize_filename("script.js").is_ok());
        // Subdirectories are allowed
        assert!(sanitize_filename("css/styles.css").is_ok());
        assert!(sanitize_filename("js/app.js").is_ok());
        assert!(sanitize_filename("assets/images/logo.png").is_ok());
    }

    #[test]
    fn test_sanitize_filename_invalid() {
        // Directory traversal
        assert!(sanitize_filename("../etc/passwd").is_err());
        assert!(sanitize_filename("..\\windows\\system32").is_err());
        assert!(sanitize_filename("dir/../file.txt").is_err());

        // Empty
        assert!(sanitize_filename("").is_err());
        assert!(sanitize_filename("   ").is_err());

        // Hidden files
        assert!(sanitize_filename(".htaccess").is_err());
        assert!(sanitize_filename("dir/.hidden").is_err());
    }
}
