use crate::error::{EthereumError, Result};
use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use std::env;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub contract: ContractConfig,
    pub alchemy: AlchemyConfig,
    pub signer: Option<SignerConfig>,
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub name: String,
    pub chain_id: u64,
    pub rpc_url: Url,
    pub ws_url: Option<Url>,
    pub explorer_url: Option<Url>,
    pub is_testnet: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractConfig {
    pub arithmetic_contract: Address,
    pub verifier_contract: Address,
    pub deployment_block: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlchemyConfig {
    pub api_key: String,
    pub app_id: Option<String>,
    pub webhook_url: Option<Url>,
    pub notify_addresses: Vec<Address>,
    pub rate_limit_per_second: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerConfig {
    pub private_key: String,
    pub address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_event_monitoring: bool,
    pub polling_interval_seconds: u64,
    pub max_block_range: u64,
    pub retry_attempts: u32,
    pub timeout_seconds: u64,
}

impl Config {
    #[allow(clippy::too_many_lines)]
    pub fn from_env() -> Result<Self> {
        let network_name = env::var("ETHEREUM_NETWORK").unwrap_or_else(|_| "sepolia".to_string());
        let chain_id: u64 = env::var("CHAIN_ID")
            .unwrap_or_else(|_| {
                #[allow(clippy::wildcard_in_or_patterns)]
                match network_name.as_str() {
                    "mainnet" => "1".to_string(),
                    "base" => "8453".to_string(),
                    "base-sepolia" => "84532".to_string(),
                    "arbitrum" => "42161".to_string(),
                    "arbitrum-sepolia" => "421614".to_string(),
                    "optimism" => "10".to_string(),
                    "optimism-sepolia" => "11155420".to_string(),
                    "sepolia" | _ => "11155111".to_string(),
                }
            })
            .parse()
            .map_err(|e| EthereumError::Config(format!("Invalid chain ID: {e}")))?;

        let alchemy_api_key = env::var("ALCHEMY_API_KEY")
            .map_err(|_| EthereumError::Config("ALCHEMY_API_KEY is required".to_string()))?;

        let rpc_url = Self::build_alchemy_url(&network_name, &alchemy_api_key)?;
        let ws_url = Self::build_alchemy_ws_url(&network_name, &alchemy_api_key).ok();

        let arithmetic_contract = env::var("ARITHMETIC_CONTRACT_ADDRESS")
            .map_err(|_| {
                EthereumError::Config("ARITHMETIC_CONTRACT_ADDRESS is required".to_string())
            })?
            .parse::<Address>()
            .map_err(|e| EthereumError::InvalidAddress(format!("Invalid contract address: {e}")))?;

        let verifier_contract = env::var("VERIFIER_CONTRACT_ADDRESS")
            .map_err(|_| {
                EthereumError::Config("VERIFIER_CONTRACT_ADDRESS is required".to_string())
            })?
            .parse::<Address>()
            .map_err(|e| EthereumError::InvalidAddress(format!("Invalid verifier address: {e}")))?;

        let signer = if let Ok(private_key) = env::var("PRIVATE_KEY") {
            let signer_address = env::var("SIGNER_ADDRESS")
                .map_err(|_| {
                    EthereumError::Config(
                        "SIGNER_ADDRESS is required when PRIVATE_KEY is provided".to_string(),
                    )
                })?
                .parse::<Address>()
                .map_err(|e| {
                    EthereumError::InvalidAddress(format!("Invalid signer address: {e}"))
                })?;

            Some(SignerConfig {
                private_key,
                address: signer_address,
            })
        } else {
            None
        };

        Ok(Self {
            network: NetworkConfig {
                name: network_name.clone(),
                chain_id,
                rpc_url,
                ws_url,
                explorer_url: Self::get_explorer_url(&network_name),
                is_testnet: Self::is_testnet(chain_id),
            },
            contract: ContractConfig {
                arithmetic_contract,
                verifier_contract,
                deployment_block: env::var("DEPLOYMENT_BLOCK")
                    .ok()
                    .and_then(|s| s.parse().ok()),
            },
            alchemy: AlchemyConfig {
                api_key: alchemy_api_key,
                app_id: env::var("ALCHEMY_APP_ID").ok(),
                webhook_url: env::var("ALCHEMY_WEBHOOK_URL")
                    .ok()
                    .and_then(|s| Url::parse(&s).ok()),
                notify_addresses: env::var("NOTIFY_ADDRESSES")
                    .unwrap_or_default()
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect(),
                rate_limit_per_second: env::var("RATE_LIMIT_PER_SECOND")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .unwrap_or(100),
            },
            signer,
            monitoring: MonitoringConfig {
                enable_event_monitoring: env::var("ENABLE_EVENT_MONITORING")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                polling_interval_seconds: env::var("POLLING_INTERVAL_SECONDS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
                max_block_range: env::var("MAX_BLOCK_RANGE")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .unwrap_or(1000),
                retry_attempts: env::var("RETRY_ATTEMPTS")
                    .unwrap_or_else(|_| "3".to_string())
                    .parse()
                    .unwrap_or(3),
                timeout_seconds: env::var("TIMEOUT_SECONDS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
            },
        })
    }

    fn build_alchemy_url(network: &str, api_key: &str) -> Result<Url> {
        let base_url = match network {
            "mainnet" => format!("https://eth-mainnet.g.alchemy.com/v2/{api_key}"),
            "sepolia" => format!("https://eth-sepolia.g.alchemy.com/v2/{api_key}"),
            "base" => format!("https://base-mainnet.g.alchemy.com/v2/{api_key}"),
            "base-sepolia" => format!("https://base-sepolia.g.alchemy.com/v2/{api_key}"),
            "arbitrum" => format!("https://arb-mainnet.g.alchemy.com/v2/{api_key}"),
            "arbitrum-sepolia" => format!("https://arb-sepolia.g.alchemy.com/v2/{api_key}"),
            "optimism" => format!("https://opt-mainnet.g.alchemy.com/v2/{api_key}"),
            "optimism-sepolia" => format!("https://opt-sepolia.g.alchemy.com/v2/{api_key}"),
            _ => {
                return Err(EthereumError::Config(format!(
                    "Unsupported network: {network}"
                )))
            }
        };

        Url::parse(&base_url).map_err(|e| EthereumError::Config(format!("Invalid RPC URL: {e}")))
    }

    fn build_alchemy_ws_url(network: &str, api_key: &str) -> Result<Url> {
        let base_url = match network {
            "mainnet" => format!("wss://eth-mainnet.g.alchemy.com/v2/{api_key}"),
            "sepolia" => format!("wss://eth-sepolia.g.alchemy.com/v2/{api_key}"),
            "base" => format!("wss://base-mainnet.g.alchemy.com/v2/{api_key}"),
            "base-sepolia" => format!("wss://base-sepolia.g.alchemy.com/v2/{api_key}"),
            "arbitrum" => format!("wss://arb-mainnet.g.alchemy.com/v2/{api_key}"),
            "arbitrum-sepolia" => format!("wss://arb-sepolia.g.alchemy.com/v2/{api_key}"),
            "optimism" => format!("wss://opt-mainnet.g.alchemy.com/v2/{api_key}"),
            "optimism-sepolia" => format!("wss://opt-sepolia.g.alchemy.com/v2/{api_key}"),
            _ => {
                return Err(EthereumError::Config(format!(
                    "Unsupported network: {network}"
                )))
            }
        };

        Url::parse(&base_url)
            .map_err(|e| EthereumError::Config(format!("Invalid WebSocket URL: {e}")))
    }

    fn get_explorer_url(network: &str) -> Option<Url> {
        let url_str = match network {
            "mainnet" => "https://etherscan.io",
            "sepolia" => "https://sepolia.etherscan.io",
            "base" => "https://basescan.org",
            "base-sepolia" => "https://sepolia.basescan.org",
            "arbitrum" => "https://arbiscan.io",
            "arbitrum-sepolia" => "https://sepolia.arbiscan.io",
            "optimism" => "https://optimistic.etherscan.io",
            "optimism-sepolia" => "https://sepolia-optimism.etherscan.io",
            _ => return None,
        };

        Url::parse(url_str).ok()
    }

    const fn is_testnet(chain_id: u64) -> bool {
        matches!(chain_id, 11_155_111 | 84532 | 421_614 | 11_155_420) // Sepolia, Base Sepolia, Arbitrum Sepolia, Optimism Sepolia
    }

    pub fn validate(&self) -> Result<()> {
        if self.alchemy.api_key.is_empty() {
            return Err(EthereumError::Config(
                "Alchemy API key is required".to_string(),
            ));
        }

        if self.contract.arithmetic_contract == Address::ZERO {
            return Err(EthereumError::Config(
                "Arithmetic contract address is required".to_string(),
            ));
        }

        if self.contract.verifier_contract == Address::ZERO {
            return Err(EthereumError::Config(
                "Verifier contract address is required".to_string(),
            ));
        }

        if self.monitoring.polling_interval_seconds == 0 {
            return Err(EthereumError::Config(
                "Polling interval must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: NetworkConfig {
                name: "sepolia".to_string(),
                chain_id: 11_155_111,
                rpc_url: Url::parse("https://eth-sepolia.g.alchemy.com/v2/").unwrap(),
                ws_url: None,
                explorer_url: Url::parse("https://sepolia.etherscan.io").ok(),
                is_testnet: true,
            },
            contract: ContractConfig {
                arithmetic_contract: Address::ZERO,
                verifier_contract: Address::ZERO,
                deployment_block: None,
            },
            alchemy: AlchemyConfig {
                api_key: String::new(),
                app_id: None,
                webhook_url: None,
                notify_addresses: Vec::new(),
                rate_limit_per_second: 100,
            },
            signer: None,
            monitoring: MonitoringConfig {
                enable_event_monitoring: true,
                polling_interval_seconds: 30,
                max_block_range: 1000,
                retry_attempts: 3,
                timeout_seconds: 30,
            },
        }
    }
}
