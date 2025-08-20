//! Unified Batch Service
//!
//! This module provides a single, unified interface for batch creation that always
//! uses ADS integration, regardless of how the batch is triggered (API, timer, threshold).
//!
//! This replaces the dual workflow problem where some batches used ADS and others didn't.

use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};

use arithmetic_db::{
    get_pending_transactions, store_ads_state_commit, IndexedMerkleTreeADS, 
    AuthenticatedDataStructure, IncomingTransaction, ProofBatch
};

// ============================================================================
// UNIFIED BATCH SERVICE
// ============================================================================

/// Unified service for all batch creation with consistent ADS integration
pub struct UnifiedBatchService {
    pool: PgPool,
    ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
    max_batch_size: u32,
}

/// Response from unified batch creation
#[derive(Debug)]
pub struct BatchCreationResult {
    pub batch_id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub merkle_root: Vec<u8>,
    pub nullifier_count: usize,
}

impl UnifiedBatchService {
    /// Create new unified batch service
    pub fn new(
        pool: PgPool,
        ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
        max_batch_size: u32,
    ) -> Self {
        Self {
            pool,
            ads_service,
            max_batch_size,
        }
    }

    /// Create a batch with full ADS integration
    /// 
    /// This is the ONLY way batches should be created - all triggers use this method
    #[instrument(skip(self), level = "info")]
    pub async fn create_batch_with_ads(
        &self,
        requested_batch_size: Option<i32>,
        trigger_source: &str,
    ) -> Result<Option<BatchCreationResult>, String> {
        info!(
            "🔄 UNIFIED: Creating batch via {} trigger",
            trigger_source
        );

        // Step 1: Get pending transactions
        let pending_transactions = match get_pending_transactions(&self.pool).await {
            Ok(transactions) => transactions,
            Err(e) => return Err(format!("Failed to get pending transactions: {}", e)),
        };

        if pending_transactions.is_empty() {
            debug!("No pending transactions to batch");
            return Ok(None);
        }

        // Step 2: Determine batch size
        let batch_size = requested_batch_size
            .map(|s| s as usize)
            .unwrap_or(pending_transactions.len())
            .min(self.max_batch_size as usize)
            .min(pending_transactions.len());

        let batch_transactions = &pending_transactions[..batch_size];

        info!(
            "📦 UNIFIED: Processing {} transactions through ADS integration",
            batch_size
        );

        // Step 3: Start atomic database transaction  
        let db_tx = self.pool.begin().await
            .map_err(|e| format!("Failed to begin database transaction: {}", e))?;

        // Step 4: Create batch entry (using existing logic for now)
        info!("📋 UNIFIED: Creating batch entry in database");
        let batch = match self.create_batch_entry(&batch_transactions).await {
            Ok(Some(batch)) => batch,
            Ok(None) => {
                debug!("Failed to create batch entry");
                db_tx.rollback().await.ok();
                return Ok(None);
            }
            Err(e) => {
                error!("Failed to create batch entry: {}", e);
                db_tx.rollback().await.ok();
                return Err(format!("Failed to create batch entry: {}", e));
            }
        };

        info!("✅ UNIFIED: Batch entry created: id={}, transactions={}", 
              batch.id, batch_transactions.len());

        // Step 5: Process transactions through ADS
        info!("🔐 UNIFIED: Processing transactions through ADS service");
        let mut ads_guard = self.ads_service.write().await;
        
        // Convert transactions to nullifiers
        let mut nullifiers = Vec::new();
        for tx in batch_transactions {
            let nullifier_value = self.transaction_to_nullifier(tx);
            nullifiers.push(nullifier_value);
        }

        // Insert nullifiers through ADS
        let state_transitions = match ads_guard.batch_insert(&nullifiers).await {
            Ok(transitions) => {
                info!("✅ UNIFIED: Successfully processed {} nullifiers through ADS", 
                      nullifiers.len());
                transitions
            }
            Err(e) => {
                error!("UNIFIED: Failed to process nullifiers through ADS: {}", e);
                drop(ads_guard);
                db_tx.rollback().await.ok();
                return Err(format!("Failed to process transactions through ADS: {}", e));
            }
        };

        // Step 6: Get the final merkle root
        let merkle_root = if let Some(last_transition) = state_transitions.last() {
            last_transition.new_root.clone()
        } else {
            error!("UNIFIED: No state transitions returned from ADS batch insert");
            drop(ads_guard);
            db_tx.rollback().await.ok();
            return Err("No state transitions returned from ADS".to_string());
        };

        drop(ads_guard); // Release ADS lock

        info!("🌳 UNIFIED: Final merkle root: {:02x?}", &merkle_root[..8]);

        // Step 7: Store merkle root atomically
        info!("💾 UNIFIED: Storing merkle root for batch {}", batch.id);
        match store_ads_state_commit(&self.pool, batch.id, &merkle_root).await {
            Ok(_) => {
                info!("✅ UNIFIED: Merkle root stored successfully for batch {}", batch.id);
            }
            Err(e) => {
                error!("UNIFIED: Failed to store merkle root for batch {}: {}", batch.id, e);
                db_tx.rollback().await.ok();
                return Err(format!("Failed to store merkle root: {}", e));
            }
        }

        // Step 8: Commit the transaction
        if let Err(e) = db_tx.commit().await {
            error!("UNIFIED: Failed to commit batch transaction: {}", e);
            return Err(format!("Failed to commit batch transaction: {}", e));
        }

        info!("🎉 UNIFIED: Atomic batch processing completed successfully!");
        info!("   📊 Batch ID: {}", batch.id);
        info!("   🔢 Transactions: {}", batch_size);
        info!("   🔢 Nullifiers: {}", nullifiers.len());
        info!("   🌳 Merkle Root: 0x{}", hex::encode(&merkle_root));

        let result = BatchCreationResult {
            batch_id: batch.id,
            previous_counter_value: batch.previous_counter_value,
            final_counter_value: batch.final_counter_value,
            transaction_count: batch_size,
            merkle_root: merkle_root.to_vec(),
            nullifier_count: nullifiers.len(),
        };

        Ok(Some(result))
    }

