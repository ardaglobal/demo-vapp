-- Create arithmetic_transactions table
CREATE TABLE IF NOT EXISTS arithmetic_transactions (
	id SERIAL PRIMARY KEY,
	a INTEGER NOT NULL,
	b INTEGER NOT NULL,
	result INTEGER NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
	UNIQUE(a, b, result)
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_arithmetic_result ON arithmetic_transactions(result);

CREATE INDEX IF NOT EXISTS idx_arithmetic_created_at ON arithmetic_transactions(created_at);