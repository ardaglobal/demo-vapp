-- Restore IMT (Indexed Merkle Tree) schema for ADS integration with Genesis Initialization
-- This migration replaces the original migration 012 and consolidates fixes from migrations 013-020
-- with a single, clean implementation that includes the genesis nullifier approach to eliminate empty tree edge cases.
--
-- Key Features:
-- 1. Complete indexed Merkle tree (IMT) schema
-- 2. Atomic tree index allocation to prevent race conditions  
-- 3. Genesis nullifier initialization (eliminates empty tree cases)
-- 4. Audit trail support
-- 5. Smart contract posting tracking

-- ============================================================================
-- CORE TABLES
-- ============================================================================

-- Nullifiers table: Core indexed Merkle tree data structure
CREATE TABLE IF NOT EXISTS nullifiers (
    id BIGSERIAL PRIMARY KEY,
    value BIGINT NOT NULL UNIQUE,
    next_index BIGINT, -- Points to tree_index of next higher nullifier
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

-- Merkle nodes table: Store tree structure separately from nullifiers
CREATE TABLE IF NOT EXISTS merkle_nodes (
    tree_level INTEGER NOT NULL CHECK (tree_level >= 0 AND tree_level <= 32),
    node_index BIGINT NOT NULL CHECK (node_index >= 0),
    hash_value BYTEA NOT NULL CHECK (length(hash_value) = 32), -- 32 bytes
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    PRIMARY KEY (tree_level, node_index)
);

-- Tree state table: Track tree metadata and root
CREATE TABLE IF NOT EXISTS tree_state (
    tree_id VARCHAR(50) PRIMARY KEY DEFAULT 'default',
    root_hash BYTEA NOT NULL CHECK (length(root_hash) = 32),
    next_available_index BIGINT DEFAULT 0 CHECK (next_available_index >= 0),
    tree_height INTEGER DEFAULT 32 CHECK (tree_height > 0 AND tree_height <= 32),
    total_nullifiers BIGINT DEFAULT 0 CHECK (total_nullifiers >= 0),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Audit events table: For ADS audit trails
CREATE TABLE IF NOT EXISTS audit_events (
    event_id VARCHAR PRIMARY KEY,
    nullifier_value BIGINT NOT NULL,
    event_type VARCHAR(50) NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    root_before BYTEA NOT NULL CHECK (length(root_before) = 32),
    root_after BYTEA NOT NULL CHECK (length(root_after) = 32), 
    transaction_hash VARCHAR(66),
    block_height BIGINT DEFAULT 0,
    operator VARCHAR(42),
    metadata JSONB DEFAULT '{}'
);

-- Add posted_to_contract tracking to proof_batches (if table exists)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'proof_batches') THEN
        -- Add posted_to_contract column if it doesn't exist
        IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                      WHERE table_name = 'proof_batches' AND column_name = 'posted_to_contract') THEN
            ALTER TABLE proof_batches 
            ADD COLUMN posted_to_contract BOOLEAN NOT NULL DEFAULT FALSE;
        END IF;
        
        -- Add posted_to_contract_at column if it doesn't exist
        IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                      WHERE table_name = 'proof_batches' AND column_name = 'posted_to_contract_at') THEN
            ALTER TABLE proof_batches
            ADD COLUMN posted_to_contract_at TIMESTAMP WITH TIME ZONE;
        END IF;
    END IF;
END $$;

-- ============================================================================
-- INDEXES: Optimized for O(log n) lookups
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

-- Audit events indexes
CREATE INDEX IF NOT EXISTS idx_audit_nullifier ON audit_events (nullifier_value);
CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_events (timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_type ON audit_events (event_type);
CREATE INDEX IF NOT EXISTS idx_audit_block ON audit_events (block_height);

-- Proof batches indexes (if table exists)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'proof_batches') THEN
        -- Create indexes for contract posting tracking
        CREATE INDEX IF NOT EXISTS idx_proof_batches_posted_to_contract 
        ON proof_batches(proof_status, posted_to_contract)
        WHERE proof_status = 'proven' AND posted_to_contract = FALSE;
        
        CREATE INDEX IF NOT EXISTS idx_proof_batches_posted_at 
        ON proof_batches(posted_to_contract_at);
    END IF;
