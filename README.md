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
make setup    # Install dependencies + copy .env
# Update .env file with needed env vars
make deploy   # Deploy circuit to Sindri
make up       # Start services
```

### 1. Install Dependencies
```sh
./install-dependencies.sh
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
# Deploy the circuit (uses 'latest' tag by default)
./deploy-circuit.sh

# Or deploy with a specific tag
./deploy-circuit.sh "dev-v1.0"

# Or set SINDRI_CIRCUIT_TAG in your .env
# SINDRI_CIRCUIT_TAG=dev-v1.0

# Or deploy manually:
# sindri lint
# sindri deploy                    # Uses 'latest' tag
# sindri deploy --tag "custom-tag" # Uses specific tag
```

**Note**: This step is required for proof generation. Without deploying the circuit, you can still run the server and submit transactions, but proof generation will fail.

### 4. Start the Full Stack
```sh
# Start database + API server (uses pre-built image from GitHub Container Registry)
docker-compose up -d

# Verify services are running
docker-compose ps

# Check server health
curl http://localhost:8080/api/v2/health
```

**For Local Development**: If you're actively developing and want to build the Docker image locally for faster iteration:
```sh
# Option 1: Use the development compose file
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

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

For fast local SP1 unit testing of batch processing during development:
```sh
# Quick SP1 batch processing test (generates Core proof in ~3.5 seconds)
# Tests: initial_balance=10 + [5, 7] → final_balance=22
cargo run --release

# Equivalent explicit command:
# cargo run --bin main --release
```

This provides a fast feedback loop for batch processing SP1 development without database or Sindri dependencies.
The local test proves that a batch of transactions `[5, 7]` correctly transitions the balance from `10` to `22` while keeping individual amounts private.

---

That's it! 🎉 You now have a running zero-knowledge arithmetic server with multiple interaction methods.

---

## Detailed Setup Instructions

**Note for Linux users**:
- After running the install script, you may need to log out and back in (or restart your terminal) for Docker group membership to take effect. You can verify Docker is working by running `docker --version` and `docker compose version`.
- The script installs OpenSSL development libraries (`libssl-dev`) required for Rust crates compilation.
- If you encounter OpenSSL-related compilation errors, ensure you have the latest packages: `sudo apt-get update && sudo apt-get install -y libssl-dev pkg-config`

**Installed Tools**: The script installs all necessary development tools including Rust toolchain, SP1, Foundry, Docker, Node.js, PostgreSQL client tools, sqlx-cli for database migrations, and other utilities.

## Proofs

To verify that results are stored in the database:

```sh
cd script
cargo run --release -- --verify
```

This will start an interactive CLI where you can:
- Enter a result value (e.g., 15)
- See what values of 'a' and 'b' were added to get that result
- Continue looking up different results until you press 'q' to quit

You can also verify a specific result non-interactively:

```sh
cargo run --release -- --verify --result 15
```

### Generate Zero-Knowledge Proofs via Sindri (Local Development)

