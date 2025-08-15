# CLAUDE.md

## Project Overview

SP1 zero-knowledge proof project demonstrating arithmetic addition with indexed Merkle trees and comprehensive state management. Clean architectural separation:

### 🏗️ **New Architecture (Post-Refactor)**

1. **RISC-V Program** (`program/`): Arithmetic addition in SP1 zkVM
2. **Local SP1 Testing** (`script/`): Fast unit testing with Core proofs (`cargo run`)  
3. **CLI Client** (`cli/`): Simple HTTP client for API server interaction
4. **API Server** (`api/`): Production web server with Sindri integration and complex workflows
5. **Database Module** (`db/`): PostgreSQL with indexed Merkle tree operations
6. **Smart Contracts** (`contracts/`): Solidity proof verification with state management
7. **Shared Library** (`lib/`): Pure computation logic (zkVM compatible)

### 🎯 **Clear Separation of Concerns**
- **`cargo run`** → Local SP1 development (3.5s Core proofs)
- **CLI** → Simple API client (no interactive modes, no database)  
- **API Server** → Production system (Sindri, database, complex workflows)

## Essential Commands

### Quick Start (Zero to Running Server)
```bash
# 1. Install all dependencies (Rust, SP1, Foundry, Docker, Node.js, Sindri CLI, etc.)
./install-dependencies.sh

# 2. Set environment variables
cp .env.example .env
# Edit .env and add: SINDRI_API_KEY=your_sindri_api_key_here

# 3. Deploy circuit to Sindri (required for proof generation)
export SINDRI_API_KEY=your_sindri_api_key_here
./deploy-circuit.sh                    # Uses 'latest' tag
# ./deploy-circuit.sh "custom-tag"     # Or use specific tag

# 4. Start full stack (database + server) - uses pre-built image
docker-compose up -d

# For local development (builds locally):
# docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

# 5. Test the API
curl http://localhost:8080/api/v1/health
```

### Development Commands
```bash
# 🚀 Local SP1 unit testing (fast development)
cargo run --package arithmetic-program-builder --bin local-sp1-test --release

# 🖥️ CLI client (simple API interaction) 
cargo run --package arithmetic-cli --bin arithmetic -- health-check
cargo run --package arithmetic-cli --bin arithmetic -- store-transaction --a 5 --b 10
cargo run --package arithmetic-cli --bin arithmetic -- get-transaction --result 15

# 🌐 Local API server development (alternative to Docker)
docker-compose up postgres -d
cargo run --package arithmetic-api --bin server --release

# Manual program compilation (done automatically in Docker)
cd program && cargo prove build --output-directory ../build
```

### Zero-Knowledge Proofs
```bash
# Prerequisites: Circuit must be deployed to Sindri first
# ./deploy-circuit.sh

# 🌐 Generate proof via API server (Groth16 default)
curl -X POST http://localhost:8080/api/v1/transactions \
  -H 'Content-Type: application/json' \
  -d '{"a": 5, "b": 10, "generate_proof": true}'

# 🖥️ Generate proof via CLI client  
cargo run --package arithmetic-cli --bin arithmetic -- store-transaction --a 5 --b 10

# Verify with proof ID (external - no database required)
curl -X POST http://localhost:8080/api/v1/verify \
  -H 'Content-Type: application/json' \
  -d '{"proof_id": "<PROOF_ID>", "expected_result": 15}'

# Get verification key
curl http://localhost:8080/api/v1/vkey

# Circuit management
sindri lint                           # Validate circuit
sindri deploy                         # Deploy with 'latest' tag
sindri deploy --tag "custom-tag"      # Deploy with specific tag
sindri list                           # Show deployed circuits

# ⚠️ Legacy Commands (moved to API server):
# Old: cd script && cargo run --release -- --prove --a 5 --b 10
# New: Use API server endpoints or CLI client
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

### Docker Compose Setup
```bash
# Full stack (recommended) - uses pre-built image from GitHub Container Registry
docker-compose up -d                  # Start database + server
docker-compose ps                     # Verify services
docker-compose logs server -f         # View server logs
docker-compose down                   # Stop services
docker-compose down -v                # Stop + remove data

