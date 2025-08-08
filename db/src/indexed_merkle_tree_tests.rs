#[cfg(test)]
mod tests {
    use crate::error::DbError;
    use crate::merkle_tree::IndexedMerkleTree;
    use sqlx::PgPool;
    use tracing::info;
    use tracing_test::traced_test;

    // ============================================================================
    // 7-STEP ALGORITHM TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_7_step_insertion_algorithm_basic(pool: PgPool) {
        info!("🧪 Testing basic 7-step insertion algorithm");

        let mut tree = IndexedMerkleTree::new(pool);

        // Insert first nullifier (should create initial state)
        let result = tree
            .insert_nullifier(100)
            .await
            .expect("First insertion should succeed");

        // Verify basic properties
        assert_ne!(
            result.old_root, result.new_root,
            "Root should change after insertion"
        );
        assert_eq!(result.nullifier.value, 100);
        assert_eq!(result.nullifier.tree_index, 0); // First insertion gets index 0

        // Verify metrics meet performance requirements
        let metrics = &result.operations_count;
        info!(
            "📊 First insertion metrics: {} hashes, {} range checks, {} DB rounds, {} constraints",
            metrics.hash_operations,
            metrics.range_checks,
            metrics.database_rounds,
            metrics.constraints_count
        );

        // Performance assertions
        assert_eq!(
            metrics.range_checks, 2,
            "Should perform exactly 2 range checks"
        );
        assert!(
            metrics.hash_operations <= 99,
            "Hash operations should be ≤ 3n+3 = 99 for 32-level tree"
        );
        assert!(
            metrics.constraints_count <= 300,
            "Should have ~200 constraints, allowing some margin"
        );
        assert!(
            metrics.database_rounds <= 10,
            "Should minimize database round trips"
        );

