-- Test script to verify that create_batch function handles concurrency correctly
-- This script simulates the race condition scenario and verifies the fix

-- Clean up any existing test data
DELETE FROM incoming_transactions WHERE amount >= 9000;
DELETE FROM proof_batches WHERE previous_counter_value >= 9000;

-- Insert test transactions
INSERT INTO incoming_transactions (amount) VALUES 
    (9001), (9002), (9003), (9004), (9005),
    (9006), (9007), (9008), (9009), (9010),
    (9011), (9012), (9013), (9014), (9015);

-- Show initial state
SELECT 'Initial unbatched transactions:' AS status;
SELECT id, amount, included_in_batch_id FROM incoming_transactions WHERE amount >= 9000 ORDER BY id;

-- Simulate concurrent batch creation (in practice this would be from multiple connections)
-- Create two batches of size 5 each
SELECT 'Creating first batch...' AS status;
SELECT create_batch(5) AS batch_id_1;

SELECT 'Creating second batch...' AS status;  
SELECT create_batch(5) AS batch_id_2;

SELECT 'Creating third batch (should return non-zero - third batch created)...' AS status;
SELECT create_batch(5) AS batch_id_3;

-- Verify results
SELECT 'Final state - all transactions should be properly assigned:' AS status;
SELECT id, amount, included_in_batch_id FROM incoming_transactions WHERE amount >= 9000 ORDER BY id;

SELECT 'Batches created:' AS status;
SELECT id, transaction_ids, array_length(transaction_ids, 1) as tx_count 
FROM proof_batches 
WHERE id IN (
    SELECT id FROM proof_batches 
    WHERE transaction_ids && (
        SELECT array_agg(id) FROM incoming_transactions WHERE amount >= 9000
    )
)
ORDER BY id;

-- Verify no double-assignment (each transaction should appear in exactly one batch)
SELECT 'Verification - checking for double assignments:' AS status;
WITH batch_transactions AS (
    SELECT id as batch_id, unnest(transaction_ids) as transaction_id
    FROM proof_batches
    WHERE transaction_ids && (
        SELECT array_agg(id) FROM incoming_transactions WHERE amount >= 9000
    )
),
transaction_counts AS (
    SELECT transaction_id, COUNT(*) as assignment_count
    FROM batch_transactions
    GROUP BY transaction_id
)
SELECT 
    CASE 
        WHEN COUNT(*) = 0 THEN 'SUCCESS: No double assignments detected'
        ELSE 'ERROR: ' || COUNT(*) || ' transactions assigned to multiple batches'
    END as result
FROM transaction_counts
WHERE assignment_count > 1;

-- Verify all expected transactions are assigned exactly once
SELECT 'Verification - checking all transactions assigned exactly once:' AS status;
WITH expected_transactions AS (
    SELECT id as transaction_id FROM incoming_transactions WHERE amount >= 9000
),
batch_transactions AS (
    SELECT unnest(transaction_ids) as transaction_id
    FROM proof_batches
    WHERE transaction_ids && (
        SELECT array_agg(id) FROM incoming_transactions WHERE amount >= 9000
    )
),
assignment_summary AS (
    SELECT 
        e.transaction_id,
        CASE WHEN b.transaction_id IS NOT NULL THEN 1 ELSE 0 END as is_assigned
    FROM expected_transactions e
    LEFT JOIN batch_transactions b ON e.transaction_id = b.transaction_id
)
SELECT 
    CASE 
        WHEN SUM(is_assigned) = 15 AND COUNT(*) = 15 THEN 
            'SUCCESS: All 15 transactions assigned exactly once'
        WHEN SUM(is_assigned) < 15 THEN 
            'ERROR: Only ' || SUM(is_assigned) || ' of 15 expected transactions assigned'
        ELSE 
            'ERROR: Unexpected assignment count - ' || SUM(is_assigned) || ' assigned, ' || COUNT(*) || ' expected'
    END as result
FROM assignment_summary;
