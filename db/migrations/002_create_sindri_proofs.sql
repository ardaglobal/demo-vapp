-- Create table to store Sindri proof metadata keyed by result
CREATE TABLE IF NOT EXISTS sindri_proofs (
    id SERIAL PRIMARY KEY,
    result INTEGER NOT NULL,
    proof_id TEXT NOT NULL,
    circuit_id TEXT,
    status TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (result)
);

-- Index for quick lookup by result
CREATE INDEX IF NOT EXISTS idx_sindri_proofs_result ON sindri_proofs(result);


