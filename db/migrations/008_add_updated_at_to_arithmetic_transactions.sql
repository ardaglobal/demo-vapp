-- Add updated_at column to arithmetic_transactions table
ALTER TABLE arithmetic_transactions 
ADD COLUMN updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP;

-- Create index for better query performance on updated_at
CREATE INDEX IF NOT EXISTS idx_arithmetic_updated_at ON arithmetic_transactions(updated_at);