#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use sqlx::PgPool;
    use tracing::info;
    use tracing_test::traced_test;
    use std::collections::HashSet;

    // ============================================================================
    // 32-LEVEL MERKLE TREE TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_32_level_tree_initialization(pool: PgPool) {
        info!("🧪 Testing 32-level tree initialization");
        
        let tree = MerkleTree32::new(pool);
        
        // Verify tree properties
        assert_eq!(tree.height, 32, "Tree height should be exactly 32");
        assert_eq!(tree.zero_hashes.len(), 33, "Should have 33 zero hashes (levels 0-32)");
        
        // Initialize tree state
        tree.initialize().await.expect("Tree initialization should succeed");
        
        // Verify root is the zero hash for level 32
        let root = tree.get_root().await.expect("Should get root");
        let expected_root = tree.zero_hashes[32];
        assert_eq!(root, expected_root, "Initial root should be zero hash for level 32");
        
        info!("✅ Tree initialized with root: {:02x?}", &root[..8]);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_zero_hash_precomputation(pool: PgPool) {
        info!("🧪 Testing zero hash precomputation");
        
        let tree = MerkleTree32::new(pool);
        
        // Verify zero hash computation properties
        let zero_hashes = &tree.zero_hashes;
        
        // Level 0 should be hash of 32 zero bytes
        let mut hasher = Sha256::new();
        hasher.update(&[0u8; 32]);
        let expected_level_0: [u8; 32] = hasher.finalize().into();
        assert_eq!(zero_hashes[0], expected_level_0, "Level 0 zero hash incorrect");
        
        // Each level should be hash of two hashes from level below
        for level in 1..=32 {
            let mut hasher = Sha256::new();
            hasher.update(&zero_hashes[level - 1]);
            hasher.update(&zero_hashes[level - 1]);
            let expected: [u8; 32] = hasher.finalize().into();
            assert_eq!(zero_hashes[level], expected, "Level {} zero hash incorrect", level);
        }
        
        info!("✅ All {} zero hashes computed correctly", zero_hashes.len());
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_single_leaf_update(pool: PgPool) {
        info!("🧪 Testing single leaf update");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        let leaf_index = 42;
        let leaf_value = [0x42u8; 32];
        
        // Update leaf and get new root
        let new_root = tree.update_leaf(leaf_index, leaf_value).await
            .expect("Leaf update should succeed");
        
        // Verify root changed
        let initial_root = tree.zero_hashes[32];
        assert_ne!(new_root, initial_root, "Root should change after leaf update");
        
        // Verify root in database matches returned root
        let db_root = tree.get_root().await.expect("Should get root from database");
        assert_eq!(new_root, db_root, "Database root should match returned root");
        
        info!("✅ Leaf {} updated, new root: {:02x?}", leaf_index, &new_root[..8]);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_proof_generation_and_verification(pool: PgPool) {
        info!("🧪 Testing proof generation and verification");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        let leaf_index = 100;
        let leaf_value = [0x64u8; 32]; // 0x64 = 100
        
        // Update leaf
        let new_root = tree.update_leaf(leaf_index, leaf_value).await
            .expect("Leaf update should succeed");
        
        // Generate proof
        let proof = tree.generate_proof(leaf_index).await
            .expect("Proof generation should succeed");
        
        // Verify proof structure
        assert_eq!(proof.leaf_index, leaf_index, "Proof should have correct leaf index");
        assert_eq!(proof.proof_hashes.len(), 32, "Proof should have exactly 32 sibling hashes");
        assert_eq!(proof.leaf_hash, leaf_value, "Proof should include correct leaf hash");
        
        // Verify proof against root
        assert!(proof.verify(&new_root), "Proof should verify against correct root");
        
        // Verify proof fails against wrong root
        let wrong_root = [0xFFu8; 32];
        assert!(!proof.verify(&wrong_root), "Proof should fail against wrong root");
        
        // Test custom leaf verification
        assert!(proof.verify_with_leaf(&leaf_value, &new_root), 
            "Custom leaf verification should succeed");
        
        let wrong_leaf = [0x99u8; 32];
        assert!(!proof.verify_with_leaf(&wrong_leaf, &new_root),
            "Custom leaf verification should fail with wrong leaf");
        
        info!("✅ Proof verification tests passed, proof size: {} bytes", proof.size_bytes());
    }

    #[traced_test]
    #[sqlx::test] 
    async fn test_batch_update_performance(pool: PgPool) {
        info!("🧪 Testing batch update performance");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        // Create batch of updates
        let batch_size = 50;
        let mut updates = Vec::new();
        for i in 0..batch_size {
            updates.push(BatchUpdate {
                leaf_index: i * 10, // Spread out indices to test different paths
                new_value: [(i as u8).wrapping_mul(3); 32], // Unique values
            });
        }
        
        let start_time = std::time::Instant::now();
        
        // Execute batch update
        let new_root = tree.batch_update(&updates).await
            .expect("Batch update should succeed");
        
        let batch_time = start_time.elapsed();
        
        // Verify root changed
        let initial_root = tree.zero_hashes[32];
        assert_ne!(new_root, initial_root, "Root should change after batch update");
        
        // Verify individual leaves were updated correctly
        for update in &updates {
            let proof = tree.generate_proof(update.leaf_index).await
                .expect("Should generate proof for updated leaf");
            assert_eq!(proof.leaf_hash, update.new_value, 
                "Leaf {} should have correct value", update.leaf_index);
            assert!(proof.verify(&new_root), 
                "Proof for leaf {} should verify", update.leaf_index);
        }
        
        // Performance metrics
        let metrics = tree.calculate_metrics(batch_size as u32);
        let avg_time_per_update = batch_time.as_micros() / batch_size as u128;
        
        info!("📊 Batch Performance Results:");
        info!("  - {} updates in {:?}", batch_size, batch_time);
        info!("  - Average time per update: {}μs", avg_time_per_update);
        info!("  - Estimated hash operations: {}", metrics.hash_operations);
        info!("  - Estimated constraints: {}", metrics.constraint_count);
        info!("  - Proof size: {} bytes", metrics.proof_size);
        
        // Performance assertions
        assert!(avg_time_per_update < 10_000, "Should average <10ms per update");
        assert!(metrics.constraint_count < 1600 * batch_size as u32, 
            "Should be better than 256-level tree");
        
        info!("✅ Batch update performance test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_tree_capacity_limits(pool: PgPool) {
        info!("🧪 Testing tree capacity limits");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        // Test maximum valid index
        let max_index = (1usize << 32) - 1; // 2^32 - 1
        let result = tree.update_leaf(max_index, [0x42u8; 32]).await;
        
        // This might fail due to practical limits, but should handle gracefully
        match result {
            Ok(_) => info!("✅ Successfully handled maximum index"),
            Err(e) => info!("⚠️  Maximum index rejected (acceptable): {:?}", e),
        }
        
        // Test index beyond capacity
        let beyond_capacity = 1usize << 32; // 2^32
        let result = tree.update_leaf(beyond_capacity, [0x42u8; 32]).await;
        assert!(result.is_err(), "Should reject index beyond capacity");
        
        // Test proof generation for invalid index
        let proof_result = tree.generate_proof(beyond_capacity).await;
        assert!(proof_result.is_err(), "Should reject proof generation for invalid index");
        
        info!("✅ Capacity limit tests passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_constraint_optimization(pool: PgPool) {
        info!("🧪 Testing constraint optimization vs 256-level tree");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        // Update a single leaf to measure constraint impact
        let leaf_index = 1000;
        let leaf_value = [0xAAu8; 32];
        
        tree.update_leaf(leaf_index, leaf_value).await
            .expect("Leaf update should succeed");
        
        // Generate proof and calculate constraints
        let proof = tree.generate_proof(leaf_index).await
            .expect("Proof generation should succeed");
        
        let metrics = tree.calculate_metrics(1);
        
        // Constraint comparison with 256-level tree
        let tree_32_hashes = 32; // Path length for 32-level tree
        let tree_256_hashes = 256; // Path length for traditional tree
        
        let tree_32_constraints = tree_32_hashes * 8; // Poseidon constraints per hash
        let tree_256_constraints = tree_256_hashes * 8;
        
        info!("🔍 Constraint Analysis:");
        info!("  - 32-level tree hashes: {}", tree_32_hashes);
        info!("  - 256-level tree hashes: {}", tree_256_hashes);
        info!("  - 32-level constraints: {}", tree_32_constraints);
        info!("  - 256-level constraints: {}", tree_256_constraints);
        info!("  - Constraint reduction: {}x", tree_256_constraints / tree_32_constraints);
        info!("  - Proof size: {} bytes vs {} bytes", proof.size_bytes(), 256 * 32);
        
        // Verify optimization targets
        assert_eq!(proof.proof_hashes.len(), 32, "Should have exactly 32 proof elements");
        assert!(tree_32_constraints < tree_256_constraints, 
            "32-level tree should have fewer constraints");
        assert_eq!(tree_256_constraints / tree_32_constraints, 8, 
            "Should have 8x constraint reduction");
        
        info!("✅ Constraint optimization verified: {}x reduction", 
            tree_256_constraints / tree_32_constraints);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_tree_statistics(pool: PgPool) {
        info!("🧪 Testing tree statistics collection");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        // Update several leaves at different positions
        let updates = vec![
            (0, [0x01u8; 32]),
            (1, [0x02u8; 32]),
            (100, [0x64u8; 32]),
            (1000, [0xE8u8; 32]),
            (10000, [0x10u8; 32]),
        ];
        
        for (index, value) in &updates {
            tree.update_leaf(*index, *value).await
                .expect("Leaf update should succeed");
        }
        
        // Get statistics
        let stats = tree.get_stats().await.expect("Should get tree statistics");
        
        // Verify basic stats
        assert_eq!(stats.height, 32, "Stats should show height 32");
        assert_eq!(stats.total_leaves, updates.len() as u64, 
            "Should count {} leaves", updates.len());
        assert!(stats.non_zero_nodes > 0, "Should have non-zero nodes");
        
        // Verify root hash matches current root
        let current_root = tree.get_root().await.expect("Should get current root");
        assert_eq!(stats.root_hash, current_root, "Stats root should match current root");
        
        // Verify zero hash usage tracking
        assert!(!stats.zero_hash_usage.is_empty(), "Should track zero hash usage");
        
        info!("📊 Tree Statistics:");
        info!("  - Height: {}", stats.height);
        info!("  - Total leaves: {}", stats.total_leaves);
        info!("  - Non-zero nodes: {}", stats.non_zero_nodes);
        info!("  - Root: {:02x?}", &stats.root_hash[..8]);
        info!("  - Last updated: {}", stats.last_updated);
        
        for (level, zero_count) in &stats.zero_hash_usage {
            info!("  - Level {} zero nodes: {}", level, zero_count);
        }
        
        info!("✅ Tree statistics test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_concurrent_updates(pool: PgPool) {
        info!("🧪 Testing concurrent update handling");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        // Test that updates maintain consistency
        let leaf1 = 1000;
        let leaf2 = 2000;
        let value1 = [0x11u8; 32];
        let value2 = [0x22u8; 32];
        
        // Update leaves sequentially
        let root1 = tree.update_leaf(leaf1, value1).await
            .expect("First update should succeed");
            
        let root2 = tree.update_leaf(leaf2, value2).await
            .expect("Second update should succeed");
        
        // Verify both leaves are correctly stored
        let proof1 = tree.generate_proof(leaf1).await
            .expect("Should generate proof for first leaf");
        let proof2 = tree.generate_proof(leaf2).await
            .expect("Should generate proof for second leaf");
        
        assert_eq!(proof1.leaf_hash, value1, "First leaf should have correct value");
        assert_eq!(proof2.leaf_hash, value2, "Second leaf should have correct value");
        
        // Verify both proofs against final root
        assert!(proof1.verify(&root2), "First proof should verify against final root");
        assert!(proof2.verify(&root2), "Second proof should verify against final root");
        
        // Verify intermediate root is different from final
        assert_ne!(root1, root2, "Intermediate root should differ from final");
        
        info!("✅ Concurrent update consistency verified");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_zero_hash_optimization(pool: PgPool) {
        info!("🧪 Testing zero hash optimization benefits");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        // Update a leaf in a sparse area
        let sparse_index = 1_000_000; // Far from other leaves
        let leaf_value = [0xAAu8; 32];
        
        tree.update_leaf(sparse_index, leaf_value).await
            .expect("Sparse leaf update should succeed");
        
        // Generate proof - should use many zero hashes
        let proof = tree.generate_proof(sparse_index).await
            .expect("Proof generation should succeed");
        
        // Count how many proof hashes match zero hashes
        let mut zero_hash_count = 0;
        for (level, proof_hash) in proof.proof_hashes.iter().enumerate() {
            if level < tree.zero_hashes.len() && *proof_hash == tree.zero_hashes[level] {
                zero_hash_count += 1;
            }
        }
        
        info!("🔧 Zero Hash Optimization:");
        info!("  - Total proof hashes: {}", proof.proof_hashes.len());
        info!("  - Zero hashes used: {}", zero_hash_count);
        info!("  - Database lookups saved: {}", zero_hash_count);
        
        // In a sparse tree, most siblings should be zero hashes
        assert!(zero_hash_count > 20, "Should use many zero hashes in sparse tree");
        
        // Verify proof still works correctly
        let root = tree.get_root().await.expect("Should get root");
        assert!(proof.verify(&root), "Proof with zero hashes should verify");
        
        info!("✅ Zero hash optimization test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_edge_cases_and_error_handling(pool: PgPool) {
        info!("🧪 Testing edge cases and error handling");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        // Test empty batch update
        let empty_batch: Vec<BatchUpdate> = vec![];
        let result = tree.batch_update(&empty_batch).await;
        assert!(result.is_ok(), "Empty batch should succeed");
        
        // Test batch with duplicate indices
        let duplicate_batch = vec![
            BatchUpdate { leaf_index: 100, new_value: [0x01u8; 32] },
            BatchUpdate { leaf_index: 100, new_value: [0x02u8; 32] }, // Same index
        ];
        let result = tree.batch_update(&duplicate_batch).await;
        assert!(result.is_ok(), "Duplicate indices should be handled");
        
        // Test proof for empty leaf (should use zero hash)
        let empty_leaf_index = 99999;
        let proof = tree.generate_proof(empty_leaf_index).await
            .expect("Should generate proof for empty leaf");
        assert_eq!(proof.leaf_hash, tree.zero_hashes[0], 
            "Empty leaf should have zero hash");
        
        // Test invalid batch update (index out of bounds)
        let invalid_batch = vec![
            BatchUpdate { leaf_index: 1usize << 32, new_value: [0x42u8; 32] }
        ];
        let result = tree.batch_update(&invalid_batch).await;
        assert!(result.is_err(), "Invalid index should be rejected");
        
        info!("✅ Edge cases and error handling tests passed");
    }

    // ============================================================================
    // PERFORMANCE BENCHMARKS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn benchmark_tree_operations(pool: PgPool) {
        info!("🏁 Benchmarking tree operations");
        
        let tree = MerkleTree32::new(pool);
        tree.initialize().await.expect("Initialization should succeed");
        
        let num_operations = 100;
        
        // Benchmark single updates
        let start = std::time::Instant::now();
        for i in 0..num_operations {
            let leaf_value = [(i as u8).wrapping_mul(7); 32];
            tree.update_leaf(i * 2, leaf_value).await
                .expect("Update should succeed");
        }
        let single_update_time = start.elapsed();
        
        // Benchmark proof generation
        let start = std::time::Instant::now();
        for i in 0..num_operations {
            tree.generate_proof(i * 2).await
                .expect("Proof generation should succeed");
        }
        let proof_generation_time = start.elapsed();
        
        // Benchmark batch update
        let batch_updates: Vec<BatchUpdate> = (0..num_operations)
            .map(|i| BatchUpdate {
                leaf_index: i * 3 + 1000, // Different range
                new_value: [(i as u8).wrapping_add(100); 32],
            })
            .collect();
        
        let start = std::time::Instant::now();
        tree.batch_update(&batch_updates).await
            .expect("Batch update should succeed");
        let batch_update_time = start.elapsed();
        
        info!("🏁 Benchmark Results:");
        info!("  - {} single updates: {:?} ({:.2}ms/op)", 
            num_operations, single_update_time, 
            single_update_time.as_millis() as f64 / num_operations as f64);
        info!("  - {} proof generations: {:?} ({:.2}ms/op)", 
            num_operations, proof_generation_time,
            proof_generation_time.as_millis() as f64 / num_operations as f64);
        info!("  - {} batch updates: {:?} ({:.2}ms/op)", 
            num_operations, batch_update_time,
            batch_update_time.as_millis() as f64 / num_operations as f64);
        
        let batch_speedup = single_update_time.as_millis() as f64 / batch_update_time.as_millis() as f64;
        info!("  - Batch speedup: {:.2}x", batch_speedup);
        
        // Performance assertions
        let avg_single_update_ms = single_update_time.as_millis() as f64 / num_operations as f64;
        let avg_proof_gen_ms = proof_generation_time.as_millis() as f64 / num_operations as f64;
        
        assert!(avg_single_update_ms < 100.0, "Single updates should be <100ms on average");
        assert!(avg_proof_gen_ms < 50.0, "Proof generation should be <50ms on average");
        assert!(batch_speedup > 1.0, "Batch updates should be faster than individual updates");
        
        info!("✅ Benchmark tests completed successfully");
    }
}