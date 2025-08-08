# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an SP1 (Succinct Proof) project that demonstrates zero-knowledge proof generation for arithmetic addition operations. The project consists of five main components:

1. **RISC-V Program** (`program/`): Performs arithmetic addition inside the SP1 zkVM
2. **Script** (`script/`): Generates proofs and handles execution using the SP1 SDK
3. **Smart Contracts** (`contracts/`): Solidity contracts for on-chain proof verification
4. **Database Module** (`db/`): PostgreSQL integration for storing arithmetic transactions and Sindri proof metadata
5. **Sindri Integration** (integrated into `script/src/bin/main.rs`): Serverless proof generation using Sindri's cloud infrastructure

## Common Commands

### Building and Development
```bash
# First-time setup: compile the program to RISC-V
cd program && cargo prove build --output-directory ../build

# Execute program interactively (stores results in PostgreSQL)
cd script && cargo run --release -- --execute

# Generate zero-knowledge proof via Sindri (database-free mode with explicit inputs)
cd script && cargo run --release -- --prove --a 5 --b 10

# Generate zero-knowledge proof via Sindri (database mode - lookup inputs by result)
cd script && cargo run --release -- --prove --result 15

# External verification using proof ID (recommended - no database required)
cd script && cargo run --release -- --verify --proof-id <PROOF_ID> --result 15

# Database-based verification (requires PostgreSQL)
cd script && cargo run --release -- --verify --result 15

# Generate EVM-compatible Groth16 proof (requires 16GB+ RAM)
cd script && cargo run --release --bin evm -- --system groth16

# Generate EVM-compatible PLONK proof
cd script && cargo run --release --bin evm -- --system plonk

# Retrieve verification key for on-chain contracts
cd script && cargo run --release --bin vkey
```

### Zero-Knowledge Workflow

#### Database-Free Mode (Recommended for CI/External Use)
```bash
# 1. Generate proof with explicit private inputs (no database required)
SINDRI_API_KEY=your_api_key_here cargo run --release -- --prove --a 7 --b 13
# Output: proof_id = abc123def456
# Note: Proof metadata not stored (database-free mode)

# 2. Share proof ID publicly for external verification
# Anyone can verify without knowing the private inputs (7, 13)
cargo run --release -- --verify --proof-id abc123def456 --result 20

# 3. The verifier only learns that someone knows two numbers that add to 20
# The actual inputs (7, 13) remain completely private
```

#### Database Mode (For Result Lookup)
```bash
# 1. First execute to store inputs in database
cargo run --release -- --execute
# Enter: a=7, b=13 (stored as result=20)

# 2. Generate proof by looking up inputs from stored result
cargo run --release -- --prove --result 20
# Database retrieves a=7, b=13 for the proof generation

# 3. Verify using database lookup
cargo run --release -- --verify --result 20
```

### Smart Contract Testing
```bash
# Run Foundry tests
cd contracts && forge test

# Build contracts
cd contracts && forge build
```

### Database Testing
```bash
# Run database tests (requires PostgreSQL)
cd db && cargo test

# Run database tests with output
cd db && cargo test -- --nocapture

# Run specific database test categories
cd db && cargo test db_tests
cd db && cargo test error_handling_tests
cd db && cargo test performance_tests
```

### Workspace Commands
```bash
# Build entire workspace
cargo build --release

# Run tests across workspace
cargo test

# Run tests for specific components
cargo test -p arithmetic-db
cargo test -p arithmetic-lib
```

## Architecture

### Core Components

- **arithmetic-lib** (`lib/`): Shared library containing the arithmetic computation logic and zero-knowledge public values struct
- **arithmetic-program** (`program/`): The RISC-V program that runs inside the zkVM, reading private inputs and committing only the result as public
- **arithmetic-script** (`script/`): Contains multiple binaries:
  - `main.rs`: Main script for execution, Sindri proof generation, and both database/external verification
  - `evm.rs`: EVM-compatible proof generation (Groth16/PLONK) with zero-knowledge struct
  - `vkey.rs`: Verification key retrieval

