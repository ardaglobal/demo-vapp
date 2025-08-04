# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an SP1 (Succinct Proof) project that demonstrates zero-knowledge proof generation for arithmetic computation. The project consists of three main components:

1. **RISC-V Program** (`program/`): Performs arithmetic computations inside the SP1 zkVM
2. **Script** (`script/`): Generates proofs and handles execution using the SP1 SDK  
3. **Smart Contracts** (`contracts/`): Solidity contracts for on-chain proof verification

## Common Commands

### Building and Development
```bash
# First-time setup: compile the program to RISC-V
cd program && cargo prove build

# Execute program without generating proof
cd script && cargo run --release -- --execute

# Generate SP1 core proof
cd script && cargo run --release -- --prove

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

1. The zkVM program reads arithmetic inputs
2. Performs arithmetic computations using the shared library
3. Encodes results as `PublicValuesStruct` and commits to zkVM
4. The script generates proofs that can be verified on-chain via the Solidity contract

### Key Files

- `program/src/main.rs:14`: Main zkVM entry point with input/output handling
- `lib/src/lib.rs:13`: Core arithmetic computation logic
- `contracts/src/Arithmetic.sol:35`: On-chain proof verification function
- `script/src/bin/main.rs:35`: Proof generation orchestration

## Environment Configuration

Set up environment for prover network usage:
```bash
cp .env.example .env
# Set SP1_PROVER=network and NETWORK_PRIVATE_KEY in .env
```

## Testing

The project includes comprehensive tests:
- Foundry tests for smart contracts (`contracts/test/`)
- Proof fixtures for both Groth16 and PLONK verification systems
- Execution validation in the main script

Tests use mock verification for faster execution and load proof fixtures from `contracts/src/fixtures/`.

## Sequencer Behavior Proof Rules

When implementing a sequencer (transaction ordering system) using SP1, you need to prove correct FIFO behavior through several key invariants:

### Core Properties to Prove

1. **Ordering Preservation**: Transactions are output in the same order they were received
2. **Completeness**: All valid input transactions appear in the output 
3. **Integrity**: No transactions are modified during sequencing
4. **No Duplication**: Each transaction appears exactly once in the output
5. **Temporal Consistency**: Earlier timestamps come before later timestamps

### Rule Definitions for SP1 Implementation

**Input Commitments:**
- Array of incoming transactions with timestamps: `[(tx_1, t_1), (tx_2, t_2), ..., (tx_n, t_n)]`
- Each transaction includes: `{sender, recipient, amount, nonce, timestamp}`

**Output Commitments:**
- Array of sequenced transactions: `[tx_i1, tx_i2, ..., tx_in]` where `i1, i2, ..., in` is a permutation

**Proof Rules:**
1. **FIFO Constraint**: For any two transactions `tx_i`, `tx_j`, if `timestamp_i < timestamp_j`, then `position(tx_i) < position(tx_j)` in output
2. **Bijection Proof**: Input set equals output set (prove it's a valid permutation)
3. **Monotonic Timestamps**: `output[i].timestamp ≤ output[i+1].timestamp` for all valid indices
4. **Transaction Integrity**: Hash commitments of input transactions match output transactions

### Implementation Structure

```rust
// In your SP1 program
pub struct SequencerProof {
    input_transactions: Vec<Transaction>,
    output_sequence: Vec<Transaction>,
    input_commitment: [u8; 32],
    output_commitment: [u8; 32],
}

// Key constraints to verify in zkVM:
// 1. Verify input/output hash commitments match actual data
// 2. Prove output is valid permutation of input
// 3. Verify FIFO ordering based on timestamps
// 4. Check no transaction modification occurred
```

This approach ensures that the sequencer's behavior can be cryptographically verified without revealing the actual transaction contents.