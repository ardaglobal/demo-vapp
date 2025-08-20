use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

use crate::error::DbError;
use crate::merkle_tree::{
    AlgorithmInsertionResult, IndexedMerkleTree, InsertionProof, MerkleProof,
};

// ============================================================================
// AUTHENTICATED DATA STRUCTURE SERVICE LAYER
// ============================================================================

/// Main trait for authenticated data structures
/// Provides cryptographic guarantees for state transitions and proofs
#[async_trait]
pub trait AuthenticatedDataStructure: Send + Sync {
    type Value;
    type Proof;
    type StateCommitment;

    /// Insert a value and return cryptographic state transition proof
    async fn insert(&mut self, value: Self::Value) -> Result<StateTransition, AdsError>;

    /// Generate membership proof for existing value
    async fn prove_membership(&self, value: Self::Value) -> Result<MembershipProof, AdsError>;

    /// Generate non-membership proof using low nullifier technique
    async fn prove_non_membership(
        &self,
        value: Self::Value,
    ) -> Result<NonMembershipProof, AdsError>;

    /// Get current state commitment for settlement contract
    async fn get_state_commitment(&self) -> Result<Self::StateCommitment, AdsError>;

    /// Verify a state transition is cryptographically valid
    async fn verify_state_transition(&self, transition: &StateTransition)
        -> Result<bool, AdsError>;

    /// Batch insert multiple values efficiently
    async fn batch_insert(
        &mut self,
        values: &[Self::Value],
    ) -> Result<Vec<StateTransition>, AdsError>;

    /// Get audit trail for a specific value
    async fn get_audit_trail(&self, value: Self::Value) -> Result<AuditTrail, AdsError>;
}

// ============================================================================
// CORE DATA STRUCTURES
// ============================================================================

/// State transition proof for settlement contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub id: String,                      // Unique transition ID
    pub old_root: [u8; 32],              // Previous tree root
    pub new_root: [u8; 32],              // New tree root after insertion
    pub nullifier_value: i64,            // Inserted nullifier
    pub insertion_proof: InsertionProof, // 7-step algorithm proof
    pub block_height: u64,               // vApp block height
    pub timestamp: DateTime<Utc>,        // Insertion timestamp
    pub gas_estimate: u64,               // Estimated gas for settlement
    pub witnesses: Vec<WitnessData>,     // ZK circuit witness data
}

/// Membership proof for existing nullifiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipProof {
    pub nullifier_value: i64,       // Proven nullifier
    pub merkle_proof: MerkleProof,  // Path to root
    pub root_hash: [u8; 32],        // Current tree root
    pub tree_index: i64,            // Position in tree
    pub verified_at: DateTime<Utc>, // Proof generation time
}

/// Non-membership proof using low nullifier technique
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonMembershipProof {
    pub queried_value: i64,               // Value being proven absent
    pub low_nullifier: LowNullifierProof, // Low nullifier proof
    pub root_hash: [u8; 32],              // Current tree root
    pub range_proof: RangeProof,          // Range validation proof
    pub verified_at: DateTime<Utc>,       // Proof generation time
}

/// Low nullifier proof component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowNullifierProof {
    pub value: i64,                // Low nullifier value
    pub next_value: i64,           // Next nullifier (0 = max)
    pub tree_index: i64,           // Tree position
    pub merkle_proof: MerkleProof, // Merkle proof for low nullifier
}

/// Range proof for non-membership
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeProof {
    pub lower_bound: i64,   // low_nullifier.value
    pub upper_bound: i64,   // low_nullifier.next_value
    pub queried_value: i64, // Value in range
    pub valid: bool,        // Range check result
}

/// State commitment for settlement contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCommitment {
    pub root_hash: [u8; 32],             // Merkle tree root
    pub nullifier_count: u64,            // Total nullifiers
    pub tree_height: u32,                // Tree depth (32)
    pub last_updated: DateTime<Utc>,     // Last modification
    pub commitment_hash: [u8; 32],       // Hash of commitment data
    pub settlement_data: SettlementData, // Contract settlement info
}

/// Settlement contract data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementData {
    pub contract_address: String, // Settlement contract
    pub chain_id: u64,            // Blockchain chain ID
    pub nonce: u64,               // Settlement nonce
    pub gas_price: u64,           // Gas price estimate
}