### Zero-Knowledge Data Flow

1. The zkVM program reads two **private** arithmetic inputs (`a` and `b`)
2. Performs addition using the shared library (`a + b`)
3. **Only commits the result as public** - inputs remain private within the zkVM
4. When executing locally, computed results are stored in PostgreSQL with a, b, and result values
5. **External verification** requires only proof ID and expected result (no database access)
6. **Database verification** queries stored metadata for internal use
7. The script generates proofs that demonstrate knowledge without revealing private inputs

### Sindri Integration Data Flow

1. User provides arithmetic inputs (`a` and `b`) via command-line arguments or uses previously computed results
2. SP1 inputs are serialized to JSON format expected by Sindri
3. Proof generation request is sent to Sindri's cloud infrastructure using the prebuilt `demo-vapp` circuit
4. Sindri returns proof metadata (proof ID, circuit ID, status) which is stored in PostgreSQL
5. **Proof ID is printed for external verification** - no database dependency required
6. Verification can be done via:
   - **External**: Using proof ID directly (recommended for sharing)
   - **Internal**: Database lookup by result (legacy mode)
7. **Local SP1 Verification**: Extracts SP1 proof and verification key for cryptographic verification
8. **Computation Validation**: Validates arithmetic results from proof public values
9. Proof status is updated in the database (internal mode only)

### Enhanced SP1 Local Verification

The verification logic now performs **full cryptographic verification** using Sindri's SP1 integration:

**Features**:
- **SP1 Proof Extraction**: Uses `to_sp1_proof_with_public()` to extract SP1 proof from Sindri response
- **Verification Key Access**: Uses `get_sp1_verifying_key()` to obtain the SP1 verification key
- **Cryptographic Verification**: Uses `verify_sp1_proof_locally()` for local zero-knowledge proof verification
- **Computation Validation**: Decodes and validates arithmetic computation from proof public values
- **Enhanced User Feedback**: Colored output showing detailed verification results

**Implementation** (sindri crate 0.3.1 with "sp1-v5" feature):
```rust
// Extract SP1 proof and verification key
let sp1_proof = verification_result.to_sp1_proof_with_public()?;
let sindri_verifying_key = verification_result.get_sp1_verifying_key()?;

// Perform cryptographic verification
verification_result.verify_sp1_proof_locally(&sindri_verifying_key)?;

// Validate computation results (only result is public - inputs remain private)
let decoded = PublicValuesStruct::abi_decode(sp1_proof.public_values.as_slice())?;
let PublicValuesStruct { result } = decoded;
```

### Zero-Knowledge Properties

This implementation demonstrates **true zero-knowledge proofs** with the following properties:

**Public Values Structure**:
```rust
struct PublicValuesStruct {
    int32 result;  // Only the result is revealed
    // Inputs 'a' and 'b' remain completely private
}
```

**Zero-Knowledge Guarantees**:
- ✅ **Privacy**: Input values `a` and `b` are never revealed to the verifier
- ✅ **Soundness**: The proof cryptographically guarantees that `a + b = result` 
- ✅ **Completeness**: Valid computations always produce verifiable proofs
- ✅ **Zero-Knowledge**: Verifier learns nothing beyond the fact that the prover knows inputs that produce the result

**What the Proof Demonstrates**:
- "I know two secret numbers that add up to this result"
- **NOT**: "5 + 10 = 15" (which would reveal the secret inputs)

This is the fundamental difference between a regular cryptographic proof and a zero-knowledge proof - the verifier can confirm the computation was performed correctly without learning anything about the private inputs used.

### External Verification Workflow

The system supports **database-independent verification** for sharing proofs with external users:

**Prove Flow**:
```bash
# Generate proof
cargo run --release -- --prove --a 5 --b 10

# Output includes:
🔗 PROOF ID FOR EXTERNAL VERIFICATION:
   proof_abc123def456

📋 To verify this proof externally, use:
   cargo run --release -- --verify --proof-id proof_abc123def456 --result 15
```

