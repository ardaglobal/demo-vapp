use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use tracing::{debug, info, instrument, warn};

use crate::error::DbError;

// ============================================================================
// 32-LEVEL MERKLE TREE OPTIMIZED FOR ZK CONSTRAINTS
// ============================================================================

/// Optimized Merkle tree with exactly 32 levels for ZK circuit efficiency
/// Capacity: 2^32 = ~4.3 billion entries
/// Constraint reduction: 96 hashes vs 768 for traditional 256-level trees
#[derive(Clone)]
pub struct MerkleTree32 {
    pool: PgPool,
    height: usize,              // Always 32
    zero_hashes: Vec<[u8; 32]>, // Precomputed zero hashes for each level
}

/// Merkle proof for 32-level tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof32 {
    pub leaf_index: usize,
    pub proof_hashes: Vec<[u8; 32]>, // Length = 32
    pub leaf_hash: [u8; 32],
}

/// Batch update operation for performance optimization
#[derive(Debug, Clone)]
pub struct BatchUpdate {
    pub leaf_index: usize,
    pub new_value: [u8; 32],
}

/// Performance metrics for tree operations
#[derive(Debug, Clone)]
pub struct TreeMetrics {
    pub hash_operations: u32,     // Target: 96 for 32-level tree (3 * 32)
    pub database_operations: u32, // Minimized with batch operations
    pub proof_size: usize,        // 32 hashes = 1024 bytes
    pub constraint_count: u32,    // ~96 * 8 = 768 Poseidon constraints
}

/// Tree statistics and state information
#[derive(Debug, Clone)]
pub struct Tree32Stats {
    pub height: usize,
    pub total_leaves: u64,
    pub non_zero_nodes: u64,
    pub root_hash: [u8; 32],
    pub zero_hash_usage: HashMap<usize, u64>, // Zero hash usage per level
    pub last_updated: DateTime<Utc>,
}

impl MerkleTree32 {
    /// Create new 32-level Merkle tree with precomputed zero hashes
    #[instrument(skip(pool), level = "info")]
    pub fn new(pool: PgPool) -> Self {
        info!("🌳 Initializing 32-level Merkle tree");
        let tree_height = 32;
        let zero_hashes = Self::compute_zero_hashes(tree_height);

        debug!(
            "Precomputed {} zero hashes for tree optimization",
            zero_hashes.len()
        );
        debug!(
            "Tree capacity: 2^{} = {} leaves",
            tree_height,
            (1u64 << tree_height)
        );

        Self {
            pool,
            height: tree_height,
            zero_hashes,
        }
    }

    /// Get the tree height (always 32)
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get a reference to the precomputed zero hashes
    pub fn zero_hashes(&self) -> &Vec<[u8; 32]> {
        &self.zero_hashes
    }

    /// Precompute zero hashes for empty subtrees at each level
    /// This eliminates database lookups for empty nodes, significantly improving performance
    #[instrument(level = "debug")]
    fn compute_zero_hashes(tree_height: usize) -> Vec<[u8; 32]> {
        info!("🔢 Computing zero hashes for {} levels", tree_height);
        let mut zero_hashes = Vec::with_capacity(tree_height + 1);

        // Level 0: hash of empty leaf (zero bytes)
        let mut hasher = Sha256::new();
        hasher.update(&[0u8; 32]); // Empty leaf value
        let level0_hash: [u8; 32] = hasher.finalize().into();
        zero_hashes.push(level0_hash);

        debug!("Level 0 zero hash: {:02x?}", &level0_hash[..8]);

        // Each level: hash of two zero hashes from level below
        for level in 1..=tree_height {
            let mut hasher = Sha256::new();
            hasher.update(&zero_hashes[level - 1]);
            hasher.update(&zero_hashes[level - 1]);
            let current_level_hash: [u8; 32] = hasher.finalize().into();
            zero_hashes.push(current_level_hash);

            debug!("Level {} zero hash: {:02x?}", level, &current_level_hash[..8]);
        }

        info!("✅ Zero hash precomputation complete");
        zero_hashes
    }

