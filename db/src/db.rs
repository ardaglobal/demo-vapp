use sqlx::{PgPool, Row};
use std::env;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[derive(Debug)]
pub struct ArithmeticTransaction {
    pub a: i32,
    pub b: i32,
    pub result: i32,
}

/// Initialize the database connection
///
/// # Panics
/// Panics if `DATABASE_URL` environment variable is not set or connection fails
pub async fn init_db() -> PgPool {
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL environment variable must be set");

    println!("Connecting to PostgreSQL database...");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL database");

    // Run migrations
    run_migrations(&pool).await;

    println!("Database ready");
    pool
}

/// Run database migrations
async fn run_migrations(pool: &PgPool) {
    println!("Running database migrations...");

    let migration_sql = r"
        CREATE TABLE IF NOT EXISTS arithmetic_transactions (
            id SERIAL PRIMARY KEY,
            a INTEGER NOT NULL,
            b INTEGER NOT NULL,
            result INTEGER NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(a, b, result)
        );

        CREATE INDEX IF NOT EXISTS idx_arithmetic_result ON arithmetic_transactions(result);
        CREATE INDEX IF NOT EXISTS idx_arithmetic_created_at ON arithmetic_transactions(created_at);
    ";

    sqlx::query(migration_sql)
        .execute(pool)
        .await
        .expect("Failed to run migrations");

    println!("Migrations completed");
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
    println!("Storing transaction: a={a}, b={b}, result={result}");

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

    println!("Transaction stored successfully");
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
    println!("Looking for transactions with result: {result}");

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

    println!("Found {} transactions", transactions.len());
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
    println!("Looking for single transaction with result: {result}");

    let row = sqlx::query("SELECT a, b FROM arithmetic_transactions WHERE result = $1 LIMIT 1")
        .bind(result)
        .fetch_optional(pool)
        .await?;

    row.map_or_else(
        || {
            println!("No transaction found with result: {result}");
            Ok(None)
        },
        |row| {
            let a: i32 = row.get("a");
            let b: i32 = row.get("b");
            println!("Found transaction: a={a}, b={b}");
            Ok(Some((a, b)))
        },
    )
}
