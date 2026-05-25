use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub oauth_provider: String,
    pub oauth_id: String,
    pub email: String,
    pub username: String,
    pub created_at: chrono::NaiveDateTime,
}

impl User {
    pub async fn create(
        pool: &SqlitePool,
        oauth_provider: &str,
        oauth_id: &str,
        email: &str,
        username: &str,
    ) -> Result<User, sqlx::Error> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (oauth_provider, oauth_id, email, username)
            VALUES (?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(oauth_provider)
        .bind(oauth_id)
        .bind(email)
        .bind(username)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_oauth(
        pool: &SqlitePool,
        oauth_provider: &str,
        oauth_id: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE oauth_provider = ? AND oauth_id = ?",
        )
        .bind(oauth_provider)
        .bind(oauth_id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_id(pool: &SqlitePool, id: i64) -> Result<Option<User>, sqlx::Error> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_find_user() {
        let pool = crate::test_utils::create_test_pool().await.unwrap();

        let user = User::create(&pool, "google", "123456", "test@example.com", "testuser")
            .await
            .unwrap();

        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.username, "testuser");

        let found = User::find_by_oauth(&pool, "google", "123456")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(found.id, user.id);
    }
}
