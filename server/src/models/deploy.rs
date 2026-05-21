use sqlx::{SqlitePool, Transaction, Sqlite};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Deploy {
    pub id: i64,
    pub project_id: i64,
    pub version: i64,
    pub storage_path: String,
    pub status: String,
    pub file_count: i64,
    pub total_size_bytes: i64,
    pub deployed_at: chrono::NaiveDateTime,
}

impl Deploy {
    pub async fn create(
        pool: &SqlitePool,
        project_id: i64,
        storage_path: &str,
    ) -> Result<Deploy, sqlx::Error> {
        // Get next version number
        let next_version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM deploys WHERE project_id = ?"
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;

        let deploy = sqlx::query_as::<_, Deploy>(
            r#"
            INSERT INTO deploys (project_id, version, storage_path, status)
            VALUES (?, ?, ?, 'uploading')
            RETURNING *
            "#,
        )
        .bind(project_id)
        .bind(next_version)
        .bind(storage_path)
        .fetch_one(pool)
        .await?;

        Ok(deploy)
    }

    pub async fn create_tx(
        tx: &mut Transaction<'_, Sqlite>,
        project_id: i64,
        storage_path: &str,
    ) -> Result<Deploy, sqlx::Error> {
        // Get next version number
        let next_version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM deploys WHERE project_id = ?"
        )
        .bind(project_id)
        .fetch_one(&mut **tx)
        .await?;

        let deploy = sqlx::query_as::<_, Deploy>(
            r#"
            INSERT INTO deploys (project_id, version, storage_path, status)
            VALUES (?, ?, ?, 'uploading')
            RETURNING *
            "#,
        )
        .bind(project_id)
        .bind(next_version)
        .bind(storage_path)
        .fetch_one(&mut **tx)
        .await?;

        Ok(deploy)
    }

    pub async fn update_status(
        pool: &SqlitePool,
        deploy_id: i64,
        status: &str,
        file_count: i64,
        total_size_bytes: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE deploys
            SET status = ?, file_count = ?, total_size_bytes = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(file_count)
        .bind(total_size_bytes)
        .bind(deploy_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn list_by_project(
        pool: &SqlitePool,
        project_id: i64,
        limit: i64,
    ) -> Result<Vec<Deploy>, sqlx::Error> {
        let deploys = sqlx::query_as::<_, Deploy>(
            "SELECT * FROM deploys WHERE project_id = ? ORDER BY version DESC LIMIT ?"
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(deploys)
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        id: i64,
    ) -> Result<Option<Deploy>, sqlx::Error> {
        sqlx::query_as::<_, Deploy>("SELECT * FROM deploys WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_by_version(
        pool: &SqlitePool,
        project_id: i64,
        version: i64,
    ) -> Result<Option<Deploy>, sqlx::Error> {
        let deploy = sqlx::query_as::<_, Deploy>(
            "SELECT * FROM deploys WHERE project_id = ? AND version = ?"
        )
        .bind(project_id)
        .bind(version)
        .fetch_optional(pool)
        .await?;

        Ok(deploy)
    }

    pub async fn delete_old_deploys(
        pool: &SqlitePool,
        project_id: i64,
        keep_count: i64,
    ) -> Result<Vec<String>, sqlx::Error> {
        // Get storage paths of deploys to delete
        let storage_paths: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT storage_path FROM deploys
            WHERE project_id = ?
            ORDER BY version DESC
            LIMIT -1 OFFSET ?
            "#,
        )
        .bind(project_id)
        .bind(keep_count)
        .fetch_all(pool)
        .await?;

        // Delete them
        if !storage_paths.is_empty() {
            sqlx::query(
                r#"
                DELETE FROM deploys
                WHERE project_id = ?
                AND id NOT IN (
                    SELECT id FROM deploys
                    WHERE project_id = ?
                    ORDER BY version DESC
                    LIMIT ?
                )
                "#,
            )
            .bind(project_id)
            .bind(project_id)
            .bind(keep_count)
            .execute(pool)
            .await?;
        }

        Ok(storage_paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_pool;
    use crate::models::{User, Project};

    #[tokio::test]
    async fn test_create_deploy_increments_version() {
        let pool = create_pool(":memory:").await.unwrap();

        let user = User::create(&pool, "google", "123", "test@example.com", "testuser")
            .await.unwrap();
        let project = Project::create_owned(&pool, user.id, "test", None)
            .await.unwrap();

        let deploy1 = Deploy::create(&pool, project.id, "test/deploy-1").await.unwrap();
        assert_eq!(deploy1.version, 1);

        let deploy2 = Deploy::create(&pool, project.id, "test/deploy-2").await.unwrap();
        assert_eq!(deploy2.version, 2);
    }

    #[tokio::test]
    async fn test_delete_old_deploys() {
        let pool = create_pool(":memory:").await.unwrap();

        let user = User::create(&pool, "google", "123", "test@example.com", "testuser")
            .await.unwrap();
        let project = Project::create_owned(&pool, user.id, "test", None)
            .await.unwrap();

        // Create 5 deploys
        for i in 1..=5 {
            Deploy::create(&pool, project.id, &format!("test/deploy-{}", i))
                .await.unwrap();
        }

        // Keep only 3 most recent
        let deleted = Deploy::delete_old_deploys(&pool, project.id, 3)
            .await.unwrap();

        assert_eq!(deleted.len(), 2);

        let remaining = Deploy::list_by_project(&pool, project.id, 10)
            .await.unwrap();
        assert_eq!(remaining.len(), 3);
    }
}