/// Witness data for ZK circuit generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessData {
    pub circuit_type: String,       // Circuit identifier
    pub inputs: serde_json::Value,  // Circuit inputs
    pub constraints: u32,           // Constraint count
    pub proving_key_hash: [u8; 32], // Proving key identifier
}

/// Audit trail for regulatory compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrail {
    pub nullifier_value: i64,                // Tracked nullifier
    pub operation_history: Vec<AuditEvent>,  // Historical operations
    pub compliance_status: ComplianceStatus, // Regulatory status
    pub created_at: DateTime<Utc>,           // First seen
    pub last_accessed: DateTime<Utc>,        // Last proof generation
}

/// Individual audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: String,                 // Unique event ID
    pub event_type: AuditEventType,       // Operation type
    pub timestamp: DateTime<Utc>,         // When it occurred
    pub root_before: [u8; 32],            // State before
    pub root_after: [u8; 32],             // State after
    pub transaction_hash: Option<String>, // On-chain tx hash
    pub block_height: u64,                // vApp block
    pub operator: String,                 // Who performed operation
    pub metadata: serde_json::Value,      // Additional context
}

/// Types of auditable events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    Insertion,          // Nullifier inserted
    MembershipProof,    // Membership proven
    NonMembershipProof, // Non-membership proven
    StateCommitment,    // State committed
    Settlement,         // Settled on-chain
    Verification,       // Proof verified
}

/// Compliance status for regulatory requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatus {
    pub is_compliant: bool,        // Meets regulations
    pub last_audit: DateTime<Utc>, // Last compliance check
    pub jurisdiction: String,      // Regulatory jurisdiction
    pub notes: Vec<String>,        // Compliance notes
}

/// Performance metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdsMetrics {
    pub operations_total: u64,                    // Total operations
    pub insertions_total: u64,                    // Total insertions
    pub proofs_generated: u64,                    // Total proofs
    pub avg_insertion_time_ms: f64,               // Average insertion time
    pub avg_proof_time_ms: f64,                   // Average proof time
    pub error_rate: f64,                          // Error rate percentage
    pub last_reset: DateTime<Utc>,                // Metrics reset time
    pub constraint_efficiency: ConstraintMetrics, // ZK efficiency metrics
}

