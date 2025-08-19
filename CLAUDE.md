# CLAUDE.md

## Project Overview

SP1 zero-knowledge proof project demonstrating **batch processing with continuous balance tracking**. Features batched transactions with ZK privacy, indexed Merkle trees, and comprehensive state management. Clean architectural separation:

### 🏗️ **New Architecture (Post-Refactor)**

1. **RISC-V Program** (`program/`): Arithmetic addition in SP1 zkVM
2. **Local SP1 Testing** (`script/`): Fast unit testing with Core proofs (`cargo run -p demo-vapp`)
3. **Unified CLI** (`cli/`): HTTP client + local verification tool (no database dependencies)
4. **API Server** (`api/`): Proof generation and data distribution (verification removed)
5. **Database Module** (`db/`): PostgreSQL with indexed Merkle tree operations
6. **Smart Contracts** (`contracts/`): Solidity proof verification with state management
7. **Shared Library** (`lib/`): Pure computation logic (zkVM compatible)
1. **RISC-V Program** (`program/`): Batch transaction processing in SP1 zkVM (`initial_balance + [tx1, tx2, ...] = final_balance`)
2. **Local SP1 Testing** (`script/`): Fast unit testing with batch Core proofs (`cargo run -p demo-vapp`)
3. **Unified CLI** (`cli/`): Batch processing client for transactions, batch management, and local verification
4. **API Server** (`api/`): Batch creation, ZK proof generation via Sindri, contract data generation (public/private split)
5. **Database Module** (`db/`): PostgreSQL with batch processing tables, Merkle tree state management
6. **Smart Contracts** (`contracts/`): Solidity proof verification with state management
7. **Shared Library** (`lib/`): Pure batch computation logic (zkVM compatible)

### 🎯 **Clear Separation of Concerns**
- **`cargo run -p demo-vapp`** → Local SP1 batch proof development (3.5s Core proofs, tests `10 + [5, 7] → 22`)
- **CLI** → Batch processing client for submitting transactions, creating batches, and local verification
- **API Server** → Batch creation, ZK proof generation via Sindri, contract data with public/private split

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
# 🚀 Local SP1 batch processing testing (fast development)
cargo run -p demo-vapp --bin demo-vapp --release

# 🖥️ Batch Processing CLI (complete workflow)
cargo run --bin cli -- health-check
cargo run --bin cli -- submit-transaction --amount 5
cargo run --bin cli -- submit-transaction --amount 7
cargo run --bin cli -- view-pending
cargo run --bin cli -- trigger-batch --verbose
cargo run --bin cli -- get-current-state
cargo run --bin cli -- list-batches
cargo run --bin cli -- get-batch --batch-id 1
cargo run --bin cli -- download-proof --batch-id 1
cargo run --bin cli -- verify-proof --proof-file proof_batch_1.json --expected-initial-balance 0 --expected-final-balance 12

# 🌐 Local API server development (alternative to Docker)
docker-compose up postgres -d
cargo run -p api --bin server --release

# Manual program compilation (done automatically in Docker)
cd program && cargo prove build --output-directory ../build
```

### Zero-Knowledge Batch Proofs

#### Batch Proof Generation
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
# Prerequisites: Circuit must be deployed to Sindri first (see Quick Start)

# 🌐 Generate batch proof via API server (Groth16 default)
# 1. Submit transactions to batch queue
curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 5}'

# 🖥️ Generate proof via CLI client
cargo run --bin cli -- store-transaction --a 5 --b 10 --generate-proof
curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 7}'

# 2. Create batch (triggers ZK proof generation)
curl -X POST http://localhost:8080/api/v2/batches \
  -H 'Content-Type: application/json' \
  -d '{}'

# 🖥️ Generate batch proof via CLI client
cargo run --bin cli -- submit-transaction --amount 5
cargo run --bin cli -- submit-transaction --amount 7
cargo run --bin cli -- trigger-batch --verbose
```

