# Arda Demo vApp

A simple counter demo demonstrating the [vApp Archtiecture](https://arxiv.org/pdf/2504.14809)

Based off the template for creating an end-to-end [SP1](https://github.com/succinctlabs/sp1) project
that can generate a proof of any RISC-V program.

## Requirements

- [Rust](https://rustup.rs/)
- [SP1](https://docs.succinct.xyz/docs/sp1/getting-started/install)

## You will need to install the following dependencies:

```sh
./install-dependencies.sh
```

## Running the Project

There are 3 main ways to run this project: execute a program, generate a core proof, and
generate an EVM-compatible proof.

### Environment Setup

**Required**: Copy the environment file and configure your database connection:

```sh
cp .env.example .env
```

The `.env` file contains database credentials and SP1 configuration. For development and testing, the default PostgreSQL credentials are already configured for use with Docker Compose (see Database Setup section below).

### Database Setup

This project requires a PostgreSQL database for storing arithmetic transactions. The easiest way to set this up is using Docker Compose:

```sh
# Start PostgreSQL container in the background
docker-compose up -d

# Verify the database is running
docker-compose ps
```

The database will be automatically initialized with the required schema when you first run the execute command.

To stop the database:

```sh
# Stop the container
docker-compose down

# Stop and remove all data (clean slate)
docker-compose down -v
```

### Upon first run

Before we can run the program inside the zkVM, it must be compiled to a RISC-V executable using the succinct Rust toolchain. This is called an ELF (Executable and Linkable Format).
To compile the program to the ELF, you can run the following command:

```sh
cd program && cargo prove build --output-directory ../build
```

### Build the Program

The program is automatically built through `script/build.rs` when the script is built.

### Execute the Program

To run the program interactively without generating a proof:

```sh
cd script
cargo run --release -- --execute
```

This will start an interactive CLI where you can:
- Enter pairs of numbers (a and b) to compute their sum
- See the results stored in the PostgreSQL database
- Continue entering new calculations until you press 'q' to quit

Each calculation is verified and stored in the database for later retrieval.

### Verify Stored Results

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

### Generate Zero-Knowledge Proofs via Sindri

**All proofs are now EVM-compatible by default** using Sindri's cloud infrastructure:

```sh
cd script
# Generate Groth16 proof for specific values (default)
cargo run --release -- --prove --a 5 --b 10

# Generate PLONK proof for specific values
cargo run --release -- --prove --a 5 --b 10 --system plonk

# Generate proof for a previously computed result stored in database
cargo run --release -- --prove --result 15

# Generate proof with Solidity test fixtures
cargo run --release -- --prove --a 5 --b 10 --generate-fixture
```

**Command Options:**
- `--system groth16|plonk`: Choose EVM-compatible proof system (default: groth16)
- `--generate-fixture`: Create Solidity test fixtures in `contracts/src/fixtures/`
- `--a` and `--b`: Direct input values for computation
- `--result`: Look up stored transaction inputs by result value

The `--prove` command will:
1. Create SP1 inputs and serialize them for Sindri
2. Generate EVM-compatible proofs (Groth16 or PLONK)
3. Submit proof request to Sindri using the `demo-vapp` circuit
4. Store proof metadata in PostgreSQL (database mode) or run standalone
5. Display proof ID for external verification

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

The project includes a comprehensive REST API server for external actors to interact with the vApp. The server provides HTTP endpoints for transaction submission, proof verification, and system monitoring.

### Starting the Server

```sh
cd db
cargo run --bin server --release
```

The server will start on `http://localhost:8080` by default.

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
