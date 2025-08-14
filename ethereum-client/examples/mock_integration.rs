use ethereum_client::{Config, NetworkConfig, ContractConfig, AlchemyConfig, MonitoringConfig};
use alloy_primitives::Address;
use url::Url;

/// Example of setting up a mock configuration for testing without real network access
#[tokio::main]
async fn main() -> ethereum_client::Result<()> {
    println!("Setting up mock integration test...");

    // Create a mock configuration that doesn't require real API keys
    let mock_config = Config {
        network: NetworkConfig {
            name: "mock-sepolia".to_string(),
            chain_id: 11155111,
            rpc_url: Url::parse("https://eth-sepolia.g.alchemy.com/v2/mock-key").unwrap(),
            ws_url: None,
            explorer_url: Url::parse("https://sepolia.etherscan.io").ok(),
            is_testnet: true,
        },
        contract: ContractConfig {
            arithmetic_contract: Address::random(),
            verifier_contract: Address::random(),
            deployment_block: Some(1000000),
        },
        alchemy: AlchemyConfig {
            api_key: "mock-api-key".to_string(),
            app_id: Some("mock-app-id".to_string()),
            webhook_url: None,
            notify_addresses: vec![Address::random()],
            rate_limit_per_second: 10, // Low rate limit for testing
        },
        signer: None, // No signer for mock testing
        monitoring: MonitoringConfig {
            enable_event_monitoring: false, // Disable for mock testing
            polling_interval_seconds: 60,
            max_block_range: 100,
            retry_attempts: 1,
            timeout_seconds: 10,
        },
    };

    // Validate configuration
    match mock_config.validate() {
        Ok(_) => println!("✓ Mock configuration is valid"),
        Err(e) => {
            println!("✗ Mock configuration is invalid: {}", e);
            return Err(e);
        }
    }

    // Test configuration serialization
    let json = serde_json::to_string_pretty(&mock_config)?;
    println!("✓ Configuration serialization successful");
    println!("Mock config JSON:\n{}", json);

    // Test deserialization
    let _deserialized: Config = serde_json::from_str(&json)?;
    println!("✓ Configuration deserialization successful");

    // Test error handling
    test_error_scenarios();

    // Test type conversions
    test_type_conversions();

    println!("✓ All mock integration tests passed!");
    
    Ok(())
}

fn test_error_scenarios() {
    use ethereum_client::EthereumError;

    println!("Testing error scenarios...");

    // Test configuration errors
    let mut invalid_config = Config::default();
    assert!(invalid_config.validate().is_err());

    invalid_config.alchemy.api_key = "test".to_string();
    assert!(invalid_config.validate().is_err()); // Still missing contracts

    invalid_config.contract.arithmetic_contract = Address::random();
    invalid_config.contract.verifier_contract = Address::random();
    assert!(invalid_config.validate().is_ok()); // Now should be valid

    // Test error display
    let config_error = EthereumError::Config("test error".to_string());
    let error_msg = format!("{}", config_error);
    assert!(error_msg.contains("Configuration error"));

    println!("✓ Error handling tests passed");
}

fn test_type_conversions() {
    use ethereum_client::types::*;
    use alloy_primitives::{FixedBytes, Bytes, Address, U256};

    println!("Testing type conversions...");

    // Test state update
    let state_update = StateUpdate {
        state_id: FixedBytes::random(),
        new_state_root: FixedBytes::random(),
        proof: Bytes::from(vec![1, 2, 3]),
        public_values: Bytes::from(vec![4, 5, 6]),
        block_number: Some(12345),
        transaction_hash: Some(FixedBytes::random()),
    };

    // Test serialization roundtrip
    let json = serde_json::to_string(&state_update).unwrap();
    let restored: StateUpdate = serde_json::from_str(&json).unwrap();
    assert_eq!(state_update.state_id, restored.state_id);

    // Test proof submission
    let proof_submission = ProofSubmission {
        proof_id: FixedBytes::random(),
        state_id: FixedBytes::random(),
        proof: Bytes::from(vec![7, 8, 9]),
        result: Bytes::from(vec![10, 11, 12]),
        submitter: Address::random(),
        block_number: 54321,
        transaction_hash: FixedBytes::random(),
        gas_used: U256::from(100000),
    };

    let json = serde_json::to_string(&proof_submission).unwrap();
    let restored: ProofSubmission = serde_json::from_str(&json).unwrap();
    assert_eq!(proof_submission.proof_id, restored.proof_id);

    println!("✓ Type conversion tests passed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_setup() {
        // This test ensures our mock setup works
        let result = main().await;
        assert!(result.is_ok());
    }
}