use crate::merkle_tree::IndexedMerkleTree;
use crate::{DbError, DbResult};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Configuration for the background processor
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    /// How often to check for new transactions
    pub polling_interval: Duration,
    /// Maximum number of transactions to process per batch
    pub batch_size: usize,
    /// Whether to run in continuous mode or one-shot
    pub continuous: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            polling_interval: Duration::from_secs(30),
            batch_size: 100,
            continuous: true,
        }
    }
}

/// Background processor that converts arithmetic transactions to nullifiers in the indexed Merkle tree
pub struct BackgroundProcessor {
    pool: PgPool,
    config: ProcessorConfig,
    last_processed_id: Option<i32>,
}

impl BackgroundProcessor {
    /// Create a new background processor
    pub fn new(pool: PgPool, config: ProcessorConfig) -> Self {
        Self {
            pool,
            config,
            last_processed_id: None,
        }
    }

    /// Start the background processing loop
    pub async fn start(&mut self) -> DbResult<()> {
        info!(
            "Starting background processor with polling interval: {:?}",
            self.config.polling_interval
        );

        // Initialize last processed ID
        self.last_processed_id = self.get_last_processed_id().await?;
        info!("Starting from transaction ID: {:?}", self.last_processed_id);

        if self.config.continuous {
            self.run_continuous().await
        } else {
            self.process_batch().await.map(|_| ())
        }
    }

    /// Run in continuous mode, checking for new transactions periodically
    async fn run_continuous(&mut self) -> DbResult<()> {
        let mut interval = interval(self.config.polling_interval);

        loop {
            interval.tick().await;
            
            match self.process_batch().await {
                Ok(processed_count) => {
                    if processed_count > 0 {
                        debug!("Processed {} transactions in batch", processed_count);
                    }
                }
                Err(e) => {
                    error!("Error processing batch: {}", e);
                    // Continue processing despite errors
                }
            }
        }
    }

    /// Process a batch of new arithmetic transactions
    pub async fn process_batch(&mut self) -> DbResult<usize> {
        debug!("Checking for new arithmetic transactions...");

        let transactions = self.fetch_new_transactions().await?;
        
        if transactions.is_empty() {
            debug!("No new transactions to process");
            return Ok(0);
        }

        info!("Processing {} new arithmetic transactions", transactions.len());

        let mut processed_count = 0;
        let mut tree = IndexedMerkleTree::new(self.pool.clone());

        for transaction in transactions {
            match self.process_single_transaction(&mut tree, &transaction).await {
                Ok(()) => {
                    processed_count += 1;
                    self.last_processed_id = Some(transaction.id);
                }
                Err(e) => {
                    warn!(
                        "Failed to process transaction ID {}: {}",
                        transaction.id, e
                    );
                    // Continue with next transaction
                }
            }
        }

        if processed_count > 0 {
            // Update the last processed ID in persistent storage
            self.update_last_processed_id().await?;
            info!("Successfully processed {} transactions", processed_count);
        }

        Ok(processed_count)
    }

    /// Process a single arithmetic transaction into the merkle tree
    async fn process_single_transaction(
        &self,
        tree: &mut IndexedMerkleTree,
        transaction: &ArithmeticTransactionWithId,
    ) -> DbResult<()> {
        debug!(
            "Processing transaction ID {}: {} + {} = {}",
            transaction.id, transaction.a, transaction.b, transaction.result
        );

        // Convert transaction to a nullifier value
        // Using a hash-based approach to create a unique nullifier from the transaction
        let nullifier_value = self.transaction_to_nullifier(transaction);

        // Insert into the indexed merkle tree
        tree.insert_nullifier(nullifier_value).await?;

        debug!(
            "Inserted nullifier {} for transaction ID {}",
            nullifier_value, transaction.id
        );

        Ok(())
    }

    /// Convert an arithmetic transaction to a nullifier value
    fn transaction_to_nullifier(&self, transaction: &ArithmeticTransactionWithId) -> i64 {
        // Create a deterministic nullifier from the transaction data
        // This ensures the same transaction always produces the same nullifier
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        transaction.a.hash(&mut hasher);
        transaction.b.hash(&mut hasher);
        transaction.result.hash(&mut hasher);
        transaction.id.hash(&mut hasher);
        
        // Convert to positive i64 to comply with nullifier constraints
        let hash = hasher.finish();
        (hash as i64).abs()
    }

