# Arda Demo vApp

A batch processing arithmetic demo demonstrating the [vApp Architecture](https://arxiv.org/pdf/2504.14809)

Based off the template for creating an end-to-end [SP1](https://github.com/succinctlabs/sp1) project
that can generate Zero-Knowledge proofs for batched transactions with continuous balance tracking.

## Architecture Overview

This project demonstrates **batch processing with Zero-Knowledge proofs** for continuous balance tracking:

### Core Concept
- **Continuous Balance**: An internal counter is continuously updated by user transactions
- **Batch Processing**: Transactions are grouped into batches (FIFO) and proven together
- **Zero-Knowledge Privacy**: Individual transaction amounts remain private in batches
- **Authenticated Data Structure**: Merkle roots link counter states to cryptographic commitments

### Example Flow
```
Initial balance: 10
User submits: +5, +7 → Batched together
ZK Proof: "Balance went from 10 to 22" (without revealing +5, +7)
New balance: 22
```

### Clean Architecture
- **`script/`** - Local SP1 unit testing for batch proofs (`cargo run` for fast development)
- **`cli/`** - Batch processing client for submitting transactions and managing batches
- **`api/`** - Production server with batch creation and ZK proof generation via Sindri
- **`db/`** - PostgreSQL with batch processing tables and Merkle tree state management
- **`lib/`** - Pure computation logic for processing transaction batches (zkVM compatible)
- **`program/`** - RISC-V program for proving batch transitions: `initial_balance + [tx1, tx2, ...] = final_balance`

## Requirements

- [Rust](https://rustup.rs/)
- [SP1](https://docs.succinct.xyz/docs/sp1/getting-started/install)
- [Foundry](https://book.getfoundry.sh/getting-started/installation) (for smart contracts)
- [Docker](https://docs.docker.com/get-docker/) (for database)
- [Node.js](https://nodejs.org/) (for Sindri CLI)

## Quick Start (Zero to Running Server)

**💡 Pro Tip**: Use the included `Makefile` for even simpler commands:
```sh
make setup    # Install dependencies + copy .env + initialize database
# Update .env file with needed env vars
make deploy   # Deploy circuit to Sindri
make up       # Start services
```

**Database Initialization**: If you just need to set up the database for offline development:
```sh
make initDB   # Start DB, run migrations, generate SQLx cache, then stop DB
```

## Running the vApp

### 1. Install Dependencies
```sh
make setup
# This calls ./install-dependencies.sh
```

### 2. Set Environment Variables
```sh
cp .env.example .env
# Edit .env and add your Sindri API key for proof generation and circuit deployment:
# Get your API key from https://sindri.app
# SINDRI_API_KEY=your_sindri_api_key_here
```

### 3. Deploy Circuit to Sindri (Required for Proof Generation)
```sh
# Set the SINDRI_CIRCUIT_TAG in the .env file
make deploy
# This calls ./deploy-circuit.sh
```

**Note**: This step is required for proof generation. Without deploying the circuit, you can still run the server and submit transactions, but proof generation will fail.

### 4. Start the Full Stack
```sh
# Start database + API server (uses pre-built image from GitHub Container Registry)
make up

# Verify services are running
docker-compose ps

# Check server health
curl http://localhost:8080/api/v2/health
```

**For Local Development**: If you're actively developing and want to build the Docker image locally for faster iteration:
```sh
# Option 1: Use the development compose file, this will re-build the server dockerfile
make up-dev

# Option 2: Run API server locally (requires PostgreSQL running)
docker-compose up postgres -d
cargo run --bin server --release
```

### 5. Test the Batch Processing API

**Option A: Direct HTTP API**
```sh
# Submit individual transactions to batch queue
curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 5}'

curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 7}'

# View pending (unbatched) transactions
curl http://localhost:8080/api/v2/transactions/pending

# Trigger batch creation and get contract data (public/private split)
curl -X POST http://localhost:8080/api/v2/batches \
  -H 'Content-Type: application/json' \
  -d '{}'

# Get current counter state
curl http://localhost:8080/api/v2/state/current
```

**Option B: CLI Client (Recommended)**

> **💡 Note**: The CLI now supports the complete batch processing workflow with ZK proof verification.

```sh
# Check API server health
cargo run --bin cli -- health-check

# Submit transactions to the batch queue
cargo run --bin cli -- submit-transaction --amount 5
cargo run --bin cli -- submit-transaction --amount 7

# View all pending transactions
cargo run --bin cli -- view-pending

# Trigger batch creation with verbose contract data
cargo run --bin cli -- trigger-batch --verbose

# Get current counter state and Merkle root
cargo run --bin cli -- get-current-state

# List all historical batches
cargo run --bin cli -- list-batches

# Get specific batch details
cargo run --bin cli -- get-batch --batch-id 1

# Download proof data for local verification (when ready)
cargo run --bin cli -- download-proof --batch-id 1

# Verify proof locally using the downloaded JSON file
cargo run --bin cli -- verify-proof \
  --proof-file proof_batch_1.json \
  --expected-initial-balance 0 \
  --expected-final-balance 12
```

### 6. Local SP1 Development

For fast local SP1 unit testing during zkVM program development:

```sh
# Quick SP1 batch processing test from root directory
cargo run --release

# This tests: initial_balance=10 + [5, 7] → final_balance=22
# Generates Core proof in ~8 seconds without database dependencies
```

This provides a fast feedback loop for SP1 development, testing that a batch of transactions `[5, 7]` correctly transitions the balance from `10` to `22` while keeping individual amounts private.

---

That's it! 🎉 You now have a running zero-knowledge arithmetic server with multiple interaction methods.

---

## Detailed Setup Instructions

**Note for Linux users**:
- After running the install script, you may need to log out and back in (or restart your terminal) for Docker group membership to take effect. You can verify Docker is working by running `docker --version` and `docker compose version`.
- The script installs OpenSSL development libraries (`libssl-dev`) required for Rust crates compilation.
- If you encounter OpenSSL-related compilation errors, ensure you have the latest packages: `sudo apt-get update && sudo apt-get install -y libssl-dev pkg-config`

**Installed Tools**: The script installs all necessary development tools including Rust toolchain, SP1, Foundry, Docker, Node.js, PostgreSQL client tools, sqlx-cli for database migrations, and other utilities.

## Offline Development (No Database Required)

For developers who want to run `cargo check` or work on non-database code without starting PostgreSQL, this project supports **SQLx offline mode**:

### Quick Start - Offline Mode
```sh
# Set offline mode and run cargo check
export SQLX_OFFLINE=true
cargo check --workspace

# This works without any database connection!
```

### How It Works
The project includes pre-generated SQLx query cache files (`.sqlx/` directory) that contain compile-time query metadata. This allows SQLx macros to validate SQL queries during compilation without connecting to a live database.

### When to Regenerate the Cache
You need to regenerate the SQLx cache when:
- Database schema changes (new tables, columns, migrations)
- SQL queries in the code are modified
- You encounter compilation errors about missing query metadata

### Regenerating the Cache
```sh
# Recommended way: Use the make command (fully automated)
make initDB

# Manual way:
docker-compose up postgres -d
cd db && sqlx migrate run && cd ..
cargo sqlx prepare --workspace
```

The cache files in `.sqlx/` are committed to version control, so most developers won't need to regenerate them unless they're working on database-related changes.

### Benefits
- ✅ **Fast compilation** - No database connection required for `cargo check`
- ✅ **CI/CD friendly** - Works in environments without PostgreSQL
- ✅ **Developer productivity** - Work on application logic without database setup
- ✅ **Offline development** - Code and compile anywhere

## Makefile Commands

The project includes a comprehensive `Makefile` with convenient shortcuts for common tasks:

### Setup & Installation
- `make help` - Show all available commands with descriptions
- `make install` - Install all dependencies via `./install-dependencies.sh`
- `make env` - Copy `.env.example` to `.env`
- `make setup` - Complete setup: install dependencies, copy .env, initialize database
- `make initDB` - Initialize database (start, migrate, generate SQLx cache, stop)

### Development
- `make run` - Local SP1 unit testing (~8s Core proofs, no database needed)
- `make cli ARGS="..."` - Run CLI client (e.g., `make cli ARGS="health-check"`)
- `make server` - Start API server locally (requires database)
- `make test` - Run all tests
- `make forge-build` - Build smart contracts
- `make forge-test` - Run smart contract tests

### Services
- `make up` - Start services using pre-built Docker image
- `make up-dev` - Start services with local Docker build
- `make down` - Stop all services
- `make deploy` - Deploy circuit to Sindri
- `make deploy-contract` - Deploy Arithmetic smart contract
- `make deploy-contract-help` - Show smart contract deployment help

### Docker Operations
- `make docker-build` - Build Docker image locally
- `make docker-push` - Build and push image to GitHub registry

### Cleanup
- `make clean-docker` - Clean up Docker resources
- `make clean-sqlx` - Remove `.sqlx/` cache directory
- `make clean-builds` - Remove `target/`, `build/`, `ADS/` directories
- `make clean` - Clean up all resources (docker, sqlx, builds)

### Usage Examples
```sh
# Complete setup from scratch
make setup

# Quick SP1 test
make run

# Start production services
make up

# CLI examples
make cli ARGS="submit-transaction --amount 5"
make cli ARGS="list-batches"
make cli ARGS="health-check"
```

## Production Batch Processing & Proof Verification

For full batch processing with database and ZK proof generation:

### 1. Start the Full Stack
```sh
# Start database + API server
make up

# Verify services
curl http://localhost:8080/api/v2/health
```

### 2. Submit Transactions and Create Batches
```sh
# Submit individual transactions
cargo run --bin cli -- submit-transaction --amount 5
cargo run --bin cli -- submit-transaction --amount 7

# View pending transactions
cargo run --bin cli -- view-pending

# Create batch and trigger ZK proof generation
cargo run --bin cli -- trigger-batch --verbose

# Monitor batch status
cargo run --bin cli -- list-batches
cargo run --bin cli -- get-batch --batch-id 1
```

### 3. Download and Verify Batch Proofs
```sh
# Download proof data when ready
cargo run --bin cli -- download-proof --batch-id 1

# Verify locally (no network dependencies)
cargo run --bin cli -- verify-proof \
  --proof-file proof_batch_1.json \
  --expected-initial-balance 0 \
  --expected-final-balance 12
```

### Benefits of Batch Proof Verification
- **Privacy**: Individual transaction amounts `[5, 7]` remain hidden
- **Correctness**: Balance transition cryptographically verified
- **Trustless**: External parties can verify without database access
- **Offline**: Works without network once proof data is downloaded

## Smart Contract Integration

The project features **automated smart contract posting** for proven batches. After batches are created and proven, a background process automatically detects proven batches and posts state roots to the Ethereum smart contract, providing a fully automated pipeline from batch creation to on-chain state updates.

### Automated Batch Posting Flow

**🔄 Complete End-to-End Automation:**
1. **Submit Transactions**: Use CLI to submit individual transactions
2. **Create Batch**: Trigger batch creation from pending transactions
3. **ZK Proof Generation**: Background process generates proof via Sindri
4. **Smart Contract Posting**: Background process automatically posts state roots to contract
5. **Status Tracking**: Database tracks posting status and timestamps

### Usage Examples

The CLI workflow is the same as described in [Production Batch Processing](#production-batch-processing--proof-verification). The background process automatically handles:
- ✅ ZK proof generation via Sindri
- ✅ Smart contract posting when proof is ready
- ✅ Database status updates with timestamps

### Environment Setup

For automated smart contract posting, configure these environment variables:

```bash
# Required for smart contract integration
export ETHEREUM_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/demo
export ETHEREUM_CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890
export ETHEREUM_WALLET_PRIVATE_KEY=your_private_key_without_0x_prefix
export ETHEREUM_DEPLOYER_ADDRESS=0x1234567890123456789012345678901234567890
export SINDRI_API_KEY=your_sindri_api_key_here
```

### Background Process Features

- **Automatic Detection**: Scans for proven batches every 30 seconds
- **Smart Contract Submission**: Posts state roots using ethereum-client
- **Random State Roots**: Uses 32-byte hashes (temporary until ADS integration)
- **Error Handling**: Graceful fallback if Ethereum client not configured
- **Rate Limiting**: Controlled submission rate to avoid network congestion
- **Audit Trail**: Complete database tracking of posting status and timestamps

### Database Schema

New tracking columns in `proof_batches`:
```sql
posted_to_contract BOOLEAN DEFAULT FALSE  -- Tracks if batch posted to contract
posted_to_contract_at TIMESTAMP           -- Timestamp of successful posting
```

### Benefits

- **Zero Manual Intervention**: Fully automated pipeline from CLI to blockchain
- **Fault Tolerance**: Background process handles retries and error recovery
- **Audit Trail**: Complete database tracking of batch lifecycle
- **Scalable**: Handles multiple batches concurrently with rate limiting
- **Production Ready**: Comprehensive error handling and monitoring

## Smart Contract Deployment

Deploy the Arithmetic smart contract to Ethereum-compatible networks using the included Makefile commands.

### Prerequisites

Before deploying, you'll need:
- An Ethereum RPC endpoint (e.g., Alchemy, Infura, or local node)
- A wallet private key for deployment
- The SP1 verifier contract address for your target network
- Your program verification key (generated during circuit compilation)

### Deployment Commands

```bash
# 1. Set required environment variables
export ETHEREUM_RPC_URL="https://eth-mainnet.g.alchemy.com/v2/your-api-key"
export ETHEREUM_WALLET_PRIVATE_KEY="your-private-key-without-0x-prefix"
export VERIFIER_CONTRACT_ADDRESS="0x1234567890123456789012345678901234567890"
export PROGRAM_VKEY="0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab"

# 2. Deploy the contract
make deploy-contract
```

**Alternative using .env file:**
```bash
# Add the variables to your .env file, then:
export $(cat .env | grep -v '^#' | xargs) && make deploy-contract
```

**Get help with deployment:**
```bash
make deploy-contract-help
```

### Additional Smart Contract Commands

```bash
# Build smart contracts
make forge-build

# Run smart contract tests
make forge-test
```

### Example Output

When deployment succeeds, you'll see output similar to:
```
✅ All environment variables are set

🚀 Deploying contract...
📡 RPC URL: https://eth-mainnet.g.alchemy.com/v2/your-api-key
🔑 Verifier: 0x1234567890123456789012345678901234567890
🗝️  Program VKey: 0xabcdef...

[⠊] Compiling...
[⠘] Deploying Arithmetic on eth...
✅ Hash: 0xabc123...
Contract Address: 0xdef456...
✅ Contract deployed successfully!
```

Save the contract address for use in your application's environment configuration.

## Sindri Integration for Serverless ZK Proofs

This project integrates with [Sindri](https://sindri.app) for serverless zero-knowledge proof generation, providing a scalable alternative to local SP1 proving.

### Setup

For Sindri API key setup, see the [Environment Variables](#2-set-environment-variables) section in Quick Start.
The proof generation process:
1. Creates SP1 inputs and serializes them for Sindri
2. Generates EVM-compatible proofs (Groth16 by default)
3. Submits proof request to Sindri using the `demo-vapp` circuit
4. Stores proof metadata in PostgreSQL
5. Returns proof ID for external verification
## Batch Processing Proofs

The batch proof generation process:
1. Collects pending transactions into a batch (FIFO order)
2. Creates SP1 inputs: `initial_balance` and `transactions: Vec<i32>`
3. Generates EVM-compatible batch proofs (Groth16 by default) via Sindri
4. Submits proof request to Sindri using the `demo-vapp` circuit
5. Stores batch metadata and proof ID in PostgreSQL
6. Associates Merkle root with the proven state transition
7. Returns batch ID and contract submission data (public/private split)

For Smart Contract Integration environment variables, see the [Environment Setup](#environment-setup) section under Smart Contract Integration.

### Continuous Integration

The project includes a comprehensive GitHub Actions workflow (`.github/workflows/sindri.yml`) that:

1. **Lints** the circuit using Sindri CLI
2. **Builds** the SP1 program with the current branch/PR
3. **Deploys** the circuit to Sindri with a unique tag
4. **Generates** a zero-knowledge proof (7 + 13 = 20)
5. **Verifies** the proof using external verification (no database required)

**Branch Tagging Strategy:**
- **Main branch**: `main-<commit-sha>`
- **Pull requests**: `pr-<number>-<branch-name>`

This ensures each deployment is uniquely tagged and traceable to the source code.

### What This Proves

The zero-knowledge batch proofs demonstrate that:
- You know the individual transaction amounts in a batch (e.g., `[5, 7]`)
- The batch correctly transitions the balance (e.g., `10 → 22`)
- The computation was performed correctly according to the batching rules
- **Privacy**: Individual transaction amounts remain hidden from public view
- **Integrity**: The balance transition is cryptographically proven without revealing private data
- **Authenticity**: No one can forge this proof without knowing the actual transaction batch

### Batch Privacy Example
```
Public: "Balance went from 10 to 22" + ZK Proof + Merkle Root
Private: Individual amounts [5, 7] (never revealed on-chain)
```

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

### Benefits of Using Sindri

- **Serverless Proving:** No need to set up SP1 proving infrastructure
- **Scalable:** Generate multiple proofs in parallel
- **Optimized:** Sindri's infrastructure is optimized for proof generation
- **Verified:** Server-side verification ensures proof correctness
- **Production Ready:** Suitable for production ZK applications

## REST API Server

The API server will start on `http://localhost:8080` by default.

### API Endpoints

**Transaction Operations:**
- `POST /api/v2/transactions` - Submit individual transactions to batch processing queue
- `GET /api/v2/transactions/pending` - View all pending (unbatched) transactions

**Batch Operations:**
- `POST /api/v2/batches` - Create batch from pending transactions and get contract data
- `GET /api/v2/batches` - List all historical batches
- `GET /api/v2/batches/{batch_id}` - Get specific batch details
- `POST /api/v2/batches/{batch_id}/proof` - Update batch with ZK proof from Sindri

**State Operations:**
- `GET /api/v2/state/current` - Get current counter state and Merkle root status
- `GET /api/v2/state/{batch_id}/contract` - Get contract submission data (public/private split)

**System Operations:**
- `GET /api/v2/health` - Health check and service status
- `GET /api/v2/info` - API information and capabilities

### Usage Examples

For complete usage examples with curl commands, see the [Quick Start](#quick-start-zero-to-running-server) section's "Option A: Direct HTTP API".

### Local Verification Workflow

The system provides a clean separation between batch proof generation (via the API server) and proof verification (done locally):

#### 1. Submit Transactions and Create Batch
Use the CLI or HTTP API as described in the [Quick Start](#quick-start-zero-to-running-server) section to submit transactions and create batches. The response includes `batch_id` for later verification.

#### 2. Download Batch Proof Data
```sh
# Get raw proof data for local verification (when proof is ready)
cargo run --bin cli -- download-proof --batch-id 1

# Downloads proof_batch_1.json containing:
# - proof_data: hex-encoded SP1 proof
# - public_values: hex-encoded public values
# - verifying_key: hex-encoded verification key
# - initial_balance and final_balance for verification
```

#### 3. Verify Locally (No Network Dependencies)
```sh
# Run local verification tool with downloaded batch proof
cargo run --bin cli -- verify-proof \
  --proof-file proof_batch_1.json \
  --expected-initial-balance 0 \
  --expected-final-balance 12 \
  --verbose

# Output:
# ✅ Balance validation PASSED (0 → 12)
# ✅ Structure validation PASSED
# 🎉 Batch proof structure successfully verified!
#     • Privacy: Individual transaction amounts [5, 7] remain hidden
#     • Correctness: Balance transition verified
```

### Benefits of Local Verification

- **No Docker Dependencies:** Verification runs on any machine with Rust
- **Trustless:** No need to trust the API server for verification
- **Privacy:** All verification happens locally
- **Offline:** Works without network access once proof data is downloaded
- **Portable:** Verification can be done on any machine or integrated into other systems

This enables trustless verification where external parties can cryptographically verify computation results without seeing private inputs, requiring database access, or trusting external services.