/// ZK constraint efficiency metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintMetrics {
    pub avg_constraints_per_op: f64,         // Constraints per operation
    pub target_constraints: u32,             // Target constraint count
    pub efficiency_ratio: f64,               // Actual vs target
    pub circuit_types: HashMap<String, u32>, // Constraints by circuit
}

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, Error)]
pub enum AdsError {
    #[error("Database error: {0}")]
    Database(#[from] DbError),

    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Nullifier {0} already exists")]
    NullifierExists(i64),

    #[error("Nullifier {0} not found")]
    NullifierNotFound(i64),

    #[error("Insertion failed: {0}")]
    InsertionFailed(String),

    #[error("Invalid range: {0}")]
    InvalidRange(String),

    #[error("Proof verification failed: {0}")]
    ProofVerificationFailed(String),

    #[error("State commitment error: {0}")]
    StateCommitmentError(String),

    #[error("Audit trail error: {0}")]
    AuditTrailError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Concurrent access error: {0}")]
    ConcurrencyError(String),

    #[error("Invalid configuration: {0}")]
    ConfigurationError(String),
}

// ============================================================================
// INDEXED MERKLE TREE ADS IMPLEMENTATION
// ============================================================================

/// Thread-safe ADS implementation using indexed Merkle tree
pub struct IndexedMerkleTreeADS {
    tree: Arc<RwLock<IndexedMerkleTree>>, // Thread-safe tree access
    state_cache: Arc<RwLock<HashMap<[u8; 32], StateCommitment>>>, // State cache
    audit_storage: Arc<RwLock<HashMap<i64, AuditTrail>>>, // Audit trails
    metrics: Arc<RwLock<AdsMetrics>>,     // Performance metrics
    config: AdsConfig,                    // Service configuration
    pool: PgPool,                         // Database connection
}

/// Configuration for ADS service
#[derive(Debug, Clone)]
pub struct AdsConfig {
    pub settlement_contract: String, // Settlement contract address
    pub chain_id: u64,               // Blockchain chain ID
    pub audit_enabled: bool,         // Enable audit trails
    pub metrics_enabled: bool,       // Enable metrics collection
    pub cache_size_limit: usize,     // Max cache entries
    pub batch_size_limit: usize,     // Max batch size
    pub gas_price: u64,              // Default gas price
}

impl Default for AdsConfig {
    fn default() -> Self {
        Self {
            settlement_contract: "0x0000000000000000000000000000000000000000".to_string(),
            chain_id: 1,
            audit_enabled: true,
            metrics_enabled: true,
            cache_size_limit: 10_000,
            batch_size_limit: 1_000,
            gas_price: 20_000_000_000, // 20 gwei
        }
    }
}

impl IndexedMerkleTreeADS {
    /// Create new ADS service instance
    #[instrument(skip(pool), level = "info")]
    pub async fn new(pool: PgPool, config: AdsConfig) -> Result<Self, AdsError> {
        info!("🚀 Initializing IndexedMerkleTreeADS service");

        let mut tree = IndexedMerkleTree::new(pool.clone());

        // Recover state from database if it exists
        if let Some(state) = tree
            .db
            .state
            .get_state(None)
            .await
            .map_err(AdsError::Database)?
        {
            info!(
                "🔄 Found existing tree state with {} nullifiers, recovering...",
                state.total_nullifiers
            );
            tree.recover_state(state)
                .await
                .map_err(AdsError::Database)?;
            info!("✅ Tree state recovered successfully");
        } else {
            info!("📝 No existing tree state found, starting with empty tree");
        }

        // Initialize metrics - if we recovered state, update them accordingly
        let mut metrics = AdsMetrics {
            operations_total: 0,
            insertions_total: 0,
            proofs_generated: 0,
            avg_insertion_time_ms: 0.0,
            avg_proof_time_ms: 0.0,
            error_rate: 0.0,
            last_reset: Utc::now(),
            constraint_efficiency: ConstraintMetrics {
                avg_constraints_per_op: 0.0,
                target_constraints: 200,
                efficiency_ratio: 1.0,
                circuit_types: HashMap::new(),
            },
        };

        // If we recovered state, update metrics to reflect the recovered data
        if let Ok(Some(state)) = tree.db.state.get_state(None).await {
            metrics.insertions_total = state.total_nullifiers as u64;
            info!(
                "📊 Updated metrics from recovered state: {} insertions",
                metrics.insertions_total
            );
        }

        let service = Self {
            tree: Arc::new(RwLock::new(tree)),
            state_cache: Arc::new(RwLock::new(HashMap::new())),
            audit_storage: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(metrics)),
            config,
            pool,
        };

        // Initialize audit storage table if needed
        if service.config.audit_enabled {
            service.init_audit_storage().await?;
        }

        // Rebuild state cache from persisted commitments if we have recovered data
        service.rebuild_state_cache().await?;

        info!("✅ IndexedMerkleTreeADS service initialized successfully");
        Ok(service)
    }

    /// Initialize audit storage in database
    #[instrument(skip(self), level = "debug")]
    async fn init_audit_storage(&self) -> Result<(), AdsError> {
        // Audit events table is created by migration 003_create_audit_events.sql
        // Just verify the table exists
        sqlx::query!("SELECT 1 as status FROM audit_events LIMIT 0")
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AdsError::Database(DbError::Database(e)))?;

