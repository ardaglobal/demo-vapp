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

# Background processing (integrated with execute mode)
cd script && cargo run --release -- --execute --bg-interval 60 --bg-batch-size 50

# REST API Server
# Prerequisites: DATABASE_URL environment variable must be set
# Note: Database migrations are applied automatically on startup
cd db && cargo run --bin server --release -- --host 0.0.0.0 --port 8080 --cors --graphql --playground
```

### Zero-Knowledge Proofs
```bash
# Generate EVM-compatible proof and submit to smart contract (Groth16 default)
cd script && cargo run --release -- --prove --a 5 --b 10

# Generate PLONK proof and submit to smart contract
cd script && cargo run --release -- --prove --a 5 --b 10 --system plonk

# Generate proof with Solidity fixtures and submit to contract
cd script && cargo run --release -- --prove --a 5 --b 10 --generate-fixture

# Generate proof with fixtures and submit to contract
cd script && cargo run --release -- --prove --a 7 --b 8 --generate-fixture

# Generate proof only (skip smart contract submission)
cd script && cargo run --release -- --prove --a 5 --b 10 --skip-contract-submission

# Verify with proof ID (external)
cd script && cargo run --release -- --verify --proof-id <PROOF_ID> --result 15

# Get verification key
cd script && cargo run --release -- --vkey
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
- Asynchronous background processor (integrated with `--execute` mode)
- Periodically reads new arithmetic transactions from database
- Converts transactions to nullifiers and builds indexed Merkle tree
- Configurable polling intervals and batch processing
- No impact on user CLI performance

### Core Components
- **arithmetic-lib** (`lib/`): Shared arithmetic computation logic
- **arithmetic-program** (`program/`): RISC-V program for zkVM (private inputs → public result)
- **arithmetic-script** (`script/`): Single unified binary (`main.rs`) with EVM-compatible proving via Sindri
- **background-processor** (`db/src/background_processor.rs`): Asynchronous Merkle tree construction
- **state-management-system** (`contracts/src/interfaces/`): Complete state lifecycle management with proof verification

### Zero-Knowledge Properties
```rust
struct PublicValuesStruct {
    int32 result;  // Only result is public - inputs remain private
}
```

**ZK Guarantees**: Privacy (inputs hidden), Soundness (proof correctness), Completeness (valid proofs always verify)

### Zero-Knowledge Verification Mental Model

Understanding the verification flow through analogy:

**1. In Digital Signing:**
- *Private key:* Can only sign messages
- *Public key:* Can only verify signatures
- The only "computation" being proven is "I signed this message"

**2. In ZK Proving:**
- *Proving key:* Can only generate proofs for a specific compiled program (circuit) with specific public inputs and some private witness
- *Verification key:* Can only verify proofs for that exact program, using the same public inputs
- The "computation" being proven is whatever the compiled program defines — e.g., "I took oldRoot and a private batch of transactions, applied the rules, and got newRoot"

**3. Key Difference from Normal Signatures:**
- In signatures, the message can be arbitrary; the private key doesn't "know" or "care" about what's inside, it just signs bytes
- In ZK, the PK/VK pair encodes the program itself — the rules for what constitutes a valid computation
- Change the program → you must regenerate both PK and VK

**4. Why Both PK and VK Contain the "Same Compiled Program Steps":**
When you do the "setup" for a circuit (trusted or transparent), the compiler:
- Turns your high-level program into a low-level constraint system (R1CS, AIR, etc.)
- Generates a proving key containing all the extra metadata needed to construct a proof from a witness
- Generates a verification key containing the compressed commitments needed to check that a proof corresponds to that exact constraint system
- Because they are derived from the same constraints, PK and VK are inseparable as a pair — a VK from one circuit can't verify proofs from another

**5. In Your vApp Case:**
- *PK* = off-chain, owned by your prover (Arda sequencer/prover cluster)
- *VK* = on-chain, baked into the global settlement contract for that namespace
- *Proof* = ephemeral artifact generated per batch, posted with public inputs
- *Verification* = anyone with VK + proof + public inputs can check correctness — no need for the PK or the private data

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

# Smart contract integration (required for --submit-to-contract)
export ETHEREUM_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/demo
export ARITHMETIC_CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890
export VERIFIER_CONTRACT_ADDRESS=0x0987654321098765432109876543210987654321
export PRIVATE_KEY=your_private_key_without_0x_prefix
```

## Database Architecture

**PostgreSQL Features**:
- 32-level indexed Merkle trees (8x fewer constraints than 256-level)
- 7-step insertion algorithm from transparency dictionaries paper
- ~200 ZK constraints per operation (vs ~1600 traditional)
- Atomic transactions with O(log n) operations

**Key Tables**: `arithmetic_transactions`, `nullifiers`, `merkle_nodes`, `tree_state`, `sindri_proofs`, `global_state`, `state_transitions`

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

**State Operations**:
- `GET /api/v1/state` - Get current global state counter
- `GET /api/v1/state/history` - State transition history with audit trail
- `GET /api/v1/state/validate` - State integrity validation across all transactions

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

# Submit a transaction with proof generation (continuous state)
curl -X POST http://localhost:8080/api/v1/transactions \
  -H 'Content-Type: application/json' \
  -d '{"a": 5, "b": 10, "generate_proof": true}'

# Get current global state
curl http://localhost:8080/api/v1/state

# Get state transition history
curl http://localhost:8080/api/v1/state/history

# Validate state integrity
curl http://localhost:8080/api/v1/state/validate

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

**Configuration Options** (integrated with `--execute` mode):
```bash
# Run execute mode with custom background processing settings
cd script && cargo run --release -- --execute --bg-interval 60 --bg-batch-size 50