#### Local Verification Workflow
```bash
# 1. Download batch proof data from API server
cargo run --bin cli -- download-proof --batch-id <BATCH_ID>

# 2. Verify batch proof locally (no server/database dependencies)
cargo run --bin cli -- verify-proof \
  --proof-file proof_batch_<BATCH_ID>.json \
  --expected-initial-balance 0 \
  --expected-final-balance 12 \
  --verbose

# Alternative: Direct hex data verification (advanced)
cargo run --bin cli -- verify-proof \
  --proof-data <hex_proof> \
  --public-values <hex_values> \
  --verifying-key <hex_vkey> \
  --expected-initial-balance 0 \
  --expected-final-balance 12
```

#### Circuit Management
```bash
sindri lint                           # Validate circuit
sindri list                           # Show deployed circuits
sindri deploy --tag "v1.0.0"         # Deploy with version tag
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
- `cargo run -p demo-vapp` → Fast SP1 batch proof testing (~3.5s Core proofs)
- Tests batch processing: `initial_balance=10 + [5, 7] → final_balance=22`
- Zero dependencies on database, Sindri, or production workflows
- Perfect for SP1 batch program development and quick verification

**🖥️ Unified CLI Path (`cli/`):**
- Batch processing client: submit transactions, create batches, manage state
- CLI commands: submit-transaction, view-pending, trigger-batch, list-batches, get-batch
- Local verification: verify-proof with batch support (no server dependencies)
- Contract data visualization: public/private split with verbose output
- Environment variable configuration: `ARITHMETIC_API_URL`

**🌐 Production API Path (`api/` + `db/`):**
- Batch creation and ZK proof generation server
- Sindri integration for batch proof generation
- Database operations with batch processing tables and Merkle tree state
- Contract data generation with public/private split
- REST endpoints focused on batch processing workflow

### Core Components
- **lib** (`lib/`): Shared batch computation logic for processing transaction arrays (zkVM compatible)
- **program** (`program/`): RISC-V program for zkVM batch processing (`initial_balance + [tx1, tx2, ...] → final_balance`)
- **demo-vapp** (`script/`): Local SP1 batch proof testing with fast Core proofs
- **cli** (`cli/`): Unified batch processing client + local verification tool
- **api** (`api/`): Batch creation and ZK proof generation server with Sindri integration
- **db** (`db/`): PostgreSQL with batch processing tables and indexed Merkle tree state management
- **state-management-system** (`contracts/src/interfaces/`): Complete state lifecycle management with batch proof verification

### Zero-Knowledge Properties
```rust
struct PublicValuesStruct {
    int32 initial_balance;  // Starting balance (public)
    int32 final_balance;    // Ending balance (public)
    // Individual transaction amounts [5, 7] remain private
}
```

**ZK Guarantees**:
- **Privacy**: Individual transaction amounts in batches remain hidden
- **Soundness**: Batch transitions are cryptographically proven correct
- **Completeness**: Valid batch proofs always verify
- **Batch Privacy**: Only balance transitions are public, not individual amounts

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
- **Local Verification**: Trustless proof verification without server dependencies
- **Unified CLI Tool**: Single binary for API interaction and local verification
- **Sindri Integration**: Cloud proof generation with SP1 v5
- **32-Level Merkle Trees**: 8x fewer constraints than traditional 256-level trees
- **REST/GraphQL APIs**: Production-ready endpoints for tree operations
- **Comprehensive State Management**: Complete state lifecycle with ZK proof verification
- **Batch Operations**: Gas-optimized batch state updates and reads

### Key Files
- `program/src/main.rs`: SP1 zkVM program (ZK public values: result only)
- `script/src/bin/main.rs`: Local SP1 unit testing (fast Core proofs)
- `cli/src/bin/cli.rs`: Unified CLI for API interaction and local verification
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

**Key Environment Variables:**
```bash
# Start PostgreSQL
docker-compose up -d

# Environment variables
cp .env.example .env
export SINDRI_API_KEY=your_api_key_here

