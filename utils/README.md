# Utils

This folder contains basic utilities for local development and testing with the SP1 zkVM stack.

## `generate_a_local_proof.rs`

This Rust file provides a simple example of how to generate a Groth16 proof for a sample arithmetic program using the SP1 SDK. Proof generation is computationally expensive, so you can use this utility to generate a proof once and reuse it for local testing or integration with smart contracts.

### Usage

1. **Build the utility:**

   ```sh
   cargo build --release --package utils --bin generate_a_local_proof
   ```

2. **Run the proof generator:**

   ```sh
   cargo run --release --package utils --bin generate_a_local_proof
   ```

   This will:
   - Set up the SP1 prover client.
   - Generate a Groth16 proof for the arithmetic program (with example inputs).
   - Output the verification key, public values, and proof bytes to the console.
   - Save the proof to `arithmetic-groth16.bin` in the current directory.

3. **Reusing the proof:**

   The generated proof file (`arithmetic-groth16.bin`) can be reused for local contract testing or integration, saving time on repeated proof generation.
