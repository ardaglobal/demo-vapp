# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an SP1 (Succinct Proof) project that demonstrates zero-knowledge proof generation for arithmetic addition operations. The project consists of five main components:

1. **RISC-V Program** (`program/`): Performs arithmetic addition inside the SP1 zkVM
2. **Script** (`script/`): Generates proofs and handles execution using the SP1 SDK
3. **Smart Contracts** (`contracts/`): Solidity contracts for on-chain proof verification
4. **Database Module** (`db/`): PostgreSQL integration for storing arithmetic transactions and Sindri proof metadata
5. **Sindri Integration** (integrated into `script/src/bin/main.rs`): Serverless proof generation using Sindri's cloud infrastructure

## Common Commands

### Building and Development
```bash
# First-time setup: compile the program to RISC-V
cd program && cargo prove build

# Execute program interactively without generating proof (stores results in PostgreSQL)
cd script && cargo run --release -- --execute

# Execute program non-interactively (legacy mode)
cd script && cargo run --release -- --execute --a 5 --b 10

# Generate SP1 core proof
cd script && cargo run --release -- --prove --a 5 --b 10

# Verify stored data interactively in PostgreSQL 
cd script && cargo run --release -- --verify

# Verify stored data for a specific result (non-interactive)
cd script && cargo run --release -- --verify --result 15

# Generate EVM-compatible Groth16 proof (requires 16GB+ RAM)
cd script && cargo run --release --bin evm -- --system groth16

# Generate EVM-compatible PLONK proof
cd script && cargo run --release --bin evm -- --system plonk

# Retrieve verification key for on-chain contracts
cd script && cargo run --release --bin vkey

# Generate ZK proof using Sindri cloud infrastructure  
SINDRI_API_KEY=your_api_key_here cargo run --release -- --prove --a 5 --b 10

# Generate proof for previously computed result
SINDRI_API_KEY=your_api_key_here cargo run --release -- --prove --result 15
```

### Smart Contract Testing
```bash
# Run Foundry tests
cd contracts && forge test

# Build contracts
cd contracts && forge build
```

### Database Testing
```bash
# Run database tests (requires PostgreSQL)
cd db && cargo test

# Run database tests with output
cd db && cargo test -- --nocapture

# Run specific database test categories
cd db && cargo test db_tests
cd db && cargo test error_handling_tests
cd db && cargo test performance_tests
```

### Workspace Commands
```bash
# Build entire workspace
cargo build --release

# Run tests across workspace
cargo test

# Run tests for specific components
cargo test -p arithmetic-db
cargo test -p arithmetic-lib
```

## Architecture

### Core Components

- **arithmetic-lib** (`lib/`): Shared library containing the arithmetic computation logic and Solidity type definitions
- **arithmetic-program** (`program/`): The RISC-V program that runs inside the zkVM, reading input and committing public values
- **arithmetic-script** (`script/`): Contains multiple binaries:
  - `main.rs`: Main script for execution, Sindri proof generation, and verification
  - `evm.rs`: EVM-compatible proof generation (Groth16/PLONK)
  - `vkey.rs`: Verification key retrieval

### Data Flow

1. The zkVM program reads two arithmetic inputs (`a` and `b`)
2. Performs addition using the shared library (`a + b`)
3. Encodes inputs and result as `PublicValuesStruct` and commits to zkVM
4. When executing (not proving), computed results are stored in PostgreSQL as transactions with a, b, and result values
5. The script can verify previously computed results by querying PostgreSQL
6. The script generates proofs that can be verified on-chain via the Solidity contract

### Sindri Integration Data Flow

1. User provides arithmetic inputs (`a` and `b`) via command-line arguments or uses previously computed results
2. SP1 inputs are serialized to JSON format expected by Sindri
3. Proof generation request is sent to Sindri's cloud infrastructure using the prebuilt `demo-vapp` circuit
4. Sindri returns proof metadata (proof ID, circuit ID, status) which is stored in PostgreSQL
5. Verification queries Sindri's API using stored proof metadata
6. Proof status is updated in the database and displayed to the user
### Interactive CLI Features

**Execute Mode**: The `--execute` command now runs interactively by default:
- Prompts users to enter values for 'a' and 'b'
- Computes the arithmetic operation in the zkVM
- Stores results automatically in PostgreSQL
- Continues in a loop until user presses 'q' to quit
- Shows real-time feedback on computation and database storage

**Verify Mode**: The `--verify` command supports interactive verification:
- When run without `--result`, starts interactive mode
- Prompts users to enter result values to look up
- Shows the original 'a' and 'b' values that produced each result  
- Continues in a loop until user presses 'q' to quit
- Supports legacy mode with `--result` flag for specific lookups

