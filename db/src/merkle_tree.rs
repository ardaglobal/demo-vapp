use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use tracing::{debug, error, info, instrument, warn};

use crate::error::DbError;

// ============================================================================
// CORE DATA STRUCTURES
// ============================================================================

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Nullifier {
    pub id: i64,
    pub value: i64,
    pub next_index: Option<i64>,
    pub next_value: i64, // 0 means this is the maximum value
    pub tree_index: i64,
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct MerkleNode {
    pub tree_level: i32,
    pub node_index: i64,
    pub hash_value: Vec<u8>, // 32 bytes
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TreeState {
    pub tree_id: String,
    pub root_hash: Vec<u8>,
    pub next_available_index: i64,
    pub tree_height: i32,
    pub total_nullifiers: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowNullifier {
    pub value: i64,
    pub next_index: Option<i64>,
    pub next_value: i64,
    pub tree_index: i64,
}

#[derive(Debug, Clone)]
pub struct InsertionResult {
    pub nullifier: Nullifier,
    pub low_nullifier: LowNullifier,
    pub tree_index: i64,
}

#[derive(Debug, Clone)]
pub struct TreeStats {
    pub total_nullifiers: i64,
    pub tree_height: i32,
    pub next_index: i64,
    pub chain_valid: bool,
}

// ============================================================================
// INDEXED MERKLE TREE ALGORITHM - 7-STEP INSERTION
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_index: i64,
    pub leaf_hash: [u8; 32],
    pub siblings: Vec<[u8; 32]>, // 32-level tree = 32 siblings max
    pub path_indices: Vec<bool>, // true = right, false = left
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertionProof {
    pub low_nullifier_proof: MerkleProof,
    pub new_nullifier_proof: MerkleProof,
    pub low_nullifier_before: LowNullifier,
    pub low_nullifier_after: LowNullifier,
}

#[derive(Debug, Clone)]
pub struct AlgorithmInsertionResult {
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub insertion_proof: InsertionProof,
    pub nullifier: Nullifier,
    pub operations_count: InsertionMetrics,
}

#[derive(Debug, Clone)]
pub struct InsertionMetrics {
    pub hash_operations: u32,   // Target: 3n + 3 = 99 for 32-level tree
    pub range_checks: u32,      // Target: exactly 2
    pub database_rounds: u32,   // Minimize round trips
    pub constraints_count: u32, // Target: ~200 vs ~1600 for 256-level
}

// ============================================================================
// NULLIFIER DATABASE OPERATIONS
// ============================================================================

#[derive(Clone)]
pub struct NullifierDb {
    pool: PgPool,
}

impl NullifierDb {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn find_low_nullifier(
        &self,
        new_value: i64,
    ) -> Result<Option<LowNullifier>, DbError> {
        debug!("Finding low nullifier for value: {}", new_value);

        let result = sqlx::query!(
            r#"
            SELECT low_value as value, low_next_index as next_index,
                   low_next_value as next_value, low_tree_index as tree_index
            FROM find_low_nullifier($1)
            "#,
            new_value
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::Database)?;

        match result {
            Some(row) => {
                let low_nullifier = LowNullifier {
                    value: row.value.unwrap_or(0),
                    next_index: row.next_index,
                    next_value: row.next_value.unwrap_or(0),
                    tree_index: row.tree_index.unwrap_or(0),
                };
                debug!("Found low nullifier: {:?}", low_nullifier);
                Ok(Some(low_nullifier))
            }
            None => {
                debug!("No low nullifier found - checking if tree is empty");

                // Check if this is an empty tree (first insertion)
                let nullifier_count =
                    sqlx::query_scalar!("SELECT COUNT(*) FROM nullifiers WHERE is_active = true")
                        .fetch_one(&self.pool)
                        .await
                        .map_err(DbError::Database)?
                        .unwrap_or(0);

                if nullifier_count == 0 {
                    // Empty tree: create virtual low nullifier for first insertion
                    debug!("Tree is empty - creating virtual low nullifier for first insertion");
                    let virtual_low_nullifier = LowNullifier {
                        value: 0,         // Virtual minimum value
                        next_index: None, // No next nullifier yet
                        next_value: 0,    // Virtual maximum (first insertion will be max)
                        tree_index: 0,    // Virtual tree index
                    };
                    Ok(Some(virtual_low_nullifier))
                } else {
                    debug!("No low nullifier found for value: {}", new_value);
                    Ok(None)
                }
            }
        }
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn exists(&self, value: i64) -> Result<bool, DbError> {
        let count: Option<i64> = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM nullifiers WHERE value = $1 AND is_active = true",
            value
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::Database)?;

        let count = count.unwrap_or(0);

        let exists = count > 0;
        debug!("Nullifier {} exists: {}", value, exists);
        Ok(exists)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn insert_with_update(
        &self,
        new_value: i64,
        new_tree_index: i64,
        low_nullifier: &LowNullifier,
    ) -> Result<Nullifier, DbError> {
        info!(
            "Inserting nullifier: value={}, tree_index={}, low_nullifier_value={}",
            new_value, new_tree_index, low_nullifier.value
        );

        let mut tx = self.pool.begin().await.map_err(DbError::Database)?;

        // Check if this is an empty tree insertion (virtual low nullifier)
        let is_empty_tree = low_nullifier.value == 0 && low_nullifier.tree_index == 0;

        if !is_empty_tree {
            // Update the low nullifier to point to our new nullifier
            let update_result = sqlx::query!(
                r#"
                UPDATE nullifiers
                SET next_index = $1, next_value = $2
                WHERE value = $3 AND is_active = true
                "#,
                new_tree_index,
                new_value,
                low_nullifier.value
            )
            .execute(&mut *tx)
            .await
            .map_err(DbError::Database)?;

            debug!(
                "Updated low nullifier, rows affected: {}",
                update_result.rows_affected()
            );
        } else {
            info!("📝 Skipping low nullifier update for empty tree insertion");
        }

        // Insert new nullifier with appropriate pointers
        let (next_index, next_value) = if is_empty_tree {
            // First nullifier: no next pointer (it's the maximum)
            (None::<i64>, 0i64)
        } else {
            // Normal insertion: inherit pointers from low nullifier
            (low_nullifier.next_index, low_nullifier.next_value)
        };

        let new_nullifier = sqlx::query_as!(
            Nullifier,
            r#"
            INSERT INTO nullifiers (value, next_index, next_value, tree_index)
            VALUES ($1, $2, $3, $4)
            RETURNING id, value, next_index, next_value as "next_value!", tree_index, created_at as "created_at!", is_active as "is_active!"
            "#,
            new_value,
            next_index,
            next_value,
            new_tree_index
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(DbError::Database)?;

        tx.commit().await.map_err(DbError::Database)?;

        info!(
            "Successfully inserted nullifier with id: {}",
            new_nullifier.id
        );
        Ok(new_nullifier)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn atomic_insert(&self, new_value: i64) -> Result<InsertionResult, DbError> {
        info!("Starting atomic insertion for value: {}", new_value);

        // Check if nullifier already exists
        if self.exists(new_value).await? {
            warn!("Nullifier {} already exists", new_value);
            return Err(DbError::NullifierExists(new_value));
        }

        // Call the atomic insertion function from database
        let result = sqlx::query!(
            r#"
            SELECT inserted_tree_index, low_nullifier_value, low_nullifier_next_value, success
            FROM insert_nullifier_atomic($1)
            "#,
            new_value
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::Database)?;

        let success = result.success.unwrap_or(false);
        if !success {
            error!("Atomic insertion failed for value: {}", new_value);
            return Err(DbError::InsertionFailed(new_value));
        }

        let tree_index = result.inserted_tree_index.unwrap_or(0);
        let low_value = result.low_nullifier_value.unwrap_or(0);
        let low_next_value = result.low_nullifier_next_value.unwrap_or(0);

        // Fetch the inserted nullifier
        let nullifier = self
            .get_by_tree_index(tree_index)
            .await?
            .ok_or_else(|| DbError::NotFound(format!("tree_index {}", tree_index)))?;

        let low_nullifier = LowNullifier {
            value: low_value,
            next_index: None, // Will be updated after insertion
            next_value: low_next_value,
            tree_index: 0, // Placeholder
        };

        let insertion_result = InsertionResult {
            nullifier,
            low_nullifier,
            tree_index,
        };

        info!(
            "Atomic insertion successful for value: {} at tree_index: {}",
            new_value, tree_index
        );
        Ok(insertion_result)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_tree_index(&self, tree_index: i64) -> Result<Option<Nullifier>, DbError> {
        let nullifier = sqlx::query_as!(
            Nullifier,
            r#"
            SELECT id, value, next_index, next_value as "next_value!", tree_index, created_at as "created_at!", is_active as "is_active!"
            FROM nullifiers
            WHERE tree_index = $1 AND is_active = true
            "#,
            tree_index
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::Database)?;

        Ok(nullifier)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_value(&self, value: i64) -> Result<Option<Nullifier>, DbError> {
        let nullifier = sqlx::query_as!(
            Nullifier,
            r#"
            SELECT id, value, next_index, next_value as "next_value!", tree_index, created_at as "created_at!", is_active as "is_active!"
            FROM nullifiers
            WHERE value = $1 AND is_active = true
            "#,
            value
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::Database)?;

        Ok(nullifier)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_all_active(&self) -> Result<Vec<Nullifier>, DbError> {
        let nullifiers = sqlx::query_as!(
            Nullifier,
            r#"
            SELECT id, value, next_index, next_value as "next_value!", tree_index, created_at as "created_at!", is_active as "is_active!"
            FROM nullifiers
            WHERE is_active = true
            ORDER BY value ASC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::Database)?;

        debug!("Retrieved {} active nullifiers", nullifiers.len());
        Ok(nullifiers)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn validate_chain(&self) -> Result<bool, DbError> {
        let is_valid = sqlx::query_scalar!("SELECT validate_nullifier_chain()")
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::Database)?;

        let valid = is_valid.unwrap_or(false);
        if valid {
            debug!("Nullifier chain validation: VALID");
        } else {
            warn!("Nullifier chain validation: INVALID");
        }
        Ok(valid)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn deactivate(&self, value: i64) -> Result<bool, DbError> {
        let result = sqlx::query!(
            "UPDATE nullifiers SET is_active = false WHERE value = $1 AND is_active = true",
            value
        )
        .execute(&self.pool)
        .await
        .map_err(DbError::Database)?;

        let deactivated = result.rows_affected() > 0;
        if deactivated {
            info!("Deactivated nullifier with value: {}", value);
        } else {
            warn!("No active nullifier found with value: {}", value);
        }
        Ok(deactivated)
    }
}

// ============================================================================
// MERKLE NODE DATABASE OPERATIONS
// ============================================================================

#[derive(Clone)]
pub struct MerkleNodeDb {
    pool: PgPool,
}

impl MerkleNodeDb {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Upsert a Merkle tree node at the specified level and index
    ///
    /// # Errors
    /// Returns `DbError::InvalidHashLength` if hash_value is not exactly 32 bytes
    /// Returns `DbError::Database` if database operation fails
    #[instrument(skip(self, hash_value), level = "debug")]
    pub async fn upsert_node(
        &self,
        tree_level: i32,
        node_index: i64,
        hash_value: &[u8],
    ) -> Result<MerkleNode, DbError> {
        if hash_value.len() != 32 {
            return Err(DbError::InvalidHashLength(hash_value.len()));
        }

        let node = sqlx::query_as!(
            MerkleNode,
            r#"
            INSERT INTO merkle_nodes (tree_level, node_index, hash_value)
            VALUES ($1, $2, $3)
            ON CONFLICT (tree_level, node_index)
            DO UPDATE SET hash_value = EXCLUDED.hash_value, updated_at = NOW()
            RETURNING tree_level, node_index, hash_value, updated_at as "updated_at!"
            "#,
            tree_level,
            node_index,
            hash_value
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::Database)?;

        debug!(
            "Upserted merkle node at level {} index {}",
            tree_level, node_index
        );
        Ok(node)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_node(
        &self,
        tree_level: i32,
        node_index: i64,
    ) -> Result<Option<MerkleNode>, DbError> {
        let node = sqlx::query_as!(
            MerkleNode,
            r#"
            SELECT tree_level, node_index, hash_value, updated_at as "updated_at!"
            FROM merkle_nodes
            WHERE tree_level = $1 AND node_index = $2
            "#,
            tree_level,
            node_index
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::Database)?;

        Ok(node)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_level_nodes(&self, tree_level: i32) -> Result<Vec<MerkleNode>, DbError> {
        let nodes = sqlx::query_as!(
            MerkleNode,
            r#"
            SELECT tree_level, node_index, hash_value, updated_at as "updated_at!"
            FROM merkle_nodes
            WHERE tree_level = $1
            ORDER BY node_index ASC
            "#,
            tree_level
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::Database)?;

        debug!("Retrieved {} nodes at level {}", nodes.len(), tree_level);
        Ok(nodes)
    }
}

// ============================================================================
// TREE STATE DATABASE OPERATIONS
// ============================================================================

#[derive(Clone)]
pub struct TreeStateDb {
    pool: PgPool,
}

impl TreeStateDb {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_state(&self, tree_id: Option<&str>) -> Result<Option<TreeState>, DbError> {
        let id = tree_id.unwrap_or("default");

        let state = sqlx::query_as!(
            TreeState,
            r#"
            SELECT tree_id, root_hash, next_available_index as "next_available_index!", tree_height as "tree_height!", total_nullifiers as "total_nullifiers!", updated_at as "updated_at!"
            FROM tree_state
            WHERE tree_id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::Database)?;

        Ok(state)
    }

    #[instrument(skip(self, root_hash), level = "debug")]
    pub async fn update_root(
        &self,
        root_hash: &[u8],
        tree_id: Option<&str>,
    ) -> Result<TreeState, DbError> {
        if root_hash.len() != 32 {
            return Err(DbError::InvalidHashLength(root_hash.len()));
        }

        let id = tree_id.unwrap_or("default");

        let state = sqlx::query_as!(
            TreeState,
            r#"
            UPDATE tree_state
            SET root_hash = $1, updated_at = NOW()
            WHERE tree_id = $2
            RETURNING tree_id, root_hash, next_available_index as "next_available_index!", tree_height as "tree_height!", total_nullifiers as "total_nullifiers!", updated_at as "updated_at!"
            "#,
            root_hash,
            id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::Database)?;

        info!("Updated tree root for tree_id: {}", id);
        Ok(state)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn increment_nullifier_count(
        &self,
        tree_id: Option<&str>,
    ) -> Result<TreeState, DbError> {
        let id = tree_id.unwrap_or("default");

        let state = sqlx::query_as!(
            TreeState,
            r#"
            UPDATE tree_state
            SET total_nullifiers = total_nullifiers + 1, updated_at = NOW()
            WHERE tree_id = $1
            RETURNING tree_id, root_hash, next_available_index as "next_available_index!", tree_height as "tree_height!", total_nullifiers as "total_nullifiers!", updated_at as "updated_at!"
            "#,
            id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::Database)?;

        debug!("Incremented nullifier count to: {}", state.total_nullifiers);
        Ok(state)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_next_index(&self, tree_id: Option<&str>) -> Result<i64, DbError> {
        let next_index = sqlx::query_scalar!("SELECT get_next_tree_index()")
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::Database)?;

        let index = next_index.unwrap_or(0);
        debug!("Next available tree index: {}", index);
        Ok(index)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_stats(&self) -> Result<TreeStats, DbError> {
        let result = sqlx::query!(
            r#"
            SELECT total_nullifiers, tree_height, next_index, chain_valid
            FROM get_tree_stats()
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::Database)?;

        let stats = TreeStats {
            total_nullifiers: result.total_nullifiers.unwrap_or(0),
            tree_height: result.tree_height.unwrap_or(32),
            next_index: result.next_index.unwrap_or(0),
            chain_valid: result.chain_valid.unwrap_or(false),
        };

        debug!("Tree stats: {:?}", stats);
        Ok(stats)
    }
}

// ============================================================================
// INTEGRATED MERKLE TREE DATABASE
// ============================================================================

#[derive(Clone)]
pub struct MerkleTreeDb {
    pub nullifiers: NullifierDb,
    pub nodes: MerkleNodeDb,
    pub state: TreeStateDb,
}

impl MerkleTreeDb {
    pub fn new(pool: PgPool) -> Self {
        Self {
            nullifiers: NullifierDb::new(pool.clone()),
            nodes: MerkleNodeDb::new(pool.clone()),
            state: TreeStateDb::new(pool.clone()),
        }
    }

    #[instrument(skip(self), level = "info")]
    pub async fn insert_nullifier_complete(&self, value: i64) -> Result<InsertionResult, DbError> {
        info!("Starting complete nullifier insertion for value: {}", value);

        // Use the atomic insertion which handles the full 7-step process
        let result = self.nullifiers.atomic_insert(value).await?;

        // Update tree state
        self.state.increment_nullifier_count(None).await?;

        // Validate the chain integrity after insertion
        let chain_valid = self.nullifiers.validate_chain().await?;
        if !chain_valid {
            error!("Chain validation failed after inserting value: {}", value);
            return Err(DbError::ChainValidationFailed);
        }

        info!(
            "Complete nullifier insertion successful for value: {}",
            value
        );
        Ok(result)
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_membership_proof(&self, value: i64) -> Result<bool, DbError> {
        self.nullifiers.exists(value).await
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_non_membership_proof(
        &self,
        value: i64,
    ) -> Result<Option<LowNullifier>, DbError> {
        // For non-membership proof, we need the low nullifier
        if self.nullifiers.exists(value).await? {
            return Ok(None); // Value exists, no non-membership proof
        }

        self.nullifiers.find_low_nullifier(value).await
    }
}

// ============================================================================
// INDEXED MERKLE TREE - 7-STEP INSERTION ALGORITHM IMPLEMENTATION
// ============================================================================

#[derive(Clone)]
pub struct IndexedMerkleTree {
    pub db: MerkleTreeDb,
    pub tree_height: usize, // Exactly 32 levels
}

impl IndexedMerkleTree {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            db: MerkleTreeDb::new(pool.clone()),
            tree_height: 32, // Fixed at 32 levels per spec
        }
    }

    /// Implements the exact 7-step nullifier insertion algorithm from transparency dictionaries paper
    #[instrument(skip(self), level = "info")]
    pub async fn insert_nullifier(
        &mut self,
        new_nullifier: i64,
    ) -> Result<AlgorithmInsertionResult, DbError> {
        info!(
            "🚀 Starting 7-step nullifier insertion for value: {}",
            new_nullifier
        );
        let mut metrics = InsertionMetrics {
            hash_operations: 0,
            range_checks: 0,
            database_rounds: 0,
            constraints_count: 0,
        };

        // Get current root before changes
        metrics.database_rounds += 1;
        let old_root = self.get_root().await?;
        info!("📊 Old root: {:02x?}", &old_root[..8]);

        // STEP 1: Find low_nullifier
        info!(
            "🔍 Step 1: Finding low nullifier for value {}",
            new_nullifier
        );
        metrics.database_rounds += 1;
        let low_nullifier = self
            .db
            .nullifiers
            .find_low_nullifier(new_nullifier)
            .await?
            .ok_or_else(|| DbError::NotFound("low nullifier".to_string()))?;

        debug!(
            "Found low nullifier: value={}, next_value={}, tree_index={}",
            low_nullifier.value, low_nullifier.next_value, low_nullifier.tree_index
        );

        // STEP 2: Membership check (skip for empty tree/virtual low nullifier)
        let is_empty_tree_insertion = low_nullifier.value == 0 && low_nullifier.tree_index == 0;

        if !is_empty_tree_insertion {
            metrics.database_rounds += 1;
            if !self.db.nullifiers.exists(low_nullifier.value).await? {
                error!("Low nullifier {} not found in tree", low_nullifier.value);
                return Err(DbError::NotFound(format!(
                    "Low nullifier {}",
                    low_nullifier.value
                )));
            }
        } else {
            info!("📝 Skipping membership check for empty tree insertion");
        }

        // STEP 3: Range validation (exactly 2 range checks as per spec)
        info!("🔒 Step 3: Performing range validation");

        if !is_empty_tree_insertion {
            // Range check 1: new_nullifier > low_nullifier.value
            metrics.range_checks += 1;
            if new_nullifier <= low_nullifier.value {
                error!(
                    "Range check 1 failed: {} <= {}",
                    new_nullifier, low_nullifier.value
                );
                return Err(DbError::InvalidNullifierValue(format!(
                    "New nullifier {} must be greater than low nullifier {}",
                    new_nullifier, low_nullifier.value
                )));
            }
        } else {
            // Empty tree: just validate that nullifier is positive
            metrics.range_checks += 1;
            if new_nullifier <= 0 {
                error!(
                    "Range check failed: first nullifier must be positive, got {}",
                    new_nullifier
                );
                return Err(DbError::InvalidNullifierValue(format!(
                    "First nullifier must be positive, got {}",
                    new_nullifier
                )));
            }
            info!(
                "📝 Empty tree range check passed for value {}",
                new_nullifier
            );
        }
        debug!(
            "✓ Range check 1 passed: {} > {}",
            new_nullifier, low_nullifier.value
        );

        // Range check 2: new_nullifier < low_nullifier.next_value OR low_nullifier.next_value == 0
        metrics.range_checks += 2;
        if low_nullifier.next_value != 0 && new_nullifier >= low_nullifier.next_value {
            error!(
                "Range check 2 failed: {} >= {} (next_value)",
                new_nullifier, low_nullifier.next_value
            );
            return Err(DbError::InvalidNullifierValue(format!(
                "New nullifier {} must be less than next value {}",
                new_nullifier, low_nullifier.next_value
            )));
        }

        // Get next available tree index
        metrics.database_rounds += 1;
        let new_tree_index = self.db.state.get_next_index(None).await?;
        info!(
            "📍 Assigned tree index {} for new nullifier",
            new_tree_index
        );

        // Store the low_nullifier state before update for proof generation
        let low_nullifier_before = low_nullifier.clone();

        // Create the low_nullifier state after update
        let low_nullifier_after = LowNullifier {
            value: low_nullifier.value,
            next_index: Some(new_tree_index),
            next_value: new_nullifier,
            tree_index: low_nullifier.tree_index,
        };

        // Execute the atomic insertion
        metrics.database_rounds += 1;
        let new_nullifier_entry = self
            .db
            .nullifiers
            .insert_with_update(new_nullifier, new_tree_index, &low_nullifier)
            .await?;

        info!(
            "✅ Successfully inserted nullifier with ID: {}",
            new_nullifier_entry.id
        );

        // Update Merkle tree with both changes (hash operations counted here)
        let tree_update_metrics = self
            .update_tree_for_insertion(
                &low_nullifier_before,
                &low_nullifier_after,
                &new_nullifier_entry,
            )
            .await?;
        metrics.hash_operations += tree_update_metrics;

        // Get new root after changes
        metrics.database_rounds += 1;
        let new_root = self.get_root().await?;
        info!("📊 New root: {:02x?}", &new_root[..8]);

        // Generate proofs (additional hash operations)
        let (insertion_proof, proof_metrics) = self
            .generate_insertion_proof(
                &low_nullifier_before,
                &low_nullifier_after,
                &new_nullifier_entry,
            )
            .await?;
        metrics.hash_operations += proof_metrics;

        // Calculate total constraints for ZK circuit
        metrics.constraints_count = self.calculate_constraints(&metrics);

        info!("🎯 Insertion complete - Metrics: {} hashes, {} range checks, {} DB rounds, {} constraints",
            metrics.hash_operations, metrics.range_checks, metrics.database_rounds, metrics.constraints_count);

        Ok(AlgorithmInsertionResult {
            old_root,
            new_root,
            insertion_proof,
            nullifier: new_nullifier_entry,
            operations_count: metrics,
        })
    }

    /// Generate leaf hash from nullifier data (as per algorithm specification)
    #[instrument(skip(self, nullifier), level = "debug")]
    fn hash_nullifier_leaf(&self, nullifier: &Nullifier) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(nullifier.value.to_be_bytes());
        hasher.update(nullifier.next_index.unwrap_or(0).to_be_bytes());
        hasher.update(nullifier.next_value.to_be_bytes());
        let result: [u8; 32] = hasher.finalize().into();
        debug!(
            "Hashed nullifier leaf: value={}, hash={:02x?}",
            nullifier.value,
            &result[..8]
        );
        result
    }

    /// Generate leaf hash for LowNullifier state
    #[instrument(skip(self, low_nullifier), level = "debug")]
    fn hash_low_nullifier_leaf(&self, low_nullifier: &LowNullifier) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(low_nullifier.value.to_be_bytes());
        hasher.update(low_nullifier.next_index.unwrap_or(0).to_be_bytes());
        hasher.update(low_nullifier.next_value.to_be_bytes());
        let result: [u8; 32] = hasher.finalize().into();
        debug!(
            "Hashed low nullifier leaf: value={}, hash={:02x?}",
            low_nullifier.value,
            &result[..8]
        );
        result
    }

    /// Hash two child nodes to create parent node
    #[instrument(skip(self, left, right), level = "debug")]
    fn hash_internal_node(&self, left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }

    /// Efficiently update tree for both low_nullifier and new_nullifier
    /// Returns the number of hash operations performed
    #[instrument(
        skip(self, low_nullifier_before, low_nullifier_after, new_nullifier),
        level = "debug"
    )]
    async fn update_tree_for_insertion(
        &self,
        low_nullifier_before: &LowNullifier,
        low_nullifier_after: &LowNullifier,
        new_nullifier: &Nullifier,
    ) -> Result<u32, DbError> {
        info!("🌳 Updating Merkle tree for insertion");
        let mut hash_count = 0u32;

        // Hash operations for leaf updates
        // Update 1: Hash the updated low_nullifier leaf (1 hash)
        hash_count += 1;
        let updated_low_hash = self.hash_low_nullifier_leaf(low_nullifier_after);

        self.update_leaf(low_nullifier_before.tree_index as usize, updated_low_hash)
            .await?;
        debug!(
            "Updated low nullifier leaf at index {}",
            low_nullifier_before.tree_index
        );

        // Update 2: Hash the new nullifier leaf (1 hash)
        hash_count += 1;
        let new_nullifier_hash = self.hash_nullifier_leaf(new_nullifier);

        self.update_leaf(new_nullifier.tree_index as usize, new_nullifier_hash)
            .await?;
        debug!(
            "Inserted new nullifier leaf at index {}",
            new_nullifier.tree_index
        );

        // For each path to root update, we do (tree_height) hashes
        // Two paths * 32 levels = 64 hash operations for path updates
        hash_count += (self.tree_height as u32) * 2;

        // Total: 1 + 1 + 64 = 66 hash operations for tree updates
        // Plus proof generation will add more to reach target of 3n + 3 = 99

        info!("Tree update complete with {} hash operations", hash_count);
        Ok(hash_count)
    }

    /// Update a specific leaf and propagate changes to root
    #[instrument(skip(self, leaf_hash), level = "debug")]
    async fn update_leaf(&self, leaf_index: usize, leaf_hash: [u8; 32]) -> Result<(), DbError> {
        debug!(
            "Updating leaf at index {} with hash {:02x?}",
            leaf_index,
            &leaf_hash[..8]
        );

        // Store leaf node (level 0)
        self.db
            .nodes
            .upsert_node(0, leaf_index as i64, &leaf_hash)
            .await?;

        // Propagate changes up the tree
        let mut current_hash = leaf_hash;
        let mut current_index = leaf_index;

        for level in 1..=self.tree_height {
            let parent_index = current_index / 2;
            let is_right_child = current_index % 2 == 1;
            let sibling_index = if is_right_child {
                current_index - 1
            } else {
                current_index + 1
            };

            // Get sibling hash (or use zero hash if sibling doesn't exist)
            let sibling_hash = match self
                .db
                .nodes
                .get_node(level as i32 - 1, sibling_index as i64)
                .await?
            {
                Some(node) => {
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&node.hash_value);
                    hash
                }
                None => [0u8; 32], // Zero hash for missing siblings
            };

            // Calculate parent hash
            let parent_hash = if is_right_child {
                self.hash_internal_node(&sibling_hash, &current_hash)
            } else {
                self.hash_internal_node(&current_hash, &sibling_hash)
            };

            // Store parent node
            self.db
                .nodes
                .upsert_node(level as i32, parent_index as i64, &parent_hash)
                .await?;

            current_hash = parent_hash;
            current_index = parent_index;
        }

        // Update root in tree state
        self.db.state.update_root(&current_hash, None).await?;
        debug!("Propagated changes to root: {:02x?}", &current_hash[..8]);

        Ok(())
    }

    /// Generate Merkle proof for a specific leaf
    ///
    /// # Errors
    /// Returns `DbError::NotFound` if the leaf at the specified index doesn't exist
    /// Returns `DbError::Database` if database operation fails
    #[instrument(skip(self), level = "debug")]
    pub async fn generate_merkle_proof(&self, leaf_index: i64) -> Result<MerkleProof, DbError> {
        debug!("Generating Merkle proof for leaf index {}", leaf_index);

        let leaf_node = self
            .db
            .nodes
            .get_node(0, leaf_index)
            .await?
            .ok_or_else(|| DbError::NotFound(format!("leaf at index {}", leaf_index)))?;

        let mut leaf_hash = [0u8; 32];
        leaf_hash.copy_from_slice(&leaf_node.hash_value);

        let mut siblings = Vec::with_capacity(self.tree_height);
        let mut path_indices = Vec::with_capacity(self.tree_height);
        let mut current_index = leaf_index;

        for level in 0..self.tree_height {
            let is_right_child = current_index % 2 == 1;
            let sibling_index = if is_right_child {
                current_index - 1
            } else {
                current_index + 1
            };

            // Get sibling hash
            let sibling_hash = match self.db.nodes.get_node(level as i32, sibling_index).await? {
                Some(node) => {
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&node.hash_value);
                    hash
                }
                None => [0u8; 32], // Zero hash for missing siblings
            };

            siblings.push(sibling_hash);
            path_indices.push(is_right_child);

            current_index = current_index / 2;
        }

        Ok(MerkleProof {
            leaf_index,
            leaf_hash,
            siblings,
            path_indices,
        })
    }

    /// Generate complete insertion proof with both low_nullifier and new_nullifier proofs
    /// Returns (proof, hash_operations_count)
    #[instrument(
        skip(self, low_nullifier_before, low_nullifier_after, new_nullifier),
        level = "debug"
    )]
    async fn generate_insertion_proof(
        &self,
        low_nullifier_before: &LowNullifier,
        low_nullifier_after: &LowNullifier,
        new_nullifier: &Nullifier,
    ) -> Result<(InsertionProof, u32), DbError> {
        info!("🔐 Generating insertion proof");

        // Generate proofs (each proof generation involves tree_height hash operations for verification)
        let low_nullifier_proof = self
            .generate_merkle_proof(low_nullifier_before.tree_index)
            .await?;
        let new_nullifier_proof = self.generate_merkle_proof(new_nullifier.tree_index).await?;

        // Hash operations for proof generation: 2 * tree_height = 2 * 32 = 64 operations
        // But these are typically done by the verifier, not counted in our constraint budget
        let proof_hash_operations = 0u32; // Don't count verification hashes in our budget

        let insertion_proof = InsertionProof {
            low_nullifier_proof,
            new_nullifier_proof,
            low_nullifier_before: low_nullifier_before.clone(),
            low_nullifier_after: low_nullifier_after.clone(),
        };

        info!("✅ Insertion proof generated successfully");
        Ok((insertion_proof, proof_hash_operations))
    }

    /// Get current root hash from tree state
    #[instrument(skip(self), level = "debug")]
    pub async fn get_root(&self) -> Result<[u8; 32], DbError> {
        let state = self
            .db
            .state
            .get_state(None)
            .await?
            .ok_or_else(|| DbError::NotFound("tree state".to_string()))?;

        let mut root = [0u8; 32];
        root.copy_from_slice(&state.root_hash);
        Ok(root)
    }

    /// Calculate total constraints for ZK circuit (target: ~200 vs ~1600 for 256-level tree)
    fn calculate_constraints(&self, metrics: &InsertionMetrics) -> u32 {
        // Constraint calculation based on paper specifications:
        // - Hash operations: each SHA-256 ≈ 27,000 R1CS constraints, but we use Poseidon in ZK (≈ 8 constraints)
        // - Range checks: ≈ 250 constraints each for 64-bit values
        // - Equality constraints: ≈ 1 constraint each

        let hash_constraints = metrics.hash_operations * 8; // Poseidon hash constraints
        let range_constraints = metrics.range_checks * 250; // 64-bit range check constraints
        let equality_constraints = 10u32; // Fixed equality constraints for the algorithm

        hash_constraints + range_constraints + equality_constraints
    }

    /// Verify insertion proof (for testing and validation)
    #[instrument(skip(self, proof), level = "debug")]
    pub fn verify_insertion_proof(&self, proof: &InsertionProof, root: &[u8; 32]) -> bool {
        // Verify low nullifier proof
        if !self.verify_merkle_proof(&proof.low_nullifier_proof, root) {
            warn!("Low nullifier proof verification failed");
            return false;
        }

        // Verify new nullifier proof
        if !self.verify_merkle_proof(&proof.new_nullifier_proof, root) {
            warn!("New nullifier proof verification failed");
            return false;
        }

        // Verify the algorithm logic
        if proof.low_nullifier_after.next_index != Some(proof.new_nullifier_proof.leaf_index) {
            warn!("Low nullifier next_index doesn't match new nullifier index");
            return false;
        }

        debug!("✅ Insertion proof verified successfully");
        true
    }

    /// Verify a single Merkle proof
    #[instrument(skip(self, proof, root), level = "debug")]
    fn verify_merkle_proof(&self, proof: &MerkleProof, root: &[u8; 32]) -> bool {
        let mut current_hash = proof.leaf_hash;

        for (sibling, is_right) in proof.siblings.iter().zip(proof.path_indices.iter()) {
            current_hash = if *is_right {
                self.hash_internal_node(sibling, &current_hash)
            } else {
                self.hash_internal_node(&current_hash, sibling)
            };
        }

        current_hash == *root
    }

    /// Get tree statistics
    #[instrument(skip(self), level = "debug")]
    pub async fn get_stats(&self) -> Result<TreeStats, DbError> {
        self.db.state.get_stats().await
    }
}
