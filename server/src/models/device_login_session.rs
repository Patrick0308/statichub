use sqlx::SqlitePool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceLoginStatus {
    Pending,
    Verified,
    Approved,
    Denied,
    Expired,
    Consumed,
}

impl DeviceLoginStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceLoginStatus::Pending => "pending",
            DeviceLoginStatus::Verified => "verified",
            DeviceLoginStatus::Approved => "approved",
            DeviceLoginStatus::Denied => "denied",
            DeviceLoginStatus::Expired => "expired",
            DeviceLoginStatus::Consumed => "consumed",
        }
    }

    pub fn from_str(value: &str) -> DeviceLoginStatus {
        match value {
            "verified" => DeviceLoginStatus::Verified,
            "approved" => DeviceLoginStatus::Approved,
            "denied" => DeviceLoginStatus::Denied,
            "expired" => DeviceLoginStatus::Expired,
            "consumed" => DeviceLoginStatus::Consumed,
            "pending" => DeviceLoginStatus::Pending,
            _ => DeviceLoginStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeviceLoginSession {
    pub id: i64,
    pub device_code_hash: String,
    pub user_code: String,
    pub oauth_state: Option<String>,
    pub status: String,
    pub token: Option<String>,
    pub poll_interval_seconds: i64,
    pub last_polled_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
}

impl DeviceLoginSession {
    pub async fn create(
        pool: &SqlitePool,
        device_code_hash: &str,
        user_code: &str,
        expires_at: chrono::NaiveDateTime,
        poll_interval_seconds: i64,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            r#"
            INSERT INTO device_login_sessions (
                device_code_hash,
                user_code,
                status,
                expires_at,
                poll_interval_seconds
            )
            VALUES (?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(device_code_hash)
        .bind(user_code)
        .bind(DeviceLoginStatus::Pending.as_str())
        .bind(expires_at)
        .bind(poll_interval_seconds)
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_device_code_hash(
        pool: &SqlitePool,
        device_code_hash: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            "SELECT * FROM device_login_sessions WHERE device_code_hash = ? LIMIT 1",
        )
        .bind(device_code_hash)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_user_code(
        pool: &SqlitePool,
        user_code: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            "SELECT * FROM device_login_sessions WHERE user_code = ? LIMIT 1",
        )
        .bind(user_code)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_oauth_state(
        pool: &SqlitePool,
        oauth_state: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            "SELECT * FROM device_login_sessions WHERE oauth_state = ? LIMIT 1",
        )
        .bind(oauth_state)
        .fetch_optional(pool)
        .await
    }

    pub async fn attach_oauth_state(
        pool: &SqlitePool,
        id: i64,
        oauth_state: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            r#"
            UPDATE device_login_sessions
            SET oauth_state = ?, status = ?
            WHERE id = ? AND status = ?
            RETURNING *
            "#,
        )
        .bind(oauth_state)
        .bind(DeviceLoginStatus::Verified.as_str())
        .bind(id)
        .bind(DeviceLoginStatus::Pending.as_str())
        .fetch_one(pool)
        .await
    }

    pub async fn approve(pool: &SqlitePool, id: i64, token: &str) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, DeviceLoginSession>(
            r#"
            UPDATE device_login_sessions
            SET token = ?, status = ?
            WHERE id = ? AND status = ?
            RETURNING *
            "#,
        )
        .bind(token)
        .bind(DeviceLoginStatus::Approved.as_str())
        .bind(id)
        .bind(DeviceLoginStatus::Verified.as_str())
        .fetch_one(pool)
        .await
    }

    pub async fn mark_polled(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE device_login_sessions SET last_polled_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn consume_token(pool: &SqlitePool, id: i64) -> Result<Option<String>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let token = sqlx::query_scalar::<_, String>(
            r#"
            SELECT token
            FROM device_login_sessions
            WHERE id = ? AND status = ? AND token IS NOT NULL
            LIMIT 1
            "#,
        )
        .bind(id)
        .bind(DeviceLoginStatus::Approved.as_str())
        .fetch_optional(&mut *tx)
        .await?;

        if token.is_some() {
            sqlx::query(
                r#"
                UPDATE device_login_sessions
                SET token = NULL, status = ?, consumed_at = CURRENT_TIMESTAMP
                WHERE id = ?
                "#,
            )
            .bind(DeviceLoginStatus::Consumed.as_str())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(token)
    }

    pub async fn expire_old(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE device_login_sessions
            SET status = ?, token = NULL
            WHERE expires_at <= CURRENT_TIMESTAMP
                AND status IN (?, ?, ?)
            "#,
        )
        .bind(DeviceLoginStatus::Expired.as_str())
        .bind(DeviceLoginStatus::Pending.as_str())
        .bind(DeviceLoginStatus::Verified.as_str())
        .bind(DeviceLoginStatus::Approved.as_str())
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub fn status(&self) -> DeviceLoginStatus {
        DeviceLoginStatus::from_str(&self.status)
    }

    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().naive_utc() >= self.expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_find_approve_and_consume_session() {
        let pool = crate::test_utils::create_test_pool().await.unwrap();
        let expires_at = chrono::Utc::now().naive_utc() + chrono::Duration::minutes(10);

        let session =
            DeviceLoginSession::create(&pool, "device-hash-123", "USER123", expires_at, 5)
                .await
                .unwrap();

        let found_by_user_code = DeviceLoginSession::find_by_user_code(&pool, "USER123")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_by_user_code.id, session.id);

        let verified = DeviceLoginSession::attach_oauth_state(&pool, session.id, "oauth-state-123")
            .await
            .unwrap();
        assert_eq!(verified.status(), DeviceLoginStatus::Verified);

        let approved = DeviceLoginSession::approve(&pool, session.id, "jwt123")
            .await
            .unwrap();
        assert_eq!(approved.status(), DeviceLoginStatus::Approved);

        let token = DeviceLoginSession::consume_token(&pool, session.id)
            .await
            .unwrap();
        assert_eq!(token.as_deref(), Some("jwt123"));

        let token = DeviceLoginSession::consume_token(&pool, session.id)
            .await
            .unwrap();
        assert!(token.is_none());
    }
}
