-- Create global state management for continuous ledger functionality
-- This table maintains a single running counter that accumulates all arithmetic results

-- ============================================================================
-- GLOBAL STATE TABLE: Track continuous state accumulator
-- ============================================================================
CREATE TABLE global_state (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1), -- Enforce single row
    current_state BIGINT NOT NULL DEFAULT 0,
    transaction_count BIGINT NOT NULL DEFAULT 0,
    last_transaction_id INTEGER REFERENCES arithmetic_transactions(id),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT single_global_state CHECK (id = 1)
);

-- ============================================================================
-- STATE TRANSITIONS TABLE: Track each state change for audit/history
-- ============================================================================
CREATE TABLE state_transitions (
    id SERIAL PRIMARY KEY,
    transaction_id INTEGER NOT NULL REFERENCES arithmetic_transactions(id),
    previous_state BIGINT NOT NULL,
    arithmetic_result INTEGER NOT NULL,
    new_state BIGINT NOT NULL,
    a INTEGER NOT NULL,
    b INTEGER NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Verification constraint
    CONSTRAINT valid_state_transition CHECK (new_state = previous_state + arithmetic_result)
);

-- ============================================================================
-- INDEXES: Optimized for state queries and transitions
-- ============================================================================
CREATE INDEX idx_state_transitions_transaction_id ON state_transitions(transaction_id);
CREATE INDEX idx_state_transitions_created_at ON state_transitions(created_at);
CREATE INDEX idx_state_transitions_new_state ON state_transitions(new_state);

-- ============================================================================
-- FUNCTIONS: Atomic state management operations
-- ============================================================================

-- Function to get current global state
CREATE OR REPLACE FUNCTION get_current_state()
RETURNS TABLE(
    current_state BIGINT,
    transaction_count BIGINT,
    last_updated TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT gs.current_state, gs.transaction_count, gs.updated_at
    FROM global_state gs
    WHERE gs.id = 1;
    
    -- If no state exists, return defaults
    IF NOT FOUND THEN
        RETURN QUERY SELECT 0::BIGINT, 0::BIGINT, NOW();
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Function to atomically update state with arithmetic result
CREATE OR REPLACE FUNCTION update_global_state(
    transaction_id INTEGER,
    arithmetic_result INTEGER,
    a INTEGER,
    b INTEGER
)
RETURNS TABLE(
    previous_state BIGINT,
    new_state BIGINT,
    success BOOLEAN
) AS $$
DECLARE
    prev_state BIGINT := 0;
    new_state_value BIGINT;
BEGIN
    -- Begin atomic transaction
    BEGIN
        -- Get current state (with row lock for consistency)
        SELECT gs.current_state INTO prev_state
        FROM global_state gs
        WHERE gs.id = 1
        FOR UPDATE;
        
        -- If no state row exists, create it
        IF NOT FOUND THEN
            INSERT INTO global_state (id, current_state, transaction_count)
            VALUES (1, 0, 0);
            prev_state := 0;
        END IF;
        
        -- Calculate new state
        new_state_value := prev_state + arithmetic_result;
        
        -- Update global state
        UPDATE global_state
        SET 
            current_state = new_state_value,
            transaction_count = transaction_count + 1,
            last_transaction_id = transaction_id,
            updated_at = NOW()
        WHERE id = 1;
        
        -- Record state transition
        INSERT INTO state_transitions (
            transaction_id,
            previous_state,
            arithmetic_result,
            new_state,
            a,
            b
        ) VALUES (
            transaction_id,
            prev_state,
            arithmetic_result,
            new_state_value,
            a,
            b
        );
        
        -- Return success result
        RETURN QUERY SELECT prev_state, new_state_value, TRUE;
        
    EXCEPTION WHEN OTHERS THEN
        -- Return failure on any error
        RETURN QUERY SELECT prev_state, prev_state, FALSE;
    END;
END;
$$ LANGUAGE plpgsql;

-- Function to get state history
CREATE OR REPLACE FUNCTION get_state_history(
    limit_count INTEGER DEFAULT 10
)
RETURNS TABLE(
    transition_id INTEGER,
    transaction_id INTEGER,
    previous_state BIGINT,
    arithmetic_result INTEGER,
    new_state BIGINT,
    a INTEGER,
    b INTEGER,
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        st.id,
        st.transaction_id,
        st.previous_state,
        st.arithmetic_result,
        st.new_state,
        st.a,
        st.b,
        st.created_at
    FROM state_transitions st
    ORDER BY st.created_at DESC
    LIMIT limit_count;
END;
$$ LANGUAGE plpgsql;

-- Function to validate state integrity
CREATE OR REPLACE FUNCTION validate_state_integrity()
RETURNS TABLE(
    is_valid BOOLEAN,
    expected_state BIGINT,
    actual_state BIGINT,
    transaction_count BIGINT,
    error_message TEXT
) AS $$
DECLARE
    calculated_state BIGINT := 0;
    actual_state_val BIGINT;
    trans_count BIGINT;
    rec RECORD;
BEGIN
    -- Calculate expected state by summing all arithmetic results
    SELECT COALESCE(SUM(result), 0) INTO calculated_state
    FROM arithmetic_transactions;
    
    -- Get current state from global_state table
    SELECT gs.current_state, gs.transaction_count 
    INTO actual_state_val, trans_count
    FROM global_state gs
    WHERE gs.id = 1;
    
    IF NOT FOUND THEN
        actual_state_val := 0;
        trans_count := 0;
    END IF;
    
    -- Check if states match
    IF calculated_state = actual_state_val THEN
        RETURN QUERY SELECT TRUE, calculated_state, actual_state_val, trans_count, 'State integrity verified'::TEXT;
    ELSE
        RETURN QUERY SELECT FALSE, calculated_state, actual_state_val, trans_count, 
            format('State mismatch: expected %s, actual %s', calculated_state, actual_state_val)::TEXT;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- INITIALIZATION: Set up initial state
-- ============================================================================

-- Initialize global state with existing data if any
DO $$
DECLARE
    existing_sum BIGINT;
    existing_count BIGINT;
BEGIN
    -- Calculate sum of existing arithmetic results
    SELECT COALESCE(SUM(result), 0), COUNT(*) 
    INTO existing_sum, existing_count
    FROM arithmetic_transactions;
    
    -- Insert initial global state
    INSERT INTO global_state (id, current_state, transaction_count)
    VALUES (1, existing_sum, existing_count)
    ON CONFLICT (id) DO NOTHING;
    
    -- Create state transition entries for existing transactions (if any)
    -- This helps maintain audit trail
    INSERT INTO state_transitions (transaction_id, previous_state, arithmetic_result, new_state, a, b, created_at)
    SELECT 
        at.id,
        (SELECT COALESCE(SUM(at2.result), 0) 
         FROM arithmetic_transactions at2 
         WHERE at2.id < at.id) as previous_state,
        at.result,
        (SELECT COALESCE(SUM(at2.result), 0) 
         FROM arithmetic_transactions at2 
         WHERE at2.id <= at.id) as new_state,
        at.a,
        at.b,
        at.created_at
    FROM arithmetic_transactions at
    ORDER BY at.id
    ON CONFLICT DO NOTHING; -- In case we run this migration multiple times
    
END $$;