-- PERMANENT FIX: Correct find_low_nullifier function to handle all edge cases
-- This fixes the "Resource not found: low nullifier" error that occurs on fresh database setups
-- 
-- The issue: Previous versions of find_low_nullifier didn't handle cases where:
-- 1. Tree is completely empty (first insertion)
-- 2. New value is smaller than all existing nullifiers
-- 
-- This migration ensures the function works correctly from fresh database initialization.

-- ============================================================================
-- DEFINITIVE find_low_nullifier FUNCTION
-- ============================================================================

DROP FUNCTION IF EXISTS find_low_nullifier(BIGINT);

CREATE OR REPLACE FUNCTION find_low_nullifier(new_value BIGINT)
RETURNS TABLE(
    low_value BIGINT,
    low_next_index BIGINT,
    low_next_value BIGINT,
    low_tree_index BIGINT
) AS $$
BEGIN
    -- CASE 1: Empty tree - return virtual low nullifier
    IF NOT EXISTS (SELECT 1 FROM nullifiers WHERE is_active = true) THEN
        RETURN QUERY SELECT 0::BIGINT, NULL::BIGINT, 0::BIGINT, 0::BIGINT;
        RETURN;
    END IF;
    
    -- CASE 2: Normal case - find actual low nullifier
    -- Find nullifier where: low_nullifier.value < new_value
    -- AND (low_nullifier.next_value > new_value OR low_nullifier.next_value = 0)
    RETURN QUERY
    SELECT 
        n.value,
        n.next_index,
        n.next_value,
        n.tree_index
    FROM nullifiers n
    WHERE n.is_active = true
      AND n.value < new_value
      AND (n.next_value > new_value OR n.next_value = 0)
    ORDER BY n.value DESC
    LIMIT 1;
    
    -- CASE 3: If no low nullifier found, new_value is smaller than all existing
    IF NOT FOUND THEN
        -- Check if new_value is smaller than the smallest existing nullifier
        IF EXISTS (SELECT 1 FROM nullifiers WHERE is_active = true AND value > new_value) THEN
            -- Return virtual minimum nullifier pointing to the actual minimum
            RETURN QUERY 
            SELECT 
                0::BIGINT as low_value, 
                NULL::BIGINT as low_next_index, 
                MIN(value) as low_next_value, 
                0::BIGINT as low_tree_index
            FROM nullifiers 
            WHERE is_active = true;
        END IF;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- ENHANCED tree_state CONSISTENCY FUNCTION
-- ============================================================================

-- Function to fix tree_state inconsistencies that can occur during initialization
CREATE OR REPLACE FUNCTION fix_tree_state_consistency()
RETURNS VOID AS $$
BEGIN
    -- Ensure tree_state reflects actual nullifier data
    UPDATE tree_state
    SET 
        total_nullifiers = (SELECT COUNT(*) FROM nullifiers WHERE is_active = true),
        next_available_index = (SELECT COALESCE(MAX(tree_index), -1) + 1 FROM nullifiers WHERE is_active = true),
        updated_at = NOW()
    WHERE tree_id = 'default';
    
    -- Ensure the row exists
    INSERT INTO tree_state (tree_id, root_hash, next_available_index, tree_height, total_nullifiers)
    VALUES ('default', '\x0000000000000000000000000000000000000000000000000000000000000000', 0, 32, 0)
    ON CONFLICT (tree_id) DO NOTHING;
    
    -- Fix it again after ensuring row exists
    UPDATE tree_state
    SET 
        total_nullifiers = (SELECT COUNT(*) FROM nullifiers WHERE is_active = true),
        next_available_index = (SELECT COALESCE(MAX(tree_index), -1) + 1 FROM nullifiers WHERE is_active = true),
        updated_at = NOW()
    WHERE tree_id = 'default';
END;
$$ LANGUAGE plpgsql;

-- Apply the consistency fix immediately
SELECT fix_tree_state_consistency();

-- ============================================================================
-- COMMENTS FOR MAINTAINERS
-- ============================================================================

COMMENT ON FUNCTION find_low_nullifier(BIGINT) IS 
'DEFINITIVE low nullifier finder that handles all edge cases:
1. Empty tree -> virtual low nullifier (0, NULL, 0, 0)
2. Normal case -> proper low nullifier where low_value < new_value < next_value
3. New value smaller than all existing -> virtual minimum (0, NULL, min_existing_value, 0)
This prevents "Resource not found: low nullifier" errors on fresh setups.';

COMMENT ON FUNCTION fix_tree_state_consistency() IS 
'Ensures tree_state table is consistent with actual nullifier data. 
Call this after any manual nullifier operations or during initialization.';