        debug!("📝 Audit storage initialized");
        Ok(())
    }

    /// Rebuild state cache from persisted ADS state commits
    #[instrument(skip(self), level = "info")]
    async fn rebuild_state_cache(&self) -> Result<(), AdsError> {
        info!("🔄 Rebuilding ADS state cache from database");

        // Get recent ADS state commits to populate the cache
        let recent_commits = sqlx::query!(
            r#"
            SELECT ads.batch_id, ads.merkle_root, ads.created_at,
                   pb.previous_counter_value, pb.final_counter_value
            FROM ads_state_commits ads
            JOIN proof_batches pb ON ads.batch_id = pb.id
            WHERE pb.proof_status = 'proven'
            ORDER BY ads.created_at DESC
            LIMIT 100
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AdsError::Database(crate::error::DbError::Database(e)))?;

        if !recent_commits.is_empty() {
            let mut cache = self.state_cache.write().await;

            for commit in recent_commits {
                let mut root_key = [0u8; 32];
                root_key.copy_from_slice(&commit.merkle_root);

                let state_commitment = StateCommitment {
                    root_hash: root_key,
                    nullifier_count: 0, // We could query this if needed
                    tree_height: 32,
                    last_updated: commit.created_at.unwrap_or_else(|| chrono::Utc::now()),
                    commitment_hash: root_key, // Use root as commitment hash for simplicity
                    settlement_data: SettlementData {
                        contract_address: "0x0000000000000000000000000000000000000000".to_string(),
                        chain_id: 1, // Default to mainnet
                        nonce: 0,
                        gas_price: 20_000_000_000, // 20 gwei
                    },
                };

                cache.insert(root_key, state_commitment);
            }

            info!(
                "✅ Rebuilt state cache with {} entries from proven batches",
                cache.len()
            );
        } else {
            info!("📝 No proven batches found, state cache remains empty");
        }

        Ok(())
    }

    /// Generate unique transaction ID
    fn generate_transaction_id() -> String {
        use sha2::{Digest, Sha256};
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap();
        let random_bytes = rand::random::<[u8; 8]>();
        let mut hasher = Sha256::new();
        hasher.update(timestamp.to_be_bytes());
        hasher.update(random_bytes);
        let hash = hasher.finalize();
        hex::encode(&hash[..16]) // First 16 bytes as hex
    }

    /// Record audit event
    #[instrument(skip(self, metadata), level = "debug")]
    async fn record_audit_event(
        &self,
        nullifier_value: i64,
        event_type: AuditEventType,
        root_before: [u8; 32],
        root_after: [u8; 32],
        block_height: u64,
        metadata: serde_json::Value,
    ) -> Result<(), AdsError> {
        if !self.config.audit_enabled {
            return Ok(());
        }

        let event_id = Self::generate_transaction_id();
        let event = AuditEvent {
            event_id: event_id.clone(),
            event_type: event_type.clone(),
            timestamp: Utc::now(),
            root_before,
            root_after,
            transaction_hash: None,
            block_height,
            operator: "vapp-server".to_string(), // Could be configurable
            metadata,
        };

        // Store in database
        sqlx::query!(
            r#"
            INSERT INTO audit_events (
                event_id, nullifier_value, event_type, timestamp, root_before,
                root_after, transaction_hash, block_height, operator, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            event_id,
            nullifier_value,
            serde_json::to_string(&event_type)?,
            event.timestamp,
            root_before.as_slice(),
            root_after.as_slice(),
            event.transaction_hash,
            block_height as i64,
            event.operator,
            event.metadata,
        )
        .execute(&self.pool)
        .await?;

        // Update in-memory audit trail
        let mut audit_storage = self.audit_storage.write().await;
        let audit_trail = audit_storage
            .entry(nullifier_value)
            .or_insert_with(|| AuditTrail {
                nullifier_value,
                operation_history: Vec::new(),
                compliance_status: ComplianceStatus {
                    is_compliant: true,
                    last_audit: Utc::now(),
                    jurisdiction: "US".to_string(), // Configurable
                    notes: Vec::new(),
                },
                created_at: Utc::now(),
                last_accessed: Utc::now(),
            });

        audit_trail.operation_history.push(event);
        audit_trail.last_accessed = Utc::now();

        debug!("📋 Audit event recorded for nullifier {}", nullifier_value);
        Ok(())
    }

    /// Update performance metrics
    #[instrument(skip(self), level = "debug")]
    async fn update_metrics(&self, operation_type: &str, duration_ms: f64) -> Result<(), AdsError> {
        if !self.config.metrics_enabled {
            return Ok(());
        }

        let mut metrics = self.metrics.write().await;
        metrics.operations_total += 1;

        match operation_type {
            "insertion" => {
                metrics.insertions_total += 1;
                // Update running average
                let total = metrics.insertions_total as f64;
                metrics.avg_insertion_time_ms =
                    (metrics.avg_insertion_time_ms * (total - 1.0) + duration_ms) / total;
            }
            "proof" => {
                metrics.proofs_generated += 1;
                let total = metrics.proofs_generated as f64;
                metrics.avg_proof_time_ms =
                    (metrics.avg_proof_time_ms * (total - 1.0) + duration_ms) / total;
            }
            _ => {}
        }

        debug!(
            "📊 Metrics updated for {}: {:.2}ms",
            operation_type, duration_ms
        );
        Ok(())
    }

    /// Get current block height (placeholder - integrate with actual vApp)
    fn get_current_block_height(&self) -> Result<u64, AdsError> {
        // This would integrate with your vApp's block tracking
        // For now, return a placeholder value
        Ok(1000) // Placeholder
    }

    /// Generate witness data for ZK circuits
    fn generate_witness_data(
        &self,
        insertion_result: &AlgorithmInsertionResult,
    ) -> Vec<WitnessData> {
        vec![WitnessData {
            circuit_type: "merkle_inclusion".to_string(),
            inputs: serde_json::json!({
                "nullifier": insertion_result.nullifier.value,
                "tree_index": insertion_result.nullifier.tree_index,
                "old_root": hex::encode(insertion_result.old_root),
                "new_root": hex::encode(insertion_result.new_root),
            }),
            constraints: insertion_result.operations_count.constraints_count,
            proving_key_hash: [0u8; 32], // Placeholder
        }]
    }

    /// Calculate gas estimate for settlement
    fn calculate_gas_estimate(&self, _state_transition: &StateTransition) -> u64 {
        // Base gas for Merkle proof verification + settlement
        // This would be calibrated based on actual contract gas usage
        150_000 + (32 * 5_000) // Base + (proof_size * per_hash_gas)
    }
}

