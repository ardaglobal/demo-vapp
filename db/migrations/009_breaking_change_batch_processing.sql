-- BREAKING CHANGE: Restructure for batch processing with ZK proofs
-- This migration drops all existing tables and starts fresh with the new architecture
-- for continuous counter with batched transaction processing and ZK proofs

-- ============================================================================
-- DROP EXISTING TABLES (Breaking Change)
-- ============================================================================

-- Drop dependent tables first
DROP TABLE IF EXISTS audit_events CASCADE;
DROP TABLE IF EXISTS sindri_proofs CASCADE;  
DROP TABLE IF EXISTS state_transitions CASCADE;
DROP TABLE IF EXISTS processor_state CASCADE;
DROP TABLE IF EXISTS merkle_nodes CASCADE;
DROP TABLE IF EXISTS tree_state CASCADE;
DROP TABLE IF EXISTS nullifier_queue CASCADE;
DROP TABLE IF EXISTS indexed_merkle_tree CASCADE;
DROP TABLE IF EXISTS global_state CASCADE;
DROP TABLE IF EXISTS arithmetic_transactions CASCADE;

-- Drop any remaining functions
DROP FUNCTION IF EXISTS get_current_state() CASCADE;
DROP FUNCTION IF EXISTS update_global_state(INTEGER, INTEGER, INTEGER, INTEGER) CASCADE;
DROP FUNCTION IF EXISTS get_state_history(INTEGER) CASCADE;
DROP FUNCTION IF EXISTS validate_state_integrity() CASCADE;
DROP FUNCTION IF EXISTS get_next_tree_index() CASCADE;
DROP FUNCTION IF EXISTS get_tree_stats() CASCADE;

-- ============================================================================
-- CREATE NEW BATCH PROCESSING SCHEMA
-- ============================================================================