**Note**: This section covers local SP1 testing in the `script/` directory. For production batch processing with automated smart contract posting, see the [Smart Contract Integration](#smart-contract-integration) section.

**All proofs are now EVM-compatible by default** using Sindri's cloud infrastructure:

```sh
cd script
# Generate Groth16 proof for specific values (default)
cargo run --release -- --prove --a 5 --b 10

# Generate PLONK proof for specific values
cargo run --release -- --prove --a 5 --b 10 --system plonk

# Generate proof for a previously computed result stored in database
cargo run --release -- --prove --result 15

# Generate proof with Solidity test fixtures and submit to contract
cargo run --release -- --prove --a 5 --b 10 --generate-fixture

# Generate proof only (skip smart contract submission)
cargo run --release -- --prove --a 5 --b 10 --skip-contract-submission
```

**Command Options:**
- `--system groth16|plonk`: Choose EVM-compatible proof system (default: groth16)
- `--generate-fixture`: Create Solidity test fixtures in `contracts/src/fixtures/`
- `--skip-contract-submission`: Skip automatic smart contract submission (smart contract submission is enabled by default)
- `--a` and `--b`: Direct input values for computation
- `--result`: Look up stored transaction inputs by result value

The `--prove` command will:
1. Create SP1 inputs and serialize them for Sindri
2. Generate EVM-compatible proofs (Groth16 or PLONK)
3. Submit proof request to Sindri using the `demo-vapp` circuit
4. Automatically submit proof to smart contract (unless `--skip-contract-submission` is used)
5. Store proof metadata in PostgreSQL (database mode) or run standalone
6. Display proof ID for external verification

### Verify Sindri Proofs

There are two ways to verify proofs generated via Sindri:

#### External Verification (Recommended for sharing proofs)

Use the proof ID printed during the prove flow:

```sh
cd script
# Verify using proof ID (no database required)
cargo run --release -- --verify --proof-id <PROOF_ID> --result <EXPECTED_RESULT>

# Example:
cargo run --release -- --verify --proof-id "proof_abc123def456" --result 15
```

This method:
- ✅ Works for external users without database access
- ✅ Only requires the proof ID and expected result
- ✅ Performs full cryptographic verification using Sindri's verification key
- ✅ Demonstrates true zero-knowledge properties

#### Database Verification (Internal use)

For internal use with database access:

```sh
cd script
# Interactive verification mode
cargo run --release -- --verify

# Verify specific result
cargo run --release -- --verify --result 15
```

This method:
1. Looks up the stored Sindri proof metadata by result
2. Queries Sindri's API to get the current proof status
3. Displays verification results and updates the stored status

### Generate EVM-Compatible Proofs via Sindri

All proofs generated through the main CLI are now EVM-compatible by default, using Sindri's cloud infrastructure. This provides scalable, production-ready proof generation without requiring local GPU resources.

To generate a Groth16 proof (default):

```sh
cd script
# Using specific inputs
cargo run --release -- --prove --a 5 --b 10 --system groth16

# Using database lookup by result
cargo run --release -- --prove --result 15 --system groth16
```

To generate a PLONK proof:

```sh
cd script
# Using specific inputs
cargo run --release -- --prove --a 5 --b 10 --system plonk

# Using database lookup by result
cargo run --release -- --prove --result 15 --system plonk
```

To generate Solidity test fixtures for on-chain verification:

```sh
cd script
# Generate proof with EVM fixture files
cargo run --release -- --prove --a 5 --b 10 --system groth16 --generate-fixture
```

These commands will:
1. Generate EVM-compatible proofs (Groth16/PLONK) via Sindri
2. Optionally create fixtures for Solidity contract testing (with `--generate-fixture`)
3. Provide proof IDs for external verification
4. Store proof metadata for later verification (database mode only)

### Retrieve the Verification Key

To retrieve your `programVKey` for your on-chain contract, run the following command in `script`:

```sh
cargo run --release -- --vkey
```

## Smart Contract Integration

The project features **automated smart contract posting** for proven batches. After batches are created and proven, a background process automatically detects proven batches and posts state roots to the Ethereum smart contract, providing a fully automated pipeline from batch creation to on-chain state updates.

### Automated Batch Posting Flow

**🔄 Complete End-to-End Automation:**
1. **Submit Transactions**: Use CLI to submit individual transactions
2. **Create Batch**: Trigger batch creation from pending transactions
3. **ZK Proof Generation**: Background process generates proof via Sindri
4. **Smart Contract Posting**: Background process automatically posts state roots to contract
5. **Status Tracking**: Database tracks posting status and timestamps

### Quick Deployment

```bash
# 1. Apply database migration (adds contract posting tracking)
cd db && sqlx migrate run

# 2. Start API server with automated background processing
cargo run -p api --bin server
```

### Usage Examples

```bash
# Submit transactions and create batch
cargo run --bin cli -- submit-transaction --amount 5
cargo run --bin cli -- submit-transaction --amount 7
cargo run --bin cli -- trigger-batch --verbose

# The background process automatically handles:
# ✅ ZK proof generation via Sindri
# ✅ Smart contract posting when proof is ready  
# ✅ Database status updates with timestamps

# Monitor batch progress
cargo run --bin cli -- list-batches
cargo run --bin cli -- get-batch --batch-id 1
```

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

## Sindri Integration for Serverless ZK Proofs

This project integrates with [Sindri](https://sindri.app) for serverless zero-knowledge proof generation, providing a scalable alternative to local SP1 proving.

### Setup

1. **Get your Sindri API key:**
   - Sign up at [sindri.app](https://sindri.app)
   - Create an API key from your account dashboard

2. **Set your API key as an environment variable:**
   ```bash
   export SINDRI_API_KEY=your_api_key_here
   ```
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

3. **For Smart Contract Integration (Optional):**
   ```bash
   # Required for --submit-to-contract flag
   export ETHEREUM_RPC_URL=https://eth-mainnet.alchemyapi.io/v2/demo
   export ETHEREUM_CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890
   export ETHEREUM_WALLET_PRIVATE_KEY=your_private_key_without_0x_prefix
   export ETHEREUM_DEPLOYER_ADDRESS=0x1234567890123456789012345678901234567890
   ```

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

**Note**: `curl` is installed by the dependency script and ready for API testing.

```sh
# Submit transactions to batch processing queue
curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 5}'

curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 7}'

# View pending transactions
curl http://localhost:8080/api/v2/transactions/pending

# Create batch and get contract submission data (public/private split)
curl -X POST http://localhost:8080/api/v2/batches \
  -H 'Content-Type: application/json' \
  -d '{}'

# Get current counter state and Merkle root
curl http://localhost:8080/api/v2/state/current

# List all batches
curl http://localhost:8080/api/v2/batches

# Get specific batch details
curl http://localhost:8080/api/v2/batches/1

# Get contract submission data for batch
curl http://localhost:8080/api/v2/state/1/contract

# Health check
curl http://localhost:8080/api/v2/health
```

### Local Verification Workflow

The system provides a clean separation between batch proof generation (via the API server) and proof verification (done locally):

#### 1. Submit Transactions and Create Batch
```sh
# Submit transactions to batch queue
curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 5}'

curl -X POST http://localhost:8080/api/v2/transactions \
  -H 'Content-Type: application/json' \
  -d '{"amount": 7}'

# Create batch (triggers ZK proof generation)
curl -X POST http://localhost:8080/api/v2/batches \
  -H 'Content-Type: application/json' \
  -d '{}'

# Response includes batch_id for later verification
```

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
