//! Local SP1 Continuous Balance Tracking Testing
//!
//! This program provides a simple way to test the SP1 continuous balance tracking program locally.
//! It tests the ability to prove balance transitions from an initial state through multiple
//! transactions without revealing the individual transaction amounts.
//!
//! Usage:
//! ```shell
//! cargo run --package demo-vapp --bin demo-vapp
//! ```

use alloy_sol_types::SolType;
use arithmetic_lib::PublicValuesStruct;
use eyre::Result;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use tracing::info;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
/// This is built by build.rs from the program/ directory.
pub const ARITHMETIC_ELF: &[u8] = include_elf!("program");

fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("🧮 Starting local SP1 continuous balance tracking test");

    // Create a prover client for local testing
    let client = ProverClient::from_env();

    // Test case: Initial balance 10, transactions [5, 7] -> final balance 22
    let initial_balance = 10i32;
    let transactions = vec![5i32, 7i32];
    let expected_final_balance = initial_balance + transactions.iter().sum::<i32>();

    info!("Testing continuous balance tracking:");
    info!("  Initial balance: {}", initial_balance);
    info!("  Transactions: {:?}", transactions);
    info!("  Expected final balance: {}", expected_final_balance);

    // Create inputs for the zkVM program
    let mut stdin = SP1Stdin::new();
    stdin.write(&initial_balance);
    stdin.write(&transactions);

    info!("🔄 Generating Core proof (fast, for development)...");

    // Generate a Core proof (fast for local development)
    let (pk, vk) = client.setup(ARITHMETIC_ELF);
    let proof = client
        .prove(&pk, &stdin)
        .core() // Use Core proof mode for speed
        .run()
        .expect("Failed to generate proof");

    info!("✅ Core proof generated successfully!");

    // Verify the proof
    info!("🔍 Verifying proof...");

    client.verify(&proof, &vk).expect("Failed to verify proof");

    info!("✅ Proof verification passed!");

    // Check the public outputs
    let public_values = proof.public_values;
    let output = PublicValuesStruct::abi_decode(&public_values.as_slice())
        .expect("Failed to decode public values");

    info!("📤 Public output:");
    info!("  Initial balance: {}", output.initial_balance);
    info!("  Final balance: {}", output.final_balance);

    // Verify the computation is correct
    if output.initial_balance == initial_balance && output.final_balance == expected_final_balance {
        info!("✅ Continuous balance tracking verified:");
        info!("  Balance transition: {} -> {} (transactions: {:?})", 
              output.initial_balance, output.final_balance, transactions);
        info!("🎉 The individual transaction amounts ({:?}) remain private!", transactions);
        info!("🎉 Local SP1 continuous balance tracking test completed successfully!");
    } else {
        eyre::bail!(
            "❌ Balance tracking mismatch: expected {} -> {}, got {} -> {}",
            initial_balance,
            expected_final_balance,
            output.initial_balance,
            output.final_balance
        );
    }

    Ok(())
}
