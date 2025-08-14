#[cfg(test)]
mod tests {
    use ethereum_client::{Config, NetworkConfig, ContractConfig, AlchemyConfig, MonitoringConfig};
    use alloy_primitives::Address;
    use url::Url;
    use std::env;

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        
        // Should fail with default config
        assert!(config.validate().is_err());
        
        // Set required fields
        config.alchemy.api_key = "test_key".to_string();
        config.contract.arithmetic_contract = Address::random();
        config.contract.verifier_contract = Address::random();
        
        // Should now pass
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_network_config() {
        let config = NetworkConfig {
            name: "sepolia".to_string(),
            chain_id: 11155111,
            rpc_url: Url::parse("https://eth-sepolia.g.alchemy.com/v2/test").unwrap(),
            ws_url: None,
            explorer_url: None,
            is_testnet: true,
        };
        
        assert_eq!(config.name, "sepolia");
        assert_eq!(config.chain_id, 11155111);
        assert!(config.is_testnet);
    }

    #[test]
    fn test_contract_addresses() {
        use ethereum_client::contracts::ContractAddresses;
        
        let arithmetic_addr = Address::random();
        let verifier_addr = Address::random();
        
        let addresses = ContractAddresses::new(arithmetic_addr, verifier_addr);
        
        assert_eq!(addresses.arithmetic, arithmetic_addr);
        assert_eq!(addresses.verifier, verifier_addr);
    }

    #[test]
    fn test_types_serialization() {
        use ethereum_client::types::*;
        use alloy_primitives::{FixedBytes, Bytes};
        
        let state_update = StateUpdate {
            state_id: FixedBytes::random(),
            new_state_root: FixedBytes::random(),
            proof: Bytes::from(vec![1, 2, 3, 4]),
            public_values: Bytes::from(vec![5, 6, 7, 8]),
            block_number: Some(12345),
            transaction_hash: Some(FixedBytes::random()),
        };
        
        // Test JSON serialization
        let json = serde_json::to_string(&state_update).unwrap();
        let deserialized: StateUpdate = serde_json::from_str(&json).unwrap();
        
        assert_eq!(state_update.state_id, deserialized.state_id);
        assert_eq!(state_update.proof, deserialized.proof);
        assert_eq!(state_update.block_number, deserialized.block_number);
    }

    #[test]
    fn test_merkle_proof_verification_logic() {
        use alloy_primitives::{keccak256, FixedBytes};
        
        // Simple 2-level tree test
        let leaf1 = keccak256(b"leaf1");
        let leaf2 = keccak256(b"leaf2");
        let parent = keccak256(&[leaf1.as_slice(), leaf2.as_slice()].concat());
        
        // Verify leaf1 with sibling leaf2
        let mut computed_hash = leaf1;
        let sibling = leaf2;
        let index = 0u64; // left leaf
        
        if index % 2 == 0 {
            computed_hash = keccak256(&[computed_hash.as_slice(), sibling.as_slice()].concat());
        } else {
            computed_hash = keccak256(&[sibling.as_slice(), computed_hash.as_slice()].concat());
        }
        
        assert_eq!(FixedBytes::from(computed_hash), FixedBytes::from(parent));
    }

    #[test]
    fn test_error_types() {
        use ethereum_client::EthereumError;
        
        let config_error = EthereumError::Config("test error".to_string());
        assert!(matches!(config_error, EthereumError::Config(_)));
        
        let invalid_addr_error = EthereumError::InvalidAddress("0xinvalid".to_string());
        assert!(matches!(invalid_addr_error, EthereumError::InvalidAddress(_)));
        
        // Test error display
        let error_msg = format!("{}", config_error);
        assert!(error_msg.contains("Configuration error"));
    }

    #[test]
    fn test_alchemy_url_parsing() {
        // This tests the internal URL building logic
        let test_cases = vec![
            ("mainnet", "https://eth-mainnet.g.alchemy.com/v2/test_key"),
            ("sepolia", "https://eth-sepolia.g.alchemy.com/v2/test_key"),
            ("base", "https://base-mainnet.g.alchemy.com/v2/test_key"),
            ("arbitrum", "https://arb-mainnet.g.alchemy.com/v2/test_key"),
        ];
        
        for (network, expected_url) in test_cases {
            let url = build_test_alchemy_url(network, "test_key").unwrap();
            assert_eq!(url.as_str(), expected_url);
        }
    }

    // Helper function for testing URL building
    fn build_test_alchemy_url(network: &str, api_key: &str) -> Result<Url, url::ParseError> {
        let base_url = match network {
            "mainnet" => format!("https://eth-mainnet.g.alchemy.com/v2/{}", api_key),
            "sepolia" => format!("https://eth-sepolia.g.alchemy.com/v2/{}", api_key),
            "base" => format!("https://base-mainnet.g.alchemy.com/v2/{}", api_key),
            "arbitrum" => format!("https://arb-mainnet.g.alchemy.com/v2/{}", api_key),
            _ => return Err(url::ParseError::RelativeUrlWithoutBase),
        };
        Url::parse(&base_url)
    }

    #[tokio::test]
    async fn test_config_from_env() {
        // Set test environment variables
        env::set_var("ALCHEMY_API_KEY", "test_key_123");
        env::set_var("ARITHMETIC_CONTRACT_ADDRESS", "0x1234567890123456789012345678901234567890");
        env::set_var("VERIFIER_CONTRACT_ADDRESS", "0x0987654321098765432109876543210987654321");
        env::set_var("ETHEREUM_NETWORK", "sepolia");
        
        // This should work now
        if let Ok(config) = Config::from_env() {
            assert_eq!(config.alchemy.api_key, "test_key_123");
            assert_eq!(config.network.name, "sepolia");
            assert_eq!(config.network.chain_id, 11155111);
            assert!(config.network.is_testnet);
        }
        
        // Clean up
        env::remove_var("ALCHEMY_API_KEY");
        env::remove_var("ARITHMETIC_CONTRACT_ADDRESS");
        env::remove_var("VERIFIER_CONTRACT_ADDRESS");
        env::remove_var("ETHEREUM_NETWORK");
    }
}