# Smart contract integration (required for --submit-to-contract)
export ETHEREUM_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/your_api_key_here
export ETHEREUM_CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890
export ETHEREUM_WALLET_PRIVATE_KEY=your_private_key_without_0x_prefix
export ETHEREUM_DEPLOYER_ADDRESS=0x1234567890123456789012345678901234567890
# Required in .env file:
DATABASE_URL=postgresql://postgres:password@localhost:5432/arithmetic_db
SINDRI_API_KEY=your_sindri_api_key_here
SINDRI_CIRCUIT_TAG=latest
RUST_LOG=info
SERVER_PORT=8080
```

*Complete setup flow: See "Quick Start" section above.*

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

**REST Endpoints** (`/api/v2/`):
- `POST /transactions` - Submit individual transactions to batch queue
- `GET /transactions/pending` - View pending (unbatched) transactions
- `POST /batches` - Create batch from pending transactions
- `GET /batches` - List all historical batches
- `GET /batches/{id}` - Get specific batch details
- `POST /batches/{id}/proof` - Update batch with ZK proof
- `GET /state/current` - Get current counter state and Merkle root
- `GET /state/{batch_id}/contract` - Get contract data (public/private split)

**GraphQL** (`/graphql`): Flexible queries, mutations, and real-time subscriptions

**Features**: Rate limiting, authentication, health checks, Prometheus metrics

## REST API Server

**Server Binary** (`api/src/bin/server.rs`):

The project includes a comprehensive REST API server located in the `api/` directory that provides HTTP endpoints for external actors to interact with the vApp. The server integrates with the existing database, Merkle tree infrastructure, and Sindri proof generation.

### API Endpoints

**Transaction Operations**:
- `POST /api/v2/transactions` - Submit individual transactions to batch processing queue
- `GET /api/v2/transactions/pending` - View all pending (unbatched) transactions

**Batch Operations**:
- `POST /api/v2/batches` - Create batch from pending transactions and get contract data
- `GET /api/v2/batches` - List all historical batches
- `GET /api/v2/batches/{batch_id}` - Get specific batch details
- `POST /api/v2/batches/{batch_id}/proof` - Update batch with ZK proof from Sindri

**State Operations**:
- `GET /api/v2/state/current` - Get current counter state and Merkle root status
- `GET /api/v2/state/{batch_id}/contract` - Get contract submission data (public/private split)

**System Operations**:
- `GET /api/v2/health` - Health check and service status
- `GET /api/v2/info` - API information and capabilities

**GraphQL** (Optional):
- `POST /graphql` - GraphQL endpoint for complex queries
- `GET /playground` - Interactive GraphQL playground (development only)

### Usage Examples

```bash
# Start the API server
cargo run -p api --bin server --release

# Submit transactions to batch processing queue
curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 5}'

curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 7}'

# View pending (unbatched) transactions
curl http://localhost:8080/api/v2/transactions/pending

# Create batch from pending transactions (triggers ZK proof generation)
curl -X POST http://localhost:8080/api/v2/batches \
  -H 'Content-Type: application/json' \
  -d '{}'

# Get current counter state and Merkle root
curl http://localhost:8080/api/v2/state/current

# List all historical batches
curl http://localhost:8080/api/v2/batches

# Get specific batch details
curl http://localhost:8080/api/v2/batches/1

# Get contract submission data (public/private split)
curl http://localhost:8080/api/v2/state/1/contract

# Health check
curl http://localhost:8080/api/v2/health
```

### Server Configuration

The server supports various configuration options via command line arguments:
- `--host`: Bind host address (default: 0.0.0.0)
- `--port`: Bind port (default: 8080)
- `--cors`: Enable CORS (default: true)
- `--graphql`: Enable GraphQL endpoint (default: true)
- `--playground`: Enable GraphQL playground (default: true)
- `--log-level`: Log level (trace, debug, info, warn, error)

### Local Verification Workflow

External actors can verify batch proofs without server dependencies:
1. Submit transactions to batch queue: `POST /api/v2/transactions`
2. Create batch: `POST /api/v2/batches` (triggers ZK proof generation)
3. Receive batch ID in response
4. Download batch proof data: `CLI download-proof --batch-id <id>`
5. Verify locally using CLI: `cargo run --bin cli -- verify-proof --proof-file proof_batch_<id>.json --expected-initial-balance <initial> --expected-final-balance <final>`

This enables trustless verification where external parties can cryptographically verify batch transitions without seeing individual transaction amounts, requiring database access, or trusting the API server for verification.

**Key Privacy Feature**: The verification process proves balance transitions (e.g., `10 → 22`) without revealing individual transaction amounts (`[5, 7]`).

**Note**: Proof generation requires a valid `SINDRI_API_KEY` environment variable and deployed circuit. Without the API key, transactions will be stored successfully but proof generation will fail with a 401 Unauthorized error. Without circuit deployment, proof generation will fail with circuit not found errors. The REST API endpoints remain fully functional for transaction storage and retrieval.

## Deployment & Development Workflow

### Fresh Environment Setup

*See "Quick Start" section at the top for complete setup instructions.*

### Development Workflow
```bash
# Update batch processing circuit and deploy new version
./deploy-circuit.sh "dev-$(date +%s)"

