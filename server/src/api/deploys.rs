use crate::{
    error::{AppError, Result},
    middleware::RequestHost,
    models::{Deploy, Project},
    storage::Storage,
};
use axum::{
    extract::{Multipart, State},
    Json,
};
use sqlx::SqlitePool;
use statichub_shared::{build_project_url, DeployResponse};
use std::sync::Arc;

pub struct DeployState {
    pub pool: SqlitePool,
    pub storage: Arc<dyn Storage>,
}

pub async fn create_anonymous_deploy(
    State(state): State<Arc<DeployState>>,
    axum::http::request::Parts { extensions, .. }: axum::http::request::Parts,
    mut multipart: Multipart,
) -> Result<Json<DeployResponse>> {
    // Extract host from request
    let request_host = extensions
        .get::<RequestHost>()
        .ok_or(AppError::MissingHost)?;

    // Create anonymous project
    let project = Project::create_anonymous(&state.pool, None).await?;
    let subdomain = project.subdomain.clone();

    // Create deploy record
    let storage_path = format!("{}/deploy-1", subdomain);
    let deploy = Deploy::create(&state.pool, project.id, &storage_path).await?;

    // Extract and store files from multipart
    // Process files with proper error handling and atomicity
    let upload_result =
        super::upload::process_multipart_files(&mut multipart, &state.storage, &storage_path).await;

    // If storage fails, mark deploy as failed before returning error
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

    // Update project current_deploy_id
    sqlx::query("UPDATE projects SET current_deploy_id = ? WHERE id = ?")
        .bind(deploy.id)
        .bind(project.id)
        .execute(&state.pool)
        .await?;

    Ok(Json(DeployResponse {
        url: build_project_url(&project.subdomain, &request_host.to_string()),
        subdomain: project.subdomain.clone(),
        version: None,
        deploy_id: deploy.id,
        project_id: Some(project.id),
    }))
}
