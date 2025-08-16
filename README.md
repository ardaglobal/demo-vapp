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
cargo run -p api --bin server --release
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
```sh
# Check API server health
cargo run -p cli --bin cli -- health-check

# Store a transaction via CLI
cargo run -p cli --bin cli -- store-transaction --a 5 --b 10

# Query a transaction by result
cargo run -p cli --bin cli -- get-transaction --result 15
```

### 6. Local SP1 Development

For fast local SP1 unit testing during development:
```sh
# Quick SP1 unit test (generates Core proof in ~3.5 seconds)
cargo run -p demo-vapp --bin demo-vapp --release
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

## Running the Project

This project provides multiple interaction methods based on your development needs:

### 🚀 **Local SP1 Development** (Fast Unit Testing)
For rapid SP1 development with instant feedback:
```sh
# Fast Core proof generation (~3.5 seconds)
cargo run -p demo-vapp --bin demo-vapp --release
```
✅ **Perfect for**: SP1 program development, quick verification, unit testing  
❌ **Not included**: Database, Sindri, production workflows

### 🖥️ **CLI Client** (Simple API Interaction)  
For basic API server interaction:
```sh
# Available commands:
cargo run -p cli --bin cli -- health-check
cargo run -p cli --bin cli -- store-transaction --a 5 --b 10  
cargo run -p cli --bin cli -- get-transaction --result 15

# Configure API server URL (default: http://localhost:8080)
export ARITHMETIC_API_URL=http://your-server:8080
```
✅ **Perfect for**: API testing, scripting, external tool integration  
❌ **Not included**: Interactive modes, direct database access

### 🌐 **API Server** (Production System)
For full-featured production deployment:
```sh
# Start with Docker (recommended)
docker-compose up -d

# Or run locally
docker-compose up postgres -d
cargo run -p api --bin server --release
```
✅ **Includes**: Complex workflows, Sindri integration, interactive features, database operations

### Environment Setup

**Required**: Copy the environment file and configure your database connection:

```sh
cp .env.example .env
```

The `.env` file contains database credentials and SP1 configuration. For development and testing, the default PostgreSQL credentials are already configured for use with Docker Compose (see Database Setup section below).

### Database Setup

This project requires a PostgreSQL database for storing arithmetic transactions. The easiest way to set this up is using Docker Compose:

#### Option 1: Database Only (for local development)

```sh
# Start only PostgreSQL container
docker-compose up postgres -d

# Verify the database is running
docker-compose ps
```

#### Option 2: Full Stack (database + server)

```sh
# Start both PostgreSQL and the REST API server (uses pre-built image)
docker-compose up -d

# For local development (builds image locally):
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

# Verify both services are running
docker-compose ps

# View server logs
docker-compose logs server -f
```

The database will be automatically initialized with the required schema when the server starts.

#### Stopping Services

```sh
# Stop all services
docker-compose down

# Stop and remove all data (clean slate)
docker-compose down -v
```

#### Service URLs

When running the full stack:
- **Database**: `localhost:5432` 
- **REST API Server**: `http://localhost:8080`
- **Health Check**: `http://localhost:8080/api/v1/health`

### Program Compilation

The SP1 program is automatically compiled during the build process. For manual compilation:

```sh
cd program && cargo prove build --output-directory ../build
```

The program is also automatically built through `script/build.rs` when building the `demo-vapp` package.

### Legacy Commands (Replaced by CLI)

**⚠️ The following interactive modes have been moved to the API server:**

**Interactive Transaction Submission**: Use the API server's endpoints or the CLI client instead:
```sh
# Old: cd script && cargo run --release -- --execute
# New: Use API server + CLI client
cargo run -p cli --bin cli -- store-transaction --a 5 --b 10
```

**Result Verification**: Use the CLI client for simple queries:
```sh
# Old: cd script && cargo run --release -- --verify
# New: Use CLI client
cargo run -p cli --bin cli -- get-transaction --result 15
```

For complex interactive workflows, use the API server's REST endpoints or run the API server locally.

### Generate Zero-Knowledge Proofs via Sindri

**All proofs are now EVM-compatible by default** using Sindri's cloud infrastructure.

Proof generation is handled by the API server. You can generate proofs in two ways:

