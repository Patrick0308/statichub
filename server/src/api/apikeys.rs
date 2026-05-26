use crate::{
    api::DeployState,
    error::{AppError, Result},
    middleware::{AuthMethod, AuthUser},
    models::ApiKey,
};
use axum::{
    extract::{Extension, Path, State},
    response::Json,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: i64,
    pub name: String,
    pub prefix: String,
    pub api_key: String,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyItem {
    pub id: i64,
    pub name: String,
    pub prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Serialize)]
pub struct RevokeApiKeyResponse {
    pub ok: bool,
}

pub async fn create_api_key(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>> {
    require_jwt(&auth_user)?;

    let name = payload.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("API key name cannot be empty".to_string()));
    }

    let plaintext = generate_api_key();
    let prefix = plaintext.chars().take(10).collect::<String>();
    let key_hash = crate::middleware::hash_api_key(&plaintext);

    let created = ApiKey::create(&state.pool, auth_user.user_id, name, &prefix, &key_hash).await?;

    Ok(Json(CreateApiKeyResponse {
        id: created.id,
        name: created.name,
        prefix: created.key_prefix,
        api_key: plaintext,
    }))
}

pub async fn list_api_keys(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<ApiKeyItem>>> {
    require_jwt(&auth_user)?;

    let keys = ApiKey::list_by_user(&state.pool, auth_user.user_id).await?;
    let items = keys
        .into_iter()
        .map(|k| ApiKeyItem {
            id: k.id,
            name: k.name,
            prefix: k.key_prefix,
            created_at: k.created_at.to_string(),
            last_used_at: k.last_used_at.map(|dt| dt.to_string()),
            revoked: k.revoked_at.is_some(),
        })
        .collect();

    Ok(Json(items))
}

pub async fn revoke_api_key(
    State(state): State<Arc<DeployState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<RevokeApiKeyResponse>> {
    require_jwt(&auth_user)?;

    let affected = ApiKey::revoke_by_id_and_user(&state.pool, id, auth_user.user_id).await?;
    if affected == 0 {
        return Err(AppError::NotFound("API key not found".to_string()));
    }

    Ok(Json(RevokeApiKeyResponse { ok: true }))
}

fn require_jwt(auth_user: &AuthUser) -> Result<()> {
    if auth_user.method != AuthMethod::Jwt {
        return Err(AppError::Forbidden(
            "API key management requires login".to_string(),
        ));
    }
    Ok(())
}

fn generate_api_key() -> String {
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();
    format!("shk_{}", suffix)
}
