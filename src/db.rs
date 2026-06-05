use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

pub async fn create_pool(db_path: &str) -> Result<SqlitePool, sqlx::Error> {
    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(db_path).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite:{db_path}?mode=rwc"))
        .await?;

    // Enable WAL mode (must be done outside a transaction)
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;

    // Enable foreign key enforcement
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    // Wait up to 5s when the database is locked before returning SQLITE_BUSY
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(&pool)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
