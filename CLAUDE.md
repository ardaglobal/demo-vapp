# CLAUDE.md

## Project Overview

SP1 zero-knowledge proof project demonstrating arithmetic addition with indexed Merkle trees and comprehensive state management. Seven main components:

1. **RISC-V Program** (`program/`): Arithmetic addition in SP1 zkVM
2. **Script** (`script/`): Proof generation using SP1 SDK and Sindri integration
3. **Smart Contracts** (`contracts/`): Solidity proof verification with state management
4. **Database Module** (`db/`): PostgreSQL with indexed Merkle tree operations
5. **API Layer** (`db/src/api/`): REST and GraphQL APIs for tree operations
6. **Background Processor** (`db/src/background_processor.rs`): Asynchronous indexed Merkle tree construction
7. **State Management System** (`contracts/src/interfaces/`): Complete state lifecycle management with ZK proof verification

## Essential Commands

### Build & Setup
```bash
# First-time setup
cd program && cargo prove build --output-directory ../build

# Interactive execution (hot path: stores in PostgreSQL)
cd script && cargo run --release -- --execute

# Background processing (cold path: builds indexed Merkle tree)
cd script && cargo run --bin background

# REST API Server
# Prerequisites: DATABASE_URL environment variable must be set
# Note: Database migrations are applied automatically on startup
cd db && cargo run --bin server --release -- --host 0.0.0.0 --port 8080 --cors --graphql --playground
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
# Smart contracts (includes state management tests)
cd contracts && forge test

# Run specific state management tests
cd contracts && forge test --match-contract StateManagementTest

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
- **state-management-system** (`contracts/src/interfaces/`): Complete state lifecycle management with proof verification

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
- **Comprehensive State Management**: Complete state lifecycle with ZK proof verification
- **Batch Operations**: Gas-optimized batch state updates and reads

### Key Files
- `program/src/main.rs:25-28`: ZK public values (result only)
- `script/src/bin/main.rs`: Main CLI with Sindri integration
- `db/src/merkle_tree.rs`: 32-level indexed Merkle tree
- `db/src/api/`: REST and GraphQL APIs
- `contracts/src/Arithmetic.sol`: On-chain verification with state management
- `contracts/src/interfaces/IStateManager.sol`: State management interface
- `contracts/test/StateManagement.t.sol`: Comprehensive state management tests

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
- State management system (comprehensive test suite)
- Database operations (unit, integration, performance)  
- Merkle tree operations (7-step insertion algorithm)
- API endpoints (REST/GraphQL)
- Gas optimization and batch operations

## API Layer

**REST Endpoints** (`/api/v1/`):
- `POST /nullifiers` - Insert nullifier
- `GET /nullifiers/{value}/membership` - Generate membership proof
- `GET /tree/stats` - Tree statistics and performance metrics

**GraphQL** (`/graphql`): Flexible queries, mutations, and real-time subscriptions

**Features**: Rate limiting, authentication, health checks, Prometheus metrics

## REST API Server

**Server Binary** (`db/src/bin/server.rs`):

The project includes a comprehensive REST API server that provides HTTP endpoints for external actors to interact with the vApp. The server integrates with the existing database, Merkle tree infrastructure, and Sindri proof generation.

### API Endpoints

**Transaction Operations**:
- `POST /api/v1/transactions` - Submit new transactions (a + b), optionally generate ZK proofs
- `GET /api/v1/results/{result}` - Query transaction inputs (a,b) by result value
- `POST /api/v1/results/{result}/verify` - Verify stored proof for a specific result

**Proof Operations**:
- `GET /api/v1/proofs/{proof_id}` - Retrieve proof information by Sindri proof ID
- `POST /api/v1/verify` - Verify proof independently with proof ID and expected result

**System Operations**:
- `GET /api/v1/health` - Health check and service status
- `GET /api/v1/info` - API information and capabilities
- `GET /api/v1/tree/stats` - Merkle tree statistics and performance metrics

**GraphQL** (Optional):
- `POST /graphql` - GraphQL endpoint for complex queries
- `GET /playground` - Interactive GraphQL playground (development only)

### Usage Examples

```bash
# Start the server
cd db && cargo run --bin server --release

# Submit a transaction with proof generation
curl -X POST http://localhost:8080/api/v1/transactions \
  -H 'Content-Type: application/json' \
  -d '{"a": 5, "b": 10, "generate_proof": true}'

# Query transaction by result
curl http://localhost:8080/api/v1/results/15

# Verify proof for result
curl -X POST http://localhost:8080/api/v1/results/15/verify

# Get proof information
curl http://localhost:8080/api/v1/proofs/proof_abc123