# Update environment to use new circuit version
echo "SINDRI_CIRCUIT_TAG=dev-1234567890" >> .env

# Restart services to pick up new configuration
docker-compose restart server

# Test batch processing with new circuit version
curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 5}'

curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 7}'

curl -X POST http://localhost:8080/api/v2/batches \
  -H 'Content-Type: application/json' \
  -d '{}'
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
cargo run -p api --bin server --release

# Background processing is automatically enabled when the API server starts
# Configuration is handled via environment variables in .env file
```

**Legacy Commands** (moved to API server):
```bash
# Old: cd script && cargo run --release -- --execute --bg-interval 60 --bg-batch-size 50
# New: Background processing is integrated into the API server
```

**Database Tables:**
- `incoming_transactions`: Individual transactions submitted to batch processing queue
- `proof_batches`: Batch metadata with counter transitions and ZK proof references
- `ads_state_commits`: Merkle root commitments linked to batch state transitions

**Batch Processing Flow:**
1. Users submit individual transactions to `incoming_transactions` table
2. API endpoint triggers batch creation from unbatched transactions (FIFO order)
3. Batch is created in `proof_batches` table with previous and final counter values
4. ZK proof is generated via Sindri for the batch transition
5. Merkle root commitment is stored in `ads_state_commits` linked to the batch
6. Contract submission data is generated with public/private split for on-chain posting

## Key Features

### 🏗️ **Clean Architecture**
- **Separation of Concerns**: Local development, batch processing CLI, and ZK proof generation server as separate packages
- **Fast Local Testing**: SP1 batch Core proofs in ~3.5s for development workflows
- **Unified CLI Tool**: Complete batch processing workflow + local verification in a single binary
- **Focused API Server**: Batch creation, ZK proof generation, and contract data distribution

### 🚀 **Development Experience**
- **Automated Setup**: One-command dependency installation for all platforms (`./install-dependencies.sh`)
- **Docker Integration**: Full-stack deployment with automatic program compilation
- **Multiple Interaction Methods**: `cargo run`, CLI client, HTTP API, Docker deployment
- **Cross-Platform Support**: Works on macOS (Intel/Apple Silicon) and Linux (Ubuntu/Debian)

### 🔒 **Zero-Knowledge Features**
- **Batch Privacy**: Private transaction amounts (`[5, 7]`) → public balance transitions only (`10 → 22`)
- **Local Verification**: Trustless batch proof verification without server dependencies
- **Sindri Integration**: Cloud batch proof generation with SP1 v5 and configurable circuit versions
- **Circuit Management**: Configurable Sindri circuit deployment with version tagging
- **Contract Data Split**: Public information (Merkle roots, proofs) vs Private information (transaction amounts)

### 🌐 **Production Ready**
- **32-Level Merkle Trees**: 8x constraint reduction vs traditional implementations
- **Background Processing**: Asynchronous indexed Merkle tree construction with resume capability
- **Production APIs**: REST/GraphQL with rate limiting and authentication
- **State Management**: Complete state lifecycle management with proof verification and batch operations
- **Continuous Ledger State**: Global state counter with atomic transitions and audit trail
- **RESTful API Server**: HTTP API server for external transaction submission and proof verification
- **Smart Contract Integration**: Automated background posting of proven batches to Ethereum contracts

## Smart Contract Integration

### Overview

The project features **automated smart contract posting** for proven batches. After batches are created via the CLI and proven by Sindri, a background process automatically detects proven batches and posts state roots to the Ethereum smart contract. This provides a fully automated pipeline from batch creation to on-chain state updates.

### Automated Batch Posting Flow

**🔄 Complete Automation:**
1. **Batch Creation**: `cargo run --bin cli -- trigger-batch` creates batch with `posted_to_contract = FALSE`
2. **Proof Generation**: Background process generates ZK proof via Sindri
3. **Smart Contract Posting**: Background process detects proven batches and posts state roots to contract
4. **Status Tracking**: Batches marked as `posted_to_contract = TRUE` after successful submission

### Deployment Commands

```bash
# 1. Apply database migration (adds contract posting tracking)
cd /Users/horizon/Desktop/work/demo-vapp/db
sqlx migrate run

