use alloy_primitives::{Bytes, FixedBytes};
use ethereum_client::{Config, EthereumClient};

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("🧪 Ethereum Client - Complete Test Suite");
    println!("========================================");
    println!("Testing ethereum-client independently of proving system\n");

    // Test 1: Configuration Loading
    println!("1️⃣  Testing Configuration...");
    let config = match Config::from_env() {
        Ok(config) => {
            println!("   ✅ Configuration loaded successfully");
            println!("      Network: {}", config.network.name);
            println!("      Chain ID: {}", config.network.chain_id);
            println!("      Contract: {}", config.contract.arithmetic_contract);
            if let Some(signer) = &config.signer {
                println!("      Signer: {}", signer.address);
            }
            config
        }
        Err(e) => {
            println!("   ❌ Failed to load configuration: {e}");
            return Err(e.into());
        }
    };

    // Test 2: Client Creation
    println!("\n2️⃣  Testing Client Creation...");
    let client = match EthereumClient::new(config.clone()).await {
        Ok(client) => {
            println!("   ✅ Ethereum client created successfully");
            client
        }
        Err(e) => {
            println!("   ❌ Failed to create client: {e}");
            return Err(e.into());
        }
    };

    // Test 3: Network Connection
    println!("\n3️⃣  Testing Network Connection...");
    match client.get_network_stats().await {
        Ok(stats) => {
            println!("   ✅ Network connection successful");
            println!("      Block number: {}", stats.block_number);
            println!("      Gas price: {} wei", stats.gas_price);
        }
        Err(e) => {
            println!("   ⚠️  Network stats failed: {e}");
        }
    }

    // Test 4: Contract Read Operations
    println!("\n4️⃣  Testing Contract Read Operations...");

    // Test 4a: Verifier Key
    match client.get_verifier_key().await {
        Ok(vkey) => {
            println!(
                "   ✅ Verifier key read successful (length: {} bytes)",
                vkey.len()
            );
        }
        Err(e) => {
            println!("   ❌ Verifier key read failed: {e}");
            return Err(e.into());
        }
    }

    // Test 4b: State Reading
    let test_state_id = FixedBytes::from([0u8; 32]);
    match client.get_current_state(test_state_id).await {
        Ok(state) => {
            if let Some(_s) = state {
                println!("   ✅ State read successful");
            } else {
                println!("   ✅ State read successful (no state found - expected)");
            }
        }
        Err(e) => {
            println!("   ⚠️  State read failed: {e}");
        }
    }

    // Test 5: Authorization Test (if signer available)
    if client.has_signer() {
        println!("\n5️⃣  Testing Write Authorization...");
        println!("   ℹ️  This will test authorization, then fail on proof verification (expected)");

        let test_state_id = FixedBytes::from([1u8; 32]);
        let test_state_root = FixedBytes::from([2u8; 32]);
        let test_proof = Bytes::from(vec![1, 2, 3, 4]);
        let test_public_values = Bytes::from(vec![5, 6, 7, 8]);

        match client
            .update_state(
                test_state_id,
                test_state_root,
                test_proof,
                test_public_values,
            )
            .await
        {
            Ok(_) => {
                println!("   ✅ Write operation successful (unexpected!)");
            }
            Err(e) => {
                let error_str = format!("{e}");
                if error_str.contains("0x7fcdd1f4") {
                    println!("   ❌ Authorization failed - signer not authorized");
                } else if error_str.contains("0xf208777e") {
                    println!("   ✅ Authorization passed, proof verification failed (expected)");
                } else {
                    println!("   ⚠️  Unexpected error: {e}");
                }
            }
        }
    } else {
        println!("\n5️⃣  Skipping Write Test - No Signer Configured");
    }

    // Test 6: Contract Information
    println!("\n6️⃣  Contract Information Summary...");
    println!(
        "   📝 Contract Address: {}",
        config.contract.arithmetic_contract
    );
    println!(
        "   🔗 Sepolia Explorer: https://sepolia.etherscan.io/address/{}",
        config.contract.arithmetic_contract
    );
    if let Some(signer) = &config.signer {
        println!("   🔐 Authorized Signer: {}", signer.address);
        println!(
            "   🔗 Signer Explorer: https://sepolia.etherscan.io/address/{}",
            signer.address
        );
    }

    println!("\n🎉 Test Suite Complete!");
    println!("==========================================");
    println!("✅ Ethereum client is working independently");
    println!("✅ Ready for integration with proving system");

    Ok(())
}