**External Verify Flow**:
```bash
# Anyone can verify using just the proof ID and expected result
cargo run --release -- --verify --proof-id proof_abc123def456 --result 15

# Output:
=== External Verification Mode ===
Verifying proof ID: proof_abc123def456
Expected result: 15
✓ ZERO-KNOWLEDGE PROOF VERIFIED: result = 15 (ZKP verified)
🎭 Private inputs remain hidden - only the result is revealed
```

**Benefits**:
- ✅ **No Database Required**: External users don't need database access
- ✅ **Shareable**: Proof IDs can be shared publicly for verification
- ✅ **Self-Contained**: Only requires proof ID and expected result
- ✅ **True ZK**: Demonstrates zero-knowledge properties to external verifiers

### Continuous Integration Workflow

The GitHub Actions workflow (`.github/workflows/sindri.yml`) provides **end-to-end ZK proof testing** in CI:

**Workflow Steps**:
1. **Environment Setup**: Node.js, Rust nightly, SP1 toolchain
2. **Circuit Linting**: `sindri lint` validates circuit structure
3. **Program Building**: Compiles SP1 program to RISC-V ELF
4. **Dynamic Tagging**: Creates unique tags based on branch/PR
5. **Circuit Deployment**: Deploys to Sindri with branch-specific tag
6. **Proof Generation**: Creates ZK proof for `7 + 13 = 20` (no database)
7. **External Verification**: Verifies proof using only proof ID and expected result

**Branch Tagging**:
- **Main**: `main-a1b2c3d` (branch + commit SHA)
- **PRs**: `pr-42-feature-branch` (PR number + branch name)

**Zero-Knowledge Testing**:
```yaml
# Generate proof (inputs 7, 13 remain private)
cargo run --release -- --prove --a 7 --b 13

# Verify proof (only sees result = 20)
cargo run --release -- --verify --proof-id $PROOF_ID --result 20
```

This demonstrates **production-ready ZK workflows** with:
- No database dependencies in CI
- Automated proof generation and verification
- Branch-specific circuit deployments
- True zero-knowledge properties (inputs hidden, result verified)

### Intelligent Database Detection

The CLI now intelligently determines whether database access is required based on the command arguments:

**Database Detection Logic**:
```rust
let needs_database = (args.a != 0 && args.b != 0) && args.result == 0;
```

**Database-Free Mode** (when `needs_database = false`):
- **Explicit Inputs**: `--prove --a 5 --b 10` (uses provided values directly)
- **Default Calculation**: `--prove` (uses default a=1, b=1, calculates result=2)
- **Mixed Arguments**: `--prove --a 5 --b 10 --result 999` (ignores result, uses inputs)
- **Benefits**: No database connection required, perfect for CI/external environments

**Database Mode** (when `needs_database = true`):
- **Result Lookup**: `--prove --result 15` (looks up stored inputs that produced result=15)
- **Requirements**: Requires database with previously executed transactions
- **Use Case**: When you want to prove a specific result but don't remember the original inputs

**Command Examples**:
```bash
# Database-free (CI-friendly)
cargo run --release -- --prove --a 7 --b 13        # ✅ Direct inputs
cargo run --release -- --prove                     # ✅ Default values (1+1=2)
cargo run --release -- --prove --a 5 --b 10 --result 999  # ✅ Ignores result

# Database required
cargo run --release -- --prove --result 20         # 🔍 Looks up inputs for result=20
```

**Error Handling**:
- Database-free mode: Proceeds immediately without database connection attempts
- Database mode: Provides clear error message if database is unavailable
- No time wasted on unnecessary database connections

### Interactive CLI Features

**Execute Mode**: The `--execute` command now runs interactively by default:
- Prompts users to enter values for 'a' and 'b'
- Computes the arithmetic operation in the zkVM
- Stores results automatically in PostgreSQL
- Continues in a loop until user presses 'q' to quit
- Shows real-time feedback on computation and database storage

**Verify Mode**: The `--verify` command supports interactive verification:
- When run without `--result`, starts interactive mode
- Prompts users to enter result values to look up
- Shows the original 'a' and 'b' values that produced each result  
- Continues in a loop until user presses 'q' to quit
- Supports legacy mode with `--result` flag for specific lookups