# 2. Start API server with automated background processing
cd /Users/horizon/Desktop/work/demo-vapp
cargo run -p api --bin server
```

### Background Process Features

- **Automatic Detection**: Scans for proven batches not yet posted to contract every 30 seconds
- **Smart Contract Submission**: Posts state roots using ethereum-client integration
- **Random State Roots**: Uses temporary 32-byte hashes until ADS integration is complete
- **Error Handling**: Graceful fallback if Ethereum client is not configured
- **Audit Trail**: Tracks posting timestamps and status in database
- **Rate Limiting**: Controlled submission rate to avoid overwhelming the network

### Usage Examples

```bash
# Submit transactions and trigger batch creation
cargo run --bin cli -- submit-transaction --amount 5
cargo run --bin cli -- submit-transaction --amount 7
cargo run --bin cli -- trigger-batch --verbose

# Background process automatically handles:
# - ZK proof generation via Sindri
# - Smart contract posting when proof is ready
# - Database status updates

# Check batch status
cargo run --bin cli -- list-batches
cargo run --bin cli -- get-batch --batch-id 1
```

### Environment Requirements

For smart contract posting to work, configure these environment variables:

- `ETHEREUM_RPC_URL` - Ethereum RPC endpoint (e.g., Alchemy, Infura)
- `ETHEREUM_CONTRACT_ADDRESS` - Address of deployed Arithmetic contract
- `ETHEREUM_WALLET_PRIVATE_KEY` - Private key for signing transactions (without 0x prefix)
- `ETHEREUM_DEPLOYER_ADDRESS` - Address that deployed the contract
- `SINDRI_API_KEY` - For ZK proof generation

### Database Schema Updates

New columns in `proof_batches` table:
- `posted_to_contract BOOLEAN DEFAULT FALSE` - Tracks posting status
- `posted_to_contract_at TIMESTAMP` - Audit trail for successful postings

### Error Handling & Monitoring

- **Graceful Fallback**: Background process continues if smart contract posting fails
- **Environment Validation**: Checks for required Ethereum configuration
- **Transaction Feedback**: Logs transaction hashes, gas usage, and confirmation details
- **Retry Logic**: Failed batches remain unposted for retry on next cycle
- **Status Tracking**: Complete audit trail of batch lifecycle in database
- **Comprehensive Testing**: End-to-end CI with automated ZK validation
- **32-Level Merkle Trees**: 8x constraint reduction vs traditional implementations
- **Batch Processing**: Efficient transaction batching with FIFO ordering and ZK proof generation
- **Production APIs**: REST endpoints with rate limiting and authentication
- **State Management**: Complete batch lifecycle management with proof verification
- **Continuous Balance Tracking**: Counter state transitions with atomic batch updates
- **Comprehensive Testing**: End-to-end CI with automated batch ZK validation
- **Development Tools**: Automated circuit linting, deployment, and management via Sindri CLI

### 🔐 **Local Verification Benefits**

The new local batch verification approach provides several key advantages:

**🛡️ Trustless Verification:**
- No need to trust the API server for batch proof verification
- Cryptographic verification happens entirely in your local environment
- External parties can verify batch transitions without any server dependencies

**🚀 Performance & Scalability:**
- API server no longer performs expensive cryptographic operations
- Batch verification can be done offline once proof data is downloaded
- Reduces server load and improves response times

**🔒 Security & Privacy:**
- All verification happens locally where you control the environment
- Batch privacy preserved: individual amounts never leave local verification
- Proof verification can be done in air-gapped environments

**🧹 Clean Architecture:**
- Clear separation: API server handles batch creation, CLI handles verification
- Unified CLI tool provides single entry point for all batch operations
- Simplified API server focused on batch processing and proof generation

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