END $$;

-- ============================================================================
-- CORE FUNCTIONS
-- ============================================================================

-- Function to find the appropriate low_nullifier for insertion
-- This version works correctly with genesis-initialized trees
CREATE OR REPLACE FUNCTION find_low_nullifier(new_value BIGINT)
RETURNS TABLE(
    low_value BIGINT,
    low_next_index BIGINT,
    low_next_value BIGINT,
    low_tree_index BIGINT
) AS $$
BEGIN
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
    
    -- If no result found, new_value must be smaller than all existing nullifiers
    -- In genesis-initialized tree, this means it should be inserted after genesis (value=0)
    IF NOT FOUND THEN
        RETURN QUERY
        SELECT 
            n.value,
            n.next_index,
            n.next_value,
            n.tree_index
        FROM nullifiers n
        WHERE n.is_active = true
        ORDER BY n.value DESC
        LIMIT 1;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Atomic tree index allocation function (prevents race conditions)
CREATE OR REPLACE FUNCTION get_next_tree_index()
RETURNS BIGINT AS $$
DECLARE
    next_idx BIGINT;
BEGIN
    -- Use FOR UPDATE to lock the row and ensure atomic increment
    SELECT next_available_index INTO next_idx
    FROM tree_state
    WHERE tree_id = 'default'
    FOR UPDATE;
    
    -- If tree_state doesn't exist, handle gracefully
    IF next_idx IS NULL THEN
        next_idx := 0;
    END IF;
    
    -- Increment the next_available_index for the next call
    UPDATE tree_state
    SET next_available_index = next_idx + 1,
        updated_at = NOW()
    WHERE tree_id = 'default';
    
    RETURN next_idx;
END;
$$ LANGUAGE plpgsql;

-- Drop any existing versions to avoid conflicts (including from migration 002)
DROP FUNCTION IF EXISTS insert_nullifier_atomic CASCADE;
DROP FUNCTION IF EXISTS insert_nullifier_atomic(BIGINT) CASCADE;
DROP FUNCTION IF EXISTS insert_nullifier_atomic(bigint) CASCADE;

-- Atomic nullifier insertion function (7-step algorithm with genesis support)
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
BEGIN
    -- Step 1: Find low nullifier (works correctly with genesis nullifier)
    SELECT * INTO low_null
    FROM find_low_nullifier(new_value)
    LIMIT 1;
    
    -- Step 2: Validate insertion is possible
    IF EXISTS (SELECT 1 FROM nullifiers WHERE value = new_value AND is_active = true) THEN
        -- Nullifier already exists
        RETURN QUERY SELECT NULL::BIGINT, NULL::BIGINT, NULL::BIGINT, FALSE;
        RETURN;
    END IF;
    
    -- Step 3: Get tree index atomically
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

-- Function to fix tree_state inconsistencies (required by Rust code)
CREATE OR REPLACE FUNCTION fix_tree_state_consistency()
RETURNS VOID AS $$
DECLARE
    actual_nullifier_count BIGINT;
    actual_next_index BIGINT;
    current_count BIGINT;
    current_next BIGINT;
BEGIN
    -- Get the actual data from nullifiers table
    SELECT COUNT(*) INTO actual_nullifier_count
    FROM nullifiers WHERE is_active = true;

    SELECT COALESCE(MAX(tree_index), -1) + 1 INTO actual_next_index
    FROM nullifiers WHERE is_active = true;

    -- Ensure the row exists first
    INSERT INTO tree_state (tree_id, root_hash, next_available_index, tree_height, total_nullifiers)
    VALUES ('default', '\x0000000000000000000000000000000000000000000000000000000000000000', 0, 32, 0)
    ON CONFLICT (tree_id) DO NOTHING;

    -- Get current values
    SELECT total_nullifiers, next_available_index INTO current_count, current_next
    FROM tree_state WHERE tree_id = 'default';

    -- Update with correct values only if they differ
    IF current_count != actual_nullifier_count OR current_next != actual_next_index THEN
        UPDATE tree_state
        SET
            total_nullifiers = actual_nullifier_count,
            next_available_index = actual_next_index,
            updated_at = NOW()
        WHERE tree_id = 'default';
    END IF;