#[async_trait]
impl AuthenticatedDataStructure for IndexedMerkleTreeADS {
    type Value = i64;
    type Proof = MerkleProof;
    type StateCommitment = StateCommitment;

    #[instrument(skip(self), level = "info")]
    async fn insert(&mut self, value: i64) -> Result<StateTransition, AdsError> {
        info!("🔄 Inserting nullifier: {}", value);
        let start_time = std::time::Instant::now();

        // Get exclusive access to tree for modification
        let mut tree_guard = self.tree.write().await;

        // Check if nullifier already exists
        if tree_guard.db.nullifiers.exists(value).await? {
            return Err(AdsError::NullifierExists(value));
        }

        // Perform insertion using 7-step algorithm
        let insertion_result = tree_guard
            .insert_nullifier(value)
            .await
            .map_err(|e| AdsError::InsertionFailed(e.to_string()))?;

        drop(tree_guard); // Release lock early

        // Generate witness data for ZK circuits
        let witnesses = self.generate_witness_data(&insertion_result);

        // Get current block height
        let block_height = self.get_current_block_height()?;

        // Create state transition
        let transition_id = Self::generate_transaction_id();
        let state_transition = StateTransition {
            id: transition_id.clone(),
            old_root: insertion_result.old_root,
            new_root: insertion_result.new_root,
            nullifier_value: value,
            insertion_proof: insertion_result.insertion_proof.clone(),
            block_height,
            timestamp: Utc::now(),
            gas_estimate: self.calculate_gas_estimate(&StateTransition {
                id: transition_id,
                old_root: insertion_result.old_root,
                new_root: insertion_result.new_root,
                nullifier_value: value,
                insertion_proof: insertion_result.insertion_proof.clone(),
                block_height,
                timestamp: Utc::now(),
                gas_estimate: 0,
                witnesses: witnesses.clone(),
            }),
            witnesses,
        };

        // Update state cache
        let nullifier_count = self.get_nullifier_count().await?;
        let new_commitment = StateCommitment {
            root_hash: insertion_result.new_root,
            nullifier_count,
            tree_height: 32,
            last_updated: Utc::now(),
            commitment_hash: self
                .calculate_commitment_hash(&insertion_result.new_root, nullifier_count),
            settlement_data: SettlementData {
                contract_address: self.config.settlement_contract.clone(),
                chain_id: self.config.chain_id,
                nonce: nullifier_count,
                gas_price: self.config.gas_price,
            },
        };

        {
            let mut cache = self.state_cache.write().await;
            cache.insert(insertion_result.new_root, new_commitment);

            // Limit cache size
            if cache.len() > self.config.cache_size_limit {
                // Remove oldest entries (simple LRU would be better)
                let keys_to_remove: Vec<_> = cache
                    .keys()
                    .take(cache.len() - self.config.cache_size_limit)
                    .cloned()
                    .collect();
                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }
        }

        // Record audit event
        self.record_audit_event(
            value,
            AuditEventType::Insertion,
            insertion_result.old_root,
            insertion_result.new_root,
            block_height,
            serde_json::json!({
                "transition_id": state_transition.id,
                "constraints": insertion_result.operations_count.constraints_count,
                "hash_operations": insertion_result.operations_count.hash_operations,
                "gas_estimate": state_transition.gas_estimate,
            }),
        )
        .await?;

        // Update metrics
        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.update_metrics("insertion", duration_ms).await?;

        info!(
            "✅ Nullifier {} inserted successfully in {:.2}ms",
            value, duration_ms
        );
        Ok(state_transition)
    }