# Health check
curl http://localhost:8080/api/v1/health
```

### Server Configuration

The server supports various configuration options via command line arguments:
- `--host`: Bind host address (default: 0.0.0.0)
- `--port`: Bind port (default: 8080)
- `--cors`: Enable CORS (default: true)
- `--graphql`: Enable GraphQL endpoint (default: true)
- `--playground`: Enable GraphQL playground (default: true)
- `--log-level`: Log level (trace, debug, info, warn, error)

### External Verification

External actors can verify proofs without access to the database:
1. Submit transaction with `generate_proof: true`
2. Receive proof ID in response
3. Share proof ID with external verifiers
4. External verifiers use proof verification endpoints or CLI tools

This enables trustless verification where external parties can cryptographically verify computation results without seeing private inputs or requiring database access.

**Note**: Proof generation requires a valid `SINDRI_API_KEY` environment variable. Without it, transactions will be stored successfully but proof generation will fail with a 401 Unauthorized error. The REST API endpoints remain fully functional for transaction storage and retrieval.

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
- **State Management**: Complete state lifecycle management with proof verification and batch operations
- **RESTful API Server**: HTTP API server for external transaction submission and proof verification

## State Management System

### Overview

The state management system provides a comprehensive solution for storing, reading, and validating zero-knowledge proof-verified state transitions on-chain. Built on top of the SP1 arithmetic proof verification, it offers enterprise-grade state management with gas optimization and security best practices.

### Core Components

**IStateManager Interface** (`contracts/src/interfaces/IStateManager.sol`):
- Standardized interface for state management operations
- Core state functions: `updateState()`, `getCurrentState()`, `getStoredProof()`, `getStoredResult()`
- Batch operations: `batchUpdateStates()`, `batchReadStates()`
- Proof management: `getProofById()`, `isProofVerified()`, `getVerificationResult()`

**Arithmetic Contract** (`contracts/src/Arithmetic.sol`):
- Implements IStateManager interface
- SP1 proof verification with state storage
- Access control and authorization system
- Event system for monitoring and analytics
- Proof metadata and enumeration capabilities


### Key Features

**Gas Optimization**:
- Batch operations for multiple state updates/reads
- Local caching to reduce contract calls
- Optimized storage patterns
- Gas cost estimates in documentation

**Security**:
- Comprehensive access control system
- Proof validation before state updates
- Parameter validation and sanitization
- Reentrancy protection patterns

**Monitoring & Analytics**:
- Detailed event system for all operations
- Usage statistics and performance metrics
- Error tracking and diagnostics
- Integration with monitoring tools

### State Management Commands

```bash
# Deploy state management contracts
cd contracts && forge script script/DeployStateManager.s.sol --broadcast

# Run state management tests
cd contracts && forge test --match-contract StateManagementTest


# Run gas optimization tests
cd contracts && forge test --match-test test_Gas
```

### Usage Patterns

**Single State Update**:
```solidity
// Direct update through Arithmetic contract
arithmetic.postStateUpdate(stateId, newState, proof, result);
```

**Batch Operations**:
```solidity
// Batch state updates (gas efficient)
bool[] memory successes = arithmetic.batchUpdateStates(
    stateIds, newStates, proofs, results
);

// Batch state reads
bytes32[] memory states = arithmetic.batchReadStates(stateIds);
```

**Safe State Reading**:
```solidity
// Direct from Arithmetic contract
bytes32 currentState = arithmetic.getCurrentState(stateId);
```

**Proof Verification**:
```solidity
// Check proof verification status
bool isVerified = arithmetic.isProofVerified(proofId);

// Get proof with verification result
(bool verified, bytes memory result) = arithmetic.getVerificationResult(proofId);
```

### Gas Cost Estimates

**State Operations**:
- Single state update: ~200,000 - 400,000 gas
- Batch update (10 states): ~2,000,000 - 3,000,000 gas
- Single state read: ~5,000 - 25,000 gas
- Batch read (10 states): ~50,000 - 150,000 gas

**Proof Operations**:
- Proof storage: ~50,000 - 100,000 gas
- Proof reading: ~10,000 - 50,000 gas
- Verification check: ~2,000 - 5,000 gas

### Integration Best Practices

1. **Always use batch operations** when processing multiple states
2. **Validate inputs** before submitting to state manager
3. **Monitor gas usage** and optimize storage patterns
4. **Implement proper access control** for state updates
5. **Use events for monitoring** and analytics

### Error Handling

The system provides comprehensive error handling:
- Custom error types for gas optimization
- Detailed error messages for debugging
- Graceful failure handling in batch operations
- Event-based error reporting and monitoring

### Security Considerations

- **Access Control**: Multi-layered authorization system
- **Proof Validation**: Comprehensive proof verification before state updates  
- **Parameter Validation**: Input sanitization and bounds checking
- **Reentrancy Protection**: Safe external call patterns
- **State Consistency**: Validation of state transitions