### Key Files

- `program/src/main.rs:25-28`: Zero-knowledge public values commitment (only result is public)
- `lib/src/lib.rs:6-8`: Zero-knowledge PublicValuesStruct definition (result only)
- `contracts/src/Arithmetic.sol:35`: On-chain proof verification function
- `script/src/bin/main.rs:83-96`: Intelligent database detection logic for prove operations
- `script/src/bin/main.rs:380-486`: Database-enabled Sindri proof generation (`run_prove_via_sindri`)
- `script/src/bin/main.rs:488-551`: Database-free Sindri proof generation (`run_prove_via_sindri_no_db`)
- `script/src/bin/main.rs:229-280`: Database-based verification function (`verify_result_via_sindri`)
- `script/src/bin/main.rs:282-314`: External verification function (`run_external_verify`)
- `script/src/bin/main.rs:318-377`: Local SP1 verification with cryptographic proof validation (`perform_local_verification`)
- `script/Cargo.toml:30`: Sindri dependency with SP1-v5 feature flag enabled
- `.github/workflows/sindri.yml`: Complete CI pipeline with zero-knowledge testing (database-free)
- `db/src/db.rs:160`: Sindri proof database operations (`upsert_sindri_proof`, `get_sindri_proof_by_result`)

## Environment Configuration

### Database Setup (Docker - Recommended)

For easy testing and development, use Docker Compose to run PostgreSQL:

```bash
# Start PostgreSQL container
docker-compose up -d

# Set up environment variables
cp .env.example .env
# DATABASE_URL is already configured for Docker setup

# Run database tests
cd db && cargo test
```

### Manual Database Setup

Alternatively, install and configure PostgreSQL manually:

```bash
# Set up environment variables
cp .env.example .env
# Edit .env and set DATABASE_URL for your PostgreSQL connection
# Set SP1_PROVER=network and NETWORK_PRIVATE_KEY for prover network usage
# Set SINDRI_API_KEY for Sindri cloud proof generation
export SINDRI_API_KEY=your_api_key_here
```

### Stopping the Database

```bash
# Stop the PostgreSQL container
docker-compose down

# Stop and remove data (clean slate)
docker-compose down -v
```

## PostgreSQL Integration

This project uses PostgreSQL as the database for storing and retrieving arithmetic computation results.

### PostgreSQL Features Used

- **Relational Storage**: Structured data storage with ACID compliance
- **Async Operations**: Non-blocking database operations using sqlx
- **Connection Pooling**: Efficient database connection management
- **Automatic Migrations**: Schema initialization on startup

### Database Operations

The project provides the following PostgreSQL operations through the `arithmetic-db` crate:

- `init_db()`: Initialize PostgreSQL connection pool and run migrations
- `store_arithmetic_transaction(pool, a, b, result)`: Store an arithmetic transaction
- `get_value_by_result(pool, result)`: Retrieve the first transaction by result value
- `get_transactions_by_result(pool, result)`: Retrieve all transactions with a specific result
- `upsert_sindri_proof(pool, result, proof_id, circuit_id, status)`: Store/update Sindri proof metadata
- `get_sindri_proof_by_result(pool, result)`: Retrieve Sindri proof metadata by result

### Storage Schema

Arithmetic transactions are stored in the `arithmetic_transactions` table:
```sql
CREATE TABLE arithmetic_transactions (
    id SERIAL PRIMARY KEY,
    a INTEGER NOT NULL,
    b INTEGER NOT NULL,
    result INTEGER NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(a, b, result)
);
```

Sindri proof metadata is stored in the `sindri_proofs` table:
```sql
CREATE TABLE sindri_proofs (
    id SERIAL PRIMARY KEY,
    result INTEGER NOT NULL,
    proof_id TEXT NOT NULL,
    circuit_id TEXT,
    status TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (result)
);
```

### Database Configuration

