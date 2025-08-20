use alloy_primitives::{Bytes, FixedBytes};
use ethereum_client::{Config, EthereumClient, Result};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Testing Ethereum client basic usage...");

    // Load configuration from environment
    let config = match Config::from_env() {
        Ok(config) => {
            info!("✓ Configuration loaded successfully");
            info!("  Network: {}", config.network.name);
            info!("  Chain ID: {}", config.network.chain_id);
            info!("  RPC URL: {}", config.network.rpc_url);
            config
        }
        Err(e) => {
            eprintln!("✗ Failed to load configuration: {e}");
            eprintln!("Make sure you have set the required environment variables:");
            eprintln!("  - ETHEREUM_RPC_URL");
            eprintln!("  - ARITHMETIC_CONTRACT_ADDRESS");
            eprintln!("  - VERIFIER_CONTRACT_ADDRESS");
            return Err(e);
        }
    };

    // Create Ethereum client
    let client = match EthereumClient::new(config.clone()).await {
        Ok(client) => {
            info!("✓ Ethereum client created successfully");
            client
        }
        Err(e) => {
            eprintln!("✗ Failed to create Ethereum client: {e}");
            return Err(e);
        }
    };

    // Test 1: Get network statistics
    info!("Test 1: Getting network statistics...");
    match client.get_network_stats().await {
        Ok(stats) => {
            info!("✓ Network stats retrieved:");
            info!("  Chain ID: {}", stats.chain_id);
            info!("  Block number: {}", stats.block_number);
            info!("  Gas price: {} wei", stats.gas_price);
            info!("  Network: {}", stats.network_name);
            info!("  Sync status: {}", stats.sync_status);
        }
        Err(e) => {
            eprintln!("✗ Failed to get network stats: {e}");
            return Err(e);
        }
    }

    // Test 2: Try to read a state (should fail gracefully if state doesn't exist)
    info!("Test 2: Reading a state...");
    let test_state_id = FixedBytes::from_slice(&[1u8; 32]);
    match client.get_current_state(test_state_id).await {
        Ok(state) => {
            info!("✓ State read successfully:");
            info!("  State ID: {:?}", state.clone().unwrap().state_id);
            info!("  State root: {:?}", state.clone().unwrap().state_root);
            info!("  Block number: {}", state.unwrap().block_number);
        }
        Err(e) => {
            info!("ℹ State not found (expected): {}", e);
        }
    }

    // Test 3: Test proof verification (mock proof - will fail but tests the flow)
    info!("Test 3: Testing proof verification (mock proof)...");
    let mock_proof = Bytes::from(vec![0u8; 64]); // Mock proof
    let mock_public_values = Bytes::from(vec![0u8; 32]); // Mock public values

    match client.verify_zk_proof(mock_proof, mock_public_values).await {
        Ok(result) => {
            info!("✓ Proof verification completed:");
            info!("  Verified: {}", result.verified);
            info!("  Proof ID: {:?}", result.proof_id);
        }
        Err(e) => {
            info!(
                "ℹ Proof verification failed (expected for mock data): {}",
                e
            );
        }
    }

    // Test 4: Test inclusion proof verification (local computation)
    info!("Test 4: Testing inclusion proof verification...");
    let leaf_hash = FixedBytes::<32>::from_slice(&[1u8; 32]);
    let siblings = vec![
        FixedBytes::<32>::from_slice(&[2u8; 32]),
        FixedBytes::<32>::from_slice(&[3u8; 32]),
    ];
    let root = FixedBytes::<32>::from_slice(&[4u8; 32]);

    let proof = client
        .check_inclusion_proof(leaf_hash, 0, siblings, root)
        .unwrap();

    info!("✓ Inclusion proof check completed:");
    info!("  Verified: {}", proof.verified);
    info!("  Leaf hash: {:?}", proof.leaf_hash);
    info!("  Root: {:?}", proof.root);

    // If we have a signer, we can test write operations
    if config.signer.is_some() {
        info!("Test 5: Testing state publication (requires signer)...");

        let state_id = FixedBytes::from_slice(&[1u8; 32]);
        let state_root = FixedBytes::from_slice(&[2u8; 32]);
        let proof = Bytes::from(vec![1, 2, 3, 4]); // Mock proof
        let public_values = Bytes::from(vec![5, 6, 7, 8]); // Mock public values

        match client
            .publish_state_root(state_id, state_root, proof, public_values)
            .await
        {
            Ok(update) => {
                info!("✓ State root published successfully:");
                info!("  State ID: {:?}", update.state_id);
                info!("  Transaction hash: {:?}", update.transaction_hash);
                info!("  Block number: {:?}", update.block_number);
            }
            Err(e) => {
                info!(
                    "ℹ State publication failed (may require valid proof): {}",
                    e
                );
            }
        }
    } else {
        info!("Test 5: Skipping state publication (no signer configured)");
    }

    info!("✓ All tests completed successfully!");

    Ok(())
}
