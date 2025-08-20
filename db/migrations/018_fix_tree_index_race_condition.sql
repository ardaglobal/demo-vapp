-- Fix tree index allocation race condition that causes duplicate key constraint violations
-- The issue: get_next_tree_index() and tree_state updates are not atomic within insert_nullifier_atomic()
-- This causes multiple nullifiers in a batch to get the same tree_index

-- ============================================================================
-- ATOMIC TREE INDEX ALLOCATION
-- ============================================================================

-- Replace get_next_tree_index with an atomic version that increments the counter
DROP FUNCTION IF EXISTS get_next_tree_index();

CREATE OR REPLACE FUNCTION get_and_increment_tree_index()
RETURNS BIGINT AS $$
DECLARE
    next_idx BIGINT;
BEGIN
    -- Atomically get current index and increment it in one operation
    UPDATE tree_state 
    SET next_available_index = next_available_index + 1,
        updated_at = NOW()
    WHERE tree_id = 'default'
    RETURNING next_available_index - 1 INTO next_idx;
    
    -- If no row was updated (shouldn't happen), initialize and try again
    IF next_idx IS NULL THEN
        INSERT INTO tree_state (tree_id, root_hash, next_available_index, tree_height, total_nullifiers)
        VALUES ('default', '\x0000000000000000000000000000000000000000000000000000000000000000', 1, 32, 0)
        ON CONFLICT (tree_id) DO UPDATE SET 
            next_available_index = tree_state.next_available_index + 1,
            updated_at = NOW()
        RETURNING next_available_index - 1 INTO next_idx;
    END IF;
    
    RETURN COALESCE(next_idx, 0);
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- UPDATE insert_nullifier_atomic TO USE ATOMIC INDEX ALLOCATION
-- ============================================================================

DROP FUNCTION IF EXISTS insert_nullifier_atomic(BIGINT);

CREATE FUNCTION insert_nullifier_atomic(new_value BIGINT)
RETURNS TABLE(
    inserted_tree_index BIGINT,
    low_nullifier_value BIGINT,
    low_nullifier_next_value BIGINT,
    success BOOLEAN
) AS $$
DECLARE
    low_null RECORD;
    actual_tree_index BIGINT;
    tree_is_empty BOOLEAN DEFAULT FALSE;
BEGIN
    -- Step 1: Check if tree is completely empty (first insertion case)
    SELECT COUNT(*) = 0 INTO tree_is_empty 
    FROM nullifiers WHERE is_active = true;
    
    -- Step 2: Handle empty tree case vs normal case
    IF tree_is_empty THEN
        -- EMPTY TREE: First nullifier insertion (genesis case)
        
        -- Validate insertion is possible
        IF EXISTS (SELECT 1 FROM nullifiers WHERE value = new_value AND is_active = true) THEN
            RETURN QUERY SELECT NULL::BIGINT, NULL::BIGINT, NULL::BIGINT, FALSE;
            RETURN;
        END IF;
        
        -- Get tree index atomically (this also increments the counter)
        actual_tree_index := get_and_increment_tree_index();
        
        BEGIN
            -- Insert first nullifier with next_value = 0 (indicating it's the maximum)
            INSERT INTO nullifiers (value, next_index, next_value, tree_index)
            VALUES (new_value, NULL, 0, actual_tree_index);
            
            -- Update total_nullifiers count (next_available_index already updated above)
            UPDATE tree_state
            SET 
                total_nullifiers = total_nullifiers + 1,
                updated_at = NOW()
            WHERE tree_id = 'default';
            
            -- Return success for empty tree insertion
            RETURN QUERY SELECT 
                actual_tree_index,
                new_value,  -- The inserted value becomes the "low nullifier" for next insertion
                0::BIGINT,  -- next_value is 0 (maximum)
                TRUE;
                
        EXCEPTION WHEN OTHERS THEN
            RETURN QUERY SELECT actual_tree_index, NULL::BIGINT, NULL::BIGINT, FALSE;
        END;
        
    ELSE
        -- NON-EMPTY TREE: Normal 7-step indexed Merkle tree insertion
        
        -- Step 1: Find low nullifier
        SELECT * INTO low_null 
        FROM find_low_nullifier(new_value) 
        LIMIT 1;
        
        -- Validate we found a low nullifier (this should always succeed for non-empty tree)
        IF low_null.low_value IS NULL THEN
            -- This shouldn't happen in a non-empty tree, but handle gracefully
            RETURN QUERY SELECT NULL::BIGINT, NULL::BIGINT, NULL::BIGINT, FALSE;
            RETURN;
        END IF;
        
        -- Step 2: Validate insertion is possible
        IF EXISTS (SELECT 1 FROM nullifiers WHERE value = new_value AND is_active = true) THEN
            -- Nullifier already exists
            RETURN QUERY SELECT NULL::BIGINT, NULL::BIGINT, NULL::BIGINT, FALSE;
            RETURN;
        END IF;
        
        -- Step 3: Get tree index atomically (this also increments the counter)
        actual_tree_index := get_and_increment_tree_index();
        
        BEGIN
            -- Step 4: Insert new nullifier
            INSERT INTO nullifiers (value, next_index, next_value, tree_index)
            VALUES (
                new_value,
                low_null.low_next_index,
                low_null.low_next_value,
                actual_tree_index
            );
            
            -- Step 5: Update the low nullifier to point to new nullifier
            UPDATE nullifiers
            SET 
                next_index = actual_tree_index,
                next_value = new_value
            WHERE value = low_null.low_value AND is_active = true;
            
            -- Step 6: Update nullifier count (next_available_index already updated above)
            UPDATE tree_state
            SET 
                total_nullifiers = total_nullifiers + 1,
                updated_at = NOW()
            WHERE tree_id = 'default';
            
            -- Step 7: Return success result
            RETURN QUERY SELECT 
                actual_tree_index,
                low_null.low_value,
                low_null.low_next_value,
                TRUE;
                
        EXCEPTION WHEN OTHERS THEN
            -- Rollback on any error
            RETURN QUERY SELECT actual_tree_index, NULL::BIGINT, NULL::BIGINT, FALSE;
        END;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- APPLY CONSISTENCY FIX
-- ============================================================================

-- Fix current tree_state to be consistent
SELECT fix_tree_state_consistency();

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON FUNCTION get_and_increment_tree_index() IS 
'ATOMIC tree index allocation that prevents race conditions during batch processing.
Atomically reads current next_available_index and increments it in a single operation.';

COMMENT ON FUNCTION insert_nullifier_atomic(BIGINT) IS 
'RACE-CONDITION-FREE nullifier insertion using atomic tree index allocation.
Prevents duplicate key constraint violations during concurrent batch processing.';
