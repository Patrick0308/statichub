use crate::{
    api::DeployState,
    error::{AppError, Result},
    middleware::AuthUser,
    models::{Deploy, Domain, Project},
};
use axum::{
    extract::{Extension, Path, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct AddDomainRequest {
    pub domain: String,
}

#[derive(Debug, Serialize)]
pub struct DomainResponse {
    pub id: i64,
    pub domain: String,
    pub status: String,
    pub verification_token: String,
    pub verification_instructions: String,
    pub created_at: String,
    pub verified_at: Option<String>,
}

impl From<Domain> for DomainResponse {
    fn from(d: Domain) -> Self {
        let instructions = format!(
            "Upload a file named 'statichub-verify.txt' to your domain root containing: {}",
            d.verification_token
        );

        Self {
            id: d.id,
            domain: d.domain,
            status: d.status,
            verification_token: d.verification_token,
            verification_instructions: instructions,
            created_at: d.created_at.to_string(),
            verified_at: d.verified_at.map(|dt| dt.to_string()),
        }
    }
}

pub async fn add_domain(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(project_name): Path<String>,
    Json(payload): Json<AddDomainRequest>,
) -> Result<Json<DomainResponse>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Validate domain format
    let domain = payload.domain.trim().to_lowercase();
    if domain.is_empty() || !domain.contains('.') {
        return Err(AppError::BadRequest("Invalid domain format".to_string()));
    }

    // Check if domain already exists
    if let Some(_) = Domain::find_by_domain(&state.pool, &domain).await? {
        return Err(AppError::Conflict("Domain already in use".to_string()));
    }

    // Generate verification token
    let verification_token = uuid::Uuid::new_v4().to_string();

    // Create domain
    let domain = Domain::create(&state.pool, project.id, &domain, &verification_token).await?;

    Ok(Json(domain.into()))
}

pub async fn list_domains(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(project_name): Path<String>,
) -> Result<Json<Vec<DomainResponse>>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Get domains
    let domains = Domain::list_by_project(&state.pool, project.id).await?;

    Ok(Json(domains.into_iter().map(|d| d.into()).collect()))
}

pub async fn verify_domain(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((project_name, domain_name)): Path<(String, String)>,
) -> Result<Json<DomainResponse>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Find domain
    let domain = Domain::find_by_domain(&state.pool, &domain_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Domain not found: {}", domain_name)))?;

    // Verify domain belongs to project
    if domain.project_id != project.id {
        return Err(AppError::Forbidden("Domain does not belong to this project".to_string()));
    }

    // Already verified?
    if domain.status == "verified" {
        return Ok(Json(domain.into()));
    }

    // Get current deploy
    let deploy_id = project.current_deploy_id.ok_or_else(|| {
        AppError::BadRequest("Project has no deployment yet".to_string())
    })?;

    let deploy = Deploy::find_by_id(&state.pool, deploy_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Deploy not found: {}", deploy_id)))?;

    // Try to fetch verification file
    let verification_path = "statichub-verify.txt";
    match state.storage.get_file(&deploy.storage_path, verification_path).await {
        Ok(content) => {
            let content_str = String::from_utf8_lossy(&content).trim().to_string();

            if content_str == domain.verification_token {
                // Verification successful
                Domain::mark_verified(&state.pool, domain.id).await?;

                let updated_domain = Domain::find_by_domain(&state.pool, &domain_name)
                    .await?
                    .unwrap();

                Ok(Json(updated_domain.into()))
            } else {
                // Token mismatch
                Domain::mark_failed(&state.pool, domain.id).await?;
                Err(AppError::BadRequest("Verification token mismatch".to_string()))
            }
        }
        Err(_) => {
            // File not found
            Domain::mark_failed(&state.pool, domain.id).await?;
            Err(AppError::BadRequest("Verification file not found".to_string()))
        }
    }
}

pub async fn remove_domain(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((project_name, domain_name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Delete domain
    Domain::delete(&state.pool, project.id, &domain_name).await?;

    Ok(Json(serde_json::json!({ "success": true })))
}
