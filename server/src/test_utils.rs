use sqlx::{SqlitePool, sqlite::{SqlitePoolOptions, SqliteConnectOptions}};
use std::str::FromStr;

/// Create an in-memory database pool with migrations applied for testing
pub async fn create_test_pool() -> Result<SqlitePool, sqlx::Error> {
    let connect_options = SqliteConnectOptions::from_str(":memory:")?
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options)
        .await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    Ok(pool)
}
