use crate::db::{
    get_pending_transactions, init_db, submit_transaction,
};
use crate::test_utils::TestDatabase;
use std::env;

#[cfg(test)]
mod db_tests {
    use super::*;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_init_db_missing_env_var() {
        // Save original DATABASE_URL if it exists
        let original_db_url = env::var("DATABASE_URL").ok();

        // Temporarily remove DATABASE_URL
        env::remove_var("DATABASE_URL");

        let result = init_db().await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(matches!(error, sqlx::Error::Configuration(_)));

        // Restore original DATABASE_URL if it existed
        if let Some(url) = original_db_url {
            env::set_var("DATABASE_URL", url);
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_submit_and_retrieve_transaction() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Submit a transaction
        let amount = 15;

        let transaction = submit_transaction(&test_db.pool, amount)
            .await
            .expect("Failed to submit transaction");

        // Retrieve pending transactions
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve pending transactions");

        assert_eq!(pending_transactions.len(), 1);
        assert_eq!(pending_transactions[0].amount, amount);
        assert_eq!(pending_transactions[0].id, transaction.id);
        assert!(pending_transactions[0].included_in_batch_id.is_none());

        // Verify that created_at is recent (within last minute)
        let now = chrono::Utc::now();
        assert!(now.signed_duration_since(pending_transactions[0].created_at).num_seconds() < 60);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_submit_multiple_transactions() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Submit multiple transactions
        let amounts = vec![5, 7, 3];
        let mut submitted_ids = vec![];

        for amount in &amounts {
            let transaction = submit_transaction(&test_db.pool, *amount)
                .await
                .expect("Failed to submit transaction");
            submitted_ids.push(transaction.id);
        }

        // Retrieve all pending transactions
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve pending transactions");

        assert_eq!(pending_transactions.len(), 3);

        // Verify all amounts are present
        let mut found_amounts: Vec<i32> = pending_transactions.iter().map(|t| t.amount).collect();
        found_amounts.sort_unstable();
        let mut expected_amounts = amounts;
        expected_amounts.sort_unstable();
        assert_eq!(found_amounts, expected_amounts);

        // Verify all are pending (not batched)
        for transaction in pending_transactions {
            assert!(transaction.included_in_batch_id.is_none());
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_multiple_same_amount_transactions() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Submit the same amount multiple times (this should be allowed)
        let amount = 15;
        let mut submitted_ids = vec![];

        for _ in 0..3 {
            let transaction = submit_transaction(&test_db.pool, amount)
                .await
                .expect("Failed to submit transaction");
            submitted_ids.push(transaction.id);
        }

        // Should have three separate transactions (each with unique ID)
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve transactions");

        assert_eq!(pending_transactions.len(), 3);
        for transaction in &pending_transactions {
            assert_eq!(transaction.amount, amount);
            assert!(submitted_ids.contains(&transaction.id));
        }

        // Verify IDs are unique
        let mut found_ids: Vec<i32> = pending_transactions.iter().map(|t| t.id).collect();
        found_ids.sort_unstable();
        submitted_ids.sort_unstable();
        assert_eq!(found_ids, submitted_ids);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_transaction_amount_validation() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Test various transaction amounts
        let test_amounts = vec![15, 0, -2, 50, -30];

        for amount in test_amounts {
            let transaction = submit_transaction(&test_db.pool, amount)
                .await
                .expect("Failed to submit transaction");

            assert_eq!(transaction.amount, amount);
            assert!(transaction.id > 0);
            assert!(transaction.included_in_batch_id.is_none());
        }

        // Verify all transactions are pending
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve pending transactions");

        assert_eq!(pending_transactions.len(), 5);
        
        // Verify all expected amounts are present
        let mut found_amounts: Vec<i32> = pending_transactions.iter().map(|t| t.amount).collect();
        found_amounts.sort_unstable();
        let expected_amounts = vec![-30, -2, 0, 15, 50];
        assert_eq!(found_amounts, expected_amounts);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_empty_pending_transactions() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Query for pending transactions when none exist
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to query for pending transactions");

        assert!(pending_transactions.is_empty());

        // Submit a transaction
        submit_transaction(&test_db.pool, 3)
            .await
            .expect("Failed to submit transaction");

        // Now should have one pending transaction
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to query for pending transactions");

        assert_eq!(pending_transactions.len(), 1);
        assert_eq!(pending_transactions[0].amount, 3);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_large_number_handling() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Test with large numbers (within i32 range)
        let large_amount = i32::MAX / 2;

        let transaction = submit_transaction(&test_db.pool, large_amount)
            .await
            .expect("Failed to submit large number transaction");

        assert_eq!(transaction.amount, large_amount);

        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve large number transaction");

        assert_eq!(pending_transactions.len(), 1);
        assert_eq!(pending_transactions[0].amount, large_amount);
        assert_eq!(pending_transactions[0].id, transaction.id);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_concurrent_operations() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Create multiple tasks that store transactions concurrently
        let mut tasks = vec![];

        for i in 0..10 {
            let pool = test_db.pool.clone();
            let task = tokio::spawn(async move {
                submit_transaction(&pool, i * 3)
                    .await
                    .expect("Failed to submit transaction in concurrent test");
            });
            tasks.push(task);
        }

        // Wait for all tasks to complete
        for task in tasks {
            task.await.expect("Task failed");
        }

        // Verify all transactions were submitted
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve transactions in concurrent test");

        assert_eq!(pending_transactions.len(), 10);
        
        // Verify all expected amounts are present
        let mut found_amounts: Vec<i32> = pending_transactions.iter().map(|t| t.amount).collect();
        found_amounts.sort_unstable();
        let expected_amounts: Vec<i32> = (0..10).map(|i| i * 3).collect();
        assert_eq!(found_amounts, expected_amounts);
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_bulk_insert_performance() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let start = Instant::now();

        // Submit 1000 transactions
        for i in 0..1000 {
            submit_transaction(&test_db.pool, i * 2 + 1)
                .await
                .expect("Failed to submit bulk transaction");
        }

        let duration = start.elapsed();
        println!("Bulk insert of 1000 transactions took: {duration:?}");

        // Verify all transactions were submitted
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve bulk transactions");

        assert_eq!(pending_transactions.len(), 1000);
        
        // Verify first 100 amounts to avoid too much verification overhead
        let expected_amounts: Vec<i32> = (0..100).map(|i| i * 2 + 1).collect();
        let mut found_amounts: Vec<i32> = pending_transactions
            .iter()
            .take(100)
            .map(|t| t.amount)
            .collect();
        found_amounts.sort_unstable();
        
        // Check that all expected amounts are present (may not be in order)
        for expected in expected_amounts {
            assert!(
                pending_transactions.iter().any(|t| t.amount == expected),
                "Expected amount {} not found in pending transactions",
                expected
            );
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_query_performance_with_large_dataset() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Create a dataset with the same amount to test processing efficiency
        for _ in 0..500 {
            submit_transaction(&test_db.pool, 1000)
                .await
                .expect("Failed to submit performance test transaction");
        }

        let start = Instant::now();
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve performance test transactions");
        let duration = start.elapsed();

        println!("Query for pending transactions took: {duration:?}");
        assert_eq!(pending_transactions.len(), 500);

        // Verify all transactions have the correct amount
        for transaction in pending_transactions {
            assert_eq!(transaction.amount, 1000);
            assert!(transaction.included_in_batch_id.is_none());
        }
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_boundary_values() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Test edge cases for i32 amounts
        let test_amounts = vec![
            i32::MIN,
            i32::MAX,
            i32::MIN + 1,
            i32::MAX - 1,
            0,
        ];

        for amount in test_amounts {
            let transaction = submit_transaction(&test_db.pool, amount)
                .await
                .expect("Failed to submit boundary value transaction");

            assert_eq!(transaction.amount, amount);
            assert!(transaction.id > 0);
        }

        // Verify all boundary value transactions are pending
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve boundary value transactions");

        assert_eq!(pending_transactions.len(), 5);
        
        // Verify all amounts are present
        let expected_amounts = vec![i32::MIN, i32::MIN + 1, 0, i32::MAX - 1, i32::MAX];
        for expected in expected_amounts {
            assert!(
                pending_transactions.iter().any(|t| t.amount == expected),
                "Boundary value amount {} not found",
                expected
            );
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_zero_operations() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Test zero and offsetting amounts
        let zero_amounts = vec![0, 0, 0, 0]; // Submit multiple zero amounts

        for amount in zero_amounts {
            submit_transaction(&test_db.pool, amount)
                .await
                .expect("Failed to submit zero operation transaction");
        }

        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve zero amount transactions");

        assert_eq!(pending_transactions.len(), 4);

        for transaction in &pending_transactions {
            assert_eq!(transaction.amount, 0);
            assert!(transaction.included_in_batch_id.is_none());
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_negative_number_operations() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let negative_amounts = vec![-8, -5, -5, -300];

        for amount in negative_amounts {
            let transaction = submit_transaction(&test_db.pool, amount)
                .await
                .expect("Failed to submit negative number transaction");

            assert_eq!(transaction.amount, amount);
            assert!(transaction.id > 0);
        }

        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve negative number transactions");

        assert_eq!(pending_transactions.len(), 4);
        
        // Verify all negative amounts are present
        let expected_amounts = vec![-300, -8, -5, -5];
        let mut found_amounts: Vec<i32> = pending_transactions.iter().map(|t| t.amount).collect();
        found_amounts.sort_unstable();
        let mut expected_sorted = expected_amounts;
        expected_sorted.sort_unstable();
        assert_eq!(found_amounts, expected_sorted);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_arithmetic_validation_with_incorrect_results() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Test that we store transaction amounts as given
        // (This tests our database stores what we submit)
        let test_amounts = vec![16, 9, 0];

        for amount in test_amounts {
            let transaction = submit_transaction(&test_db.pool, amount)
                .await
                .expect("Failed to submit transaction");

            assert_eq!(transaction.amount, amount);
            assert!(transaction.id > 0);
        }

        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve transactions");

        assert_eq!(pending_transactions.len(), 3);
        
        // Verify that what we submitted is what we get back
        let expected_amounts = vec![0, 9, 16];
        let mut found_amounts: Vec<i32> = pending_transactions.iter().map(|t| t.amount).collect();
        found_amounts.sort_unstable();
        assert_eq!(found_amounts, expected_amounts);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_stress_duplicate_handling() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let result = 100;

        // Attempt to submit the same amount 100 times (should create 100 transactions)
        for _ in 0..100 {
            submit_transaction(&test_db.pool, result)
                .await
                .expect("Failed to submit duplicate stress test transaction");
        }

        // Should have 100 separate transactions with same amount
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve duplicate stress test transactions");

        // Filter to our specific amount
        let our_transactions: Vec<_> = pending_transactions
            .iter()
            .filter(|t| t.amount == result)
            .collect();

        assert_eq!(our_transactions.len(), 100);
        
        // Verify all have unique IDs
        let mut ids: Vec<i32> = our_transactions.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 100, "All transaction IDs should be unique");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use arithmetic_lib::{addition, PublicValuesStruct};
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_integration_with_arithmetic_lib() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Use the actual arithmetic library to compute amounts for testing
        let test_cases = vec![(5, 10), (25, 75), (-10, 30), (0, 0), (i32::MAX / 2, 1000)];
        let mut submitted_amounts = vec![];

        for (a, b) in test_cases {
            // Use the actual arithmetic function from the lib to compute amount
            let computed_amount = addition(a, b);
            submitted_amounts.push(computed_amount);

            // Submit the transaction with the computed amount
            let transaction = submit_transaction(&test_db.pool, computed_amount)
                .await
                .expect("Failed to submit integration test transaction");

            assert_eq!(transaction.amount, computed_amount);
        }

        // Verify retrieval of all submitted transactions
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve integration test transactions");

        assert_eq!(pending_transactions.len(), 5);
        
        // Verify all computed amounts are present
        for expected_amount in submitted_amounts {
            assert!(
                pending_transactions.iter().any(|t| t.amount == expected_amount),
                "Integration test transaction not found with amount: {}",
                expected_amount
            );
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_public_values_struct_compatibility() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Test that our database operations work with the PublicValuesStruct format
        let a = 15;
        let b = 25;
        let computed_amount = addition(a, b);

        // Create a PublicValuesStruct like the zkVM would (only final_balance is public)
        let public_values = PublicValuesStruct {
            initial_balance: 0,
            final_balance: computed_amount,
        };

        // Submit using computed amount (a and b are private inputs, not stored)
        let transaction = submit_transaction(&test_db.pool, public_values.final_balance)
            .await
            .expect("Failed to submit PublicValuesStruct transaction");

        // Retrieve and verify
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to retrieve PublicValuesStruct transaction");

        assert_eq!(pending_transactions.len(), 1);
        let stored_transaction = &pending_transactions[0];

        // Verify the amount matches the PublicValuesStruct final_balance
        assert_eq!(stored_transaction.amount, public_values.final_balance);
        assert_eq!(stored_transaction.id, transaction.id);

        // Verify the computation was correct
        assert_eq!(addition(a, b), stored_transaction.amount);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_workflow_simulation() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Simulate the full workflow: zkVM computes, system submits, later batch processing
        let zkvm_computations = vec![
            (7, 13),    // zkVM computes 7 + 13 = 20
            (100, 200), // zkVM computes 100 + 200 = 300
            (-50, 75),  // zkVM computes -50 + 75 = 25
        ];

        let mut expected_amounts = vec![];

        // Phase 1: zkVM execution and transaction submission (simulated)
        for (a, b) in &zkvm_computations {
            let amount = addition(*a, *b); // Simulate zkVM computation
            expected_amounts.push(amount);

            // System submits the transaction amount
            submit_transaction(&test_db.pool, amount)
                .await
                .expect("Failed to submit workflow simulation transaction");
        }

        // Phase 2: Later batch processing verification (simulated)
        let pending_transactions = get_pending_transactions(&test_db.pool)
            .await
            .expect("Failed to get pending transactions for workflow verification");

        assert_eq!(pending_transactions.len(), 3);
        
        // Verify all computed amounts are present and ready for batching
        for expected_amount in expected_amounts {
            assert!(
                pending_transactions.iter().any(|t| t.amount == expected_amount),
                "Workflow verification failed: amount {} not found in pending transactions",
                expected_amount
            );
        }
        
        // Verify all transactions are pending (not yet batched)
        for transaction in &pending_transactions {
            assert!(transaction.included_in_batch_id.is_none(), "Transaction should not be batched yet");
        }
    }
}
