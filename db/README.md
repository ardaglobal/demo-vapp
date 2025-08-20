# Database Module

PostgreSQL integration for arithmetic computation storage and ADS (Authenticated Data Structure) operations with Indexed Merkle Trees.

## Features

- **Arithmetic Storage**: Transaction storage with duplicate prevention
- **ADS Integration**: Indexed Merkle Tree (IMT) with 32-level optimization
- **Batch Processing**: Atomic batch operations with nullifier generation
- **Async Operations**: Non-blocking database operations using sqlx
- **Connection Pooling**: Efficient database connection management
- **Automatic Migrations**: Schema initialization on startup
- **ACID Compliance**: Reliable data integrity

## Database Schema

### Core Tables

- **`arithmetic_transactions`**: Basic arithmetic computations
- **`incoming_transactions`**: Pending batch processing queue
- **`proof_batches`**: Batch metadata with ZK proof references
- **`nullifiers`**: IMT nullifier values with tree structure
- **`ads_state_commits`**: Merkle roots linked to batches
- **`tree_state`**: Global IMT state (root, counter, etc.)
- **`merkle_nodes`**: 32-level indexed Merkle tree nodes

### ADS Integration Flow

1. **Transaction submitted** → stored in `incoming_transactions`
2. **Batch trigger** → calls `BackgroundBatchProcessor::process_batch()`
3. **ADS processing** → converts transaction to positive nullifier value
4. **IMT insertion** → stores in `nullifiers` table with tree structure
5. **Merkle root** → computed and stored in `ads_state_commits`
6. **Batch completion** → proof generation triggered

```
🔄 Background Processor → 🔐 ADS Service → 🌳 IndexedMerkleTree
                                              ↓
📦 Batch Creation → 💾 Database Tables → ⚡ ZK Proof Generation
```

## Prerequisites

Before running tests or operations:

1. **PostgreSQL Server**: Running instance (see `docker-compose.yml`)
   ```bash
   docker compose up postgres -d
   ```

2. **Database URL**: Set environment variable:
   ```bash
   export DATABASE_URL="postgres://postgres:password@localhost:5432/arithmetic_db"
   ```
   Or use `.env` file:
   ```
   DATABASE_URL=postgres://postgres:password@localhost:5432/arithmetic_db
   ```

## Testing

### Running Tests

Comprehensive test coverage for all database operations:

```bash
# Run all tests
cargo test

# Run with debug output
RUST_LOG=debug cargo test -- --nocapture

# Run specific test modules
cargo test db_tests
cargo test error_handling_tests
cargo test performance_tests
```

### Test Categories

#### 1. **Basic Operations Tests** (`tests.rs`)
- Database initialization and connection
- Transaction storage and retrieval
- Duplicate handling
- Multiple transactions with same result
- Edge cases (negative numbers, zero values, large numbers)
- Concurrent operations

#### 2. **Error Handling Tests** (`error_tests.rs`)
- Invalid database URLs
- Connection failures
- Operations on closed pools
- Boundary value testing (i32::MIN, i32::MAX)
- Migration idempotency
- Stress testing with 1000+ operations

#### 3. **Performance Tests** (`error_tests.rs`)
- Bulk insert performance benchmarks
- Concurrent read performance testing
- Connection pool efficiency

### Test Database Management

Tests automatically:
- Create isolated test databases with unique names
- Clean up test data after each test
- Handle database creation and destruction
- Maintain test isolation

### Example Test Output

```bash
$ cargo test

running 15 tests
test tests::db_tests::test_init_db_success ... ok
test tests::db_tests::test_store_arithmetic_transaction ... ok
test tests::db_tests::test_get_transactions_by_result_empty ... ok
test tests::db_tests::test_concurrent_operations ... ok
test error_tests::error_handling_tests::test_boundary_values ... ok
test error_tests::performance_tests::test_bulk_insert_performance ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## API Documentation

### Core Functions

- `init_db()`: Initialize database connection pool and run migrations
- `store_arithmetic_transaction(pool, a, b, result)`: Store an arithmetic transaction
- `get_value_by_result(pool, result)`: Get first transaction by result value
- `get_transactions_by_result(pool, result)`: Get all transactions by result value

### Data Structures

- `ArithmeticTransaction`: Represents a stored computation with fields `a`, `b`, and `result`

### Error Handling

All functions return `Result<T, sqlx::Error>` for proper error handling. Common error scenarios include:

- Database connection failures
- Invalid SQL operations
- Pool exhaustion
- Migration failures

## ADS Database Verification

### Quick Verification Commands

#### 1. Test ADS Integration
```bash
# Submit transaction and trigger batch
make cli ARGS="submit-transaction --amount 42"
curl -X POST http://localhost:8080/api/v2/batches/trigger
```

#### 2. Debug Logging
```bash
RUST_LOG=debug cargo run --bin server
```

**Expected debug logs:**
- `🔄 Processing batch with ADS integration`
- `📦 Processing transactions through ADS batch workflow`
- `🔐 ADS Service: Batch inserting N nullifiers`
- `🌳 IndexedMerkleTree: insert_nullifier`

#### 3. Database Queries
```sql
-- Count active nullifiers
SELECT COUNT(*) FROM nullifiers WHERE is_active = true;

-- Recent nullifiers
SELECT value, tree_index, created_at 
FROM nullifiers 
WHERE is_active = true 
ORDER BY created_at DESC 
LIMIT 5;

-- ADS state commits
SELECT batch_id, created_at 
FROM ads_state_commits 
ORDER BY created_at DESC 
LIMIT 5;

-- Tree state
SELECT total_nullifiers, next_available_index, updated_at 
FROM tree_state 
WHERE tree_id = 'default';
```

#### 4. Batch-Root Mapping
```sql
SELECT pb.id as batch_id, pb.transaction_count, 
       ads.merkle_root, ads.created_at
FROM proof_batches pb
JOIN ads_state_commits ads ON pb.id = ads.batch_id
ORDER BY pb.id DESC
LIMIT 5;
```

## Integration with Main Project

The database module provides:

1. **Arithmetic Storage**: Basic computation storage and retrieval
2. **ADS Integration**: Indexed Merkle Tree operations with batch processing
3. **Batch Processing**: Atomic operations with nullifier generation
4. **ZK Proof Support**: State management for proof generation
5. **Audit Trail**: Complete transaction history with timestamps

### Usage Patterns

- **Execution Storage**: `--execute` stores computed results
- **Verification**: `--verify` queries stored results for validation
- **Batch Operations**: Background processor handles ADS integration
- **Proof Generation**: Results feed into zero-knowledge proof workflows