    #[instrument(skip(self), level = "info")]
    async fn prove_membership(&self, value: i64) -> Result<MembershipProof, AdsError> {
        info!("🔍 Generating membership proof for: {}", value);
        let start_time = std::time::Instant::now();

        let tree_guard = self.tree.read().await;

        // Check if nullifier exists and get details
        let nullifier = tree_guard
            .db
            .nullifiers
            .get_by_value(value)
            .await?
            .ok_or(AdsError::NullifierNotFound(value))?;

        // Generate Merkle proof
        let merkle_proof = tree_guard
            .generate_merkle_proof(nullifier.tree_index)
            .await?;
        let root_hash = tree_guard.get_root().await?;

        drop(tree_guard);

        let membership_proof = MembershipProof {
            nullifier_value: value,
            merkle_proof,
            root_hash,
            tree_index: nullifier.tree_index,
            verified_at: Utc::now(),
        };

        // Record audit event
        let block_height = self.get_current_block_height()?;
        self.record_audit_event(
            value,
            AuditEventType::MembershipProof,
            root_hash,
            root_hash, // Same root for proof generation
            block_height,
            serde_json::json!({
                "tree_index": nullifier.tree_index,
                "proof_size": membership_proof.merkle_proof.siblings.len(),
            }),
        )
        .await?;

        // Update metrics
        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.update_metrics("proof", duration_ms).await?;

        info!(
            "✅ Membership proof generated for {} in {:.2}ms",
            value, duration_ms
        );
        Ok(membership_proof)
    }

    #[instrument(skip(self), level = "info")]
    async fn prove_non_membership(&self, value: i64) -> Result<NonMembershipProof, AdsError> {
        info!("🔍 Generating non-membership proof for: {}", value);
        let start_time = std::time::Instant::now();

        let tree_guard = self.tree.read().await;

        // Find low nullifier
        let low_nullifier = tree_guard
            .db
            .nullifiers
            .find_low_nullifier(value)
            .await?
            .ok_or(AdsError::InvalidRange("No low nullifier found".to_string()))?;

        // Verify range: low_nullifier.value < value < low_nullifier.next_value
        if value <= low_nullifier.value {
            return Err(AdsError::InvalidRange(
                "Value not greater than low nullifier".into(),
            ));
        }
        if low_nullifier.next_value != 0 && value >= low_nullifier.next_value {
            return Err(AdsError::InvalidRange(
                "Value not less than next nullifier".into(),
            ));
        }

        // Generate proof for low nullifier
        let merkle_proof = tree_guard
            .generate_merkle_proof(low_nullifier.tree_index)
            .await?;
        let root_hash = tree_guard.get_root().await?;

        drop(tree_guard);

        let range_valid = value > low_nullifier.value
            && (low_nullifier.next_value == 0 || value < low_nullifier.next_value);

        let range_proof = RangeProof {
            lower_bound: low_nullifier.value,
            upper_bound: low_nullifier.next_value,
            queried_value: value,
            valid: range_valid,
        };

        let non_membership_proof = NonMembershipProof {
            queried_value: value,
            low_nullifier: LowNullifierProof {
                value: low_nullifier.value,
                next_value: low_nullifier.next_value,
                tree_index: low_nullifier.tree_index,
                merkle_proof,
            },
            root_hash,
            range_proof,
            verified_at: Utc::now(),
        };

        // Record audit event
        let block_height = self.get_current_block_height()?;
        self.record_audit_event(
            value,
            AuditEventType::NonMembershipProof,
            root_hash,
            root_hash,
            block_height,
            serde_json::json!({
                "low_nullifier": low_nullifier.value,
                "range_valid": range_valid,
                "tree_index": low_nullifier.tree_index,
            }),
        )
        .await?;

        // Update metrics
        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.update_metrics("proof", duration_ms).await?;

        info!(
            "✅ Non-membership proof generated for {} in {:.2}ms",
            value, duration_ms
        );
        Ok(non_membership_proof)
    }

