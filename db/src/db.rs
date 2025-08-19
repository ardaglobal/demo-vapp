use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::env;
use std::str::FromStr;
use tracing::debug;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

// ============================================================================
// NEW BATCH PROCESSING TYPES
// ============================================================================

/// Incoming transaction waiting to be batched
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncomingTransaction {
    pub id: i32,
    pub amount: i32,
    pub included_in_batch_id: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// Batch of transactions with ZK proof
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofBatch {
    pub id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_ids: Vec<i32>,
    pub sindri_proof_id: Option<String>,
    pub proof_status: String, // pending, proven, failed
    pub created_at: DateTime<Utc>,
    pub proven_at: Option<DateTime<Utc>>,
}

/// ADS/Merkle tree state commitment for smart contract
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsStateCommit {
    pub id: i32,
    pub batch_id: i32,
    pub merkle_root: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

/// Current counter state with Merkle root
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterState {
    pub counter_value: i64,
    pub merkle_root: Option<Vec<u8>>,
    pub last_batch_id: Option<i32>,
}

/// Contract submission data (public/private split)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractSubmissionData {
    pub public: ContractPublicData,
    pub private: ContractPrivateData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractPublicData {
    pub prev_merkle_root: String,
    pub new_merkle_root: String,
    pub zk_proof: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractPrivateData {
    pub prev_counter_value: i64,
    pub new_counter_value: i64,
    pub transactions: Vec<i32>,
}

// ============================================================================
// DATABASE CONNECTION FUNCTIONS
// ============================================================================

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

// ============================================================================
// TRANSACTION FUNCTIONS
// ============================================================================

/// Submit a new transaction to the queue
///
/// # Errors
/// Returns error if database operation fails
pub async fn submit_transaction(
    pool: &PgPool,
    amount: i32,
) -> Result<IncomingTransaction, sqlx::Error> {
    debug!("Submitting transaction: amount={amount}");

    let row = sqlx::query!(
        "INSERT INTO incoming_transactions (amount) VALUES ($1) RETURNING id, amount, included_in_batch_id, created_at",
        amount
    )
    .fetch_one(pool)
    .await?;

    let transaction = IncomingTransaction {
        id: row.id,
        amount: row.amount,
        included_in_batch_id: row.included_in_batch_id,
        created_at: row.created_at.unwrap_or_else(|| Utc::now()),
    };

    debug!("Transaction submitted: id={}", transaction.id);
    Ok(transaction)
}

/// Get pending transactions (not yet batched)
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_pending_transactions(
    pool: &PgPool,
) -> Result<Vec<IncomingTransaction>, sqlx::Error> {
    debug!("Getting pending transactions");

    let rows = sqlx::query!(
        "SELECT transaction_id as id, amount, created_at FROM get_unbatched_transactions(1000)"
    )
    .fetch_all(pool)
    .await?;

    let transactions: Vec<IncomingTransaction> = rows
        .into_iter()
        .map(|row| IncomingTransaction {
            id: row.id.unwrap_or(0),
            amount: row.amount.unwrap_or(0),
            included_in_batch_id: None,
            created_at: row.created_at.unwrap_or_else(|| Utc::now()),
        })
        .collect();

    debug!("Found {} pending transactions", transactions.len());
    Ok(transactions)
}

// ============================================================================
// BATCH FUNCTIONS
// ============================================================================

/// Create a new batch from pending transactions
///
/// # Errors
/// Returns error if database operation fails
pub async fn create_batch(
    pool: &PgPool,
    batch_size: Option<i32>,
) -> Result<Option<ProofBatch>, sqlx::Error> {
    let size = batch_size.unwrap_or(10);
    debug!("Creating batch with size: {size}");

    let batch_id: i32 = sqlx::query_scalar!("SELECT create_batch($1)", size)
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

    if batch_id == 0 {
        debug!("No transactions to batch");
        return Ok(None);
    }

    // Get the created batch details
    get_batch_by_id(pool, batch_id).await.map(Some)
}

/// Get batch by ID
///
/// # Errors
/// Returns error if database operation fails or batch not found
pub async fn get_batch_by_id(pool: &PgPool, batch_id: i32) -> Result<ProofBatch, sqlx::Error> {
    debug!("Getting batch: id={batch_id}");

    let row = sqlx::query!(
        r"
        SELECT id, previous_counter_value, final_counter_value, transaction_ids, 
               sindri_proof_id, proof_status, created_at, proven_at
        FROM proof_batches 
        WHERE id = $1
        ",
        batch_id
    )
    .fetch_one(pool)
    .await?;

    let batch = ProofBatch {
        id: row.id,
        previous_counter_value: row.previous_counter_value,
        final_counter_value: row.final_counter_value,
        transaction_ids: row.transaction_ids,
        sindri_proof_id: row.sindri_proof_id,
        proof_status: row.proof_status.unwrap_or_else(|| "pending".to_string()),
        created_at: row.created_at.unwrap_or_else(|| Utc::now()),
        proven_at: row.proven_at,
    };

    debug!(
        "Found batch: id={}, status={}",
        batch.id, batch.proof_status
    );
    Ok(batch)
}

/// Get all batches (paginated)
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_all_batches(
    pool: &PgPool,
    limit: Option<i32>,
) -> Result<Vec<ProofBatch>, sqlx::Error> {
    let limit_val = limit.unwrap_or(50);
    debug!("Getting batches with limit: {limit_val}");

    let rows = sqlx::query!(
        r"
        SELECT id, previous_counter_value, final_counter_value, transaction_ids,
               sindri_proof_id, proof_status, created_at, proven_at
        FROM proof_batches 
        ORDER BY id DESC 
        LIMIT $1
        ",
        limit_val as i64
    )
    .fetch_all(pool)
    .await?;

    let batches: Vec<ProofBatch> = rows
        .into_iter()
        .map(|row| ProofBatch {
            id: row.id,
            previous_counter_value: row.previous_counter_value,
            final_counter_value: row.final_counter_value,
            transaction_ids: row.transaction_ids,
            sindri_proof_id: row.sindri_proof_id,
            proof_status: row.proof_status.unwrap_or_else(|| "pending".to_string()),
            created_at: row.created_at.unwrap_or_else(|| Utc::now()),
            proven_at: row.proven_at,
        })
        .collect();

    debug!("Found {} batches", batches.len());
    Ok(batches)
}

/// Update batch with Sindri proof ID and status
///
/// # Errors
/// Returns error if database operation fails
pub async fn update_batch_proof(
    pool: &PgPool,
    batch_id: i32,
    proof_id: &str,
    status: &str,
) -> Result<(), sqlx::Error> {
    debug!("Updating batch {batch_id} with proof {proof_id}, status: {status}");

    let proven_at = if status == "proven" {
        Some(Utc::now())
    } else {
        None
    };

    sqlx::query!(
        r"
        UPDATE proof_batches 
        SET sindri_proof_id = $1, proof_status = $2, proven_at = $3
        WHERE id = $4
        ",
        proof_id,
        status,
        proven_at,
        batch_id
    )
    .execute(pool)
    .await?;

    debug!("Batch updated successfully");
    Ok(())
}

// ============================================================================
// STATE FUNCTIONS
// ============================================================================

/// Get current counter value
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_current_counter_value(pool: &PgPool) -> Result<i64, sqlx::Error> {
    debug!("Getting current counter value");

    let value: i64 = sqlx::query_scalar!("SELECT get_current_counter_value()")
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

    debug!("Current counter value: {value}");
    Ok(value)
}

/// Get current counter state with latest Merkle root
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_current_state(pool: &PgPool) -> Result<CounterState, sqlx::Error> {
    debug!("Getting current counter state");

    let counter_value = get_current_counter_value(pool).await?;

    // Get latest proven batch and its Merkle root
    let row = sqlx::query!(
        r"
        SELECT pb.id, ads.merkle_root
        FROM proof_batches pb
        LEFT JOIN ads_state_commits ads ON pb.id = ads.batch_id
        WHERE pb.proof_status = 'proven'
        ORDER BY pb.id DESC
        LIMIT 1
        "
    )
    .fetch_optional(pool)
    .await?;

    let (last_batch_id, merkle_root) = match row {
        Some(row) => (Some(row.id), Some(row.merkle_root)),
        None => (None, None),
    };

    let state = CounterState {
        counter_value,
        merkle_root,
        last_batch_id,
    };

    debug!(
        "Current state: counter={}, batch_id={:?}",
        state.counter_value, state.last_batch_id
    );
    Ok(state)
}

// ============================================================================
// ADS/MERKLE TREE FUNCTIONS
// ============================================================================

/// Store Merkle root for a batch
///
/// # Errors
/// Returns error if database operation fails
pub async fn store_ads_state_commit(
    pool: &PgPool,
    batch_id: i32,
    merkle_root: &[u8],
) -> Result<AdsStateCommit, sqlx::Error> {
    debug!("Storing ADS state commit for batch {batch_id}");

    let row = sqlx::query!(
        r"
        INSERT INTO ads_state_commits (batch_id, merkle_root)
        VALUES ($1, $2)
        RETURNING id, batch_id, merkle_root, created_at
        ",
        batch_id,
        merkle_root
    )
    .fetch_one(pool)
    .await?;

    let commit = AdsStateCommit {
        id: row.id,
        batch_id: row.batch_id,
        merkle_root: row.merkle_root,
        created_at: row.created_at.unwrap_or_else(|| Utc::now()),
    };

    debug!("ADS state commit stored: id={}", commit.id);
    Ok(commit)
}

/// Get contract submission data for a batch (public/private split)
///
/// # Errors
/// Returns error if database operation fails or batch not found
pub async fn get_contract_submission_data(
    pool: &PgPool,
    batch_id: i32,
) -> Result<ContractSubmissionData, sqlx::Error> {
    debug!("Getting contract submission data for batch {batch_id}");

    // Get batch details
    let batch = get_batch_by_id(pool, batch_id).await?;

    // Get previous and new Merkle roots
    let prev_root = if batch.previous_counter_value == 0 {
        // Genesis state - use empty root
        "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
    } else {
        // Get previous batch's Merkle root
        let prev_row = sqlx::query!(
            r"
            SELECT ads.merkle_root
            FROM proof_batches pb
            JOIN ads_state_commits ads ON pb.id = ads.batch_id
            WHERE pb.final_counter_value = $1 AND pb.proof_status = 'proven'
            ORDER BY pb.id DESC
            LIMIT 1
            ",
            batch.previous_counter_value
        )
        .fetch_optional(pool)
        .await?;

        match prev_row {
            Some(row) => format!("0x{}", hex::encode(row.merkle_root)),
            None => {
                "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
            }
        }
    };

    // Get new Merkle root
    let new_row = sqlx::query!(
        "SELECT merkle_root FROM ads_state_commits WHERE batch_id = $1",
        batch_id
    )
    .fetch_optional(pool)
    .await?;

    let new_root = match new_row {
        Some(row) => format!("0x{}", hex::encode(row.merkle_root)),
        None => return Err(sqlx::Error::RowNotFound),
    };

    // Get transaction amounts for the batch
    let transaction_amounts: Vec<i32> = sqlx::query!(
        "SELECT amount FROM incoming_transactions WHERE id = ANY($1) ORDER BY id",
        &batch.transaction_ids
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| row.amount)
    .collect();

    let data = ContractSubmissionData {
        public: ContractPublicData {
            prev_merkle_root: prev_root,
            new_merkle_root: new_root,
            zk_proof: batch
                .sindri_proof_id
                .unwrap_or_else(|| "pending".to_string()),
        },
        private: ContractPrivateData {
            prev_counter_value: batch.previous_counter_value,
            new_counter_value: batch.final_counter_value,
            transactions: transaction_amounts,
        },
    };

    debug!("Contract submission data prepared for batch {batch_id}");
    Ok(data)
}

/// Get proven batches that haven't been posted to the smart contract yet
///
/// # Errors
/// Returns error if database operation fails
pub async fn get_proven_unposted_batches(
    pool: &PgPool,
    limit: Option<i32>,
) -> Result<Vec<ProofBatch>, sqlx::Error> {
    let limit = limit.unwrap_or(10);
    debug!("Getting proven unposted batches with limit: {limit}");

    let rows = sqlx::query!(
        r"
        SELECT id, previous_counter_value, final_counter_value, transaction_ids,
               sindri_proof_id, proof_status, created_at, proven_at
        FROM proof_batches
        WHERE proof_status = 'proven' 
          AND posted_to_contract = FALSE
          AND sindri_proof_id IS NOT NULL
        ORDER BY id ASC
        LIMIT $1
        ",
        limit as i64
    )
    .fetch_all(pool)
    .await?;

    let batches: Vec<ProofBatch> = rows
        .into_iter()
        .map(|row| ProofBatch {
            id: row.id,
            previous_counter_value: row.previous_counter_value,
            final_counter_value: row.final_counter_value,
            transaction_ids: row.transaction_ids,
            sindri_proof_id: row.sindri_proof_id,
            proof_status: row.proof_status.unwrap_or_else(|| "pending".to_string()),
            created_at: row.created_at.unwrap_or_else(|| Utc::now()),
            proven_at: row.proven_at,
        })
        .collect();

    debug!("Found {} proven unposted batches", batches.len());
    Ok(batches)
}

/// Mark a batch as posted to the smart contract
///
/// # Errors
/// Returns error if database operation fails
pub async fn mark_batch_posted_to_contract(
    pool: &PgPool,
    batch_id: i32,
) -> Result<(), sqlx::Error> {
    debug!("Marking batch {batch_id} as posted to contract");

    sqlx::query!(
        r"
        UPDATE proof_batches
        SET posted_to_contract = TRUE,
            posted_to_contract_at = NOW()
        WHERE id = $1
        ",
        batch_id
    )
    .execute(pool)
    .await?;

    debug!("Successfully marked batch {batch_id} as posted to contract");
    Ok(())
}