- **Connection**: Uses DATABASE_URL environment variable (optional for external verification)
- **Pooling**: sqlx PgPool for connection management
- **Migrations**: Automatic schema creation and indexing
- **Indexing**: Optimized queries on result values and timestamps
- **Conditional Initialization**: Database only initialized when needed (execute/prove/database-verify operations)

## Testing

The project includes comprehensive testing across all components:

### Smart Contract Tests
- Foundry tests for smart contracts (`contracts/test/`)
- Proof fixtures for both Groth16 and PLONK verification systems
- Mock verification for faster execution using fixtures from `contracts/src/fixtures/`

### Database Tests
- **Unit Tests**: Core database operations (init, store, retrieve)
- **Integration Tests**: Full workflow testing with real PostgreSQL
- **Error Handling**: Invalid URLs, connection failures, closed pools
- **Performance Tests**: Bulk operations and concurrent access
- **Edge Cases**: Boundary values, negative numbers, zero handling
- **Stress Tests**: 1000+ operations to validate reliability

### Test Prerequisites
- **PostgreSQL**: Database tests require a running PostgreSQL instance
- **Environment**: Set `DATABASE_URL` environment variable for database tests
- **Isolation**: Tests automatically create/destroy isolated test databases

### Running All Tests
```bash
# Run all tests (requires PostgreSQL for database tests)
cargo test

# Run tests excluding database tests
cargo test -p arithmetic-lib
cargo test -p arithmetic-program

# Run only database tests
cargo test -p arithmetic-db
```

## Summary of Enhancements

This project has been enhanced throughout development to demonstrate **true zero-knowledge proofs** with **production-ready workflows**:

### 🔐 Zero-Knowledge Implementation
- **Private Inputs**: `a` and `b` values remain completely hidden within zkVM execution
- **Public Output**: Only the `result` is revealed in the proof
- **True ZK Properties**: Verifiers learn nothing beyond "someone knows inputs that produce this result"
- **Clean Architecture**: Removed legacy backward-compatibility structs for clarity

### 🌐 External Verification Workflow
- **Database-Independent**: External users can verify proofs without any database setup
- **Shareable Proof IDs**: Generated proofs include publicly shareable identifiers
- **Self-Contained**: Verification requires only proof ID and expected result
- **Production Ready**: Suitable for real-world ZK applications and public proof sharing

### 🔧 Sindri Integration with SP1 Support
- **Feature Flag**: `sindri = { version = "0.3.1", features = ["sp1-v5"] }` enables full SP1 integration
- **Local Verification**: Cryptographic proof validation using Sindri's verification keys
- **SP1 Methods**: `to_sp1_proof_with_public()`, `get_sp1_verifying_key()`, `verify_sp1_proof_locally()`
- **Cloud Infrastructure**: Serverless proof generation via Sindri's optimized infrastructure

### 🚀 Continuous Integration Pipeline
- **End-to-End Testing**: Complete ZK workflow validation in GitHub Actions
- **Branch-Specific Deployments**: Unique circuit tags for each branch/PR
- **Database-Free CI**: Demonstrates external verification workflow in automated testing
- **Zero-Knowledge Validation**: Proves ZK properties work correctly in production environment

### 📊 Intelligent Database Detection
- **Proactive Logic**: Determines database need upfront based on command arguments
- **Database-Free Prove**: `--prove --a X --b Y` works without any database connection
- **Database-Required Prove**: `--prove --result Z` requires database for input lookup
- **External Verification**: `--verify --proof-id` works without any database dependency
- **Internal Verification**: `--verify --result` uses database for metadata lookup
- **CI-Optimized**: Perfect for continuous integration environments with no database setup
- **Zero Latency**: No time wasted attempting unnecessary database connections

### 🎯 Production Benefits
- **Scalable**: Uses cloud infrastructure for proof generation
- **Shareable**: Proofs can be distributed publicly for verification
- **Secure**: True zero-knowledge properties maintained throughout
- **Testable**: Comprehensive CI validates all workflows automatically
- **Educational**: Perfect demonstration of real zero-knowledge proof systems

This implementation serves as a **complete reference** for building production zero-knowledge applications with SP1, Sindri, and external verification capabilities.