# One-shot processing (exit after processing current batch)
cd script && cargo run --release -- --execute --bg-one-shot

# Default background processing with execute mode
cd script && cargo run --release -- --execute
```

**Database Tables:**
- `arithmetic_transactions`: Hot path storage for user inputs
- `nullifiers`: Indexed Merkle tree nodes (cold path output)
- `processor_state`: Tracks last processed transaction ID for resume capability
- `global_state`: Continuous ledger state tracking
- `state_transitions`: Audit trail for all state changes

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
- **Continuous Ledger State**: Global state counter with atomic transitions and audit trail
- **RESTful API Server**: HTTP API server for external transaction submission and proof verification
- **Smart Contract Integration**: Automatic proof submission to Ethereum contracts with `--submit-to-contract` flag

## Smart Contract Integration

### Overview

The project now includes seamless integration between Sindri proof generation and Ethereum smart contract submission. After generating a zero-knowledge proof via Sindri, the system can automatically submit the proof to the on-chain Arithmetic contract for verification and state updates.

### Features

- **Automatic Submission**: Smart contract submission is enabled by default for all `--prove` commands
- **SP1 Proof Extraction**: Automatically extracts SP1 proof data and verification keys from Sindri responses
- **Ethereum Client**: Integrated ethereum client for contract interaction with signing capability
- **State Management**: Generates deterministic state IDs and state roots for proof organization
- **Transaction Feedback**: Provides detailed transaction hashes, block numbers, and gas usage

### Usage Examples

```bash
# Generate proof and submit to smart contract (default behavior)
cargo run --release -- --prove --a 5 --b 10

# Generate proof with result lookup and submit to contract  
cargo run --release -- --prove --result 15

# Generate proof, create EVM fixture, and submit to contract
cargo run --release -- --prove --a 7 --b 8 --generate-fixture

# Generate proof only (skip smart contract submission)
cargo run --release -- --prove --a 5 --b 10 --skip-contract-submission
```

### Environment Requirements

For smart contract integration to work, the following environment variables must be configured:

- `ETHEREUM_RPC_URL` - Ethereum RPC endpoint (e.g., Alchemy, Infura)
- `ARITHMETIC_CONTRACT_ADDRESS` - Address of deployed Arithmetic contract
- `VERIFIER_CONTRACT_ADDRESS` - Address of SP1 verifier contract  
- `PRIVATE_KEY` - Private key for signing transactions (without 0x prefix)
- `SINDRI_API_KEY` - For proof generation

### Integration Flow

1. **Proof Generation**: `prove_via_sindri_core()` generates `ProofInfoResponse` 
2. **Data Extraction**: Extract SP1 proof and verification key using `.to_sp1_proof_with_public()` and `.get_sp1_verifying_key()`
3. **Client Initialization**: Create ethereum client from environment configuration
4. **State Generation**: Generate deterministic state IDs and state roots based on arithmetic result
5. **Contract Submission**: Submit proof to Arithmetic contract via `update_state()` method
6. **Transaction Confirmation**: Wait for transaction confirmation and provide feedback

### Error Handling

- **Graceful Fallback**: Proof generation continues even if contract submission fails
- **Environment Validation**: Checks for required environment variables before attempting submission  
- **Signer Validation**: Ensures ethereum client has signing capability for transactions
- **Detailed Error Messages**: Provides specific error messages for troubleshooting

## State Management System

### Overview

The state management system provides both on-chain (smart contracts) and off-chain (database) solutions for storing, reading, and validating zero-knowledge proof-verified state transitions. Built on top of SP1 arithmetic proof verification, it offers enterprise-grade state management with gas optimization, continuous ledger functionality, and security best practices.

### Continuous Ledger State

**Database-Level State Management**:
- **Global State Counter**: Maintains running total across all transactions (`global_state` table)
- **Continuous Ledger**: Each transaction builds on previous state: `previous_state + result = new_state`
- **Atomic Transitions**: PostgreSQL functions ensure consistency and prevent race conditions
- **Audit Trail**: Complete history of all state transitions (`state_transitions` table)
- **State Integrity**: Built-in validation ensures mathematical consistency across all transactions

**State Progression Example**:
```
Initial state: 0
Transaction 1: 0 + 15 = 15 (inputs: 7 + 8)
Transaction 2: 15 + 25 = 40 (inputs: 12 + 13)  
Transaction 3: 40 + 10 = 50 (inputs: 3 + 7)
```

**Enhanced Transaction Response**:
```json
{
  "previous_state": 40,
  "new_state": 50,
  "state_info": {
    "state_updated": true,
    "continuous_ledger": true,
    "state_description": "State transition: 40 + 10 = 50"
  }
}
```

**State API Endpoints**:
- `GET /api/v1/state` - Current global state counter
- `GET /api/v1/state/history` - Full state transition audit trail
- `GET /api/v1/state/validate` - State integrity validation

### Smart Contract Components

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


### Smart Contract Features

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
