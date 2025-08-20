-- Restore compatibility for Rust code that still calls get_next_tree_index()
-- The 018 migration renamed it to get_and_increment_tree_index() but we need both functions

-- ============================================================================
-- COMPATIBILITY WRAPPER
-- ============================================================================

-- Create a wrapper function with the old name that calls the new atomic version
CREATE OR REPLACE FUNCTION get_next_tree_index()
RETURNS BIGINT AS $$
BEGIN
    -- Call the atomic version for consistency
    RETURN get_and_increment_tree_index();
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- ENSURE CONSISTENCY
-- ============================================================================

-- Apply consistency fix to ensure everything is in sync
SELECT fix_tree_state_consistency();

COMMENT ON FUNCTION get_next_tree_index() IS 
'Compatibility wrapper for old function name. Calls get_and_increment_tree_index() for atomic allocation.';
