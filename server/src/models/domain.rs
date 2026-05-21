use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Domain {
    pub id: i64,
    pub project_id: i64,
    pub domain: String,
    pub status: String,
    pub verification_token: String,
    pub created_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

impl Domain {
    pub fn new(project_id: i64, domain: String, verification_token: String) -> Self {
        Self {
            id: 0,
            project_id,
            domain,
            status: "pending_verification".to_string(),
            verification_token,
            created_at: Utc::now(),
            verified_at: None,
        }
    }

    pub async fn create(
        pool: &SqlitePool,
        project_id: i64,
        domain: &str,
        verification_token: &str,
    ) -> Result<Self, sqlx::Error> {
        let domain = sqlx::query_as::<_, Domain>(
            "INSERT INTO domains (project_id, domain, verification_token, status)
             VALUES (?, ?, ?, 'pending_verification')
             RETURNING *"
        )
        .bind(project_id)
        .bind(domain)
        .bind(verification_token)
        .fetch_one(pool)
        .await?;

        Ok(domain)
    }

    pub async fn find_by_domain(
        pool: &SqlitePool,
        domain: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        let domain = sqlx::query_as::<_, Domain>(
            "SELECT * FROM domains WHERE domain = ?"
        )
        .bind(domain)
        .fetch_optional(pool)
        .await?;

        Ok(domain)
    }

    pub async fn list_by_project(
        pool: &SqlitePool,
        project_id: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let domains = sqlx::query_as::<_, Domain>(
            "SELECT * FROM domains WHERE project_id = ? ORDER BY created_at DESC"
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;

        Ok(domains)
    }

    pub async fn mark_verified(
        pool: &SqlitePool,
        domain_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE domains SET status = 'verified', verified_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(domain_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn mark_failed(
        pool: &SqlitePool,
        domain_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE domains SET status = 'failed' WHERE id = ?"
        )
        .bind(domain_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn delete(
        pool: &SqlitePool,
        project_id: i64,
        domain: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM domains WHERE project_id = ? AND domain = ?"
        )
        .bind(project_id)
        .bind(domain)
        .execute(pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_new() {
        let domain = Domain::new(1, "example.com".to_string(), "token123".to_string());
        assert_eq!(domain.project_id, 1);
        assert_eq!(domain.domain, "example.com");
        assert_eq!(domain.status, "pending_verification");
        assert_eq!(domain.verification_token, "token123");
    }
}
