//! Local SP1 Unit Testing
//! 
//! This program provides a simple way to test the SP1 arithmetic program locally.
//! It generates fast Core proofs for development and testing purposes.
//! 
//! Usage:
//! ```shell
//! cargo run --package arithmetic-program-builder --bin local-sp1-test
//! ```

use alloy_sol_types::SolType;
use arithmetic_lib::PublicValuesStruct;
use eyre::Result;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use tracing::info;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
/// This is built by build.rs from the program/ directory.
pub const ARITHMETIC_ELF: &[u8] = include_elf!("arithmetic-program");

fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    info!("🧮 Starting local SP1 arithmetic unit test");

    // Create a prover client for local testing
    let client = ProverClient::from_env();
    
    // Test case: 5 + 3 = 8
    let a = 5i32;
    let b = 3i32;
    let expected_result = a + b;
    
    info!("Testing: {} + {} = {}", a, b, expected_result);
    
    // Create inputs for the zkVM program
    let mut stdin = SP1Stdin::new();
    stdin.write(&a);
    stdin.write(&b);
    
    info!("🔄 Generating Core proof (fast, for development)...");
    
    // Generate a Core proof (fast for local development)
    let (pk, vk) = client.setup(ARITHMETIC_ELF);
    let proof = client
        .prove(&pk, &stdin)
        .core()  // Use Core proof mode for speed
        .run()
        .expect("Failed to generate proof");
        
    info!("✅ Core proof generated successfully!");
    
    // Verify the proof
    info!("🔍 Verifying proof...");
    
    client.verify(&proof, &vk)
        .expect("Failed to verify proof");
        
    info!("✅ Proof verification passed!");
    
    // Check the public outputs
    let public_values = proof.public_values;
    let output = PublicValuesStruct::abi_decode(&public_values.as_slice())
        .expect("Failed to decode public values");
        
    info!("📤 Public output: result = {}", output.result);
    
    // Verify the computation is correct
    if output.result == expected_result {
        info!("✅ Computation verified: {} + {} = {}", a, b, output.result);
        info!("🎉 Local SP1 unit test completed successfully!");
    } else {
        eyre::bail!(
            "❌ Computation mismatch: expected {}, got {}", 
            expected_result, 
            output.result
        );
    }
    
    Ok(())
}