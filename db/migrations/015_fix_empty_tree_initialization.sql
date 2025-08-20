-- Fix empty tree initialization for indexed Merkle tree
-- This addresses the "Resource not found: low nullifier" error that occurs
-- when trying to insert the first nullifier into a completely empty tree

-- ============================================================================
-- ENHANCED EMPTY TREE HANDLING
-- ============================================================================

-- Drop and recreate the insert_nullifier_atomic function with proper empty tree handling
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
        -- No low nullifier exists, so we insert as the first element
        
        -- Validate insertion is possible
        IF EXISTS (SELECT 1 FROM nullifiers WHERE value = new_value AND is_active = true) THEN
            RETURN QUERY SELECT NULL::BIGINT, NULL::BIGINT, NULL::BIGINT, FALSE;
            RETURN;
        END IF;
        
        -- Get tree index
        actual_tree_index := get_next_tree_index();
        
        BEGIN
            -- Insert first nullifier with next_value = 0 (indicating it's the maximum)
            INSERT INTO nullifiers (value, next_index, next_value, tree_index)
            VALUES (new_value, NULL, 0, actual_tree_index);
            
            -- Update tree state
            UPDATE tree_state
            SET 
                next_available_index = GREATEST(next_available_index, actual_tree_index + 1),
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
        
        -- Step 3: Get tree index
        actual_tree_index := get_next_tree_index();
        
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
            
            -- Step 6: Update tree state
            UPDATE tree_state
            SET 
                next_available_index = GREATEST(next_available_index, actual_tree_index + 1),
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
-- ENSURE TREE STATE IS PROPERLY INITIALIZED
-- ============================================================================

-- Make sure the default tree state exists
INSERT INTO tree_state (tree_id, root_hash, next_available_index, tree_height, total_nullifiers)
VALUES ('default', '\x0000000000000000000000000000000000000000000000000000000000000000', 0, 32, 0)
ON CONFLICT (tree_id) DO NOTHING;

-- ============================================================================
-- ENHANCED DIAGNOSTICS FUNCTION
-- ============================================================================

-- Function to check tree initialization status
CREATE OR REPLACE FUNCTION check_tree_initialization()
RETURNS TABLE(
    tree_initialized BOOLEAN,
    nullifier_count BIGINT,
    next_available_index BIGINT,
    tree_state_exists BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        CASE 
            WHEN EXISTS(SELECT 1 FROM tree_state WHERE tree_id = 'default') THEN TRUE
            ELSE FALSE
        END as tree_initialized,
        COALESCE((SELECT COUNT(*) FROM nullifiers WHERE is_active = true), 0) as nullifier_count,
        COALESCE((SELECT next_available_index FROM tree_state WHERE tree_id = 'default'), -1) as next_available_index,
        EXISTS(SELECT 1 FROM tree_state WHERE tree_id = 'default') as tree_state_exists;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION insert_nullifier_atomic(BIGINT) IS 'Enhanced nullifier insertion with proper empty tree (genesis) handling';
COMMENT ON FUNCTION check_tree_initialization() IS 'Diagnostic function to check tree initialization status';