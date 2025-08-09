-- Create processor_state table to track background processing progress
CREATE TABLE IF NOT EXISTS processor_state (
    processor_id VARCHAR(50) PRIMARY KEY,
    last_processed_transaction_id INTEGER,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Insert default processor state
INSERT INTO processor_state (processor_id, last_processed_transaction_id)
VALUES ('default', NULL)
ON CONFLICT (processor_id) DO NOTHING;