    /// Initialize tree state in database if not exists
    #[instrument(skip(self), level = "info")]
    pub async fn initialize(&self) -> Result<(), DbError> {
        info!("🚀 Initializing tree state in database");

        // Insert default tree state with root being the zero hash for level 32
        let root_hash = self.zero_hashes[self.height];

        sqlx::query!(
            r#"
            INSERT INTO tree_state (tree_id, root_hash, next_available_index, tree_height, total_nullifiers)
            VALUES ('default', $1, 0, 32, 0)
            ON CONFLICT (tree_id) DO NOTHING
            "#,
            root_hash.as_slice()
        )
        .execute(&self.pool)
        .await
        .map_err(DbError::Database)?;

        info!(
            "✅ Tree state initialized with root: {:02x?}",
            &root_hash[..8]
        );
        Ok(())
    }

    /// Update a single leaf and propagate changes up to root
    /// Returns the new root hash
    #[instrument(skip(self, new_leaf_value), level = "info")]
    pub async fn update_leaf(
        &self,
        leaf_index: usize,
        new_leaf_value: [u8; 32],
    ) -> Result<[u8; 32], DbError> {
        info!(
            "🍃 Updating leaf {} with value {:02x?}",
            leaf_index,
            &new_leaf_value[..8]
        );

        // Validate leaf index is within bounds
        if leaf_index >= (1 << self.height) {
            return Err(DbError::InvalidTreeParameter(format!(
                "Leaf index {} exceeds tree capacity 2^{}",
                leaf_index, self.height
            )));
        }

        let mut tx = self.pool.begin().await.map_err(DbError::Database)?;
        let root_hash = self
            .update_leaf_internal(leaf_index, new_leaf_value, &mut tx)
            .await?;
        tx.commit().await.map_err(DbError::Database)?;

        info!(
            "✅ Leaf update complete, new root: {:02x?}",
            &root_hash[..8]
        );
        Ok(root_hash)
    }