### Key Files

- `program/src/main.rs:14`: Main zkVM entry point with input/output handling
- `lib/src/lib.rs:14`: Core arithmetic addition logic
- `contracts/src/Arithmetic.sol:35`: On-chain proof verification function
- `script/src/bin/main.rs:45`: Proof generation orchestration including Sindri integration
- `script/src/bin/main.rs:272`: Sindri proof generation function (`run_prove_via_sindri`)
- `script/src/bin/main.rs:221`: Sindri proof verification function (`verify_result_via_sindri`)
- `db/src/db.rs:160`: Sindri proof database operations (`upsert_sindri_proof`, `get_sindri_proof_by_result`)

## Environment Configuration

### Database Setup (Docker - Recommended)

For easy testing and development, use Docker Compose to run PostgreSQL:

```bash
# Start PostgreSQL container
docker-compose up -d

# Set up environment variables
cp .env.example .env
# DATABASE_URL is already configured for Docker setup

# Run database tests
cd db && cargo test
```

### Manual Database Setup

Alternatively, install and configure PostgreSQL manually:

```bash
# Set up environment variables
cp .env.example .env
# Edit .env and set DATABASE_URL for your PostgreSQL connection
# Set SP1_PROVER=network and NETWORK_PRIVATE_KEY for prover network usage
# Set SINDRI_API_KEY for Sindri cloud proof generation
export SINDRI_API_KEY=your_api_key_here
```

### Stopping the Database

```bash
# Stop the PostgreSQL container
docker-compose down

# Stop and remove data (clean slate)
docker-compose down -v
```

## PostgreSQL Integration

This project uses PostgreSQL as the database for storing and retrieving arithmetic computation results.

### PostgreSQL Features Used

- **Relational Storage**: Structured data storage with ACID compliance
- **Async Operations**: Non-blocking database operations using sqlx
- **Connection Pooling**: Efficient database connection management
- **Automatic Migrations**: Schema initialization on startup

### Database Operations

The project provides the following PostgreSQL operations through the `arithmetic-db` crate:

- `init_db()`: Initialize PostgreSQL connection pool and run migrations
- `store_arithmetic_transaction(pool, a, b, result)`: Store an arithmetic transaction
- `get_value_by_result(pool, result)`: Retrieve the first transaction by result value
- `get_transactions_by_result(pool, result)`: Retrieve all transactions with a specific result
- `upsert_sindri_proof(pool, result, proof_id, circuit_id, status)`: Store/update Sindri proof metadata
- `get_sindri_proof_by_result(pool, result)`: Retrieve Sindri proof metadata by result

### Storage Schema

Arithmetic transactions are stored in the `arithmetic_transactions` table:
```sql
CREATE TABLE arithmetic_transactions (
    id SERIAL PRIMARY KEY,
    a INTEGER NOT NULL,
    b INTEGER NOT NULL,
    result INTEGER NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(a, b, result)
);
```

Sindri proof metadata is stored in the `sindri_proofs` table:
```sql
CREATE TABLE sindri_proofs (
    id SERIAL PRIMARY KEY,
    result INTEGER NOT NULL,
    proof_id TEXT NOT NULL,
    circuit_id TEXT,
    status TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (result)
);
```

### Database Configuration

- **Connection**: Uses DATABASE_URL environment variable
- **Pooling**: sqlx PgPool for connection management
- **Migrations**: Automatic schema creation and indexing
- **Indexing**: Optimized queries on result values and timestamps

## Testing

The project includes comprehensive testing across all components:

### Smart Contract Tests
- Foundry tests for smart contracts (`contracts/test/`)
- Proof fixtures for both Groth16 and PLONK verification systems
- Mock verification for faster execution using fixtures from `contracts/src/fixtures/`

### Database Tests
- **Unit Tests**: Core database operations (init, store, retrieve)
- **Integration Tests**: Full workflow testing with real PostgreSQL
- **Error Handling**: Invalid URLs, connection failures, closed pools
- **Performance Tests**: Bulk operations and concurrent access
- **Edge Cases**: Boundary values, negative numbers, zero handling
- **Stress Tests**: 1000+ operations to validate reliability

### Test Prerequisites
- **PostgreSQL**: Database tests require a running PostgreSQL instance
- **Environment**: Set `DATABASE_URL` environment variable for database tests
- **Isolation**: Tests automatically create/destroy isolated test databases

### Running All Tests
```bash
# Run all tests (requires PostgreSQL for database tests)
cargo test

# Run tests excluding database tests
cargo test -p arithmetic-lib
cargo test -p arithmetic-program

# Run only database tests
cargo test -p arithmetic-db
```