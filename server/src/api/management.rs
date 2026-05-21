use crate::{
    api::DeployState,
    error::{AppError, Result},
    middleware::AuthUser,
    models::{Deploy, Project},
};
use axum::{
    extract::{Extension, Path, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ProjectListItem {
    pub id: i64,
    pub name: String,
    pub subdomain: String,
    pub url: String,
    pub current_version: Option<i64>,
    pub last_deployed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectDetail {
    pub id: i64,
    pub name: String,
    pub subdomain: String,
    pub url: String,
    pub current_version: Option<i64>,
    pub deploys: Vec<DeployInfo>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct DeployInfo {
    pub version: i64,
    pub deploy_id: i64,
    pub status: String,
    pub file_count: i64,
    pub total_size_bytes: i64,
    pub deployed_at: String,
    pub is_current: bool,
}

#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    pub version: i64,
}

pub async fn list_projects(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<ProjectListItem>>> {
    let projects = Project::list_by_owner(&state.pool, auth_user.user_id).await?;

    let items: Vec<ProjectListItem> = projects
        .into_iter()
        .map(|p| {
            let current_version = if let Some(_deploy_id) = p.current_deploy_id {
                // Get version from current deploy
                // For now, we'll fetch it. Consider caching or denormalizing.
                None // TODO: Query deploy.version
            } else {
                None
            };

            // subdomain is already stored with full domain for owned projects
            // e.g., "myapp.statichub.io" or just "x7k2m9" for anonymous
            let full_subdomain = if p.is_anonymous {
                // For anonymous projects, append base domain
                let base_domain = state
                    .base_url
                    .replace("http://", "")
                    .replace("https://", "");
                format!("{}.{}", p.subdomain, base_domain)
            } else {
                // For owned projects, subdomain already contains full domain
                p.subdomain.clone()
            };

            ProjectListItem {
                id: p.id,
                name: p.name.clone(),
                subdomain: full_subdomain.clone(),
                url: format!("https://{}", full_subdomain),
                current_version,
                last_deployed_at: Some(p.last_deployed_at.to_string()),
                created_at: p.created_at.to_string(),
            }
        })
        .collect();

    Ok(Json(items))
}

pub async fn get_project_info(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(project_name): Path<String>,
) -> Result<Json<ProjectDetail>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Get all deploys for this project
    let deploys = Deploy::list_by_project(&state.pool, project.id).await?;

    let deploy_infos: Vec<DeployInfo> = deploys
        .into_iter()
        .map(|d| DeployInfo {
            version: d.version,
            deploy_id: d.id,
            status: d.status,
            file_count: d.file_count,
            total_size_bytes: d.total_size_bytes,
            deployed_at: d.deployed_at.to_string(),
            is_current: Some(d.id) == project.current_deploy_id,
        })
        .collect();

    // subdomain is already stored with full domain for owned projects
    // e.g., "myapp.statichub.io" or just "x7k2m9" for anonymous
    let full_subdomain = if project.is_anonymous {
        // For anonymous projects, append base domain
        let base_domain = state
            .base_url
            .replace("http://", "")
            .replace("https://", "");
        format!("{}.{}", project.subdomain, base_domain)
    } else {
        // For owned projects, subdomain already contains full domain
        project.subdomain.clone()
    };

    Ok(Json(ProjectDetail {
        id: project.id,
        name: project.name.clone(),
        subdomain: full_subdomain.clone(),
        url: format!("https://{}", full_subdomain),
        current_version: deploy_infos
            .iter()
            .find(|d| d.is_current)
            .map(|d| d.version),
        deploys: deploy_infos,
        created_at: project.created_at.to_string(),
    }))
}

pub async fn rollback_project(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(project_name): Path<String>,
    Json(payload): Json<RollbackRequest>,
) -> Result<Json<ProjectDetail>> {
    // Find project
    let project = Project::find_by_name(&state.pool, &project_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", project_name)))?;

    // Verify ownership
    if project.owner_id != Some(auth_user.user_id) {
        return Err(AppError::Forbidden("You do not own this project".to_string()));
    }

    // Find the target deploy by version
    let target_deploy = Deploy::find_by_version(&state.pool, project.id, payload.version)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Version {} not found for project {}",
                payload.version, project_name
            ))
        })?;

    // Update project's current_deploy_id
    sqlx::query(
        "UPDATE projects SET current_deploy_id = ?, last_deployed_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(target_deploy.id)
    .bind(project.id)
    .execute(&state.pool)
    .await?;

    // Return updated project info
    get_project_info(State(state), Extension(auth_user), Path(project_name)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deploy_info_serialization() {
        let info = DeployInfo {
            version: 1,
            deploy_id: 123,
            status: "active".to_string(),
            file_count: 10,
            total_size_bytes: 1024,
            deployed_at: "2024-01-01T00:00:00".to_string(),
            is_current: true,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"is_current\":true"));
    }

    #[test]
    fn test_subdomain_construction_for_owned_projects() {
        // Owned projects store subdomain as "myapp.statichub.io"
        // Should NOT append base_domain again
        let subdomain = "myapp.statichub.io";
        let is_anonymous = false;

        // Simulate the logic from list_projects
        let full_subdomain = if is_anonymous {
            format!("{}.{}", subdomain, "localhost:3000")
        } else {
            subdomain.to_string()
        };

        assert_eq!(full_subdomain, "myapp.statichub.io");
        assert!(!full_subdomain.contains("localhost"));
    }

    #[test]
    fn test_subdomain_construction_for_anonymous_projects() {
        // Anonymous projects store subdomain as just "x7k2m9"
        // Should append base_domain
        let subdomain = "x7k2m9";
        let is_anonymous = true;

        // Simulate the logic from list_projects
        let full_subdomain = if is_anonymous {
            format!("{}.{}", subdomain, "localhost:3000")
        } else {
            subdomain.to_string()
        };

        assert_eq!(full_subdomain, "x7k2m9.localhost:3000");
    }
}
