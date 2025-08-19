use alloy_primitives::FixedBytes;
use ethereum_client::{Config, EthereumClient, Result};
use tracing::warn;

/// Example demonstrating how users can independently verify your vApp's behavior
/// by querying the smart contract directly without trusting the service.
#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔍 Independent Verification Example");
    println!("==================================");
    println!("This example shows how users can trustlessly verify your vApp's behavior");
    println!("by querying on-chain data directly.\n");

    // Load configuration
    let config = Config::from_env()?;
    let client = EthereumClient::new(config).await?;

    // For this example, we'll use a mock proof ID
    // In reality, users would get this from your service or events
    let mock_proof_id = FixedBytes::from_slice(&[0; 32]);

    println!(
        "Example Proof ID: 0x{}",
        hex::encode(mock_proof_id.as_slice())
    );
    println!("(In practice, users get this from your service or blockchain events)\n");

    // Step 1: Get the verifier key - this is the SP1 program verification key
    println!("📋 Step 1: Retrieving Verifier Key");
    println!("---------------------------------");

    match client.get_verifier_key().await {
        Ok(verifier_key) => {
            println!("✅ Verifier Key: 0x{}", hex::encode(verifier_key.as_ref()));
            println!(
                "💡 This is the SP1 program key that defines what computation is being verified"
            );
        }
        Err(e) => {
            warn!("Failed to get verifier key: {}", e);
            println!("❌ Could not retrieve verifier key (contract may not be deployed)");
        }
    }

    println!();

    // Step 2: Try to get proof data (will likely fail for mock proof ID)
    println!("📋 Step 2: Attempting to Retrieve Proof Data");
    println!("--------------------------------------------");

    match client.get_proof_data(mock_proof_id).await {
        Ok(proof_data) => {
            println!(
                "✅ Proof Data Retrieved: {} bytes",
                proof_data.as_ref().unwrap().len()
            );
            println!("💡 This is the actual ZK proof that can be verified with SP1");
        }
        Err(e) => {
            println!("ℹ️  No proof data found for mock ID (expected): {e}");
            println!("💡 In practice, users would use a real proof ID from your service");
        }
    }

    println!();

    // Step 3: Try to get proof result (will likely fail for mock proof ID)
    println!("📋 Step 3: Attempting to Retrieve Proof Result");
    println!("----------------------------------------------");

    match client.get_proof_result(mock_proof_id).await {
        Ok(result) => {
            println!("✅ Proof Result: {} bytes", result.as_ref().unwrap().len());
            println!(
                "   Data: 0x{}",
                hex::encode(result.as_ref().unwrap().as_ref())
            );

            // Try to decode as arithmetic result
            if result.as_ref().unwrap().len() == 4 {
                let int_result = i32::from_be_bytes([
                    result.as_ref().unwrap().as_ref()[0],
                    result.as_ref().unwrap().as_ref()[1],
                    result.as_ref().unwrap().as_ref()[2],
                    result.as_ref().unwrap().as_ref()[3],
                ]);
                println!("   Decoded as arithmetic result: {int_result}");
            }
            println!("💡 This is the public output that the proof verifies");
        }
        Err(e) => {
            println!("ℹ️  No result found for mock ID (expected): {e}");
            println!("💡 In practice, this would contain the verified computation result");
        }
    }

    println!();

    // Step 4: Try to get state root (will likely fail for mock state ID)
    println!("📋 Step 4: Attempting to Retrieve State Root");
    println!("--------------------------------------------");

    let mock_state_id = FixedBytes::from_slice(&[1u8; 32]);
    match client.get_state_root(mock_state_id).await {
        Ok(state_root) => {
            println!("✅ State Root: 0x{}", hex::encode(state_root.as_slice()));
            println!("💡 This is the current state commitment for the vApp");
        }
        Err(e) => {
            println!("ℹ️  No state found for mock ID (expected): {e}");
            println!("💡 In practice, this would show the current state of your vApp");
        }
    }

    println!();

    // Step 5: Show what complete verification would look like
    println!("📋 Step 5: Complete Independent Verification Process");
    println!("---------------------------------------------------");

    match client.verify_proof_independently(mock_proof_id) {
        Ok(result) => {
            println!("✅ Independent Verification Completed!");
            println!("   SP1 Verification: {}", result.verified);
            println!("   Result: {}", result.result.unwrap_or_default());
            println!("💡 This proves the computation was done correctly");
        }
        Err(e) => {
            println!("ℹ️  Verification failed for mock ID (expected): {e}");
            println!("💡 With real data, this would provide trustless verification");
        }
    }

    println!();

    // Demonstrate the trustless verification workflow
    println!("🎯 Trustless Verification Workflow");
    println!("==================================");
    println!("Here's how users can verify your vApp without trust:");
    println!();

    println!("1. 📡 Get Proof ID from your service or blockchain events");
    println!("   Example: Submit transaction → receive proof ID");
    println!();

    println!("2. 🔑 Query verifier key from smart contract");
    println!("   Command: get-verifier-key");
    println!("   This gives the SP1 program verification key");
    println!();

    println!("3. 📊 Query proof data from smart contract");
    println!("   Command: get-proof-data --proof-id <ID>");
    println!("   This retrieves the actual ZK proof bytes");
    println!();

    println!("4. 📋 Query proof result from smart contract");
    println!("   Command: get-proof-result --proof-id <ID>");
    println!("   This shows what the proof claims to verify");
    println!();

    println!("5. 🌱 Query state root from smart contract");
    println!("   Command: get-state-root --state-id <ID>");
    println!("   This shows the current state commitment");
    println!();

    println!("6. ✅ Verify proof independently with SP1");
    println!("   Command: verify-independently --proof-id <ID>");
    println!("   This proves the computation was done correctly");
    println!();

    println!("🚀 One-Command Trustless Verification:");
    println!("   Command: trustless-verify --proof-id <ID>");
    println!("   This performs all steps automatically!");
    println!();

    // Get verifier version for compatibility
    println!("📋 SP1 Verifier Information");
    println!("---------------------------");
    match client.get_verifier_version().await {
        Ok(version) => {
            println!("✅ SP1 Verifier Version: {version}");
            println!("💡 Users should verify they're using compatible SP1 tooling");
        }
        Err(e) => {
            println!("ℹ️  Could not get verifier version: {e}");
        }
    }

    println!();

    // Security guarantees
    println!("🔒 Security Guarantees");
    println!("======================");
    println!("✅ Cryptographic: SP1 zero-knowledge proofs are cryptographically secure");
    println!("✅ On-chain: All verification data is stored on immutable blockchain");
    println!("✅ Trustless: Users don't need to trust your service or infrastructure");
    println!("✅ Verifiable: Anyone can independently verify any computation");
    println!("✅ Transparent: All data and proofs are publicly auditable");

    println!();
    println!("🎉 Independent verification example completed!");
    println!("💡 Users can now verify your vApp's behavior without trusting you!");

    Ok(())
}