    /// Fetch new arithmetic transactions since the last processed ID
    async fn fetch_new_transactions(&self) -> DbResult<Vec<ArithmeticTransactionWithId>> {
        let query = if let Some(last_id) = self.last_processed_id {
            sqlx::query_as::<_, ArithmeticTransactionWithId>(
                "SELECT id, a, b, result, created_at 
                 FROM arithmetic_transactions 
                 WHERE id > $1 
                 ORDER BY id ASC 
                 LIMIT $2"
            )
            .bind(last_id)
            .bind(self.config.batch_size as i32)
        } else {
            sqlx::query_as::<_, ArithmeticTransactionWithId>(
                "SELECT id, a, b, result, created_at 
                 FROM arithmetic_transactions 
                 ORDER BY id ASC 
                 LIMIT $1"
            )
            .bind(self.config.batch_size as i32)
        };

        let transactions = query
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        debug!("Fetched {} new transactions", transactions.len());
        Ok(transactions)
    }

    /// Get the last processed transaction ID from persistent storage
    async fn get_last_processed_id(&self) -> DbResult<Option<i32>> {
        let row = sqlx::query(
            "SELECT last_processed_transaction_id FROM processor_state WHERE processor_id = 'default'"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(row.and_then(|r| r.get("last_processed_transaction_id")))
    }

    /// Update the last processed transaction ID in persistent storage
    async fn update_last_processed_id(&self) -> DbResult<()> {
        if let Some(last_id) = self.last_processed_id {
            sqlx::query(
                "INSERT INTO processor_state (processor_id, last_processed_transaction_id, updated_at) 
                 VALUES ('default', $1, NOW()) 
                 ON CONFLICT (processor_id) 
                 DO UPDATE SET last_processed_transaction_id = EXCLUDED.last_processed_transaction_id, 
                              updated_at = EXCLUDED.updated_at"
            )
            .bind(last_id)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;
        }

        Ok(())
    }
}

/// Arithmetic transaction with database ID for background processing
#[derive(Debug, Clone, sqlx::FromRow)]
struct ArithmeticTransactionWithId {
    pub id: i32,
    pub a: i32,
    pub b: i32,
    pub result: i32,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
}

/// Builder for background processor
pub struct ProcessorBuilder {
    config: ProcessorConfig,
}

impl ProcessorBuilder {
    pub fn new() -> Self {
        Self {
            config: ProcessorConfig::default(),
        }
    }

    pub fn polling_interval(mut self, interval: Duration) -> Self {
        self.config.polling_interval = interval;
        self
    }

    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    pub fn continuous(mut self, continuous: bool) -> Self {
        self.config.continuous = continuous;
        self
    }

    pub fn build(self, pool: PgPool) -> BackgroundProcessor {
        BackgroundProcessor::new(pool, self.config)
    }
}

impl Default for ProcessorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::store_arithmetic_transaction;
    use crate::test_utils::TestDatabase;

    #[tokio::test]
    async fn test_background_processing() {
        let test_db = TestDatabase::new().await.unwrap();
        let pool = &test_db.pool;
        
        // Store some test transactions
        store_arithmetic_transaction(pool, 5, 10, 15).await.unwrap();
        store_arithmetic_transaction(pool, 20, 30, 50).await.unwrap();
        store_arithmetic_transaction(pool, 100, 200, 300).await.unwrap();

        // Create background processor
        let config = ProcessorConfig {
            polling_interval: Duration::from_millis(100),
            batch_size: 10,
            continuous: false,
        };
        let processor = BackgroundProcessor::new(pool.clone(), config);

        // First, test fetching transactions without processing
        let transactions = processor.fetch_new_transactions().await.unwrap();
        assert_eq!(transactions.len(), 3);

        // For now, just test the basic functionality without full merkle tree integration
        // This will be enhanced once the merkle tree integration is fully working
    }

    #[tokio::test]
    async fn test_transaction_to_nullifier_deterministic() {
        let test_db = TestDatabase::new().await.unwrap();
        let processor = BackgroundProcessor::new(test_db.pool.clone(), ProcessorConfig::default());

        let transaction = ArithmeticTransactionWithId {
            id: 1,
            a: 5,
            b: 10,
            result: 15,
            created_at: Utc::now(),
        };

        let nullifier1 = processor.transaction_to_nullifier(&transaction);
        let nullifier2 = processor.transaction_to_nullifier(&transaction);

        assert_eq!(nullifier1, nullifier2, "Nullifier should be deterministic");
        assert!(nullifier1 > 0, "Nullifier should be positive");
    }
}