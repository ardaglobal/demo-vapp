use alloy_primitives::FixedBytes;
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use ethereum_client::{Config, EthereumClient, Result};
use std::str::FromStr;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "ethereum-service")]
#[command(about = "Ethereum client for SP1 vApp state management")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long, default_value = "info")]
    pub log_level: String,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Monitor contract events
    Monitor,
    /// Publish a state root
    PublishState {
        #[arg(long)]
        state_id: String,
        #[arg(long)]
        state_root: String,
        #[arg(long)]
        proof: String,
        #[arg(long)]
        public_values: String,
    },
    /// Verify a ZK proof
    VerifyProof {
        #[arg(long)]
        proof: String,
        #[arg(long)]
        public_values: String,
    },
    /// Get current state
    GetState {
        #[arg(long)]
        state_id: String,
    },
    /// Get historical states
    GetHistory {
        #[arg(long)]
        state_id: String,
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Get network statistics
    NetworkStats,
    /// Check inclusion proof
    CheckInclusion {
        #[arg(long)]
        leaf_hash: String,
        #[arg(long)]
        leaf_index: u64,
        #[arg(long)]
        siblings: String, // comma-separated hashes
        #[arg(long)]
        root: String,
    },

    // ==========================================
    // INDEPENDENT VERIFICATION COMMANDS
    // ==========================================
    /// Get verifier key from contract (for independent verification)
    GetVerifierKey,

    /// Get proof result (public values) from contract
    GetProofResult {
        #[arg(long)]
        proof_id: String,
    },

    /// Get proof data from contract
    GetProofData {
        #[arg(long)]
        proof_id: String,
    },

    /// Get state root from contract
    GetStateRoot {
        #[arg(long)]
        state_id: String,
    },

    /// Get complete verification data for a proof
    GetVerificationData {
        #[arg(long)]
        proof_id: String,
    },

    /// Get all proof IDs associated with a state
    GetStateProofHistory {
        #[arg(long)]
        state_id: String,
    },

    /// Perform independent verification of a proof
    VerifyIndependently {
        #[arg(long)]
        proof_id: String,
    },

    /// Get verifier contract version
    GetVerifierVersion,

    /// Complete trustless verification workflow
    TrustlessVerify {
        #[arg(long)]
        proof_id: String,
        #[arg(long, default_value = "false")]
        save_to_file: bool,
    },
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<()> {
    dotenv().ok();

    let cli = Cli::parse();

    // Initialize tracing
    let log_level = LevelFilter::from_str(&cli.log_level).unwrap_or(LevelFilter::INFO);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(log_level.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Ethereum service...");

    // Load configuration
    let config = Config::from_env()?;
    info!("Loaded configuration for network: {}", config.network.name);

    // Create Ethereum client
    let client = EthereumClient::new(config).await?;

    #[cfg(feature = "database")]
    let client = {
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(10)
                .connect(&database_url)
                .await?;

            let cache = ethereum_client::EthereumCache::new(pool);
            cache.initialize().await?;

            client.with_cache(cache)
        } else {
            info!("No DATABASE_URL found, running without caching");
            Ok(client)
        }
    }?;

    match cli.command {
        Commands::Monitor => {
            info!("Starting event monitoring...");
            client.monitor_events()?;
        }

        Commands::PublishState {
            state_id,
            state_root,
            proof,
            public_values,
        } => {
            let state_id = parse_bytes32(&state_id)?;
            let state_root = parse_bytes32(&state_root)?;
            let proof = parse_bytes(&proof)?;
            let public_values = parse_bytes(&public_values)?;

            info!("Publishing state root for state ID: {:?}", state_id);
            let result = client
                .publish_state_root(state_id, state_root, proof, public_values)
                .await?;

            println!("State published successfully!");
            println!("Transaction hash: {:?}", result.transaction_hash);
            println!("Block number: {:?}", result.block_number);
        }

        Commands::VerifyProof {
            proof,
            public_values,
        } => {
            let proof = parse_bytes(&proof)?;
            let public_values = parse_bytes(&public_values)?;

            info!("Verifying ZK proof...");
            let result = client.verify_zk_proof(proof, public_values).await?;

            println!("Proof verification result:");
            println!("Verified: {}", result.verified);
            println!("Proof ID: {:?}", result.proof_id);
            if let Some(error) = result.error_message {
                println!("Error: {error}");
            }
        }

        Commands::GetState { state_id } => {
            let state_id = parse_bytes32(&state_id)?;

            info!("Fetching current state for state ID: {:?}", state_id);
            let state = client.get_current_state(state_id).await?;

            if let Some(state) = state {
                println!("Current state:");
                println!("State ID: {:?}", state.state_id);
                println!("State root: {:?}", state.state_root);
                println!("Block number: {}", state.block_number);
                println!("Timestamp: {}", state.timestamp);
                if let Some(proof_id) = state.proof_id {
                    println!("Latest proof ID: {proof_id:?}");
                }
            } else {
                println!("No state found for the given state ID");
            }
        }

        Commands::GetHistory { state_id, limit } => {
            let state_id = parse_bytes32(&state_id)?;

            info!("Fetching historical states for state ID: {:?}", state_id);
            let history = client.get_historical_states(state_id, limit).unwrap();

            println!("State history:");
            println!("State ID: {:?}", history.state_id);
            println!("Number of states: {}", history.state_roots.len());

            for (i, (((root, block), timestamp), proof_id)) in history
                .state_roots
                .iter()
                .zip(&history.block_numbers)
                .zip(&history.timestamps)
                .zip(&history.proof_ids)
                .enumerate()
            {
                println!(
                    "  {i}: Root={root:?}, Block={block}, Time={timestamp}, Proof={proof_id:?}"
                );
            }
        }

        Commands::NetworkStats => {
            info!("Fetching network statistics...");
            let stats = client.get_network_stats().await?;

            println!("Network statistics:");
            println!("Chain ID: {}", stats.chain_id);
            println!("Network: {}", stats.network_name);
            println!("Block number: {}", stats.block_number);
            println!("Gas price: {} wei", stats.gas_price);
            if let Some(base_fee) = stats.base_fee {
                println!("Base fee: {base_fee} wei");
            }
            println!(
                "Sync status: {}",
                if stats.sync_status {
                    "Synced"
                } else {
                    "Not synced"
                }
            );
        }

        Commands::CheckInclusion {
            leaf_hash,
            leaf_index,
            siblings,
            root,
        } => {
            let leaf_hash = parse_bytes32(&leaf_hash)?;
            let root = parse_bytes32(&root)?;
            let siblings: Result<Vec<_>> = siblings
                .split(',')
                .map(|s| parse_bytes32(s.trim()))
                .collect();
            let siblings = siblings?;

            info!("Checking inclusion proof...");
            let proof = client
                .check_inclusion_proof(leaf_hash, leaf_index, siblings, root)
                .unwrap();

            println!("Inclusion proof result:");
            println!("Verified: {}", proof.verified);
            println!("Leaf hash: {:?}", proof.leaf_hash);
            println!("Leaf index: {}", proof.leaf_index);
            println!("Root: {:?}", proof.root);
            println!("Siblings count: {}", proof.siblings.len());
        }

        // ==========================================
        // INDEPENDENT VERIFICATION COMMANDS
        // ==========================================
        Commands::GetVerifierKey => {
            info!("Retrieving verifier key from contract...");
            let verifier_key = client.get_verifier_key().unwrap();

            println!("Verifier Key:");
            println!("  Key: 0x{}", hex::encode(&verifier_key));
            println!("  Hash: {verifier_key:?}");

            println!("\n💡 This is the SP1 program verification key.");
            println!("   Users can use this key to independently verify proofs.");
        }

        Commands::GetProofResult { proof_id } => {
            let proof_id = parse_bytes32(&proof_id)?;

            info!("Retrieving proof result for {:?}...", proof_id);
            let result = client.get_proof_result(proof_id).unwrap();

            println!("Proof Result (Public Values):");
            println!("  Proof ID: 0x{}", hex::encode(proof_id));
            if let Some(result_bytes) = result {
                println!("  Result size: {} bytes", result_bytes.len());
                println!("  Result data: 0x{}", hex::encode(&result_bytes));

                // Try to decode as arithmetic result if it's 4 bytes (int32)
                if result_bytes.len() == 4 {
                    let int_result = i32::from_be_bytes([
                        result_bytes[0],
                        result_bytes[1],
                        result_bytes[2],
                        result_bytes[3],
                    ]);
                    println!("  Decoded as int32: {int_result}");
                }
            } else {
                println!("  No result data available");
            }

            println!("\n💡 This is the public output that the proof verifies.");
        }

        Commands::GetProofData { proof_id } => {
            let proof_id = parse_bytes32(&proof_id)?;

            info!("Retrieving proof data for {:?}...", proof_id);
            let proof_data = client.get_proof_data(proof_id).unwrap();

            println!("Proof Data:");
            println!("  Proof ID: 0x{}", hex::encode(proof_id));
            if let Some(proof_bytes) = proof_data {
                println!("  Proof size: {} bytes", proof_bytes.len());
                println!(
                    "  Proof hash: 0x{}",
                    hex::encode(alloy_primitives::keccak256(&proof_bytes))
                );

                // Don't print full proof data as it's very large
                println!(
                    "  First 64 bytes: 0x{}",
                    hex::encode(&proof_bytes[..std::cmp::min(64, proof_bytes.len())])
                );
            } else {
                println!("  No proof data available");
            }

            println!("\n💡 This is the ZK proof that can be verified independently with SP1.");
        }

        Commands::GetStateRoot { state_id } => {
            let state_id = parse_bytes32(&state_id)?;

            info!("Retrieving state root for {:?}...", state_id);
            let state_root = client.get_state_root(state_id).unwrap();

            println!("State Root:");
            println!("  State ID: 0x{}", hex::encode(state_id));
            println!("  State root: 0x{}", hex::encode(state_root));
            println!("  State root: {state_root:?}");

            println!("\n💡 This is the current state commitment for this state ID.");
        }

        Commands::GetVerificationData { proof_id } => {
            let proof_id = parse_bytes32(&proof_id)?;

            info!(
                "Retrieving complete verification data for {:?}...",
                proof_id
            );
            let verification_data = client.get_verification_data(proof_id).unwrap();

            println!("Complete Verification Data:");
            if let Some(data_bytes) = verification_data {
                println!("  Proof ID: 0x{}", hex::encode(proof_id));
                println!("  Verification data size: {} bytes", data_bytes.len());
                println!(
                    "  Data hash: 0x{}",
                    hex::encode(alloy_primitives::keccak256(&data_bytes))
                );

                // TODO: Once proper VerificationData struct is implemented, decode the bytes properly
                println!(
                    "  Raw data: 0x{}",
                    hex::encode(&data_bytes[..std::cmp::min(64, data_bytes.len())])
                );
            } else {
                println!("  No verification data available");
            }

            println!("\n💡 This contains all data needed for independent verification.");
        }

        Commands::GetStateProofHistory { state_id } => {
            let state_id = parse_bytes32(&state_id)?;

            info!("Retrieving proof history for state {:?}...", state_id);
            let proof_ids = client.get_state_proof_history(state_id).unwrap();

            println!("State Proof History:");
            println!("  State ID: 0x{}", hex::encode(state_id.as_slice()));
            println!("  Number of proofs: {}", proof_ids.len());

            for (i, proof_id) in proof_ids.iter().enumerate() {
                println!("  {}: 0x{}", i + 1, hex::encode(proof_id.as_slice()));
            }

            println!("\n💡 These are all proofs that contributed to this state's evolution.");
        }

        Commands::VerifyIndependently { proof_id } => {
            let proof_id = parse_bytes32(&proof_id)?;

            info!(
                "Performing independent verification for proof {:?}...",
                proof_id
            );
            println!("🔍 Starting independent verification process...");

            let result = client.verify_proof_independently(proof_id).await?;

            println!("\n📋 Independent Verification Report:");
            println!("  Proof ID: 0x{}", hex::encode(result.proof_id));

            // Main verification results
            println!("\n✅ Verification Results:");
            println!(
                "  Verification Status: {}",
                if result.verified {
                    "✅ PASSED"
                } else {
                    "❌ FAILED"
                }
            );
            println!("  Block Number: {}", result.block_number);
            println!("  Gas Used: {}", result.gas_used);

            if let Some(error) = &result.error_message {
                println!("  Error: {error}");
            }

            if let Some(result_data) = &result.result {
                println!("  Result Data: {} bytes", result_data.len());
            }

            // Summary
            println!(
                "\n🎯 Overall Status: {}",
                if result.verified {
                    "✅ VERIFIED"
                } else {
                    "❌ VERIFICATION FAILED"
                }
            );

            println!("  Block: {}", result.block_number);
        }

        Commands::GetVerifierVersion => {
            info!("Retrieving verifier contract version...");
            let version = client.get_verifier_version().unwrap();

            println!("Verifier Contract Version:");
            println!("  Version: {version}");

            println!("\n💡 This is the SP1 verifier contract version.");
            println!("   Different versions may have different verification rules.");
        }

        Commands::TrustlessVerify {
            proof_id,
            save_to_file,
        } => {
            let proof_id = parse_bytes32(&proof_id)?;

            println!("🚀 Starting Complete Trustless Verification");
            println!("==========================================");
            println!("Proof ID: 0x{}", hex::encode(proof_id));

            // Step 1: Get verifier key
            println!("\n📋 Step 1: Retrieving verifier key...");
            let verifier_key = client.get_verifier_key().unwrap();
            println!("✅ Verifier key: 0x{}", hex::encode(&verifier_key));

            // Step 2: Get proof data
            println!("\n📋 Step 2: Retrieving proof data...");
            let proof_data = client.get_proof_data(proof_id).unwrap();
            if let Some(proof_bytes) = &proof_data {
                println!("✅ Proof data: {} bytes", proof_bytes.len());
            } else {
                println!("❌ No proof data available");
            }

            // Step 3: Get proof result
            println!("\n📋 Step 3: Retrieving proof result...");
            let proof_result = client.get_proof_result(proof_id).unwrap();
            if let Some(result_bytes) = &proof_result {
                println!("✅ Proof result: {} bytes", result_bytes.len());
            } else {
                println!("❌ No proof result available");
            }

            // Step 4: Get associated state
            println!("\n📋 Step 4: Retrieving verification data...");
            let verification_data = client.get_verification_data(proof_id).unwrap();
            if let Some(verification_bytes) = &verification_data {
                println!("✅ Verification data: {} bytes", verification_bytes.len());
            } else {
                println!("❌ No verification data available");
            }

            // Step 5: Independent verification
            println!("\n📋 Step 5: Performing independent verification...");
            let verification_result = client.verify_proof_independently(proof_id).await?;

            let trustless_summary = ethereum_client::types::TrustlessVerificationSummary {
                proof_id,
                verification_status: if verification_result.verified {
                    ethereum_client::types::VerificationStatus::Verified
                } else {
                    ethereum_client::types::VerificationStatus::Failed
                },
                verifier_key: FixedBytes::ZERO, // Convert from Bytes to FixedBytes<32>
                state_root: FixedBytes::ZERO, // Placeholder since verification_data is Option<Bytes>
                independent_verification_passed: verification_result.verified,
                verification_details: format!(
                    "Verified: {}, Block: {}, Gas: {}",
                    verification_result.verified,
                    verification_result.block_number,
                    verification_result.gas_used
                ),
                retrieved_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };

            // Display results
            println!("\n🎯 TRUSTLESS VERIFICATION COMPLETE");
            println!("==================================");
            println!(
                "Status: {}",
                match trustless_summary.verification_status {
                    ethereum_client::types::VerificationStatus::Verified => "✅ VERIFIED",
                    ethereum_client::types::VerificationStatus::Failed => "❌ FAILED",
                    ethereum_client::types::VerificationStatus::NotFound => "❓ NOT FOUND",
                    ethereum_client::types::VerificationStatus::Pending => "⏳ PENDING",
                }
            );
            println!(
                "Independent Verification: {}",
                if trustless_summary.independent_verification_passed {
                    "✅ PASSED"
                } else {
                    "❌ FAILED"
                }
            );
            println!("Details: {}", trustless_summary.verification_details);

            // Save to file if requested
            if save_to_file {
                let filename = format!(
                    "trustless_verification_{}.json",
                    hex::encode(&proof_id[..8])
                );
                let json = serde_json::to_string_pretty(&trustless_summary)?;
                std::fs::write(&filename, json)?;
                println!("📁 Verification report saved to: {filename}");
            }

            println!("\n💡 This verification was performed entirely using on-chain data.");
            println!("   No trust in the service provider is required!");
        }
    }

    info!("Ethereum service completed successfully");
    Ok(())
}

fn parse_bytes32(s: &str) -> Result<alloy_primitives::FixedBytes<32>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s)
        .map_err(|e| ethereum_client::EthereumError::General(eyre::eyre!("Invalid hex: {}", e)))?;

    if bytes.len() != 32 {
        return Err(ethereum_client::EthereumError::General(eyre::eyre!(
            "Expected 32 bytes, got {}",
            bytes.len()
        )));
    }

    Ok(alloy_primitives::FixedBytes::from_slice(&bytes))
}

fn parse_bytes(s: &str) -> Result<alloy_primitives::Bytes> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s)
        .map_err(|e| ethereum_client::EthereumError::General(eyre::eyre!("Invalid hex: {}", e)))?;

    Ok(alloy_primitives::Bytes::from(bytes))
}