# Local development (builds image locally for faster iteration)
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

# Database only (for local development)
docker-compose up postgres -d         # Start only PostgreSQL

# Service URLs
# - Database: localhost:5432
# - REST API: http://localhost:8080
# - GraphQL: http://localhost:8080/graphql
# - Health: http://localhost:8080/api/v1/health
```

### Environment Configuration
```bash
# Copy and configure environment file
cp .env.example .env

# Required variables:
# DATABASE_URL=postgresql://postgres:password@localhost:5432/arithmetic_db
# SINDRI_API_KEY=your_sindri_api_key_here        # For proof generation
# SINDRI_CIRCUIT_TAG=latest                      # Circuit version (default: latest)
# RUST_LOG=info                                  # Logging level
# SERVER_PORT=8080                               # API server port
```

## Architecture

### 🏗️ **Clean Separation Architecture**

**🚀 Local Development Path (`script/`):**
- `cargo run` → Fast SP1 unit testing (~3.5s Core proofs)
- Zero dependencies on database, Sindri, or production workflows
- Perfect for SP1 program development and quick verification

**🖥️ CLI Client Path (`cli/`):**
- Simple HTTP client for API server interaction
- Basic commands: health-check, store-transaction, get-transaction  
- No interactive modes, no direct database access
- Environment variable configuration: `ARITHMETIC_API_URL`

**🌐 Production API Path (`api/` + `db/`):**
- Full-featured API server with complex workflows
- Sindri integration for proof generation
- Database operations and indexed Merkle trees
- Background processing and state management
- GraphQL and REST endpoints

### Core Components
- **arithmetic-lib** (`lib/`): Shared arithmetic computation logic (zkVM compatible)
- **arithmetic-program** (`program/`): RISC-V program for zkVM (private inputs → public result)
- **arithmetic-program-builder** (`script/`): Local SP1 unit testing with fast Core proofs
- **arithmetic-cli** (`cli/`): Simple HTTP client for API interaction
- **arithmetic-api** (`api/`): Production web server with Sindri integration
- **arithmetic-db** (`db/`): PostgreSQL with indexed Merkle tree operations
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
- `program/src/main.rs`: SP1 zkVM program (ZK public values: result only)
- `script/src/bin/main.rs`: Local SP1 unit testing (fast Core proofs) 
- `cli/src/bin/arithmetic.rs`: Simple CLI client for API interaction
- `api/src/bin/server.rs`: Production API server with Sindri integration
- `api/src/client/mod.rs`: HTTP client library for API interaction
- `db/src/merkle_tree.rs`: 32-level indexed Merkle tree
- `lib/src/lib.rs`: Shared computation logic (zkVM compatible)
- `contracts/src/Arithmetic.sol`: On-chain verification with state management
- `contracts/src/interfaces/IStateManager.sol`: State management interface
- `contracts/test/StateManagement.t.sol`: Comprehensive state management tests
- `install-dependencies.sh`: Automated dependency installation for all platforms
- `deploy-circuit.sh`: Sindri circuit deployment with configurable tags
- `docker-compose.yml`: Full-stack container orchestration
- `Dockerfile`: Multi-stage build with SP1 program compilation
- `.env.example`: Environment variable template with all required settings

## Environment

```bash
# Complete setup flow
./install-dependencies.sh             # Install all dependencies
cp .env.example .env                   # Copy environment template
export SINDRI_API_KEY=your_api_key_here
./deploy-circuit.sh                    # Deploy circuit to Sindri
docker-compose up -d                   # Start full stack

# Environment variables in .env file:
# DATABASE_URL=postgresql://postgres:password@localhost:5432/arithmetic_db
# SINDRI_API_KEY=your_sindri_api_key_here
# SINDRI_CIRCUIT_TAG=latest
# RUST_LOG=info
# SERVER_PORT=8080
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

**Server Binary** (`api/src/bin/server.rs`):

The project includes a comprehensive REST API server located in the `api/` directory that provides HTTP endpoints for external actors to interact with the vApp. The server integrates with the existing database, Merkle tree infrastructure, and Sindri proof generation.

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
# Start the API server
cargo run --package arithmetic-api --bin server --release

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

