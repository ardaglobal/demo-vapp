use alloy_primitives::FixedBytes;
use ethereum_client::{Config, EthereumClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("🔍 Testing contract interaction...");

    let config = Config::from_env()?;
    let client = EthereumClient::new(config.clone()).await?;

    println!("✅ Client created successfully");
    println!("  Contract: {}", config.contract.arithmetic_contract);

    if let Some(signer) = &config.signer {
        println!("  Signer: {}", signer.address);
    }

    // Try to read some basic contract state (should work without authorization)
    println!("\n🔍 Testing contract read operations...");

    // Test reading verifier key (should be public)
    match client.get_verifier_key().await {
        Ok(vkey) => {
            println!(
                "✅ Verifier key read successfully (length: {} bytes)",
                vkey.len()
            );
        }
        Err(e) => {
            println!("❌ Failed to read verifier key: {e}");
        }
    }

    // Test reading a state (will likely fail but tests the connection)
    let test_state_id = FixedBytes::from([1u8; 32]);
    match client.get_current_state(test_state_id).await {
        Ok(state) => {
            if let Some(s) = state {
                println!("✅ State read successful: {:?}", s.state_root);
            } else {
                println!("ℹ️  State not found (expected)");
            }
        }
        Err(e) => {
            println!("ℹ️  State read failed (expected): {e}");
        }
    }

    Ok(())
}
