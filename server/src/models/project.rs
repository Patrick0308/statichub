use sqlx::{SqlitePool, Transaction, Sqlite};
use statichub_shared::ProjectConfig;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Project {
    pub id: i64,
    pub owner_id: Option<i64>,
    pub name: String,
    pub subdomain: String,
    pub is_anonymous: bool,
    pub current_deploy_id: Option<i64>,
    pub config: Option<String>,
    pub last_deployed_at: chrono::NaiveDateTime,
    pub created_at: chrono::NaiveDateTime,
}

impl Project {
    pub async fn create_anonymous(
        pool: &SqlitePool,
        identifier: &str,
    ) -> Result<Project, sqlx::Error> {
        let subdomain = identifier; // Store identifier only, no domain suffix
        let name = identifier;

        let project = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (name, subdomain, is_anonymous)
            VALUES (?, ?, 1)
            RETURNING *
            "#,
        )
        .bind(&name)
        .bind(&subdomain)
        .fetch_one(pool)
        .await?;

        Ok(project)
    }

    pub async fn create_owned(
        pool: &SqlitePool,
        owner_id: i64,
        name: &str,
        config: Option<&ProjectConfig>,
    ) -> Result<Project, sqlx::Error> {
        let subdomain = format!("{}.statichub.io", name);
        let config_json = config.map(|c| serde_json::to_string(c).ok()).flatten();

        let project = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (owner_id, name, subdomain, is_anonymous, config)
            VALUES (?, ?, ?, 0, ?)
            RETURNING *
            "#,
        )
        .bind(owner_id)
        .bind(name)
        .bind(&subdomain)
        .bind(config_json)
        .fetch_one(pool)
        .await?;

        Ok(project)
    }

    pub async fn create_owned_tx(
        tx: &mut Transaction<'_, Sqlite>,
        owner_id: i64,
        name: &str,
        config: Option<&ProjectConfig>,
    ) -> Result<Project, sqlx::Error> {
        let subdomain = format!("{}.statichub.io", name);
        let config_json = config.map(|c| serde_json::to_string(c).ok()).flatten();

        let project = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (owner_id, name, subdomain, is_anonymous, config)
            VALUES (?, ?, ?, 0, ?)
            RETURNING *
            "#,
        )
        .bind(owner_id)
        .bind(name)
        .bind(&subdomain)
        .bind(config_json)
        .fetch_one(&mut **tx)
        .await?;

        Ok(project)
    }

    pub async fn find_by_name(
        pool: &SqlitePool,
        name: &str,
    ) -> Result<Option<Project>, sqlx::Error> {
        let project = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE name = ?"
        )
        .bind(name)
        .fetch_optional(pool)
        .await?;

        Ok(project)
    }

    pub async fn find_by_name_tx(
        tx: &mut Transaction<'_, Sqlite>,
        name: &str,
    ) -> Result<Option<Project>, sqlx::Error> {
        let project = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE name = ?"
        )
        .bind(name)
        .fetch_optional(&mut **tx)
        .await?;

        Ok(project)
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        id: i64,
    ) -> Result<Option<Project>, sqlx::Error> {
        let project = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(project)
    }

    pub async fn find_by_subdomain(
        pool: &SqlitePool,
        subdomain: &str,
    ) -> Result<Option<Project>, sqlx::Error> {
        let project = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE subdomain = ?"
        )
        .bind(subdomain)
        .fetch_optional(pool)
        .await?;

        Ok(project)
    }

    pub async fn list_by_owner(
        pool: &SqlitePool,
        owner_id: i64,
    ) -> Result<Vec<Project>, sqlx::Error> {
        let projects = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE owner_id = ? ORDER BY created_at DESC"
        )
        .bind(owner_id)
        .fetch_all(pool)
        .await?;

        Ok(projects)
    }

    pub fn get_config(&self) -> Option<ProjectConfig> {
        self.config.as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_pool;
    use crate::models::User;

    #[tokio::test]
    async fn test_create_anonymous_project() {
        let pool = create_pool(":memory:").await.unwrap();

        let project = Project::create_anonymous(&pool, "x7k2m9").await.unwrap();
        assert!(project.is_anonymous);
        assert_eq!(project.name, "x7k2m9");
    }

    #[tokio::test]
    async fn test_create_owned_project() {
        let pool = create_pool(":memory:").await.unwrap();

        let user = User::create(&pool, "google", "123", "test@example.com", "testuser")
            .await.unwrap();

        let project = Project::create_owned(&pool, user.id, "my-app", None)
            .await.unwrap();

        assert!(!project.is_anonymous);
        assert_eq!(project.name, "my-app");
        assert_eq!(project.owner_id, Some(user.id));
    }
}
