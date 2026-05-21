use axum::{
    extract::{State, Multipart},
    Json,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use statichub_shared::DeployResponse;
use crate::{error::{Result, AppError}, storage::Storage, models::{Project, Deploy}};

pub struct DeployState {
    pub pool: SqlitePool,
    pub storage: Arc<dyn Storage>,
    pub base_url: String,
}

pub async fn create_anonymous_deploy(
    State(state): State<Arc<DeployState>>,
    mut multipart: Multipart,
) -> Result<Json<DeployResponse>> {
    // Generate random subdomain
    let subdomain = generate_random_subdomain();

    // Create anonymous project
    let project = Project::create_anonymous(&state.pool, &subdomain).await?;

    // Create deploy record
    let storage_path = format!("{}/deploy-1", subdomain);
    let deploy = Deploy::create(&state.pool, project.id, &storage_path).await?;

    // Extract and store files from multipart
    let mut file_count = 0;
    let mut total_size = 0u64;

    // Process files with proper error handling and atomicity
    let upload_result = process_multipart_files(
        &mut multipart,
        &state.storage,
        &storage_path,
        &mut file_count,
        &mut total_size,
    ).await;

    // If storage fails, mark deploy as failed before returning error
    if let Err(e) = upload_result {
        let _ = Deploy::update_status(&state.pool, deploy.id, "failed", 0, 0).await;
        return Err(e);
    }

    // Update deploy status
    Deploy::update_status(&state.pool, deploy.id, "ready", file_count, total_size as i64).await?;

    // Update project current_deploy_id
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&state.pool)
        .await?;

    Ok(Json(DeployResponse {
        url: format!("https://{}.statichub.io", subdomain),
        subdomain: format!("{}.statichub.io", subdomain),
        version: Some(deploy.version),
        deploy_id: deploy.id.to_string(),
    }))
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

    while let Some(field) = multipart.next_field().await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {}", e)))? {

        // Get and validate filename
        let filename = field.file_name()
            .ok_or_else(|| AppError::BadRequest("Missing filename".to_string()))?
            .to_string();

        let sanitized_filename = sanitize_filename(&filename)?;

        // Read file data with error handling
        let data = field.bytes().await
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
                "Total upload size exceeds maximum of 500MB".to_string()
            ));
        }

        *file_count += 1;
        if *file_count > MAX_FILE_COUNT {
            return Err(AppError::BadRequest(
                "Too many files (maximum 1000)".to_string()
            ));
        }

        // Store the file
        storage.store_file(storage_path, &sanitized_filename, &data).await
            .map_err(|e| AppError::Storage(e.to_string()))?;
    }

    Ok(())
}

fn sanitize_filename(filename: &str) -> Result<String> {
    // Reject paths with directory traversal attempts
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(AppError::BadRequest(format!(
            "Invalid filename: '{}' contains forbidden characters",
            filename
        )));
    }

    // Reject empty filenames
    if filename.trim().is_empty() {
        return Err(AppError::BadRequest("Filename cannot be empty".to_string()));
    }

    // Additional safety: reject filenames starting with dot (hidden files)
    if filename.starts_with('.') {
        return Err(AppError::BadRequest(format!(
            "Invalid filename: '{}' cannot start with a dot",
            filename
        )));
    }

    Ok(filename.to_string())
}

fn generate_random_subdomain() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();

    (0..6)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_random_subdomain() {
        let sub1 = generate_random_subdomain();
        let sub2 = generate_random_subdomain();

        assert_eq!(sub1.len(), 6);
        assert_ne!(sub1, sub2); // Likely different
        assert!(sub1.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