**Option A: Via API Server** (Recommended)
```sh
# Generate proof during transaction submission
curl -X POST http://localhost:8080/api/v1/transactions \
  -H 'Content-Type: application/json' \
  -d '{"a": 5, "b": 10, "generate_proof": true}'
```

**Option B: Via CLI Client**
```sh
# CLI client routes to API server
cargo run -p cli --bin cli -- store-transaction --a 5 --b 10
```

**⚠️ Legacy Commands**: The old CLI proof generation commands have been moved to the API server:
```sh
# Old: cd script && cargo run --release -- --prove --a 5 --b 10
# New: Use API server endpoints for proof generation
```

The proof generation process:
1. Creates SP1 inputs and serializes them for Sindri
2. Generates EVM-compatible proofs (Groth16 by default)
3. Submits proof request to Sindri using the `demo-vapp` circuit
4. Stores proof metadata in PostgreSQL  
5. Returns proof ID for external verification

### Verify Sindri Proofs

Proof verification is now handled through the API server:

#### External Verification (Recommended for sharing proofs)

```sh
# Verify using proof ID via API server
curl -X POST http://localhost:8080/api/v1/verify \
  -H 'Content-Type: application/json' \
  -d '{"proof_id": "proof_abc123def456", "expected_result": 15}'
```

This method:
- ✅ Works for external users without database access
- ✅ Only requires the proof ID and expected result
- ✅ Performs full cryptographic verification using Sindri's verification key
- ✅ Demonstrates true zero-knowledge properties

#### Database Verification (Internal use)

For internal use with database access:

```sh
# Verify proof for stored result
curl -X POST http://localhost:8080/api/v1/results/15/verify

# Get proof information
curl http://localhost:8080/api/v1/proofs/proof_abc123def456
```

**⚠️ Legacy Commands**: The old CLI verification commands have been moved to the API server:
```sh
# Old: cd script && cargo run --release -- --verify --proof-id <ID> --result 15  
# New: Use API server endpoints for proof verification
```

### Retrieve the Verification Key

The verification key is now available through the API server:

```sh
# Get verification key via API
curl http://localhost:8080/api/v1/vkey
```

**⚠️ Legacy Command**: The old verification key command has been moved:
```sh  
# Old: cd script && cargo run --release -- --vkey
# New: Use API server endpoint for verification key
```

This key is needed for on-chain contract verification and can be retrieved at any time through the API.

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

The project includes a comprehensive REST API server located in the `api/` directory. The server provides HTTP endpoints for transaction submission, proof verification, and system monitoring.

### Starting the Server

#### Option 1: Using Docker Compose (Recommended)

```sh
# Start both database and API server
docker-compose up -d

# Or start just the API server (if database is already running)
docker-compose up server -d
```

#### Option 2: Local Development

```sh
# Start PostgreSQL database
docker-compose up postgres -d

# Run API server locally
cargo run -p api --bin server --release
```

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


## Troubleshooting

### Common Linux/Ubuntu Issues

**OpenSSL compilation errors** (like `openssl-sys` build failures):
```bash
# Install missing development libraries
sudo apt-get update
sudo apt-get install -y libssl-dev pkg-config libpq-dev

# Retry the dependency installation
./install-dependencies.sh
```

**Docker permission errors**:
```bash
# Add user to docker group (requires logout/login)
sudo usermod -aG docker $USER

# Or run with sudo temporarily
sudo docker-compose up -d
```

**sqlx-cli installation failures**:
```bash
# Ensure PostgreSQL development libraries are installed
sudo apt-get install -y libpq-dev

# Install sqlx-cli manually if needed
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

**Sindri circuit deployment issues**:
```bash
# Check if circuit is properly configured
sindri lint

# Verify API key is set
echo $SINDRI_API_KEY

# Deploy with explicit tag
sindri deploy --tag "manual-$(date +%s)"

# Check deployed circuits
sindri list
```

**Proof generation failures**:
- Ensure circuit is deployed to Sindri first (`sindri deploy`)
- Verify `SINDRI_API_KEY` is set in `.env`
- Check circuit name matches in `sindri.json` (should be "demo-vapp")
- Transactions without `generate_proof: true` will work without Sindri
