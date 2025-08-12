use chrono::{DateTime, Utc};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SindriProofRecord {
    pub result: i32,
    pub proof_id: String,
    pub circuit_id: Option<String>,
    pub status: Option<String>,
}

/// Initialize the database connection with a specific URL
///
/// # Errors
/// Returns error if connection fails or migrations fail
pub async fn init_db_with_url(database_url: &str) -> Result<PgPool, sqlx::Error> {
    debug!("Connecting to PostgreSQL database...");

    // Configure pool with production settings
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(600))
        .max_lifetime(std::time::Duration::from_secs(1800))
        .connect_with(sqlx::postgres::PgConnectOptions::from_str(database_url)?)
        .await?;

    // Run migrations
    run_migrations(&pool).await?;

    debug!("Database ready");
    Ok(pool)
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

    init_db_with_url(&database_url).await
}

/// Run database migrations
///
/// # Errors
/// Returns error if migration execution fails
async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    debug!("Running database migrations...");

    sqlx::migrate!().run(pool).await?;

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
) -> Result<Option<(i32, i32, DateTime<Utc>)>, sqlx::Error> {
    debug!("Looking for single transaction with result: {result}");

    let row = sqlx::query(
        "SELECT a, b, created_at FROM arithmetic_transactions WHERE result = $1 LIMIT 1",
    )
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
            let created_at: DateTime<Utc> = row.get("created_at");
            debug!("Found transaction: a={a}, b={b}, created_at={created_at}");
            Ok(Some((a, b, created_at)))
        },
    )
}

/// Store or update a Sindri proof record by result
///
/// # Errors
/// Returns error if database operation fails
pub async fn upsert_sindri_proof(
    pool: &PgPool,
    result: i32,
    proof_id: &str,
    circuit_id: Option<String>,
    status: Option<String>,
) -> Result<(), sqlx::Error> {
    debug!("Upserting Sindri proof: result={result}, proof_id={proof_id}");

    sqlx::query(
        r"
        INSERT INTO sindri_proofs (result, proof_id, circuit_id, status)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (result)
        DO UPDATE SET proof_id = EXCLUDED.proof_id,
                      circuit_id = EXCLUDED.circuit_id,
                      status = EXCLUDED.status
        ",
    )
    .bind(result)
    .bind(proof_id)
    .bind(circuit_id.as_deref())
    .bind(status.as_deref())
    .execute(pool)
    .await?;

    Ok(())
}

/// Fetch a Sindri proof record by result
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_sindri_proof_by_result(
    pool: &PgPool,
    result: i32,
) -> Result<Option<SindriProofRecord>, sqlx::Error> {
    let row = sqlx::query(
        r"SELECT result, proof_id, circuit_id, status FROM sindri_proofs WHERE result = $1 LIMIT 1",
    )
    .bind(result)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| SindriProofRecord {
        result: row.get("result"),
        proof_id: row.get("proof_id"),
        circuit_id: row.get::<Option<String>, _>("circuit_id"),
        status: row.get::<Option<String>, _>("status"),
    }))
}