-- Table 1: Track incoming transactions (FIFO queue)
CREATE TABLE incoming_transactions (
    id SERIAL PRIMARY KEY,
    amount INTEGER NOT NULL,
    included_in_batch_id INTEGER NULL, -- References proof_batches(id), NULL = not batched yet
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Table 2: ZK proof batches (groups of transactions + proofs)
CREATE TABLE proof_batches (
    id SERIAL PRIMARY KEY,
    previous_counter_value BIGINT NOT NULL,
    final_counter_value BIGINT NOT NULL,
    transaction_ids INTEGER[] NOT NULL, -- Array of transaction IDs in this batch
    sindri_proof_id VARCHAR(255), -- From Sindri service
    proof_status VARCHAR(50) DEFAULT 'pending', -- pending, proven, failed
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    proven_at TIMESTAMP WITH TIME ZONE
);

-- Table 3: ADS/Merkle tree state tracking
CREATE TABLE ads_state_commits (
    id SERIAL PRIMARY KEY,
    batch_id INTEGER NOT NULL REFERENCES proof_batches(id),
    merkle_root BYTEA NOT NULL, -- The ADS/Merkle tree root hash for smart contract
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- ADD FOREIGN KEY CONSTRAINT (after both tables exist)
-- ============================================================================

-- Add foreign key constraint for incoming_transactions -> proof_batches
ALTER TABLE incoming_transactions 
ADD CONSTRAINT fk_incoming_transactions_batch_id 
FOREIGN KEY (included_in_batch_id) REFERENCES proof_batches(id);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Indexes for incoming_transactions
CREATE INDEX idx_incoming_transactions_not_batched ON incoming_transactions(id) 
WHERE included_in_batch_id IS NULL; -- Fast lookup for unbatched transactions

CREATE INDEX idx_incoming_transactions_batch_id ON incoming_transactions(included_in_batch_id);
CREATE INDEX idx_incoming_transactions_created_at ON incoming_transactions(created_at);

-- Indexes for proof_batches  
CREATE INDEX idx_proof_batches_status ON proof_batches(proof_status);
CREATE INDEX idx_proof_batches_sindri_proof_id ON proof_batches(sindri_proof_id);
CREATE INDEX idx_proof_batches_counter_range ON proof_batches(previous_counter_value, final_counter_value);
CREATE INDEX idx_proof_batches_created_at ON proof_batches(created_at);

-- Indexes for ads_state_commits
CREATE INDEX idx_ads_state_commits_batch_id ON ads_state_commits(batch_id);
CREATE INDEX idx_ads_state_commits_merkle_root ON ads_state_commits(merkle_root);

-- ============================================================================
-- HELPER FUNCTIONS FOR BATCH PROCESSING
-- ============================================================================

-- Function to get unbatched transactions (FIFO order)
CREATE OR REPLACE FUNCTION get_unbatched_transactions(limit_count INTEGER DEFAULT 10)
RETURNS TABLE(
    transaction_id INTEGER,
    amount INTEGER,
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        it.id,
        it.amount,
        it.created_at
    FROM incoming_transactions it
    WHERE it.included_in_batch_id IS NULL
    ORDER BY it.id ASC -- FIFO: oldest first
    LIMIT limit_count;
END;
$$ LANGUAGE plpgsql;

-- Function to get current counter value (from latest proven batch)
CREATE OR REPLACE FUNCTION get_current_counter_value()
RETURNS BIGINT AS $$
DECLARE
    current_value BIGINT := 0;
BEGIN
    SELECT pb.final_counter_value
    INTO current_value
    FROM proof_batches pb
    WHERE pb.proof_status = 'proven'
    ORDER BY pb.id DESC
    LIMIT 1;
    
    -- Return 0 if no proven batches exist yet
    RETURN COALESCE(current_value, 0);
END;
$$ LANGUAGE plpgsql;

-- Function to create a new batch from unbatched transactions
CREATE OR REPLACE FUNCTION create_batch(batch_size INTEGER DEFAULT 10)
RETURNS INTEGER AS $$
DECLARE
    new_batch_id INTEGER;
    previous_counter BIGINT;
    final_counter BIGINT;
    transaction_total INTEGER;
    transaction_id_array INTEGER[];
BEGIN
    -- Get current counter value
    SELECT get_current_counter_value() INTO previous_counter;
    
    -- Get unbatched transactions and calculate total
    SELECT 
        ARRAY_AGG(id ORDER BY id),
        SUM(amount)
    INTO transaction_id_array, transaction_total
    FROM (
        SELECT id, amount 
        FROM incoming_transactions 
        WHERE included_in_batch_id IS NULL 
        ORDER BY id ASC 
        LIMIT batch_size
    ) unbatched;
    
    -- Return 0 if no unbatched transactions
    IF transaction_id_array IS NULL OR array_length(transaction_id_array, 1) = 0 THEN
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
    
    -- Mark transactions as included in this batch
    UPDATE incoming_transactions 
    SET included_in_batch_id = new_batch_id
    WHERE id = ANY(transaction_id_array);
    
    RETURN new_batch_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- INITIALIZE WITH STARTING STATE
-- ============================================================================

-- Insert initial counter state (starts at 0)
-- This will be updated when the first batch is created and proven

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE incoming_transactions IS 'FIFO queue of integer transactions to be processed in batches';
COMMENT ON TABLE proof_batches IS 'Batches of transactions with ZK proofs showing counter transitions';
COMMENT ON TABLE ads_state_commits IS 'ADS/Merkle tree root hashes for smart contract posting';

COMMENT ON COLUMN incoming_transactions.included_in_batch_id IS 'NULL if not yet batched, otherwise references proof_batches.id';
COMMENT ON COLUMN proof_batches.transaction_ids IS 'Array of transaction IDs included in this batch';
COMMENT ON COLUMN proof_batches.sindri_proof_id IS 'Sindri service proof ID for verification';
COMMENT ON COLUMN ads_state_commits.merkle_root IS 'Merkle tree root hash posted to smart contract with ZK proof';