    #[instrument(skip(self), level = "info")]
    async fn get_state_commitment(&self) -> Result<StateCommitment, AdsError> {
        let tree_guard = self.tree.read().await;
        let root_hash = tree_guard.get_root().await?;
        drop(tree_guard);

        // Check cache first
        {
            let cache = self.state_cache.read().await;
            if let Some(cached) = cache.get(&root_hash) {
                return Ok(cached.clone());
            }
        }

        // Generate new commitment
        let nullifier_count = self.get_nullifier_count().await?;
        let commitment = StateCommitment {
            root_hash,
            nullifier_count,
            tree_height: 32,
            last_updated: Utc::now(),
            commitment_hash: self.calculate_commitment_hash(&root_hash, nullifier_count),
            settlement_data: SettlementData {
                contract_address: self.config.settlement_contract.clone(),
                chain_id: self.config.chain_id,
                nonce: nullifier_count,
                gas_price: self.config.gas_price,
            },
        };

        // Update cache
        {
            let mut cache = self.state_cache.write().await;
            cache.insert(root_hash, commitment.clone());
        }

        Ok(commitment)
    }

    #[instrument(skip(self, transition), level = "info")]
    async fn verify_state_transition(
        &self,
        transition: &StateTransition,
    ) -> Result<bool, AdsError> {
        info!("🔐 Verifying state transition: {}", transition.id);

        let tree_guard = self.tree.read().await;

        // Verify the insertion proof structure and cryptographic validity
        let verification_result =
            tree_guard.verify_insertion_proof(&transition.insertion_proof, &transition.new_root);

        drop(tree_guard);

        if !verification_result {
            warn!(
                "State transition verification failed for: {}",
                transition.id
            );
            return Ok(false);
        }

        // Additional business logic validation
        if transition.old_root == transition.new_root {
            return Err(AdsError::ProofVerificationFailed(
                "Root should change after insertion".to_string(),
            ));
        }

        if transition.nullifier_value == 0 {
            return Err(AdsError::ProofVerificationFailed(
                "Invalid nullifier value".to_string(),
            ));
        }

        info!(
            "✅ State transition verified successfully: {}",
            transition.id
        );
        Ok(true)
    }

    #[instrument(skip(self, values), level = "info")]
    async fn batch_insert(&mut self, values: &[i64]) -> Result<Vec<StateTransition>, AdsError> {
        info!("📦 Batch inserting {} nullifiers", values.len());

        if values.len() > self.config.batch_size_limit {
            return Err(AdsError::InvalidRange(format!(
                "Batch size {} exceeds limit {}",
                values.len(),
                self.config.batch_size_limit
            )));
        }

        let mut transitions = Vec::with_capacity(values.len());

        // Process each insertion sequentially for now
        // Could be optimized with true batch operations in the future
        for &value in values {
            match self.insert(value).await {
                Ok(transition) => transitions.push(transition),
                Err(e) => {
                    warn!("Batch insertion failed for value {}: {:?}", value, e);
                    return Err(e);
                }
            }
        }

        info!(
            "✅ Batch insertion completed: {} transitions",
            transitions.len()
        );
        Ok(transitions)
    }

