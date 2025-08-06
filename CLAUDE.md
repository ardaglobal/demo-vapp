# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an SP1 (Succinct Proof) project that demonstrates zero-knowledge proof generation for arithmetic addition operations. The project consists of four main components:

1. **RISC-V Program** (`program/`): Performs arithmetic addition inside the SP1 zkVM
2. **Script** (`script/`): Generates proofs and handles execution using the SP1 SDK
3. **Smart Contracts** (`contracts/`): Solidity contracts for on-chain proof verification
4. **Database Module** (`db/`): PostgreSQL integration for storing arithmetic transactions

## Common Commands

### Building and Development
```bash
# First-time setup: compile the program to RISC-V
cd program && cargo prove build

# Execute program without generating proof (stores result in PostgreSQL)
cd script && cargo run --release -- --execute --a 5 --b 10

# Generate SP1 core proof
cd script && cargo run --release -- --prove --a 5 --b 10

# Verify stored data in PostgreSQL for a specific result
cd script && cargo run --release -- --verify --result 15

# Generate EVM-compatible Groth16 proof (requires 16GB+ RAM)
cd script && cargo run --release --bin evm -- --system groth16

# Generate EVM-compatible PLONK proof
cd script && cargo run --release --bin evm -- --system plonk

# Retrieve verification key for on-chain contracts
cd script && cargo run --release --bin vkey
```

### Smart Contract Testing
```bash
# Run Foundry tests
cd contracts && forge test

# Build contracts
cd contracts && forge build
```

### Workspace Commands
```bash
# Build entire workspace
cargo build --release

# Run tests across workspace
cargo test
```

## Architecture

### Core Components

- **arithmetic-lib** (`lib/`): Shared library containing the arithmetic computation logic and Solidity type definitions
- **arithmetic-program** (`program/`): The RISC-V program that runs inside the zkVM, reading input and committing public values
- **arithmetic-script** (`script/`): Contains multiple binaries:
  - `main.rs`: Main script for execution and proof generation
  - `evm.rs`: EVM-compatible proof generation (Groth16/PLONK)
  - `vkey.rs`: Verification key retrieval

### Data Flow

1. The zkVM program reads two arithmetic inputs (`a` and `b`)
2. Performs addition using the shared library (`a + b`)
3. Encodes inputs and result as `PublicValuesStruct` and commits to zkVM
4. When executing (not proving), computed results are stored in PostgreSQL as transactions with a, b, and result values
5. The script can verify previously computed results by querying PostgreSQL
6. The script generates proofs that can be verified on-chain via the Solidity contract

### Key Files

- `program/src/main.rs:14`: Main zkVM entry point with input/output handling
- `lib/src/lib.rs:14`: Core arithmetic addition logic
- `contracts/src/Arithmetic.sol:35`: On-chain proof verification function
- `script/src/bin/main.rs:45`: Proof generation orchestration

## Environment Configuration

Set up environment variables:
```bash
cp .env.example .env
# Set DATABASE_URL for PostgreSQL connection
# Set SP1_PROVER=network and NETWORK_PRIVATE_KEY for prover network usage
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

### Database Configuration

- **Connection**: Uses DATABASE_URL environment variable
- **Pooling**: sqlx PgPool for connection management
- **Migrations**: Automatic schema creation and indexing
- **Indexing**: Optimized queries on result values and timestamps

## Testing

The project includes comprehensive tests:
- Foundry tests for smart contracts (`contracts/test/`)
- Proof fixtures for both Groth16 and PLONK verification systems
- Execution validation in the main script

Tests use mock verification for faster execution and load proof fixtures from `contracts/src/fixtures/`.