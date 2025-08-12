# SP1 Project Template

This is a template for creating an end-to-end [SP1](https://github.com/succinctlabs/sp1) project
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

### Generate a Zero-Knowledge Proof via Sindri

To generate a zero-knowledge proof using Sindri's cloud infrastructure:

```sh
cd script
# Generate proof for specific values
cargo run --release -- --prove --a 5 --b 10

# Generate proof for a previously computed result stored in database
cargo run --release -- --prove --result 15
```

The `--prove` command will:
1. Create SP1 inputs and serialize them for Sindri
2. Submit a proof request to Sindri using the `demo-vapp` circuit
3. Store the proof metadata in PostgreSQL for later verification
4. Display the proof generation status

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

### Generate an EVM-Compatible Proof

> [!WARNING]
> You will need at least 16GB RAM to generate a Groth16 or PLONK proof. View the [SP1 docs](https://docs.succinct.xyz/docs/sp1/getting-started/hardware-requirements#local-proving) for more information.

Generating a proof that is cheap to verify on the EVM (e.g. Groth16 or PLONK) is more intensive than generating a core proof.

To generate a Groth16 proof:

```sh
cd script
cargo run --release --bin evm -- --system groth16
```

To generate a PLONK proof:

```sh
cargo run --release --bin evm -- --system plonk
```

These commands will also generate fixtures that can be used to test the verification of SP1 proofs
inside Solidity.

### Retrieve the Verification Key

To retrieve your `programVKey` for your on-chain contract, run the following command in `script`:

```sh
cargo run --release --bin vkey
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

## Using the Prover Network

We highly recommend using the [Succinct Prover Network](https://docs.succinct.xyz/docs/network/introduction) for any non-trivial programs or benchmarking purposes. For more information, see the [key setup guide](https://docs.succinct.xyz/docs/network/developers/key-setup) to get started.

To get started, copy the example environment file:

```sh
cp .env.example .env
```

Then, set the `SP1_PROVER` environment variable to `network` and set the `NETWORK_PRIVATE_KEY`
environment variable to your whitelisted private key.

For example, to generate an EVM-compatible proof using the prover network, run the following
command:

```sh
SP1_PROVER=network NETWORK_PRIVATE_KEY=... cargo run --release --bin evm
```
