-- Create table for tracking Ethereum contract submissions
CREATE TABLE IF NOT EXISTS ethereum_submissions (
    id SERIAL PRIMARY KEY,
    result INTEGER NOT NULL UNIQUE,
    state_id BYTEA NOT NULL,
    state_root BYTEA NOT NULL,
    transaction_hash BYTEA,
    block_number BIGINT,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    submitted_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    confirmed_at TIMESTAMP WITH TIME ZONE,
    
    -- Indexes for efficient queries
    CONSTRAINT unique_result UNIQUE (result)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_ethereum_submissions_status ON ethereum_submissions(status);
CREATE INDEX IF NOT EXISTS idx_ethereum_submissions_submitted_at ON ethereum_submissions(submitted_at);
CREATE INDEX IF NOT EXISTS idx_ethereum_submissions_block_number ON ethereum_submissions(block_number);

-- Add comments for documentation
COMMENT ON TABLE ethereum_submissions IS 'Tracks submissions of proofs and state roots to Ethereum smart contracts';
COMMENT ON COLUMN ethereum_submissions.result IS 'The arithmetic result that was proven and submitted';
COMMENT ON COLUMN ethereum_submissions.state_id IS 'The state ID used in the smart contract';
COMMENT ON COLUMN ethereum_submissions.state_root IS 'The new state root submitted to the contract';
COMMENT ON COLUMN ethereum_submissions.transaction_hash IS 'Ethereum transaction hash of the submission';
COMMENT ON COLUMN ethereum_submissions.block_number IS 'Block number where the transaction was confirmed';
COMMENT ON COLUMN ethereum_submissions.status IS 'Status: pending, confirmed, failed';