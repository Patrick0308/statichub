use axum::{
    extract::{State, Multipart},
    Json,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use statichub_shared::DeployResponse;
use crate::{error::Result, storage::Storage, models::{Project, Deploy}};

pub struct DeployState {
    pub pool: SqlitePool,
    pub storage: Arc<dyn Storage>,
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

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.file_name().unwrap_or("file").to_string();
        let data = field.bytes().await.unwrap();

        total_size += data.len() as u64;
        file_count += 1;

        state.storage.store_file(&storage_path, &name, &data).await
            .map_err(|e| crate::error::AppError::Storage(e.to_string()))?;
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
        version: None,
        deploy_id: deploy.id.to_string(),
    }))
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
    use crate::db::create_pool;
    use crate::storage::FilesystemStorage;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_random_subdomain() {
        let sub1 = generate_random_subdomain();
        let sub2 = generate_random_subdomain();

        assert_eq!(sub1.len(), 6);
        assert_ne!(sub1, sub2); // Likely different
        assert!(sub1.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
