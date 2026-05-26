use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiKey {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub last_used_at: Option<chrono::NaiveDateTime>,
    pub revoked_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

impl ApiKey {
    pub async fn create(
        pool: &SqlitePool,
        user_id: i64,
        name: &str,
        key_prefix: &str,
        key_hash: &str,
    ) -> Result<ApiKey, sqlx::Error> {
        sqlx::query_as::<_, ApiKey>(
            r#"
            INSERT INTO api_keys (user_id, name, key_prefix, key_hash)
            VALUES (?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(name)
        .bind(key_prefix)
        .bind(key_hash)
        .fetch_one(pool)
        .await
    }

    pub async fn list_by_user(pool: &SqlitePool, user_id: i64) -> Result<Vec<ApiKey>, sqlx::Error> {
        sqlx::query_as::<_, ApiKey>(
            "SELECT * FROM api_keys WHERE user_id = ? ORDER BY created_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    pub async fn find_active_by_hash(
        pool: &SqlitePool,
        key_hash: &str,
    ) -> Result<Option<ApiKey>, sqlx::Error> {
        sqlx::query_as::<_, ApiKey>(
            "SELECT * FROM api_keys WHERE key_hash = ? AND revoked_at IS NULL LIMIT 1",
        )
        .bind(key_hash)
        .fetch_optional(pool)
        .await
    }

    pub async fn revoke_by_id_and_user(
        pool: &SqlitePool,
        id: i64,
        user_id: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE api_keys SET revoked_at = CURRENT_TIMESTAMP WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
        )
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn touch_last_used(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE api_keys SET last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::User;

    #[tokio::test]
    async fn test_create_and_lookup_api_key() {
        let pool = crate::test_utils::create_test_pool().await.unwrap();
        let user = User::create(&pool, "google", "u1", "u1@example.com", "u1")
            .await
            .unwrap();

        let created = ApiKey::create(&pool, user.id, "ci", "shk_abcd", "hash123")
            .await
            .unwrap();
        let found = ApiKey::find_active_by_hash(&pool, "hash123")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(found.id, created.id);
        assert_eq!(found.user_id, user.id);
        assert_eq!(found.name, "ci");
    }

    #[tokio::test]
    async fn test_revoke_api_key() {
        let pool = crate::test_utils::create_test_pool().await.unwrap();
        let user = User::create(&pool, "google", "u2", "u2@example.com", "u2")
            .await
            .unwrap();

        let created = ApiKey::create(&pool, user.id, "local", "shk_efgh", "hash456")
            .await
            .unwrap();

        let affected = ApiKey::revoke_by_id_and_user(&pool, created.id, user.id)
            .await
            .unwrap();
        assert_eq!(affected, 1);

        let found = ApiKey::find_active_by_hash(&pool, "hash456").await.unwrap();
        assert!(found.is_none());
    }
}
