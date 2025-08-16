# Arda Demo vApp

A simple arithmetic demo demonstrating the [vApp Architecture](https://arxiv.org/pdf/2504.14809)

Based off the template for creating an end-to-end [SP1](https://github.com/succinctlabs/sp1) project
that can generate a proof of any RISC-V program.

## Architecture Overview

This project features a clean separation of concerns:

- **`script/`** - Local SP1 unit testing (`cargo run` for fast development)
- **`cli/`** - Simple HTTP client for API server interaction
- **`api/`** - Production web server with complex logic and Sindri integration
- **`db/`** - Database layer with PostgreSQL and indexed Merkle trees
- **`lib/`** - Pure computation logic (zkVM compatible)
- **`program/`** - RISC-V program source for SP1 zkVM

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
curl http://localhost:8080/api/v1/health
```

**For Local Development**: If you're actively developing and want to build the Docker image locally for faster iteration:
```sh
# Option 1: Use the development compose file
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

# Option 2: Run API server locally (requires PostgreSQL running)
docker-compose up postgres -d
cargo run --bin server --release
```

### 5. Test the API

**Option A: Direct HTTP API**
```sh
# Submit a transaction
curl -X POST http://localhost:8080/api/v1/transactions \
  -H 'Content-Type: application/json' \
  -d '{"a": 7, "b": 13, "generate_proof": false}'

# Generate a proof via Sindri (requires SINDRI_API_KEY in .env)
curl -X POST http://localhost:8080/api/v1/transactions \
  -H 'Content-Type: application/json' \
  -d '{"a": 5, "b": 10, "generate_proof": true}'
```

**Option B: CLI Client (Recommended)**

> **💡 Note**: Since binary names are unique across packages, you can use `cargo run --bin <binary>` from the workspace root. Only use `-p <package>` if you encounter naming conflicts or want to be explicit.

```sh
# Check API server health
cargo run --bin cli -- health-check

# Store a transaction via CLI
cargo run --bin cli -- store-transaction --a 5 --b 10

# Query a transaction by result
cargo run --bin cli -- get-transaction --result 15
```

### 6. Local SP1 Development

For fast local SP1 unit testing during development:
```sh
# Quick SP1 unit test (generates Core proof in ~3.5 seconds)
# This runs the default binary (main) from the script package
cargo run --release

# Equivalent explicit command:
# cargo run --bin main --release
```

This provides a fast feedback loop for SP1 development without database or Sindri dependencies.

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

The proof generation process:
1. Creates SP1 inputs and serializes them for Sindri
2. Generates EVM-compatible proofs (Groth16 by default)
3. Submits proof request to Sindri using the `demo-vapp` circuit
4. Stores proof metadata in PostgreSQL  
5. Returns proof ID for external verification

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

The zero-knowledge proofs demonstrate that:
- You know two secret numbers (a and b)
- Their sum equals the public result
- The computation was performed correctly
- No one can forge this proof without knowing the actual computation

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
- `POST /api/v1/transactions` - Submit new transactions (a + b), optionally generate ZK proofs
- `GET /api/v1/results/{result}` - Query transaction inputs by result value
- `GET /api/v1/results/{result}/verify` - Verify stored proof for a specific result

**Proof Operations:**
- `GET /api/v1/proofs/{proof_id}` - Retrieve proof information by Sindri proof ID
- `POST /api/v1/verify` - Verify proof independently with proof ID and expected result

**System Operations:**
- `GET /api/v1/health` - Health check and service status
- `GET /api/v1/info` - API information and capabilities

### Usage Examples

**Note**: `curl` is installed by the dependency script and ready for API testing.

```sh
# Submit a transaction with proof generation
curl -X POST http://localhost:8080/api/v1/transactions \
  -H 'Content-Type: application/json' \
  -d '{"a": 5, "b": 10, "generate_proof": true}'

# Query transaction by result
curl http://localhost:8080/api/v1/results/15

# Verify proof for result
curl http://localhost:8080/api/v1/results/15/verify

# Health check
curl http://localhost:8080/api/v1/health
```

### External Actor Workflow

1. **Submit Transaction:** External actors POST to `/api/v1/transactions`
2. **Get Proof ID:** Response includes proof ID and verification command
3. **Share Proof:** Proof ID can be shared for trustless verification
4. **Verify Independently:** Others can verify using proof ID without database access
5. **Read from Smart Contract:** Can also read verified proofs from on-chain storage

This enables trustless verification where external parties can cryptographically verify computation results without seeing private inputs or requiring database access.

This enables trustless verification where external parties can cryptographically verify computation results without seeing private inputs or requiring database access.
