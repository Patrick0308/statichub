use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::time::Duration;

pub async fn create_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[tokio::test]
    async fn test_create_in_memory_db() {
        let pool = create_pool(":memory:").await.unwrap();

        // Verify tables exist
        let result = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&pool)
            .await
            .unwrap();

        assert!(result.len() >= 5); // At least 5 tables

        // Verify specific tables exist
        let table_names: Vec<String> = result
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect();

        let expected_tables = vec!["users", "projects", "deploys", "custom_domains", "deploy_tokens", "oauth_sessions"];
        for table in expected_tables {
            assert!(
                table_names.contains(&table.to_string()),
                "Expected table '{}' not found. Found tables: {:?}",
                table,
                table_names
            );
        }
    }
}
