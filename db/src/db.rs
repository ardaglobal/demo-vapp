use sqlx::{PgPool, Row};
use std::env;
use std::str::FromStr;
use tracing::debug;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArithmeticTransaction {
    pub a: i32,
    pub b: i32,
    pub result: i32,
}

/// Initialize the database connection
///
/// # Errors
/// Returns error if `DATABASE_URL` environment variable is not set, connection fails,
/// or migrations fail
pub async fn init_db() -> Result<PgPool, sqlx::Error> {
    let database_url = env::var("DATABASE_URL").map_err(|_| {
        sqlx::Error::Configuration("DATABASE_URL environment variable must be set".into())
    })?;

    debug!("Connecting to PostgreSQL database...");

    // Configure pool with production settings
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(600))
        .max_lifetime(std::time::Duration::from_secs(1800))
        .connect_with(sqlx::postgres::PgConnectOptions::from_str(&database_url)?)
        .await?;

    // Run migrations
    run_migrations(&pool).await?;

    debug!("Database ready");
    Ok(pool)
}

/// Run database migrations
///
/// # Errors
/// Returns error if migration execution fails
async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    debug!("Running database migrations...");

    sqlx::migrate!("./migrations").run(pool).await?;

    debug!("Migrations completed");
    Ok(())
}

/// Store an arithmetic transaction in the database
///
/// # Errors
/// Returns error if database operation fails
pub async fn store_arithmetic_transaction(
    pool: &PgPool,
    a: i32,
    b: i32,
    result: i32,
) -> Result<(), sqlx::Error> {
    debug!("Storing transaction: a={a}, b={b}, result={result}");

    sqlx::query(
        r"
        INSERT INTO arithmetic_transactions (a, b, result)
        VALUES ($1, $2, $3)
        ON CONFLICT (a, b, result) DO NOTHING
        ",
    )
    .bind(a)
    .bind(b)
    .bind(result)
    .execute(pool)
    .await?;

    debug!("Transaction stored successfully");
    Ok(())
}

/// Get arithmetic transactions by result value
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_transactions_by_result(
    pool: &PgPool,
    result: i32,
) -> Result<Vec<ArithmeticTransaction>, sqlx::Error> {
    debug!("Looking for transactions with result: {result}");

    let rows = sqlx::query("SELECT a, b, result FROM arithmetic_transactions WHERE result = $1")
        .bind(result)
        .fetch_all(pool)
        .await?;

    let transactions: Vec<ArithmeticTransaction> = rows
        .into_iter()
        .map(|row| ArithmeticTransaction {
            a: row.get("a"),
            b: row.get("b"),
            result: row.get("result"),
        })
        .collect();

    debug!("Found {} transactions", transactions.len());
    Ok(transactions)
}

/// Get the first arithmetic transaction by result value (for compatibility with old QMDB interface)
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_value_by_result(
    pool: &PgPool,
    result: i32,
) -> Result<Option<(i32, i32)>, sqlx::Error> {
    debug!("Looking for single transaction with result: {result}");

    let row = sqlx::query("SELECT a, b FROM arithmetic_transactions WHERE result = $1 LIMIT 1")
        .bind(result)
        .fetch_optional(pool)
        .await?;

    row.map_or_else(
        || {
            debug!("No transaction found with result: {result}");
            Ok(None)
        },
        |row| {
            let a: i32 = row.get("a");
            let b: i32 = row.get("b");
            debug!("Found transaction: a={a}, b={b}");
            Ok(Some((a, b)))
        },
    )
}
