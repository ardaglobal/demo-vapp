# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an SP1 (Succinct Proof) project that demonstrates zero-knowledge proof generation for arithmetic addition operations. The project consists of four main components:

1. **RISC-V Program** (`program/`): Performs arithmetic addition inside the SP1 zkVM
2. **Script** (`script/`): Generates proofs and handles execution using the SP1 SDK
3. **Smart Contracts** (`contracts/`): Solidity contracts for on-chain proof verification
4. **Database Module** (`db/`): QMDB integration for authenticated data storage (ADS)

## Common Commands

### Building and Development
```bash
# First-time setup: compile the program to RISC-V
cd program && cargo prove build

# Execute program without generating proof (stores result in QMDB)
cd script && cargo run --release -- --execute --a 5 --b 10

# Generate SP1 core proof
cd script && cargo run --release -- --prove --a 5 --b 10

# Verify stored data in QMDB for a specific result
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
4. When executing (not proving), computed results are stored in QMDB with the result as key and the result value as data
5. The script can verify previously computed results by querying QMDB
6. The script generates proofs that can be verified on-chain via the Solidity contract

### Key Files

- `program/src/main.rs:14`: Main zkVM entry point with input/output handling
- `lib/src/lib.rs:14`: Core arithmetic addition logic
- `contracts/src/Arithmetic.sol:35`: On-chain proof verification function
- `script/src/bin/main.rs:45`: Proof generation orchestration

## Environment Configuration

Set up environment for prover network usage:
```bash
cp .env.example .env
# Set SP1_PROVER=network and NETWORK_PRIVATE_KEY in .env
```

## QMDB Integration

This project integrates QMDB (Quantum Merkle Database) as an Authenticated Data Structure (ADS) for storing and retrieving arithmetic computation results.

### QMDB Features Used

- **Authenticated Storage**: All stored data is cryptographically authenticated using Merkle trees
- **Key-Value Operations**: Store arithmetic results with the result value as key and result data as binary data
- **Concurrent Access**: Thread-safe operations using `parking_lot::RwLock`
- **Configurable Storage**: Uses configurable entry sizes and sharding for optimal performance

### Database Operations

The project provides the following QMDB operations through the `arithmetic-db` crate:

- `init_db()`: Initialize QMDB with default configuration in `ADS/` directory
- `create_simple_task_with_addition(key, value)`: Create a task to store key-value pair
- `update_db(ads, task_list, height)`: Execute tasks and update the database
- `get_value(ads, key)`: Retrieve value by key from the latest database state

### Storage Format

Arithmetic results are stored as:
- **Key**: String representation of the result value as bytes
- **Value**: 4-byte little-endian encoding of the result

### QMDB Configuration

- **Storage Directory**: `ADS/` (auto-created on first run)
- **Allocator**: Uses jemalloc on non-MSVC targets for better performance
- **Entry Size**: Uses QMDB's default entry size configuration
- **Sharding**: Automatic sharding based on key hash for load distribution

## Testing

The project includes comprehensive tests:
- Foundry tests for smart contracts (`contracts/test/`)
- Proof fixtures for both Groth16 and PLONK verification systems
- Execution validation in the main script

Tests use mock verification for faster execution and load proof fixtures from `contracts/src/fixtures/`.