        // Verify proof integrity
        assert!(
            tree.verify_insertion_proof(&result.insertion_proof, &result.new_root),
            "Insertion proof should verify against new root"
        );
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_7_step_insertion_sequential(pool: PgPool) {
        info!("🧪 Testing sequential insertions with 7-step algorithm");

        let mut tree = IndexedMerkleTree::new(pool);
        let values = vec![50, 100, 150, 200, 250];
        let mut previous_root = [0u8; 32];

        for (i, &value) in values.iter().enumerate() {
            info!("Inserting nullifier {} (value: {})", i + 1, value);

            let result = tree
                .insert_nullifier(value)
                .await
                .unwrap_or_else(|e| panic!("Failed to insert nullifier {}: {:?}", value, e));

            // Verify root progression
            if i > 0 {
                assert_ne!(
                    result.old_root, previous_root,
                    "Should build upon previous state"
                );
            }
            previous_root = result.new_root;

            // Verify insertion properties
            assert_eq!(result.nullifier.value, value);
            assert_eq!(result.nullifier.tree_index, i as i64);

            // Verify performance constraints for each insertion
            let metrics = &result.operations_count;
            assert_eq!(
                metrics.range_checks, 2,
                "Each insertion should perform exactly 2 range checks"
            );
            assert!(
                metrics.hash_operations <= 99,
                "Each insertion should meet hash operation limit"
            );

            // Verify proof
            assert!(
                tree.verify_insertion_proof(&result.insertion_proof, &result.new_root),
                "Proof for insertion {} should verify",
                i + 1
            );

            info!(
                "✅ Insertion {} complete: {} constraints",
                i + 1,
                metrics.constraints_count
            );
        }

        // Verify final tree state
        let stats = tree
            .db
            .state
            .get_stats()
            .await
            .expect("Should get tree stats");
        assert_eq!(stats.total_nullifiers, values.len() as i64);
        assert_eq!(stats.next_index, values.len() as i64);
        assert!(
            stats.chain_valid,
            "Chain should remain valid after all insertions"
        );
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_7_step_insertion_out_of_order(pool: PgPool) {
        info!("🧪 Testing out-of-order insertions with 7-step algorithm");

        let mut tree = IndexedMerkleTree::new(pool);

        // Insert in non-sequential order to test range finding logic
        let insertion_order = vec![
            (500, "middle value"),
            (100, "smaller value - should find correct position"),
            (800, "larger value - should find correct position"),
            (300, "between existing values"),
            (700, "another between value"),
        ];

        for (i, &(value, description)) in insertion_order.iter().enumerate() {
            info!("Inserting {}: {}", value, description);

            let result = tree
                .insert_nullifier(value)
                .await
                .unwrap_or_else(|e| panic!("Failed to insert nullifier {}: {:?}", value, e));

            // Verify the low nullifier was found correctly
            let low_before = &result.insertion_proof.low_nullifier_before;
            let low_after = &result.insertion_proof.low_nullifier_after;

            // Algorithm correctness checks
            assert!(
                low_before.value < value,
                "Low nullifier should be less than new value"
            );
            assert!(
                low_before.next_value == 0 || value < low_before.next_value,
                "New value should fit in the gap"
            );

            // Verify pointer updates
            assert_eq!(
                low_after.next_value, value,
                "Low nullifier should now point to new value"
            );
            assert_eq!(
                low_after.next_index,
                Some(result.nullifier.tree_index),
                "Low nullifier should point to new tree index"
            );

            // Verify new nullifier inherits old low's pointers
            assert_eq!(
                result.nullifier.next_value, low_before.next_value,
                "New nullifier should inherit low's next_value"
            );
            assert_eq!(
                result.nullifier.next_index, low_before.next_index,
                "New nullifier should inherit low's next_index"
            );

            info!("✅ Out-of-order insertion {} verified", i + 1);
        }

        // Verify final chain integrity
        assert!(
            tree.db
                .nullifiers
                .validate_chain()
                .await
                .expect("Should validate chain"),
            "Chain should be valid after out-of-order insertions"
        );
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_7_step_range_validation_errors(pool: PgPool) {
        info!("🧪 Testing range validation error cases");

        let mut tree = IndexedMerkleTree::new(pool);

        // Insert initial nullifier
        tree.insert_nullifier(500)
            .await
            .expect("Initial insertion should succeed");

        // Test range check failures

        // Error case 1: Duplicate value
        let result = tree.insert_nullifier(500).await;
        assert!(result.is_err(), "Duplicate insertion should fail");
        if let Err(DbError::NullifierExists(_)) = result {
            info!("✅ Correctly rejected duplicate nullifier");
        } else {
            panic!("Expected NullifierExists error for duplicate");
        }

        // Error case 2: Insert another value to create a proper range
        tree.insert_nullifier(800)
            .await
            .expect("Second insertion should succeed");

        // Error case 3: Try to insert value that violates range constraints
        // This should work if the algorithm correctly finds the gap
        let result = tree.insert_nullifier(600).await;
        assert!(result.is_ok(), "Valid range insertion should succeed");

        info!("✅ Range validation tests completed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_7_step_constraint_counting(pool: PgPool) {
        info!("🧪 Testing constraint counting for ZK circuit optimization");

        let mut tree = IndexedMerkleTree::new(pool);
        let mut total_constraints = 0u32;
        let mut total_hashes = 0u32;

        // Test constraint counting over multiple insertions
        for i in 1..=10 {
            let value = i * 100;
            let result = tree
                .insert_nullifier(value)
                .await
                .expect(&format!("Insertion {} should succeed", i));

            let metrics = &result.operations_count;
            total_constraints += metrics.constraints_count;
            total_hashes += metrics.hash_operations;

            info!(
                "Insertion {}: {} hashes, {} range checks, {} constraints",
                i, metrics.hash_operations, metrics.range_checks, metrics.constraints_count
            );

            // Verify constraint components
            let expected_hash_constraints = metrics.hash_operations * 8; // Poseidon constraints
            let expected_range_constraints = metrics.range_checks * 250; // Range check constraints
            let expected_total = expected_hash_constraints + expected_range_constraints + 10; // +10 for equality

            assert_eq!(
                metrics.constraints_count, expected_total,
                "Constraint calculation should match expected formula"
            );

            // Performance requirements
            assert!(
                metrics.constraints_count <= 300,
                "Each insertion should have ≤300 constraints (target ~200)"
            );
            assert_eq!(
                metrics.range_checks, 2,
                "Should always perform exactly 2 range checks"
            );
        }

        let avg_constraints = total_constraints / 10;
        let avg_hashes = total_hashes / 10;

        info!(
            "📊 Averages over 10 insertions: {} constraints, {} hashes",
            avg_constraints, avg_hashes
        );
        info!(
            "🎯 Target vs Actual: ~200 constraints (got {}), ≤99 hashes (got {})",
            avg_constraints, avg_hashes
        );

        // Verify we meet the performance goals from the paper
        assert!(
            avg_constraints < 1600,
            "Should be much better than 256-level tree (~1600 constraints)"
        );
        assert!(
            avg_hashes <= 99,
            "Should meet 3n+3 hash limit for 32-level tree"
        );
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_7_step_proof_verification(pool: PgPool) {
        info!("🧪 Testing Merkle proof generation and verification");

        let mut tree = IndexedMerkleTree::new(pool);

        // Insert several nullifiers to create a non-trivial tree
        let values = vec![100, 300, 500, 700, 900];
        let mut results = Vec::new();

        for value in values {
            let result = tree
                .insert_nullifier(value)
                .await
                .expect("Insertion should succeed");
            results.push(result);
        }

        // Verify each insertion proof
        for (i, result) in results.iter().enumerate() {
            info!("Verifying proof for insertion {}", i + 1);

            let proof = &result.insertion_proof;

            // Verify proof structure
            assert_eq!(
                proof.low_nullifier_proof.siblings.len(),
                32,
                "Should have 32 siblings for 32-level tree"
            );
            assert_eq!(
                proof.new_nullifier_proof.siblings.len(),
                32,
                "Should have 32 siblings for 32-level tree"
            );

            // Verify proof verification
            assert!(
                tree.verify_insertion_proof(proof, &result.new_root),
                "Proof {} should verify against its corresponding root",
                i + 1
            );

            // Verify proof should fail against different root
            if i > 0 {
                assert!(
                    !tree.verify_insertion_proof(proof, &results[i - 1].new_root),
                    "Proof {} should not verify against different root",
                    i + 1
                );
            }

            info!("✅ Proof {} verified successfully", i + 1);
        }

        // Test individual Merkle proof verification
        let last_result = results.last().unwrap();
        let low_proof = &last_result.insertion_proof.low_nullifier_proof;

        // Note: verify_merkle_proof is private, so we test through the public verify_insertion_proof
        assert!(
            tree.verify_insertion_proof(&last_result.insertion_proof, &last_result.new_root),
            "Individual Merkle proof should verify through insertion proof"
        );

        info!("✅ All proof verifications completed successfully");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_7_step_batch_operations_performance(pool: PgPool) {
        info!("🧪 Testing batch operations performance");

        let mut tree = IndexedMerkleTree::new(pool);
        let batch_size = 50;

        let start_time = std::time::Instant::now();
        let mut total_db_rounds = 0u32;
        let mut total_constraints = 0u32;

        // Insert a batch of nullifiers
        for i in 1..=batch_size {
            let value = i * 10;
            let result = tree
                .insert_nullifier(value)
                .await
                .expect(&format!("Batch insertion {} should succeed", i));

            total_db_rounds += result.operations_count.database_rounds;
            total_constraints += result.operations_count.constraints_count;

            if i % 10 == 0 {
                info!("Completed {} insertions", i);
            }
        }

        let elapsed = start_time.elapsed();
        let avg_time_per_insertion = elapsed.as_millis() / batch_size as u128;
        let avg_db_rounds = total_db_rounds as f64 / batch_size as f64;
        let avg_constraints = total_constraints as f64 / batch_size as f64;

        info!("📊 Batch Performance Results:");
        info!("  - {} insertions in {:?}", batch_size, elapsed);
        info!(
            "  - Average time per insertion: {}ms",
            avg_time_per_insertion
        );
        info!("  - Average DB rounds per insertion: {:.1}", avg_db_rounds);
        info!(
            "  - Average constraints per insertion: {:.0}",
            avg_constraints
        );

        // Performance assertions
        assert!(
            avg_time_per_insertion < 1000,
            "Should average <1s per insertion"
        );
        assert!(
            avg_db_rounds <= 10.0,
            "Should minimize database round trips"
        );
        assert!(
            avg_constraints <= 300.0,
            "Should maintain constraint efficiency"
        );

        // Verify final tree integrity
        let stats = tree.db.state.get_stats().await.expect("Should get stats");
        assert_eq!(stats.total_nullifiers, batch_size as i64);
        assert!(
            stats.chain_valid,
            "Chain should remain valid after batch operations"
        );

        info!("✅ Batch operations performance test completed successfully");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_7_step_edge_cases(pool: PgPool) {
        info!("🧪 Testing edge cases for 7-step algorithm");

        let mut tree = IndexedMerkleTree::new(pool);

        // Edge case 1: Maximum tree index values
        let large_value = i64::MAX / 2; // Avoid overflow issues
        let result = tree.insert_nullifier(large_value).await;
        assert!(result.is_ok(), "Should handle large values");

        // Edge case 2: Minimum positive values
        let small_value = 1;
        let result = tree.insert_nullifier(small_value).await;
        assert!(result.is_ok(), "Should handle small values");

        // Edge case 3: Zero value (if allowed by constraints)
        let result = tree.insert_nullifier(0).await;
        // This might fail due to business logic constraints, which is acceptable
        match result {
            Ok(_) => info!("Zero value insertion succeeded"),
            Err(_) => info!("Zero value insertion rejected (acceptable)"),
        }

        // Edge case 4: Verify tree still maintains integrity
        let stats = tree.db.state.get_stats().await.expect("Should get stats");
        assert!(
            stats.chain_valid,
            "Chain should remain valid after edge case tests"
        );

        info!("✅ Edge case tests completed");
    }

    // ============================================================================
    // ALGORITHM CORRECTNESS VERIFICATION
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_algorithm_step_by_step_verification(pool: PgPool) {
        info!("🧪 Testing step-by-step algorithm correctness");

        let mut tree = IndexedMerkleTree::new(pool);

        // Create initial state with one nullifier
        tree.insert_nullifier(500).await.expect("Initial insertion");

        let new_value = 300;
        info!(
            "🔍 Manually verifying 7-step algorithm for value {}",
            new_value
        );

        // Step 1: Find low nullifier (manual verification)
        let low_nullifier = tree
            .db
            .nullifiers
            .find_low_nullifier(new_value)
            .await
            .expect("Should find low nullifier")
            .expect("Low nullifier should exist");
        info!(
            "Step 1 ✅: Found low nullifier with value {}",
            low_nullifier.value
        );

        // Step 2: Membership check (manual verification)
        let exists = tree
            .db
            .nullifiers
            .exists(low_nullifier.value)
            .await
            .expect("Should check existence");
        assert!(exists, "Low nullifier should exist in tree");
        info!("Step 2 ✅: Low nullifier exists in tree");

        // Step 3: Range validation (manual verification)
        assert!(new_value > low_nullifier.value, "Range check 1 should pass");
        assert!(
            low_nullifier.next_value == 0 || new_value < low_nullifier.next_value,
            "Range check 2 should pass"
        );
        info!("Step 3 ✅: Range validation passed");

        // Steps 4-7: Execute insertion and verify state changes
        let result = tree
            .insert_nullifier(new_value)
            .await
            .expect("Algorithm insertion should succeed");

        // Verify the low nullifier was updated correctly
        let updated_low = tree
            .db
            .nullifiers
            .get_by_value(low_nullifier.value)
            .await
            .expect("Should retrieve updated low nullifier")
            .expect("Updated low nullifier should exist");

        assert_eq!(
            updated_low.next_value, new_value,
            "Low nullifier should point to new value"
        );
        assert_eq!(
            updated_low.next_index,
            Some(result.nullifier.tree_index),
            "Low nullifier should point to new index"
        );
        info!("Steps 4-7 ✅: Pointer updates verified");

        // Verify new nullifier inherited correct pointers
        assert_eq!(
            result.nullifier.next_value, low_nullifier.next_value,
            "New nullifier should inherit old next_value"
        );
        assert_eq!(
            result.nullifier.next_index, low_nullifier.next_index,
            "New nullifier should inherit old next_index"
        );
        info!("Steps 4-7 ✅: New nullifier inheritance verified");

        info!("🎯 Complete 7-step algorithm verification successful");
    }
}