    /// Internal leaf update implementation with transaction
    #[instrument(skip(self, new_leaf_value, tx), level = "debug")]
    async fn update_leaf_internal(
        &self,
        leaf_index: usize,
        new_leaf_value: [u8; 32],
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<[u8; 32], DbError> {
        let mut hash_operations = 0u32;
        let mut current_hash = new_leaf_value;
        let mut current_index = leaf_index;

        // Update leaf node (level 0)
        sqlx::query!(
            r#"
            INSERT INTO merkle_nodes (tree_level, node_index, hash_value)
            VALUES (0, $1, $2)
            ON CONFLICT (tree_level, node_index)
            DO UPDATE SET hash_value = $2, updated_at = NOW()
            "#,
            current_index as i64,
            current_hash.as_slice()
        )
        .execute(&mut **tx)
        .await
        .map_err(DbError::Database)?;

        debug!("Stored leaf node at index {} level 0", current_index);

        // Propagate changes up the tree
        for level in 1..=self.height {
            let is_right_child = (current_index % 2) == 1;
            let sibling_index = if is_right_child {
                current_index - 1
            } else {
                current_index + 1
            };

            // Get sibling hash (or use zero hash if doesn't exist)
            let sibling_hash = self
                .get_node_hash(level - 1, sibling_index, tx)
                .await?
                .unwrap_or(self.zero_hashes[level - 1]);

            // Compute parent hash (hash operation counted)
            hash_operations += 1;
            current_hash = if is_right_child {
                self.hash_pair(&sibling_hash, &current_hash)
            } else {
                self.hash_pair(&current_hash, &sibling_hash)
            };

            // Update parent index
            current_index = current_index / 2;

            // Store parent node
            sqlx::query!(
                r#"
                INSERT INTO merkle_nodes (tree_level, node_index, hash_value)
                VALUES ($1, $2, $3)
                ON CONFLICT (tree_level, node_index)
                DO UPDATE SET hash_value = $3, updated_at = NOW()
                "#,
                level as i32,
                current_index as i64,
                current_hash.as_slice()
            )
            .execute(&mut **tx)
            .await
            .map_err(DbError::Database)?;

            debug!(
                "Updated level {} node {} with hash {:02x?}",
                level,
                current_index,
                &current_hash[..8]
            );
        }

        // Update tree state with new root
        sqlx::query!(
            r#"
            UPDATE tree_state
            SET root_hash = $1, updated_at = NOW()
            WHERE tree_id = 'default'
            "#,
            current_hash.as_slice()
        )
        .execute(&mut **tx)
        .await
        .map_err(DbError::Database)?;

        debug!(
            "Tree propagation complete: {} hash operations",
            hash_operations
        );
        Ok(current_hash)
    }

    /// Get node hash from database or return None if doesn't exist
    #[instrument(skip(self, tx), level = "debug")]
    async fn get_node_hash(
        &self,
        level: usize,
        index: usize,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<[u8; 32]>, DbError> {
        let result: Option<Vec<u8>> = sqlx::query_scalar!(
            "SELECT hash_value FROM merkle_nodes WHERE tree_level = $1 AND node_index = $2",
            level as i32,
            index as i64
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::Database)?;

        Ok(result.map(|v| {
            let mut arr = [0u8; 32];
            if v.len() >= 32 {
                arr.copy_from_slice(&v[..32]);
            }
            arr
        }))
    }

    /// Hash two 32-byte values using SHA-256
    #[instrument(skip(self, left, right), level = "debug")]
    fn hash_pair(&self, left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }

    /// Generate Merkle proof for a leaf (32 sibling hashes)
    #[instrument(skip(self), level = "info")]
    pub async fn generate_proof(&self, leaf_index: usize) -> Result<MerkleProof32, DbError> {
        info!("🔐 Generating proof for leaf {}", leaf_index);

        if leaf_index >= (1 << self.height) {
            return Err(DbError::InvalidTreeParameter(format!(
                "Leaf index {} exceeds tree capacity",
                leaf_index
            )));
        }

        let mut proof_hashes = Vec::with_capacity(self.height);
        let mut current_index = leaf_index;

        // Get the leaf hash first
        let leaf_hash = self
            .get_node_hash_public(0, leaf_index)
            .await?
            .unwrap_or(self.zero_hashes[0]);

        // Collect sibling hashes from leaf to root
        for level in 0..self.height {
            let is_right_child = (current_index % 2) == 1;
            let sibling_index = if is_right_child {
                current_index - 1
            } else {
                current_index + 1
            };

            // Get sibling hash (use zero hash if doesn't exist)
            let sibling_hash = self
                .get_node_hash_public(level, sibling_index)
                .await?
                .unwrap_or(self.zero_hashes[level]);

            proof_hashes.push(sibling_hash);
            current_index = current_index / 2;

            debug!(
                "Level {} sibling {}: {:02x?}",
                level,
                sibling_index,
                &sibling_hash[..8]
            );
        }

        let proof = MerkleProof32 {
            leaf_index,
            proof_hashes,
            leaf_hash,
        };

        info!(
            "✅ Proof generated with {} sibling hashes",
            proof.proof_hashes.len()
        );
        Ok(proof)
    }

    /// Get node hash from database (public version without transaction)
    #[instrument(skip(self), level = "debug")]
    async fn get_node_hash_public(
        &self,
        level: usize,
        index: usize,
    ) -> Result<Option<[u8; 32]>, DbError> {
        let result: Option<Vec<u8>> = sqlx::query_scalar!(
            "SELECT hash_value FROM merkle_nodes WHERE tree_level = $1 AND node_index = $2",
            level as i32,
            index as i64
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::Database)?;

        Ok(result.map(|v| {
            let mut arr = [0u8; 32];
            if v.len() >= 32 {
                arr.copy_from_slice(&v[..32]);
            }
            arr
        }))
    }

    /// Get current root hash
    #[instrument(skip(self), level = "debug")]
    pub async fn get_root(&self) -> Result<[u8; 32], DbError> {
        let root_bytes: Vec<u8> =
            sqlx::query_scalar!("SELECT root_hash FROM tree_state WHERE tree_id = 'default'")
                .fetch_one(&self.pool)
                .await
                .map_err(DbError::Database)?;

        let mut root = [0u8; 32];
        if root_bytes.len() >= 32 {
            root.copy_from_slice(&root_bytes[..32]);
        }
        Ok(root)
    }

    /// Batch update multiple leaves for optimal performance
    #[instrument(skip(self, updates), level = "info")]
    pub async fn batch_update(&self, updates: &[BatchUpdate]) -> Result<[u8; 32], DbError> {
        info!("📦 Executing batch update of {} leaves", updates.len());

        if updates.is_empty() {
            return self.get_root().await;
        }

        // Validate all indices first
        for update in updates {
            if update.leaf_index >= (1 << self.height) {
                return Err(DbError::InvalidTreeParameter(format!(
                    "Leaf index {} exceeds tree capacity",
                    update.leaf_index
                )));
            }
        }

        let mut tx = self.pool.begin().await.map_err(DbError::Database)?;

        // Apply all leaf updates
        for update in updates {
            debug!(
                "Batch updating leaf {} with {:02x?}",
                update.leaf_index,
                &update.new_value[..8]
            );

            // Store leaf directly without propagation yet
            sqlx::query!(
                r#"
                INSERT INTO merkle_nodes (tree_level, node_index, hash_value)
                VALUES (0, $1, $2)
                ON CONFLICT (tree_level, node_index)
                DO UPDATE SET hash_value = $2, updated_at = NOW()
                "#,
                update.leaf_index as i64,
                update.new_value.as_slice()
            )
            .execute(&mut *tx)
            .await
            .map_err(DbError::Database)?;
        }

        // Collect all affected paths and recompute from bottom up
        let affected_nodes = self.collect_affected_paths(updates);
        let final_root = self
            .recompute_affected_paths(&affected_nodes, &mut tx)
            .await?;

        // Update tree state
        sqlx::query!(
            r#"
            UPDATE tree_state
            SET root_hash = $1, updated_at = NOW()
            WHERE tree_id = 'default'
            "#,
            final_root.as_slice()
        )
        .execute(&mut *tx)
        .await
        .map_err(DbError::Database)?;

        tx.commit().await.map_err(DbError::Database)?;

        info!(
            "✅ Batch update complete, new root: {:02x?}",
            &final_root[..8]
        );
        Ok(final_root)
    }

    /// Collect all affected internal nodes from batch updates
    fn collect_affected_paths(&self, updates: &[BatchUpdate]) -> HashMap<(usize, usize), ()> {
        let mut affected = HashMap::new();

        for update in updates {
            let mut current_index = update.leaf_index;

            // Mark all parents as affected
            for level in 1..=self.height {
                current_index = current_index / 2;
                affected.insert((level, current_index), ());
            }
        }

        affected
    }

    /// Recompute all affected internal nodes efficiently
    #[instrument(skip(self, affected_nodes, tx), level = "debug")]
    async fn recompute_affected_paths(
        &self,
        affected_nodes: &HashMap<(usize, usize), ()>,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<[u8; 32], DbError> {
        // Process levels from bottom to top
        for level in 1..=self.height {
            let level_nodes: Vec<usize> = affected_nodes
                .keys()
                .filter_map(|(l, idx)| if *l == level { Some(*idx) } else { None })
                .collect();

            for &node_index in &level_nodes {
                let left_child = node_index * 2;
                let right_child = node_index * 2 + 1;

                // Get left child hash
                let left_hash = self
                    .get_node_hash(level - 1, left_child, tx)
                    .await?
                    .unwrap_or(self.zero_hashes[level - 1]);

                // Get right child hash
                let right_hash = self
                    .get_node_hash(level - 1, right_child, tx)
                    .await?
                    .unwrap_or(self.zero_hashes[level - 1]);

                // Compute parent hash
                let parent_hash = self.hash_pair(&left_hash, &right_hash);

                // Store updated parent
                sqlx::query!(
                    r#"
                    INSERT INTO merkle_nodes (tree_level, node_index, hash_value)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (tree_level, node_index)
                    DO UPDATE SET hash_value = $3, updated_at = NOW()
                    "#,
                    level as i32,
                    node_index as i64,
                    parent_hash.as_slice()
                )
                .execute(&mut **tx)
                .await
                .map_err(DbError::Database)?;

                debug!(
                    "Recomputed level {} node {}: {:02x?}",
                    level,
                    node_index,
                    &parent_hash[..8]
                );

                // If this is the root level, return the root hash
                if level == self.height && node_index == 0 {
                    return Ok(parent_hash);
                }
            }
        }

        // Fallback: get root from database
        self.get_node_hash(self.height, 0, tx)
            .await?
            .ok_or_else(|| DbError::NotFound("root node after recomputation".to_string()))
    }

    /// Get comprehensive tree statistics
    #[instrument(skip(self), level = "info")]
    pub async fn get_stats(&self) -> Result<Tree32Stats, DbError> {
        info!("📊 Collecting tree statistics");

        let root_hash = self.get_root().await?;

        // Count total nodes at each level
        let node_counts = sqlx::query!(
            r#"
            SELECT tree_level, COUNT(*) as count
            FROM merkle_nodes
            GROUP BY tree_level
            ORDER BY tree_level
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::Database)?;

        let mut zero_hash_usage = HashMap::new();
        let mut total_leaves = 0u64;
        let mut non_zero_nodes = 0u64;

        for row in node_counts {
            let level = row.tree_level as usize;
            let count = row.count.unwrap_or(0) as u64;

            if level == 0 {
                total_leaves = count;
            }
            non_zero_nodes += count;

            // Estimate zero hash usage (nodes that could exist but don't)
            let max_nodes_at_level = 1u64 << (self.height - level);
            zero_hash_usage.insert(level, max_nodes_at_level - count);
        }

        let tree_state =
            sqlx::query!("SELECT updated_at FROM tree_state WHERE tree_id = 'default'")
                .fetch_one(&self.pool)
                .await
                .map_err(DbError::Database)?;

        let stats = Tree32Stats {
            height: self.height,
            total_leaves,
            non_zero_nodes,
            root_hash,
            zero_hash_usage,
            last_updated: tree_state.updated_at.unwrap_or_else(|| chrono::Utc::now()),
        };

        info!(
            "📈 Tree stats: {} leaves, {} nodes, height {}",
            stats.total_leaves, stats.non_zero_nodes, stats.height
        );

        Ok(stats)
    }

    /// Calculate performance metrics for operations
    pub fn calculate_metrics(&self, operations: u32) -> TreeMetrics {
        TreeMetrics {
            hash_operations: operations * 32,     // 32 hash ops per path to root
            database_operations: operations * 33, // 32 internal nodes + 1 leaf + root update
            proof_size: 32 * 32,                  // 32 hashes * 32 bytes = 1024 bytes
            constraint_count: operations * 32 * 8, // Poseidon constraints: 8 per hash
        }
    }
}

impl MerkleProof32 {
    /// Verify proof against a root hash
    #[instrument(skip(self, root), level = "debug")]
    pub fn verify(&self, root: &[u8; 32]) -> bool {
        debug!("🔍 Verifying proof for leaf {}", self.leaf_index);

        if self.proof_hashes.len() != 32 {
            warn!(
                "Invalid proof: expected 32 hashes, got {}",
                self.proof_hashes.len()
            );
            return false;
        }

        let mut current_hash = self.leaf_hash;
        let mut current_index = self.leaf_index;

        for (level, sibling_hash) in self.proof_hashes.iter().enumerate() {
            let is_right_child = (current_index % 2) == 1;

            let mut hasher = Sha256::new();
            if is_right_child {
                hasher.update(sibling_hash);
                hasher.update(&current_hash);
            } else {
                hasher.update(&current_hash);
                hasher.update(sibling_hash);
            }
            current_hash = hasher.finalize().into();
            current_index = current_index / 2;

            debug!("Level {}: hash {:02x?}", level, &current_hash[..8]);
        }

        let verified = current_hash == *root;
        if verified {
            debug!("✅ Proof verification successful");
        } else {
            warn!("❌ Proof verification failed");
        }

        verified
    }

    /// Get the size of this proof in bytes
    pub fn size_bytes(&self) -> usize {
        32 * 32 + 32 + std::mem::size_of::<usize>() // 32 siblings + leaf + index
    }

    /// Verify proof with custom leaf hash (for external leaf values)
    #[instrument(skip(self, custom_leaf_hash, root), level = "debug")]
    pub fn verify_with_leaf(&self, custom_leaf_hash: &[u8; 32], root: &[u8; 32]) -> bool {
        let mut current_hash = *custom_leaf_hash;
        let mut current_index = self.leaf_index;

        for sibling_hash in &self.proof_hashes {
            let is_right_child = (current_index % 2) == 1;

            let mut hasher = Sha256::new();
            if is_right_child {
                hasher.update(sibling_hash);
                hasher.update(&current_hash);
            } else {
                hasher.update(&current_hash);
                hasher.update(sibling_hash);
            }
            current_hash = hasher.finalize().into();
            current_index = current_index / 2;
        }

        current_hash == *root
    }
}
