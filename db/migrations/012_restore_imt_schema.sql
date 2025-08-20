-- Restore IMT (Indexed Merkle Tree) schema for ADS integration
-- This migration recreates the IMT tables and functions that were dropped in 009
-- but are needed for the ADS (Authenticated Data Structure) system

-- ============================================================================
-- NULLIFIERS TABLE: Core indexed Merkle tree data structure
-- ============================================================================
CREATE TABLE IF NOT EXISTS nullifiers (
    id BIGSERIAL PRIMARY KEY,
    value BIGINT NOT NULL UNIQUE,
    next_index BIGINT, -- Points to index of next higher nullifier
    next_value BIGINT, -- Value of next higher nullifier (0 = max)
    tree_index BIGINT NOT NULL UNIQUE, -- Position in Merkle tree (0-2^32)
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    is_active BOOLEAN DEFAULT true,
    
    -- Constraints
    CONSTRAINT nullifiers_value_positive CHECK (value >= 0),
    CONSTRAINT nullifiers_next_value_valid CHECK (
        next_value = 0 OR next_value > value
    ),
    CONSTRAINT nullifiers_tree_index_valid CHECK (
        tree_index >= 0 AND tree_index < 4294967296 -- 2^32
    )
);

-- ============================================================================
-- MERKLE NODES TABLE: Store tree structure separately from nullifiers
-- ============================================================================
CREATE TABLE IF NOT EXISTS merkle_nodes (
    tree_level INTEGER NOT NULL CHECK (tree_level >= 0 AND tree_level <= 32),
    node_index BIGINT NOT NULL CHECK (node_index >= 0),
    hash_value BYTEA NOT NULL CHECK (length(hash_value) = 32), -- 32 bytes
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    PRIMARY KEY (tree_level, node_index)
);

-- ============================================================================
-- TREE STATE TABLE: Track tree metadata and root
-- ============================================================================
CREATE TABLE IF NOT EXISTS tree_state (
    tree_id VARCHAR(50) PRIMARY KEY DEFAULT 'default',
    root_hash BYTEA NOT NULL CHECK (length(root_hash) = 32),
    next_available_index BIGINT DEFAULT 0 CHECK (next_available_index >= 0),
    tree_height INTEGER DEFAULT 32 CHECK (tree_height > 0 AND tree_height <= 32),
    total_nullifiers BIGINT DEFAULT 0 CHECK (total_nullifiers >= 0),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- INDEXES: Optimized for O(log n) lookups and range queries
-- ============================================================================

-- Primary lookup indexes
CREATE INDEX IF NOT EXISTS idx_nullifiers_value ON nullifiers(value) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_nullifiers_next_value ON nullifiers(next_value) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_nullifiers_tree_index ON nullifiers(tree_index) WHERE is_active = true;

-- Composite index for efficient low nullifier searches
CREATE INDEX IF NOT EXISTS idx_nullifiers_value_next_value ON nullifiers(value, next_value) 
    WHERE is_active = true;

-- Range query optimization
CREATE INDEX IF NOT EXISTS idx_nullifiers_value_range ON nullifiers(value, next_value, tree_index) 
    WHERE is_active = true;

-- Merkle tree node access optimization
CREATE INDEX IF NOT EXISTS idx_merkle_nodes_level_index ON merkle_nodes(tree_level, node_index);
CREATE INDEX IF NOT EXISTS idx_merkle_nodes_updated ON merkle_nodes(updated_at);

-- ============================================================================
-- FUNCTIONS: Core indexed Merkle tree operations
-- ============================================================================

-- Function to find the appropriate low_nullifier for insertion
CREATE OR REPLACE FUNCTION find_low_nullifier(new_value BIGINT)
RETURNS TABLE(
    low_value BIGINT,
    low_next_index BIGINT,
    low_next_value BIGINT,
    low_tree_index BIGINT
) AS $$
BEGIN
    -- Find nullifier where: low_nullifier.next_value > new_value
    -- AND low_nullifier.value < new_value (largest smaller value)
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
END;
$$ LANGUAGE plpgsql;

-- Function to get next available tree index
CREATE OR REPLACE FUNCTION get_next_tree_index()
RETURNS BIGINT AS $$
DECLARE
    next_index BIGINT;
BEGIN
    SELECT next_available_index INTO next_index
    FROM tree_state
    WHERE tree_id = 'default';
    
    RETURN COALESCE(next_index, 0);
END;
$$ LANGUAGE plpgsql;

-- Function to validate nullifier chain integrity
CREATE OR REPLACE FUNCTION validate_nullifier_chain()
RETURNS BOOLEAN AS $$
DECLARE
    invalid_count INTEGER;
BEGIN
    -- Check for any chain inconsistencies
    SELECT COUNT(*) INTO invalid_count
    FROM nullifiers n1
    LEFT JOIN nullifiers n2 ON n1.next_value = n2.value AND n2.is_active = true
    WHERE n1.is_active = true
      AND n1.next_value != 0  -- 0 means this is the max value
      AND n2.value IS NULL;   -- Referenced nullifier doesn't exist
    
    RETURN invalid_count = 0;
END;
$$ LANGUAGE plpgsql;

-- Atomic nullifier insertion function (7-step algorithm)
CREATE OR REPLACE FUNCTION insert_nullifier_atomic(new_value BIGINT)
RETURNS TABLE(
    inserted_tree_index BIGINT,
    low_nullifier_value BIGINT,
    low_nullifier_next_value BIGINT,
    success BOOLEAN
) AS $$
DECLARE
    low_null RECORD;
    actual_tree_index BIGINT;
BEGIN
    -- Step 1: Find low nullifier
    SELECT * INTO low_null 
    FROM find_low_nullifier(new_value) 
    LIMIT 1;
    
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
            COALESCE(low_null.low_next_index, NULL),
            COALESCE(low_null.low_next_value, 0),
            actual_tree_index
        );
        
        -- Step 5: Update the low nullifier to point to new nullifier
        IF low_null.low_value IS NOT NULL THEN
            UPDATE nullifiers
            SET 
                next_index = actual_tree_index,
                next_value = new_value
            WHERE value = low_null.low_value AND is_active = true;
        END IF;
        
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
            COALESCE(low_null.low_value, new_value),
            COALESCE(low_null.low_next_value, 0),
            TRUE;
            
    EXCEPTION WHEN OTHERS THEN
        -- Rollback on any error
        RETURN QUERY SELECT actual_tree_index, NULL::BIGINT, NULL::BIGINT, FALSE;
    END;
