use sqlx::{SqlitePool, sqlite::{SqlitePoolOptions, SqliteConnectOptions}};
use std::time::Duration;
use std::str::FromStr;

/// Create database connection pool without running migrations
pub async fn create_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let connect_options = SqliteConnectOptions::from_str(database_url)?
        .foreign_keys(true)
        .create_if_missing(false);  // Do NOT auto-create database

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(connect_options)
        .await?;

    Ok(pool)
}

/// Run database migrations
pub async fn migrate(database_url: &str) -> Result<(), sqlx::Error> {
    // Create database if it doesn't exist (only for migrate command)
    let connect_options = SqliteConnectOptions::from_str(database_url)?
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options)
        .await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    Ok(())
}

/// Check migration status
pub async fn migration_status(database_url: &str) -> Result<Vec<MigrationInfo>, sqlx::Error> {
    let pool = create_pool(database_url).await?;

    let applied: Vec<(i64, String)> = sqlx::query_as(
        "SELECT version, description FROM _sqlx_migrations ORDER BY version"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let embedded = sqlx::migrate!("./migrations").migrations;

    let mut result = Vec::new();
    for migration in embedded.iter() {
        let applied_version = applied.iter()
            .find(|(v, _)| *v == migration.version)
            .is_some();

        result.push(MigrationInfo {
            version: migration.version,
            description: migration.description.to_string(),
            applied: applied_version,
        });
    }

    Ok(result)
}

#[derive(Debug)]
pub struct MigrationInfo {
    pub version: i64,
    pub description: String,
    pub applied: bool,
}

/// Reset database (DROP all tables and re-run migrations)
/// WARNING: This will delete all data!
pub async fn reset(database_url: &str) -> Result<(), sqlx::Error> {
    let connect_options = SqliteConnectOptions::from_str(database_url)?
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options)
        .await?;

    // Get all table names
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
    )
    .fetch_all(&pool)
    .await?;

    // Drop all tables
    for (table,) in tables {
        sqlx::query(&format!("DROP TABLE IF EXISTS {}", table))
            .execute(&pool)
            .await?;
    }

    // Re-run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[tokio::test]
    async fn test_create_in_memory_db() {
        let pool = crate::test_utils::create_test_pool().await.unwrap();

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

        let expected_tables = vec!["users", "projects", "deploys", "domains", "deploy_tokens", "oauth_sessions"];
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
