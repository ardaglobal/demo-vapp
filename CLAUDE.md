# CLAUDE.md

## Project Overview

SP1 zero-knowledge proof project demonstrating arithmetic addition with indexed Merkle trees. Five main components:

1. **RISC-V Program** (`program/`): Arithmetic addition in SP1 zkVM
2. **Script** (`script/`): Proof generation using SP1 SDK and Sindri integration
3. **Smart Contracts** (`contracts/`): Solidity proof verification
4. **Database Module** (`db/`): PostgreSQL with indexed Merkle tree operations
5. **API Layer** (`db/src/api/`): REST and GraphQL APIs for tree operations

## Essential Commands

### Build & Setup
```bash
# First-time setup
cd program && cargo prove build --output-directory ../build

# Interactive execution (stores in PostgreSQL)
cd script && cargo run --release -- --execute
```

### Zero-Knowledge Proofs
```bash
# Generate proof (database-free)
cd script && cargo run --release -- --prove --a 5 --b 10

# Verify with proof ID (external)
cd script && cargo run --release -- --verify --proof-id <PROOF_ID> --result 15

# EVM proofs (Groth16/PLONK)
cd script && cargo run --release --bin evm -- --system groth16
cd script && cargo run --release --bin vkey
```

### Testing
```bash
# Smart contracts
cd contracts && forge test

# Database tests
cd db && cargo test

# All workspace tests
cargo test
```

### Database Setup
```bash
# Start PostgreSQL (Docker recommended)
docker-compose up -d

# Set environment variables
cp .env.example .env
# Set SINDRI_API_KEY for proof generation
```

## Architecture

### Core Components
- **arithmetic-lib** (`lib/`): Shared arithmetic computation logic
- **arithmetic-program** (`program/`): RISC-V program for zkVM (private inputs → public result)
- **arithmetic-script** (`script/`): Multiple binaries - `main.rs`, `evm.rs`, `vkey.rs`

### Zero-Knowledge Properties
```rust
struct PublicValuesStruct {
    int32 result;  // Only result is public - inputs remain private
}
```

**ZK Guarantees**: Privacy (inputs hidden), Soundness (proof correctness), Completeness (valid proofs always verify)

### Key Features
- **Database-Free Verification**: External users verify with proof ID + expected result
- **Sindri Integration**: Cloud proof generation with SP1 v5
- **32-Level Merkle Trees**: 8x fewer constraints than traditional 256-level trees
- **REST/GraphQL APIs**: Production-ready endpoints for tree operations

### Key Files
- `program/src/main.rs:25-28`: ZK public values (result only)
- `script/src/bin/main.rs`: Main CLI with Sindri integration
- `db/src/merkle_tree.rs`: 32-level indexed Merkle tree
- `db/src/api/`: REST and GraphQL APIs
- `contracts/src/Arithmetic.sol`: On-chain verification

## Environment

```bash
# Start PostgreSQL
docker-compose up -d

# Environment variables
cp .env.example .env
export SINDRI_API_KEY=your_api_key_here
```

## Database Architecture

**PostgreSQL Features**:
- 32-level indexed Merkle trees (8x fewer constraints than 256-level)
- 7-step insertion algorithm from transparency dictionaries paper
- ~200 ZK constraints per operation (vs ~1600 traditional)
- Atomic transactions with O(log n) operations

**Key Tables**: `arithmetic_transactions`, `nullifiers`, `merkle_nodes`, `tree_state`, `sindri_proofs`

**API Layer**: REST (`/api/v1/`) and GraphQL (`/graphql`) endpoints for tree operations

## Testing

**Prerequisites**: Running PostgreSQL instance for database tests

**Test Coverage**:
- Smart contracts (Foundry with proof fixtures)
- Database operations (unit, integration, performance)  
- Merkle tree operations (7-step insertion algorithm)
- API endpoints (REST/GraphQL)
- Error handling and edge cases

## API Layer

**REST Endpoints** (`/api/v1/`):
- `POST /nullifiers` - Insert nullifier
- `GET /nullifiers/{value}/membership` - Generate membership proof
- `GET /tree/stats` - Tree statistics and performance metrics

**GraphQL** (`/graphql`): Flexible queries, mutations, and real-time subscriptions

**Features**: Rate limiting, authentication, health checks, Prometheus metrics

## Key Features

- **Zero-Knowledge Proofs**: Private inputs (`a`, `b`) → public result only
- **External Verification**: Database-free proof verification with shareable proof IDs  
- **Sindri Integration**: Cloud proof generation with SP1 v5 support
- **32-Level Merkle Trees**: 8x constraint reduction vs traditional implementations
- **Production APIs**: REST/GraphQL with rate limiting and authentication
- **Comprehensive Testing**: End-to-end CI with automated ZK validation
