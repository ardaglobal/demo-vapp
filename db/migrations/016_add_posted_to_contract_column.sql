-- Add posted_to_contract column to track smart contract posting status
-- This enables background processing to post proven batches to the smart contract

-- Add the new column to track whether batches have been posted to contract
ALTER TABLE proof_batches 
ADD COLUMN posted_to_contract BOOLEAN NOT NULL DEFAULT FALSE;

-- Add index for efficient queries of proven but unposted batches
CREATE INDEX idx_proof_batches_posted_to_contract ON proof_batches(proof_status, posted_to_contract)
WHERE proof_status = 'proven' AND posted_to_contract = FALSE;

-- Add timestamp for when batch was posted to contract
ALTER TABLE proof_batches
ADD COLUMN posted_to_contract_at TIMESTAMP WITH TIME ZONE;

-- Add index for posted timestamp
CREATE INDEX idx_proof_batches_posted_at ON proof_batches(posted_to_contract_at);

-- Update column comments
COMMENT ON COLUMN proof_batches.posted_to_contract IS 'Whether this proven batch has been posted to the smart contract';
COMMENT ON COLUMN proof_batches.posted_to_contract_at IS 'Timestamp when the batch was successfully posted to smart contract';