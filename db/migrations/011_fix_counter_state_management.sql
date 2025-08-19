-- Fix counter state management to use latest batch instead of only proven batches
-- 
-- The current get_current_counter_value() function only considers 'proven' batches,
-- but since proof generation is asynchronous, this means all new batches start from 0.
-- 
-- This migration updates the function to use the latest batch's final_counter_value
-- regardless of proof status, which maintains proper state continuity.

-- Drop and recreate the get_current_counter_value function
DROP FUNCTION IF EXISTS get_current_counter_value();

CREATE OR REPLACE FUNCTION get_current_counter_value()
RETURNS BIGINT AS $$
DECLARE
    current_value BIGINT := 0;
BEGIN
    -- Get the final counter value from the most recent batch (any status)
    -- This ensures proper state continuity even when proofs are still pending
    SELECT pb.final_counter_value
    INTO current_value
    FROM proof_batches pb
    ORDER BY pb.id DESC
    LIMIT 1;
    
    -- Return 0 if no batches exist yet (initial state)
    RETURN COALESCE(current_value, 0);
END;
$$ LANGUAGE plpgsql;

-- Add a comment explaining the change
COMMENT ON FUNCTION get_current_counter_value() IS 'Returns the final counter value from the most recent batch, regardless of proof status. This ensures state continuity during asynchronous proof generation.';