**Note**: Proof generation requires a valid `SINDRI_API_KEY` environment variable and deployed circuit. Without the API key, transactions will be stored successfully but proof generation will fail with a 401 Unauthorized error. Without circuit deployment, proof generation will fail with circuit not found errors. The REST API endpoints remain fully functional for transaction storage and retrieval.

## Deployment & Development Workflow

### Fresh Environment Setup
```bash
# 1. Install dependencies (one-time setup)
./install-dependencies.sh

# 2. Configure environment
cp .env.example .env
# Edit .env: Add SINDRI_API_KEY and configure SINDRI_CIRCUIT_TAG if needed

# 3. Deploy circuit to Sindri
export SINDRI_API_KEY=your_api_key_here
./deploy-circuit.sh                    # Default: latest tag
# ./deploy-circuit.sh "dev-v1.0"       # Custom tag

# 4. Start services
docker-compose up -d

# 5. Verify deployment
curl http://localhost:8080/api/v1/health
```

### Development Workflow
```bash
# Update circuit and deploy new version
./deploy-circuit.sh "dev-$(date +%s)"

# Update environment to use new circuit version
echo "SINDRI_CIRCUIT_TAG=dev-1234567890" >> .env

# Restart services to pick up new configuration
docker-compose restart server

# Test with new circuit version
curl -X POST http://localhost:8080/api/v1/transactions \
  -H 'Content-Type: application/json' \
  -d '{"a": 5, "b": 10, "generate_proof": true}'
```

### Troubleshooting
```bash
# Check Sindri circuit deployment
sindri list                            # Show deployed circuits
sindri lint                            # Validate circuit configuration

# Check Docker services
docker-compose ps                      # Service status
docker-compose logs server -f          # Server logs
docker-compose logs postgres -f        # Database logs

# Check environment configuration
cat .env                               # Show environment variables
echo $SINDRI_API_KEY                   # Verify API key

# Database connectivity
pg_isready -h localhost -p 5432 -U postgres
sqlx migrate info                      # Check migration status
```

## Background Processing

**⚠️ Background processing is now integrated into the API server:**

```bash
# Start API server with background processing enabled
cargo run --package arithmetic-api --bin server --release

# Background processing is automatically enabled when the API server starts
# Configuration is handled via environment variables in .env file
```

**Legacy Commands** (moved to API server):
```bash
# Old: cd script && cargo run --release -- --execute --bg-interval 60 --bg-batch-size 50
# New: Background processing is integrated into the API server
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

### 🏗️ **Clean Architecture**
- **Separation of Concerns**: Local development, CLI client, and production server as separate packages
- **Fast Local Testing**: SP1 Core proofs in ~3.5s for development workflows
- **Simple CLI Client**: HTTP-based interaction without complex dependencies
- **Production API Server**: Full-featured server with all complex logic

### 🚀 **Development Experience** 
- **Automated Setup**: One-command dependency installation for all platforms (`./install-dependencies.sh`)
- **Docker Integration**: Full-stack deployment with automatic program compilation
- **Multiple Interaction Methods**: `cargo run`, CLI client, HTTP API, Docker deployment
- **Cross-Platform Support**: Works on macOS (Intel/Apple Silicon) and Linux (Ubuntu/Debian)

### 🔒 **Zero-Knowledge Features**
- **Zero-Knowledge Proofs**: Private inputs (`a`, `b`) → public result only
- **External Verification**: Database-free proof verification with shareable proof IDs  
- **Sindri Integration**: Cloud proof generation with SP1 v5 and configurable circuit versions
- **Circuit Management**: Configurable Sindri circuit deployment with version tagging

### 🌐 **Production Ready**
- **32-Level Merkle Trees**: 8x constraint reduction vs traditional implementations
- **Background Processing**: Asynchronous indexed Merkle tree construction with resume capability
- **Production APIs**: REST/GraphQL with rate limiting and authentication
- **State Management**: Complete state lifecycle management with proof verification and batch operations
- **Continuous Ledger State**: Global state counter with atomic transitions and audit trail
- **Comprehensive Testing**: End-to-end CI with automated ZK validation
- **Development Tools**: Automated circuit linting, deployment, and management via Sindri CLI

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
