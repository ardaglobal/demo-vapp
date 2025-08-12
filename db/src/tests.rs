use crate::db::{
    get_transactions_by_result, get_value_by_result, init_db, store_arithmetic_transaction,
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
    async fn test_store_and_retrieve_transaction() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Store a transaction
        let a = 5;
        let b = 10;
        let result = 15;

        store_arithmetic_transaction(&test_db.pool, a, b, result)
            .await
            .expect("Failed to store transaction");

        // Retrieve by result
        let transactions = get_transactions_by_result(&test_db.pool, result)
            .await
            .expect("Failed to retrieve transactions");

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].a, a);
        assert_eq!(transactions[0].b, b);
        assert_eq!(transactions[0].result, result);

        // Test get_value_by_result
        let value = get_value_by_result(&test_db.pool, result)
            .await
            .expect("Failed to get value by result");

        assert!(value.is_some());
        let (retrieved_a, retrieved_b, created_at) = value.unwrap();
        assert_eq!(retrieved_a, a);
        assert_eq!(retrieved_b, b);
        // Verify that created_at is recent (within last minute)
        let now = chrono::Utc::now();
        assert!(now.signed_duration_since(created_at).num_seconds() < 60);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_store_multiple_transactions_same_result() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Store multiple transactions that result in the same value
        store_arithmetic_transaction(&test_db.pool, 5, 10, 15)
            .await
            .expect("Failed to store first transaction");

        store_arithmetic_transaction(&test_db.pool, 7, 8, 15)
            .await
            .expect("Failed to store second transaction");

        store_arithmetic_transaction(&test_db.pool, 3, 12, 15)
            .await
            .expect("Failed to store third transaction");

        // Retrieve all transactions with result 15
        let transactions = get_transactions_by_result(&test_db.pool, 15)
            .await
            .expect("Failed to retrieve transactions");

        assert_eq!(transactions.len(), 3);

        // Verify all combinations are present
        let mut found_combinations = vec![];
        for transaction in transactions {
            found_combinations.push((transaction.a, transaction.b));
            assert_eq!(transaction.result, 15);
        }

        found_combinations.sort_unstable();
        let mut expected = vec![(3, 12), (5, 10), (7, 8)];
        expected.sort_unstable();
        assert_eq!(found_combinations, expected);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_duplicate_transaction_handling() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Store the same transaction multiple times
        let a = 5;
        let b = 10;
        let result = 15;

        for _ in 0..3 {
            store_arithmetic_transaction(&test_db.pool, a, b, result)
                .await
                .expect("Failed to store transaction");
        }

        // Should only have one transaction due to UNIQUE constraint
        let transactions = get_transactions_by_result(&test_db.pool, result)
            .await
            .expect("Failed to retrieve transactions");

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].a, a);
        assert_eq!(transactions[0].b, b);
        assert_eq!(transactions[0].result, result);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_arithmetic_correctness_validation() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Test valid arithmetic operations
        let test_cases = vec![
            (5, 10, 15),
            (0, 0, 0),
            (-5, 3, -2),
            (100, -50, 50),
            (-10, -20, -30),
        ];

        for (a, b, expected_result) in test_cases {
            store_arithmetic_transaction(&test_db.pool, a, b, expected_result)
                .await
                .expect("Failed to store valid transaction");

            let transactions = get_transactions_by_result(&test_db.pool, expected_result)
                .await
                .expect("Failed to retrieve transactions");

            // Find our specific transaction
            let found = transactions.iter().find(|t| t.a == a && t.b == b);
            assert!(
                found.is_some(),
                "Transaction not found: {a} + {b} = {expected_result}"
            );

            // Verify the arithmetic is correct
            assert_eq!(
                a + b,
                expected_result,
                "Arithmetic validation failed for {a} + {b}"
            );
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_nonexistent_result_query() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Store some transactions
        store_arithmetic_transaction(&test_db.pool, 1, 2, 3)
            .await
            .expect("Failed to store transaction");

        // Query for a result that doesn't exist
        let transactions = get_transactions_by_result(&test_db.pool, 999)
            .await
            .expect("Failed to query for nonexistent result");

        assert!(transactions.is_empty());

        // Test get_value_by_result for nonexistent result
        let value = get_value_by_result(&test_db.pool, 999)
            .await
            .expect("Failed to query for nonexistent value");

        assert!(value.is_none());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_large_number_handling() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Test with large numbers (within i32 range)
        let large_a = i32::MAX / 2;
        let large_b = 1000;
        let large_result = large_a + large_b;

        store_arithmetic_transaction(&test_db.pool, large_a, large_b, large_result)
            .await
            .expect("Failed to store large number transaction");

        let transactions = get_transactions_by_result(&test_db.pool, large_result)
            .await
            .expect("Failed to retrieve large number transaction");

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].a, large_a);
        assert_eq!(transactions[0].b, large_b);
        assert_eq!(transactions[0].result, large_result);
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
                store_arithmetic_transaction(&pool, i, i * 2, i * 3)
                    .await
                    .expect("Failed to store transaction in concurrent test");
            });
            tasks.push(task);
        }

        // Wait for all tasks to complete
        for task in tasks {
            task.await.expect("Task failed");
        }

        // Verify all transactions were stored
        for i in 0..10 {
            let result = i * 3;
            let transactions = get_transactions_by_result(&test_db.pool, result)
                .await
                .expect("Failed to retrieve transaction in concurrent test");

            assert_eq!(transactions.len(), 1);
            assert_eq!(transactions[0].a, i);
            assert_eq!(transactions[0].b, i * 2);
            assert_eq!(transactions[0].result, result);
        }
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

        // Insert 1000 transactions
        for i in 0..1000 {
            store_arithmetic_transaction(&test_db.pool, i, i + 1, i * 2 + 1)
                .await
                .expect("Failed to store bulk transaction");
        }

        let duration = start.elapsed();
        println!("Bulk insert of 1000 transactions took: {duration:?}");

        // Verify all transactions were stored
        for i in 0..100 {
            // Check first 100 to avoid too much verification overhead
            let result = i * 2 + 1;
            let transactions = get_transactions_by_result(&test_db.pool, result)
                .await
                .expect("Failed to retrieve bulk transaction");

            assert_eq!(transactions.len(), 1);
            assert_eq!(transactions[0].a, i);
            assert_eq!(transactions[0].b, i + 1);
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_query_performance_with_large_dataset() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Create a dataset with repeated results to test index efficiency
        for i in 0..500 {
            // Create transactions that all result in 1000
            let a = i;
            let b = 1000 - i;
            store_arithmetic_transaction(&test_db.pool, a, b, 1000)
                .await
                .expect("Failed to store performance test transaction");
        }

        let start = Instant::now();
        let transactions = get_transactions_by_result(&test_db.pool, 1000)
            .await
            .expect("Failed to retrieve performance test transactions");
        let duration = start.elapsed();

        println!("Query for result 1000 took: {duration:?}");
        assert_eq!(transactions.len(), 500);

        // Verify all transactions have the correct result
        for transaction in transactions {
            assert_eq!(transaction.result, 1000);
            assert_eq!(transaction.a + transaction.b, 1000);
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

        // Test edge cases for i32
        let test_cases = vec![
            (i32::MIN, 0, i32::MIN),
            (i32::MAX, 0, i32::MAX),
            (0, i32::MIN, i32::MIN),
            (0, i32::MAX, i32::MAX),
            (i32::MIN + 1, -1, i32::MIN),
            (i32::MAX - 1, 1, i32::MAX),
        ];

        for (a, b, expected_result) in test_cases {
            store_arithmetic_transaction(&test_db.pool, a, b, expected_result)
                .await
                .expect("Failed to store boundary value transaction");

            let transactions = get_transactions_by_result(&test_db.pool, expected_result)
                .await
                .expect("Failed to retrieve boundary value transaction");

            let found = transactions.iter().find(|t| t.a == a && t.b == b);
            assert!(
                found.is_some(),
                "Boundary value transaction not found: {a} + {b} = {expected_result}"
            );
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_zero_operations() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let zero_cases = vec![(0, 0, 0), (5, -5, 0), (-10, 10, 0), (100, -100, 0)];

        for (a, b, result) in zero_cases {
            store_arithmetic_transaction(&test_db.pool, a, b, result)
                .await
                .expect("Failed to store zero operation transaction");
        }

        let transactions = get_transactions_by_result(&test_db.pool, 0)
            .await
            .expect("Failed to retrieve zero result transactions");

        assert_eq!(transactions.len(), 4);

        for transaction in &transactions {
            assert_eq!(transaction.result, 0);
            assert_eq!(transaction.a + transaction.b, 0);
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_negative_number_operations() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let negative_cases = vec![
            (-5, -3, -8),
            (-10, 5, -5),
            (10, -15, -5),
            (-100, -200, -300),
        ];

        for (a, b, expected_result) in negative_cases {
            store_arithmetic_transaction(&test_db.pool, a, b, expected_result)
                .await
                .expect("Failed to store negative number transaction");

            let transactions = get_transactions_by_result(&test_db.pool, expected_result)
                .await
                .expect("Failed to retrieve negative number transaction");

            let found = transactions.iter().find(|t| t.a == a && t.b == b);
            assert!(
                found.is_some(),
                "Negative number transaction not found: {a} + {b} = {expected_result}"
            );

            // Verify arithmetic correctness
            assert_eq!(a + b, expected_result);
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_arithmetic_validation_with_incorrect_results() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Store transactions with intentionally incorrect results
        // (This tests that our database stores what we give it, even if mathematically incorrect)
        let incorrect_cases = vec![
            (5, 10, 16), // Should be 15
            (3, 7, 9),   // Should be 10
            (-5, 3, 0),  // Should be -2
        ];

        for (a, b, incorrect_result) in incorrect_cases {
            store_arithmetic_transaction(&test_db.pool, a, b, incorrect_result)
                .await
                .expect("Failed to store incorrect arithmetic transaction");

            let transactions = get_transactions_by_result(&test_db.pool, incorrect_result)
                .await
                .expect("Failed to retrieve incorrect arithmetic transaction");

            let found = transactions.iter().find(|t| t.a == a && t.b == b);
            assert!(
                found.is_some(),
                "Incorrect arithmetic transaction not found"
            );

            // Verify that what we stored is what we get back (even if mathematically wrong)
            let transaction = found.unwrap();
            assert_eq!(transaction.a, a);
            assert_eq!(transaction.b, b);
            assert_eq!(transaction.result, incorrect_result);

            // But the arithmetic should NOT be correct
            assert_ne!(
                a + b,
                incorrect_result,
                "Arithmetic should be incorrect for test case"
            );
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn test_stress_duplicate_handling() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let a = 42;
        let b = 58;
        let result = 100;

        // Attempt to store the same transaction 100 times
        for _ in 0..100 {
            store_arithmetic_transaction(&test_db.pool, a, b, result)
                .await
                .expect("Failed to store duplicate stress test transaction");
        }

        // Should still only have one transaction
        let transactions = get_transactions_by_result(&test_db.pool, result)
            .await
            .expect("Failed to retrieve duplicate stress test transaction");

        // Filter to our specific a,b combination (in case other tests added to result 100)
        let our_transactions: Vec<_> = transactions
            .iter()
            .filter(|t| t.a == a && t.b == b)
            .collect();

        assert_eq!(our_transactions.len(), 1);
        assert_eq!(our_transactions[0].a, a);
        assert_eq!(our_transactions[0].b, b);
        assert_eq!(our_transactions[0].result, result);
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

        // Use the actual arithmetic library to compute results
        let test_cases = vec![(5, 10), (25, 75), (-10, 30), (0, 0), (i32::MAX / 2, 1000)];

        for (a, b) in test_cases {
            // Use the actual arithmetic function from the lib
            let computed_result = addition(a, b);

            // Store the transaction with the computed result
            store_arithmetic_transaction(&test_db.pool, a, b, computed_result)
                .await
                .expect("Failed to store integration test transaction");

            // Verify retrieval
            let transactions = get_transactions_by_result(&test_db.pool, computed_result)
                .await
                .expect("Failed to retrieve integration test transaction");

            let found = transactions.iter().find(|t| t.a == a && t.b == b);
            assert!(
                found.is_some(),
                "Integration test transaction not found: {a} + {b} = {computed_result}"
            );

            // Verify the stored values match the library computation
            let transaction = found.unwrap();
            assert_eq!(transaction.result, computed_result);
            assert_eq!(addition(transaction.a, transaction.b), transaction.result);
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
        let result = addition(a, b);

        // Create a PublicValuesStruct like the zkVM would (only result is public)
        let public_values = PublicValuesStruct { result };

        // Store using computed values (a and b are private, not in PublicValuesStruct)
        store_arithmetic_transaction(&test_db.pool, a, b, public_values.result)
            .await
            .expect("Failed to store PublicValuesStruct transaction");

        // Retrieve and verify
        let transactions = get_transactions_by_result(&test_db.pool, public_values.result)
            .await
            .expect("Failed to retrieve PublicValuesStruct transaction");

        assert_eq!(transactions.len(), 1);
        let stored_transaction = &transactions[0];

        // Verify the result matches the PublicValuesStruct
        assert_eq!(stored_transaction.result, public_values.result);

        // Verify arithmetic correctness
        assert_eq!(
            stored_transaction.a + stored_transaction.b,
            stored_transaction.result
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn test_workflow_simulation() {
        let test_db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        // Simulate the full workflow: zkVM computes, script stores, later verification
        let zkvm_computations = vec![
            (7, 13),    // zkVM computes 7 + 13 = 20
            (100, 200), // zkVM computes 100 + 200 = 300
            (-50, 75),  // zkVM computes -50 + 75 = 25
        ];

        // Phase 1: zkVM execution and storage (simulated)
        for (a, b) in &zkvm_computations {
            let result = addition(*a, *b); // Simulate zkVM computation

            // Script stores the result
            store_arithmetic_transaction(&test_db.pool, *a, *b, result)
                .await
                .expect("Failed to store workflow simulation transaction");
        }

        // Phase 2: Later verification (simulated)
        for (a, b) in zkvm_computations {
            let expected_result = addition(a, b);

            // Verify the computation was stored correctly
            let value = get_value_by_result(&test_db.pool, expected_result)
                .await
                .expect("Failed to get value for workflow verification");

            assert!(
                value.is_some(),
                "Workflow verification failed: result {expected_result} not found"
            );
            let (stored_a, stored_b, _created_at) = value.unwrap();

            // Verify the stored values match what we expect
            assert_eq!(stored_a, a);
            assert_eq!(stored_b, b);

            // Verify arithmetic correctness
            assert_eq!(addition(stored_a, stored_b), expected_result);
        }
    }
}
