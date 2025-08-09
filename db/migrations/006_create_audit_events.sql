-- Create audit events table for comprehensive audit trails
CREATE TABLE audit_events (
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

-- Indexes for efficient queries
CREATE INDEX idx_audit_nullifier ON audit_events (nullifier_value);
CREATE INDEX idx_audit_timestamp ON audit_events (timestamp);
CREATE INDEX idx_audit_type ON audit_events (event_type);
CREATE INDEX idx_audit_block ON audit_events (block_height);