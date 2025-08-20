-- Initialize IMT/ADS with genesis state instead of empty tree
-- This eliminates all empty tree edge cases by starting with a proper initialized state
-- 
-- Genesis approach:
-- 1. Insert genesis nullifier (value 0) at tree_index 0 
-- 2. Set tree_state to reflect this initialized state
-- 3. All subsequent insertions use normal 7-step algorithm (no empty tree handling needed)

-- ============================================================================
-- GENESIS NULLIFIER INITIALIZATION
-- ============================================================================

-- Clear any existing data to ensure clean genesis state
DELETE FROM nullifiers WHERE is_active = true;
DELETE FROM merkle_nodes;

-- Insert the genesis nullifier representing the initial balance of 0
INSERT INTO nullifiers (value, next_index, next_value, tree_index, is_active, created_at)
VALUES (0, NULL, 0, 0, true, NOW())
ON CONFLICT (value) DO NOTHING;

-- Initialize tree state to reflect genesis nullifier
UPDATE tree_state 
SET 
    total_nullifiers = 1,
    next_available_index = 1,
    root_hash = '\x0000000000000000000000000000000000000000000000000000000000000000',
    updated_at = NOW()
WHERE tree_id = 'default';

-- Ensure tree_state row exists if not present
INSERT INTO tree_state (tree_id, root_hash, next_available_index, tree_height, total_nullifiers, updated_at)
VALUES ('default', '\x0000000000000000000000000000000000000000000000000000000000000000', 1, 32, 1, NOW())
ON CONFLICT (tree_id) DO UPDATE SET
    total_nullifiers = 1,
    next_available_index = 1,
    updated_at = NOW();

-- Initialize the genesis leaf in the Merkle tree
-- Genesis nullifier (value=0) gets hashed to create the leaf at index 0
INSERT INTO merkle_nodes (tree_level, node_index, hash_value, updated_at)
VALUES (0, 0, '\x0000000000000000000000000000000000000000000000000000000000000000', NOW())
ON CONFLICT (tree_level, node_index) DO UPDATE SET
    hash_value = '\x0000000000000000000000000000000000000000000000000000000000000000',
    updated_at = NOW();

-- ============================================================================
-- VERIFICATION FUNCTIONS
-- ============================================================================

-- Function to verify genesis state is correct
CREATE OR REPLACE FUNCTION verify_genesis_state()
RETURNS TABLE(
    genesis_nullifier_exists BOOLEAN,
    tree_state_consistent BOOLEAN,
    next_index_correct BOOLEAN,
    total_count_correct BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        EXISTS(SELECT 1 FROM nullifiers WHERE value = 0 AND tree_index = 0 AND is_active = true) as genesis_nullifier_exists,
        (SELECT total_nullifiers FROM tree_state WHERE tree_id = 'default') = 1 as tree_state_consistent,
        (SELECT next_available_index FROM tree_state WHERE tree_id = 'default') = 1 as next_index_correct,
        (SELECT COUNT(*) FROM nullifiers WHERE is_active = true) = 1 as total_count_correct;
END;
$$ LANGUAGE plpgsql;

-- Apply verification to confirm genesis state
SELECT * FROM verify_genesis_state();

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON FUNCTION verify_genesis_state() IS 
'Verifies that the genesis state has been properly initialized:
- Genesis nullifier (value=0) exists at tree_index=0
- tree_state reflects exactly 1 nullifier
- next_available_index is 1 (ready for first real transaction)
This eliminates empty tree edge cases in find_low_nullifier and insertion logic.';