END;
$$ LANGUAGE plpgsql;

-- Function to get tree statistics
CREATE OR REPLACE FUNCTION get_tree_stats()
RETURNS TABLE(
    total_nullifiers BIGINT,
    tree_height INTEGER,
    next_index BIGINT,
    chain_valid BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        ts.total_nullifiers,
        ts.tree_height,
        ts.next_available_index,
        validate_nullifier_chain()
    FROM tree_state ts
    WHERE tree_id = 'default';
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- INITIALIZE DEFAULT TREE STATE
-- ============================================================================

-- Initialize default tree state with zero root hash
INSERT INTO tree_state (tree_id, root_hash, next_available_index, tree_height, total_nullifiers)
VALUES ('default', '\x0000000000000000000000000000000000000000000000000000000000000000', 0, 32, 0)
ON CONFLICT (tree_id) DO NOTHING;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE nullifiers IS 'Indexed Merkle tree nullifiers with linked-list structure for O(log n) operations';
COMMENT ON TABLE merkle_nodes IS 'Merkle tree nodes storing hash values at each level for cryptographic proofs';  
COMMENT ON TABLE tree_state IS 'Global tree state tracking root hash, total nullifiers, and next available index';

COMMENT ON COLUMN nullifiers.value IS 'The nullifier value (must be unique and positive)';
COMMENT ON COLUMN nullifiers.next_index IS 'Tree index of the next higher nullifier (NULL if this points to max)';
COMMENT ON COLUMN nullifiers.next_value IS 'Value of the next higher nullifier (0 if this is the maximum)';
COMMENT ON COLUMN nullifiers.tree_index IS 'Position in the 32-level Merkle tree (0 to 2^32-1)';

COMMENT ON COLUMN merkle_nodes.tree_level IS 'Level in the Merkle tree (0=leaf, 32=root)';
COMMENT ON COLUMN merkle_nodes.node_index IS 'Index at this level (0 to 2^(32-level)-1)';
COMMENT ON COLUMN merkle_nodes.hash_value IS '32-byte hash value for this Merkle tree node';