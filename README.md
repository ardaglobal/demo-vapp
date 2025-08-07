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
To compile the program, you can run the following command:

```sh
cd program && cargo prove build
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

### Generate an SP1 Core Proof

To generate an SP1 [core proof](https://docs.succinct.xyz/docs/sp1/generating-proofs/proof-types#core-default) for your program:

```sh
cd script
cargo run --release -- --prove
```

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
