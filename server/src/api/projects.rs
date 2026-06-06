use axum::{
    extract::{Multipart, Path, State},
    Extension, Json,
};
use statichub_shared::{build_project_url, DeployResponse};
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::{AuthUser, RequestHost},
    models::{Deploy, Project},
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
    let upload_result = super::upload::process_multipart_files(
        &mut multipart,
        &state.storage,
        &actual_storage_path,
    )
    .await;

    // If storage fails, mark deploy as failed
    let upload = match upload_result {
        Ok(upload) => upload,
        Err(e) => {
            let _ = Deploy::update_status(&state.pool, deploy.id, "failed", 0, 0).await;
            return Err(e);
        }
    };

    // Update deploy status
    Deploy::update_status(
        &state.pool,
        deploy.id,
        "ready",
        upload.file_count,
        upload.total_size as i64,
    )
    .await?;

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
}