END;
$$ LANGUAGE plpgsql;

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
        (SELECT total_nullifiers FROM tree_state WHERE tree_id = 'default') >= 1 as tree_state_consistent,
        (SELECT next_available_index FROM tree_state WHERE tree_id = 'default') >= 1 as next_index_correct,
        (SELECT COUNT(*) FROM nullifiers WHERE is_active = true) >= 1 as total_count_correct;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- GENESIS STATE INITIALIZATION
-- ============================================================================

-- Initialize default tree state
INSERT INTO tree_state (tree_id, root_hash, next_available_index, tree_height, total_nullifiers, updated_at)
VALUES ('default', '\x0000000000000000000000000000000000000000000000000000000000000000', 0, 32, 0, NOW())
ON CONFLICT (tree_id) DO NOTHING;

-- Insert the genesis nullifier representing the initial balance of 0
-- This eliminates all empty tree edge cases
INSERT INTO nullifiers (value, next_index, next_value, tree_index, is_active, created_at)
VALUES (0, NULL, 0, 0, true, NOW())
ON CONFLICT (value) DO NOTHING;

-- Update tree state to reflect genesis nullifier
UPDATE tree_state 
SET 
    total_nullifiers = 1,
    next_available_index = 1,
    root_hash = '\x0000000000000000000000000000000000000000000000000000000000000000',
    updated_at = NOW()
WHERE tree_id = 'default';

-- Initialize the genesis leaf in the Merkle tree
INSERT INTO merkle_nodes (tree_level, node_index, hash_value, updated_at)
VALUES (0, 0, '\x0000000000000000000000000000000000000000000000000000000000000000', NOW())
ON CONFLICT (tree_level, node_index) DO UPDATE SET
    hash_value = '\x0000000000000000000000000000000000000000000000000000000000000000',
    updated_at = NOW();

-- ============================================================================
-- VERIFICATION AND COMMENTS
-- ============================================================================

-- Verify genesis state was initialized correctly
SELECT * FROM verify_genesis_state();

-- Table comments
COMMENT ON TABLE nullifiers IS 'Indexed Merkle tree nullifiers with linked-list structure for O(log n) operations';
COMMENT ON TABLE merkle_nodes IS 'Merkle tree nodes storing hash values at each level for cryptographic proofs';  
COMMENT ON TABLE tree_state IS 'Global tree state tracking root hash, total nullifiers, and next available index';
COMMENT ON TABLE audit_events IS 'Audit trail for ADS operations and blockchain interactions';

-- Column comments
COMMENT ON COLUMN nullifiers.value IS 'The nullifier value (must be unique and positive)';
COMMENT ON COLUMN nullifiers.next_index IS 'Tree index of the next higher nullifier (NULL if this points to max)';
COMMENT ON COLUMN nullifiers.next_value IS 'Value of the next higher nullifier (0 if this is the maximum)';
COMMENT ON COLUMN nullifiers.tree_index IS 'Position in the 32-level Merkle tree (0 to 2^32-1)';

-- Function comments
COMMENT ON FUNCTION find_low_nullifier(BIGINT) IS 'Finds appropriate low nullifier for insertion. Works correctly with genesis-initialized trees.';
COMMENT ON FUNCTION get_next_tree_index() IS 'Atomically allocates next available tree index, preventing race conditions.';
COMMENT ON FUNCTION insert_nullifier_atomic(BIGINT) IS 'Atomic 7-step nullifier insertion with genesis support. Eliminates race conditions and empty tree issues.';
COMMENT ON FUNCTION verify_genesis_state() IS 'Verifies genesis nullifier (value=0) is properly initialized to prevent empty tree edge cases.';
COMMENT ON FUNCTION fix_tree_state_consistency() IS 'Ensures tree_state table is consistent with actual nullifier data. Called by Rust code during initialization.';

-- Add comments to proof_batches columns if they exist
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'proof_batches') THEN
        COMMENT ON COLUMN proof_batches.posted_to_contract IS 'Whether this proven batch has been posted to the smart contract';
        COMMENT ON COLUMN proof_batches.posted_to_contract_at IS 'Timestamp when the batch was successfully posted to smart contract';
    END IF;
END $$;
