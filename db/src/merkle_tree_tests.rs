#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::test_utils::{create_test_db, cleanup_test_db};
    use sqlx::PgPool;
    use tokio;
    use tracing::info;
    use tracing_test::traced_test;

    async fn setup_merkle_tree_db() -> (MerkleTreeDb, String) {
        let db_name = create_test_db().await.expect("Failed to create test database");
        let pool = init_db(&format!(
            "postgresql://postgres:password@127.0.0.1:5432/{}",
            db_name
        ))
        .await
        .expect("Failed to initialize database");
        
        let merkle_db = MerkleTreeDb::new(pool);
        (merkle_db, db_name)
    }

    async fn teardown_merkle_tree_db(db_name: String) {
        cleanup_test_db(&db_name).await.expect("Failed to cleanup test database");
    }

    #[tokio::test]
    #[traced_test]
    async fn test_nullifier_insertion_basic() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Insert first nullifier
        let result1 = merkle_db.insert_nullifier_complete(100).await;
        assert!(result1.is_ok(), "Failed to insert first nullifier: {:?}", result1);
        
        let insertion1 = result1.unwrap();
        assert_eq!(insertion1.nullifier.value, 100);
        assert_eq!(insertion1.nullifier.tree_index, 0);
        
        // Insert second nullifier (should be linked)
        let result2 = merkle_db.insert_nullifier_complete(200).await;
        assert!(result2.is_ok(), "Failed to insert second nullifier: {:?}", result2);
        
        let insertion2 = result2.unwrap();
        assert_eq!(insertion2.nullifier.value, 200);
        assert_eq!(insertion2.nullifier.tree_index, 1);
        
        // Verify chain integrity
        let chain_valid = merkle_db.nullifiers.validate_chain().await.unwrap();
        assert!(chain_valid, "Chain validation failed");
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_nullifier_insertion_ordering() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Insert nullifiers in non-sequential order
        let values = vec![50, 150, 100, 75, 125];
        
        for value in values {
            let result = merkle_db.insert_nullifier_complete(value).await;
            assert!(result.is_ok(), "Failed to insert nullifier {}: {:?}", value, result);
            
            // Verify chain is still valid after each insertion
            let chain_valid = merkle_db.nullifiers.validate_chain().await.unwrap();
            assert!(chain_valid, "Chain validation failed after inserting {}", value);
        }
        
        // Verify all nullifiers exist
        for value in [50, 75, 100, 125, 150] {
            let exists = merkle_db.get_membership_proof(value).await.unwrap();
            assert!(exists, "Nullifier {} should exist", value);
        }
        
        // Get all nullifiers and verify they're in sorted order
        let nullifiers = merkle_db.nullifiers.get_all_active().await.unwrap();
        assert_eq!(nullifiers.len(), 5);
        
        let mut prev_value = -1;
        for nullifier in nullifiers {
            assert!(nullifier.value > prev_value, "Nullifiers not in sorted order");
            prev_value = nullifier.value;
        }
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_find_low_nullifier() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Insert initial nullifiers: 10, 30, 50
        for value in [10, 30, 50] {
            merkle_db.insert_nullifier_complete(value).await.unwrap();
        }
        
        // Test finding low nullifier for value 25 (should find 10)
        let low_null = merkle_db.nullifiers.find_low_nullifier(25).await.unwrap();
        assert!(low_null.is_some());
        let low_null = low_null.unwrap();
        assert_eq!(low_null.value, 10);
        assert_eq!(low_null.next_value, 30);
        
        // Test finding low nullifier for value 40 (should find 30)
        let low_null = merkle_db.nullifiers.find_low_nullifier(40).await.unwrap();
        assert!(low_null.is_some());
        let low_null = low_null.unwrap();
        assert_eq!(low_null.value, 30);
        assert_eq!(low_null.next_value, 50);
        
        // Test finding low nullifier for value 60 (should find 50, next_value = 0)
        let low_null = merkle_db.nullifiers.find_low_nullifier(60).await.unwrap();
        assert!(low_null.is_some());
        let low_null = low_null.unwrap();
        assert_eq!(low_null.value, 50);
        assert_eq!(low_null.next_value, 0); // Maximum value
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_non_membership_proof() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Insert nullifiers: 10, 30, 50
        for value in [10, 30, 50] {
            merkle_db.insert_nullifier_complete(value).await.unwrap();
        }
        
        // Test non-membership proof for value 25
        let proof = merkle_db.get_non_membership_proof(25).await.unwrap();
        assert!(proof.is_some(), "Should have non-membership proof for 25");
        let proof = proof.unwrap();
        assert_eq!(proof.value, 10);
        assert_eq!(proof.next_value, 30);
        
        // Test membership (no non-membership proof)
        let proof = merkle_db.get_non_membership_proof(30).await.unwrap();
        assert!(proof.is_none(), "Should not have non-membership proof for existing value");
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_duplicate_insertion() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Insert nullifier
        let result1 = merkle_db.insert_nullifier_complete(100).await;
        assert!(result1.is_ok());
        
        // Try to insert same nullifier again
        let result2 = merkle_db.insert_nullifier_complete(100).await;
        assert!(result2.is_err());
        
        if let Err(DbError::NullifierExists(value)) = result2 {
            assert_eq!(value, 100);
        } else {
            panic!("Expected NullifierExists error, got: {:?}", result2);
        }
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_merkle_node_operations() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        let hash_value = vec![0u8; 32]; // 32 zero bytes
        
        // Insert merkle node
        let node = merkle_db.nodes.upsert_node(1, 0, &hash_value).await;
        assert!(node.is_ok());
        let node = node.unwrap();
        assert_eq!(node.tree_level, 1);
        assert_eq!(node.node_index, 0);
        assert_eq!(node.hash_value, hash_value);
        
        // Retrieve the node
        let retrieved = merkle_db.nodes.get_node(1, 0).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.hash_value, hash_value);
        
        // Update the node
        let new_hash = vec![1u8; 32];
        let updated_node = merkle_db.nodes.upsert_node(1, 0, &new_hash).await.unwrap();
        assert_eq!(updated_node.hash_value, new_hash);
        
        // Verify update
        let retrieved = merkle_db.nodes.get_node(1, 0).await.unwrap().unwrap();
        assert_eq!(retrieved.hash_value, new_hash);
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_invalid_hash_length() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        let invalid_hash = vec![0u8; 31]; // Invalid length
        
        let result = merkle_db.nodes.upsert_node(1, 0, &invalid_hash).await;
        assert!(result.is_err());
        
        if let Err(DbError::InvalidHashLength(len)) = result {
            assert_eq!(len, 31);
        } else {
            panic!("Expected InvalidHashLength error, got: {:?}", result);
        }
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_tree_state_operations() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Get initial state
        let state = merkle_db.state.get_state(None).await.unwrap();
        assert!(state.is_some());
        let state = state.unwrap();
        assert_eq!(state.tree_id, "default");
        assert_eq!(state.total_nullifiers, 0);
        
        // Update root
        let new_root = vec![1u8; 32];
        let updated_state = merkle_db.state.update_root(&new_root, None).await.unwrap();
        assert_eq!(updated_state.root_hash, new_root);
        
        // Increment nullifier count
        let state = merkle_db.state.increment_nullifier_count(None).await.unwrap();
        assert_eq!(state.total_nullifiers, 1);
        
        // Get tree stats
        let stats = merkle_db.state.get_stats().await.unwrap();
        assert_eq!(stats.total_nullifiers, 1);
        assert_eq!(stats.tree_height, 32);
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_large_scale_insertion() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Insert 100 nullifiers in random order
        let mut values: Vec<i64> = (1..=100).collect();
        // Shuffle using a simple algorithm instead of rand
        for i in (1..values.len()).rev() {
            let j = (i as u64 * 7 + 13) as usize % (i + 1); // Simple pseudo-random
            values.swap(i, j);
        }
        
        for (i, value) in values.iter().enumerate() {
            let result = merkle_db.insert_nullifier_complete(*value).await;
            assert!(result.is_ok(), "Failed to insert nullifier {} (iteration {}): {:?}", value, i, result);
            
            // Validate chain every 10 insertions
            if i % 10 == 9 {
                let chain_valid = merkle_db.nullifiers.validate_chain().await.unwrap();
                assert!(chain_valid, "Chain validation failed after {} insertions", i + 1);
            }
        }
        
        // Final validation
        let chain_valid = merkle_db.nullifiers.validate_chain().await.unwrap();
        assert!(chain_valid, "Final chain validation failed");
        
        // Verify all values exist
        for value in 1..=100 {
            let exists = merkle_db.get_membership_proof(value).await.unwrap();
            assert!(exists, "Nullifier {} should exist", value);
        }
        
        // Verify tree statistics
        let stats = merkle_db.state.get_stats().await.unwrap();
        assert_eq!(stats.total_nullifiers, 100);
        assert!(stats.chain_valid);
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_concurrent_insertions() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Clone the database handle for concurrent access
        let merkle_db1 = merkle_db.clone();
        let merkle_db2 = merkle_db.clone();
        let merkle_db3 = merkle_db.clone();
        
        // Launch concurrent insertions
        let handle1 = tokio::spawn(async move {
            for i in 1..=33 {
                let _ = merkle_db1.insert_nullifier_complete(i * 3).await;
            }
        });
        
        let handle2 = tokio::spawn(async move {
            for i in 1..=33 {
                let _ = merkle_db2.insert_nullifier_complete(i * 3 + 1).await;
            }
        });
        
        let handle3 = tokio::spawn(async move {
            for i in 1..=34 {
                let _ = merkle_db3.insert_nullifier_complete(i * 3 + 2).await;
            }
        });
        
        // Wait for all tasks to complete
        let _ = tokio::join!(handle1, handle2, handle3);
        
        // Validate final state
        let chain_valid = merkle_db.nullifiers.validate_chain().await.unwrap();
        assert!(chain_valid, "Chain validation failed after concurrent insertions");
        
        // Count successful insertions
        let stats = merkle_db.state.get_stats().await.unwrap();
        info!("Successful concurrent insertions: {}", stats.total_nullifiers);
        
        // Should have at least some successful insertions
        assert!(stats.total_nullifiers > 0, "No successful concurrent insertions");
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_edge_cases() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Test with zero value
        let result = merkle_db.insert_nullifier_complete(0).await;
        assert!(result.is_ok(), "Failed to insert zero value: {:?}", result);
        
        // Test with maximum i64 value
        let max_value = i64::MAX;
        let result = merkle_db.insert_nullifier_complete(max_value).await;
        assert!(result.is_ok(), "Failed to insert max value: {:?}", result);
        
        // Test with minimum positive value
        let result = merkle_db.insert_nullifier_complete(1).await;
        assert!(result.is_ok(), "Failed to insert min positive value: {:?}", result);
        
        // Validate chain
        let chain_valid = merkle_db.nullifiers.validate_chain().await.unwrap();
        assert!(chain_valid, "Chain validation failed for edge cases");
        
        teardown_merkle_tree_db(db_name).await;
    }

    #[tokio::test]
    #[traced_test]
    async fn test_nullifier_deactivation() {
        let (merkle_db, db_name) = setup_merkle_tree_db().await;
        
        // Insert nullifier
        merkle_db.insert_nullifier_complete(100).await.unwrap();
        
        // Verify it exists
        let exists = merkle_db.get_membership_proof(100).await.unwrap();
        assert!(exists, "Nullifier should exist");
        
        // Deactivate it
        let deactivated = merkle_db.nullifiers.deactivate(100).await.unwrap();
        assert!(deactivated, "Nullifier should be deactivated");
        
        // Verify it no longer exists in active queries
        let exists = merkle_db.get_membership_proof(100).await.unwrap();
        assert!(!exists, "Nullifier should not exist after deactivation");
        
        // Try to deactivate again (should return false)
        let deactivated = merkle_db.nullifiers.deactivate(100).await.unwrap();
        assert!(!deactivated, "Should not deactivate already inactive nullifier");
        
        teardown_merkle_tree_db(db_name).await;
    }
}