    #[instrument(skip(self), level = "info")]
    async fn get_audit_trail(&self, value: i64) -> Result<AuditTrail, AdsError> {
        info!("📋 Retrieving audit trail for nullifier: {}", value);

        // Check in-memory cache first
        {
            let audit_storage = self.audit_storage.read().await;
            if let Some(trail) = audit_storage.get(&value) {
                return Ok(trail.clone());
            }
        }

        // Load from database
        let events = sqlx::query!(
            r#"
            SELECT event_id, event_type, timestamp, root_before, root_after,
                   transaction_hash, block_height, operator, metadata
            FROM audit_events
            WHERE nullifier_value = $1
            ORDER BY timestamp ASC
            "#,
            value
        )
        .fetch_all(&self.pool)
        .await?;

        let mut operation_history = Vec::new();
        for row in events {
            let event = AuditEvent {
                event_id: row.event_id,
                event_type: serde_json::from_str(&row.event_type)?,
                timestamp: row.timestamp,
                root_before: {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&row.root_before);
                    arr
                },
                root_after: {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&row.root_after);
                    arr
                },
                transaction_hash: row.transaction_hash,
                block_height: row.block_height.unwrap_or(0) as u64,
                operator: row.operator.unwrap_or_else(|| "system".to_string()),
                metadata: row.metadata.unwrap_or_else(|| serde_json::json!({})),
            };
            operation_history.push(event);
        }

        if operation_history.is_empty() {
            return Err(AdsError::AuditTrailError(format!(
                "No audit trail found for nullifier {}",
                value
            )));
        }

        let audit_trail = AuditTrail {
            nullifier_value: value,
            operation_history,
            compliance_status: ComplianceStatus {
                is_compliant: true,
                last_audit: Utc::now(),
                jurisdiction: "US".to_string(), // Configurable
                notes: Vec::new(),
            },
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        // Cache for future access
        {
            let mut audit_storage = self.audit_storage.write().await;
            audit_storage.insert(value, audit_trail.clone());
        }

        info!("✅ Audit trail retrieved for nullifier: {}", value);
        Ok(audit_trail)
    }
}

// Helper methods for IndexedMerkleTreeADS
impl IndexedMerkleTreeADS {
    /// Get total nullifier count from database
    async fn get_nullifier_count(&self) -> Result<u64, AdsError> {
        let count = sqlx::query_scalar!(
            "SELECT total_nullifiers FROM tree_state WHERE tree_id = 'default'"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(count.flatten().unwrap_or(0) as u64)
    }

    /// Calculate hash of state commitment data
    fn calculate_commitment_hash(&self, root_hash: &[u8; 32], nullifier_count: u64) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(root_hash);
        hasher.update(nullifier_count.to_be_bytes());
        hasher.update(self.config.chain_id.to_be_bytes());
        hasher.finalize().into()
    }

    /// Get current performance metrics
    #[instrument(skip(self), level = "info")]
    pub async fn get_metrics(&self) -> Result<AdsMetrics, AdsError> {
        let metrics = self.metrics.read().await;
        Ok(metrics.clone())
    }

    /// Reset performance metrics
    #[instrument(skip(self), level = "info")]
    pub async fn reset_metrics(&self) -> Result<(), AdsError> {
        let mut metrics = self.metrics.write().await;
        *metrics = AdsMetrics {
            operations_total: 0,
            insertions_total: 0,
            proofs_generated: 0,
            avg_insertion_time_ms: 0.0,
            avg_proof_time_ms: 0.0,
            error_rate: 0.0,
            last_reset: Utc::now(),
            constraint_efficiency: ConstraintMetrics {
                avg_constraints_per_op: 0.0,
                target_constraints: 200,
                efficiency_ratio: 1.0,
                circuit_types: HashMap::new(),
            },
        };

        info!("🔄 Performance metrics reset");
        Ok(())
    }

    /// Health check for the service
    #[instrument(skip(self), level = "info")]
    pub async fn health_check(&self) -> Result<bool, AdsError> {
        // Check database connectivity
        sqlx::query!("SELECT 1 as health")
            .fetch_one(&self.pool)
            .await?;

        // Check tree access
        let _tree_guard = self.tree.read().await;

        // Check cache access
        let _cache_guard = self.state_cache.read().await;

        info!("✅ Health check passed");
        Ok(true)
    }
}

// ============================================================================
// SERVICE FACTORY
// ============================================================================

/// Factory for creating ADS service instances
pub struct AdsServiceFactory {
    pool: PgPool,
    config: AdsConfig,
}

impl AdsServiceFactory {
    /// Create new service factory
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            config: AdsConfig::default(),
        }
    }

    /// Create factory with custom configuration
    pub fn with_config(pool: PgPool, config: AdsConfig) -> Self {
        Self { pool, config }
    }

    /// Create indexed Merkle tree ADS instance
    pub async fn create_indexed_merkle_tree(&self) -> Result<IndexedMerkleTreeADS, AdsError> {
        IndexedMerkleTreeADS::new(self.pool.clone(), self.config.clone()).await
    }

    /// Update service configuration
    pub fn set_config(&mut self, config: AdsConfig) {
        self.config = config;
    }
}