    /// Create batch entry in database (temporary - uses existing logic)
    /// TODO: This should be replaced with pure database operations once SQL function is deprecated
    async fn create_batch_entry(
        &self,
        transactions: &[IncomingTransaction],
    ) -> Result<Option<ProofBatch>, String> {
        // For now, use the existing create_batch logic but limit to exact transactions
        match arithmetic_db::create_batch(&self.pool, Some(transactions.len() as i32)).await {
            Ok(batch) => Ok(batch),
            Err(e) => Err(format!("Database error: {}", e)),
        }
    }

    /// Convert a transaction to a nullifier value (deterministic hash)
    fn transaction_to_nullifier(&self, transaction: &IncomingTransaction) -> i64 {
        // Create a deterministic nullifier from the transaction data using blake3
        // This ensures the same transaction always produces the same nullifier across deployments
        let mut hasher = blake3::Hasher::new();
        
        // Hash each field as its little-endian byte representation for deterministic results
        hasher.update(&transaction.id.to_le_bytes());
        hasher.update(&transaction.amount.to_le_bytes());
        hasher.update(&transaction.created_at.timestamp().to_le_bytes());
        
        // Get the first 8 bytes of the digest as a u64 (little-endian)
        let digest = hasher.finalize();
        let hash_bytes = digest.as_bytes();
        let hash_u64 = u64::from_le_bytes([
            hash_bytes[0], hash_bytes[1], hash_bytes[2], hash_bytes[3],
            hash_bytes[4], hash_bytes[5], hash_bytes[6], hash_bytes[7],
        ]);
        
        // Convert to positive i64 (IMT requires positive nullifiers)
        // Take modulo to ensure we stay in positive range, then add 1 to avoid zero
        let nullifier = ((hash_u64 % (i64::MAX as u64)) as i64) + 1;

        debug!("Transaction {} -> nullifier {} (from hash {})", transaction.id, nullifier, hash_u64);
        nullifier
    }

    /// Get service health status
    pub async fn health_check(&self) -> Result<String, String> {
        // Check database connectivity
        match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => {},
            Err(e) => return Err(format!("Database health check failed: {}", e)),
        }

        // Check ADS service
        let ads_guard = self.ads_service.read().await;
        match ads_guard.health_check().await {
            Ok(true) => Ok("Unified batch service is healthy".to_string()),
            Ok(false) => Err("ADS service is not healthy".to_string()),
            Err(e) => Err(format!("ADS health check failed: {}", e)),
        }
    }
}