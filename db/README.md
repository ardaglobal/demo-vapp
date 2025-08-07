# Arithmetic Database Module

This module provides PostgreSQL integration for storing and retrieving arithmetic computation results.

## Features

- **Async Operations**: Non-blocking database operations using sqlx
- **Connection Pooling**: Efficient database connection management
- **Automatic Migrations**: Schema initialization on startup
- **ACID Compliance**: Reliable data integrity
- **Duplicate Prevention**: Unique constraints on (a, b, result) tuples

## Testing

### Prerequisites

Before running tests, ensure you have:

1. **PostgreSQL Server**: A running PostgreSQL instance ( see `docker-compose.yml` for a local instance that you can run using `docker compose up`)
2. **Database URL**: Set the `DATABASE_URL` environment variable:
   ```bash
   export DATABASE_URL="postgres://username:password@localhost:5432/database_name"
   ```
   Or create a `.env` file in the project root with:
   ```
   DATABASE_URL=postgres://username:password@localhost:5432/database_name
   ```

### Running Tests

The test suite includes comprehensive coverage of all database operations:

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test modules
cargo test db_tests
cargo test error_handling_tests
cargo test performance_tests

# Run tests with tracing output
RUST_LOG=debug cargo test -- --nocapture
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

## Integration with Main Project

This database module integrates with the SP1 arithmetic project by:

1. **Execution Storage**: When running `--execute`, computed results are stored
2. **Verification**: Using `--verify` queries stored results for validation
3. **Proof Generation**: Results can be used as inputs for zero-knowledge proofs
4. **Audit Trail**: All computations are permanently recorded with timestamps