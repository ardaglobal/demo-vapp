-- Fix race condition in create_batch function
-- 
-- The original create_batch function had a race condition where two concurrent
-- callers could select the same unbatched rows, leading to mismatched data.
-- This migration replaces the function with an atomic implementation using
-- UPDATE...RETURNING with row-level locking.

-- Drop and recreate the create_batch function with atomic row claiming
DROP FUNCTION IF EXISTS create_batch(INTEGER);

CREATE OR REPLACE FUNCTION create_batch(batch_size INTEGER DEFAULT 10)
RETURNS INTEGER AS $$
DECLARE
    new_batch_id INTEGER;
    previous_counter BIGINT;
    final_counter BIGINT;
    transaction_total INTEGER;
    transaction_id_array INTEGER[];
    claimed_count INTEGER;
BEGIN
    -- Get current counter value
    SELECT get_current_counter_value() INTO previous_counter;
    
    -- Atomically select and lock unbatched transactions
    -- Use FOR UPDATE SKIP LOCKED to prevent race conditions
    SELECT 
        ARRAY_AGG(id ORDER BY id),
        SUM(amount),
        COUNT(*)
    INTO transaction_id_array, transaction_total, claimed_count
    FROM (
        SELECT id, amount 
        FROM incoming_transactions 
        WHERE included_in_batch_id IS NULL 
        ORDER BY id ASC 
        LIMIT batch_size
        FOR UPDATE SKIP LOCKED  -- Skip rows locked by other transactions
    ) locked_transactions;
    
    -- Return 0 if no transactions were claimed
    IF transaction_id_array IS NULL OR claimed_count = 0 THEN
        RETURN 0;
    END IF;
    
    -- Calculate final counter value
    final_counter := previous_counter + transaction_total;
    
    -- Create new batch
    INSERT INTO proof_batches (
        previous_counter_value,
        final_counter_value, 
        transaction_ids
    ) VALUES (
        previous_counter,
        final_counter,
        transaction_id_array
    ) RETURNING id INTO new_batch_id;
    
    -- Update claimed transactions with the actual batch ID
    -- These rows are still locked from the SELECT FOR UPDATE above
    UPDATE incoming_transactions 
    SET included_in_batch_id = new_batch_id
    WHERE id = ANY(transaction_id_array);
    
    RETURN new_batch_id;
END;
$$ LANGUAGE plpgsql;
