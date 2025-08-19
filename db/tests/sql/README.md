# SQL Database Tests

This directory contains SQL test scripts that test database functions and procedures directly.

## Prerequisites

1. **Database Running**: Ensure PostgreSQL is running (via Docker Compose):
   ```bash
   docker-compose up postgres -d
   ```

2. **Database URL**: Set the DATABASE_URL environment variable:
   ```bash
   export DATABASE_URL="postgresql://postgres:password@localhost:5432/arithmetic_db"
   ```

3. **Migrations Applied**: Ensure all migrations are up to date:
   ```bash
   sqlx migrate run --source db/migrations
   ```

## Running Tests

### Individual Test Files

Run a specific test file:
```bash
psql $DATABASE_URL -f db/tests/sql/test_concurrent_batch_creation.sql
```

### All SQL Tests

Run all SQL tests in this directory:
```bash
./db/tests/sql/run_tests.sh
```

## Test Files

### `test_concurrent_batch_creation.sql`
Tests the `create_batch()` function for race conditions and concurrent safety.

**What it tests:**
- Multiple concurrent batch creation calls
- Proper transaction assignment without double-allocation
- Data integrity under concurrent access
- Correct batch size handling

**Expected output:**
- 15 test transactions → 3 batches of 5 transactions each
- No double assignments detected
- All transactions properly assigned to exactly one batch

## Adding New Tests

When adding new SQL test files:

1. **Naming Convention**: Use `test_*.sql` format
2. **Self-Contained**: Each test should clean up its own test data
3. **Clear Output**: Include status messages to show test progress
4. **Verification**: Include verification queries to validate results
5. **Documentation**: Update this README with test descriptions

## Integration with CI/CD

These tests can be integrated into CI/CD pipelines by:
1. Starting a test database
2. Running migrations
3. Executing all SQL test files
4. Checking exit codes and output for failures
