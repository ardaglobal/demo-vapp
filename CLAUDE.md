# CLAUDE.md

## Project Overview

SP1 zero-knowledge proof project demonstrating arithmetic addition with indexed Merkle trees. Six main components:

1. **RISC-V Program** (`program/`): Arithmetic addition in SP1 zkVM
2. **Script** (`script/`): Proof generation using SP1 SDK and Sindri integration
3. **Smart Contracts** (`contracts/`): Solidity proof verification
4. **Database Module** (`db/`): PostgreSQL with indexed Merkle tree operations
5. **API Layer** (`db/src/api/`): REST and GraphQL APIs for tree operations
6. **Background Processor** (`db/src/background_processor.rs`): Asynchronous indexed Merkle tree construction

## Essential Commands

### Build & Setup
```bash
# First-time setup
cd program && cargo prove build --output-directory ../build

# Interactive execution (hot path: stores in PostgreSQL)
cd script && cargo run --release -- --execute

# Background processing (cold path: builds indexed Merkle tree)
cd script && cargo run --bin background
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

### Hot and Cold Path Design

**Hot Path (CLI Performance):**
- User input via `cargo run --release -- --execute` → immediate storage in PostgreSQL
- Zero database-to-tree dependencies during user interaction
- Fast, responsive CLI experience without Merkle tree overhead

**Cold Path (Background Processing):**
- Asynchronous background processor (`cargo run --bin background`)
- Periodically reads new arithmetic transactions from database
- Converts transactions to nullifiers and builds indexed Merkle tree
- Configurable polling intervals and batch processing
- No impact on user CLI performance

### Core Components
- **arithmetic-lib** (`lib/`): Shared arithmetic computation logic
- **arithmetic-program** (`program/`): RISC-V program for zkVM (private inputs → public result)
- **arithmetic-script** (`script/`): Multiple binaries - `main.rs`, `evm.rs`, `vkey.rs`, `background.rs`
- **background-processor** (`db/src/background_processor.rs`): Asynchronous Merkle tree construction

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

## Background Processing

**Configuration Options** (`script/src/bin/background.rs`):
```bash
# Run with custom settings
cargo run --bin background -- --interval 60 --batch-size 50

# One-shot processing (exit after processing current batch)
cargo run --bin background -- --one-shot

# Custom logging
cargo run --bin background -- --log-level debug
```

**Database Tables:**
- `arithmetic_transactions`: Hot path storage for user inputs
- `nullifiers`: Indexed Merkle tree nodes (cold path output)
- `processor_state`: Tracks last processed transaction ID for resume capability

**Processing Flow:**
1. Poll `arithmetic_transactions` for new entries since last processed ID
2. Convert transaction data to deterministic nullifier values using hash function
3. Insert nullifiers into indexed Merkle tree using 7-step algorithm
4. Update `processor_state` with last processed transaction ID
5. Repeat based on polling interval

## Key Features

- **Hot/Cold Path Separation**: CLI performance isolated from Merkle tree operations
- **Zero-Knowledge Proofs**: Private inputs (`a`, `b`) → public result only
- **External Verification**: Database-free proof verification with shareable proof IDs  
- **Sindri Integration**: Cloud proof generation with SP1 v5 support
- **32-Level Merkle Trees**: 8x constraint reduction vs traditional implementations
- **Background Processing**: Asynchronous indexed Merkle tree construction with resume capability
- **Production APIs**: REST/GraphQL with rate limiting and authentication
- **Comprehensive Testing**: End-to-end CI with automated ZK validation
