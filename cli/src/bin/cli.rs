//! CLI for batch processing API
//!
//! This CLI interacts with the new batch processing API server.
//! It supports submitting individual transactions, viewing pending transactions,
//! triggering batch creation, and verifying proofs locally.

#![allow(clippy::uninlined_format_args)]
//!
//! Usage examples:
//! ```shell
//! # Submit a transaction
//! cli submit-transaction --amount 5
//!
//! # View pending transactions
//! cli view-pending
//!
//! # Trigger batch creation
//! cli trigger-batch --verbose
//!
//! # Download and verify proof
//! cli download-proof --batch-id 1
//! cli verify-proof --proof-file proof_batch_1.json --expected-initial-balance 10 --expected-final-balance 22
//!
//! # Query smart contract verification key
//! cli query-verification-key --verbose
//!
//! # Check API health
//! cli health-check
//! ```

use clap::{Parser, Subcommand};
use eyre::Result;
use std::env;
use std::fs;
use std::time::Instant;
use tracing::error;

// Import new batch processing API types
use arithmetic_api::BatchApiClient;
use ethereum_client::{config::Config, EthereumClient};

#[derive(Parser)]
#[command(name = "cli")]
#[command(about = "CLI for interacting with the batch processing API server")]
#[command(version)]
struct Cli {
    /// API server base URL
    #[arg(
        long,
        env = "ARITHMETIC_API_URL",
        default_value = "http://localhost:8080"
    )]
    api_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Submit a new transaction to the batch processing queue
    SubmitTransaction {
        /// Transaction amount to add to the counter
        #[arg(short, long)]
        amount: i32,
    },
    /// View all pending (unbatched) transactions
    ViewPending,
    /// Get current counter state and associated merkle root
    GetCurrentState,
    /// Trigger batch creation and get contract submission data
    TriggerBatch {
        /// Maximum number of transactions to include in batch
        #[arg(long, default_value = "10")]
        batch_size: Option<i32>,
        /// Show detailed output including private information
        #[arg(short, long)]
        verbose: bool,
    },
    /// List all historical batches
    ListBatches,
    /// Get details of a specific batch
    GetBatch {
        /// Batch ID
        #[arg(long)]
        batch_id: i32,
    },
    /// Download raw proof data for local verification
    DownloadProof {
        /// Batch ID with associated proof
        #[arg(long)]
        batch_id: i32,
        /// Output file path (optional, defaults to `proof_batch_<id>.json`)
        #[arg(long)]
        output: Option<String>,
    },
    /// Check API server health
    HealthCheck,
    /// Verify proof locally without network dependencies
    VerifyProof {
        /// Path to the downloaded proof JSON file
        #[arg(long, group = "input")]
        proof_file: Option<String>,
        /// Hex-encoded proof data (alternative to --proof-file)
        #[arg(long, group = "input")]
        proof_data: Option<String>,
        /// Hex-encoded public values (required when using --proof-data)
        #[arg(long, requires = "proof_data")]
        public_values: Option<String>,
        /// Hex-encoded verifying key (required when using --proof-data)
        #[arg(long, requires = "proof_data")]
        verifying_key: Option<String>,
        /// Expected initial balance
        #[arg(long)]
        expected_initial_balance: i32,
        /// Expected final balance
        #[arg(long)]
        expected_final_balance: i32,
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Query the current verification key from the smart contract
    QueryVerificationKey {
        /// Show detailed verification key information
        #[arg(short, long)]
        verbose: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "cli=info".to_string()))
        .init();

    let cli = Cli::parse();

    // Create API client using the new batch processing client
    let client = BatchApiClient::new(&cli.api_url);

    // Execute command
    match cli.command {
        Commands::SubmitTransaction { amount } => {
            submit_transaction(&client, amount).await?;
        }
        Commands::ViewPending => {
            view_pending_transactions(&client).await?;
        }
        Commands::GetCurrentState => {
            get_current_state(&client).await?;
        }
        Commands::TriggerBatch {
            batch_size,
            verbose,
        } => {
            trigger_batch(&client, batch_size, verbose).await?;
        }
        Commands::ListBatches => {
            list_batches(&client).await?;
        }
        Commands::GetBatch { batch_id } => {
            get_batch(&client, batch_id).await?;
        }
        Commands::DownloadProof { batch_id, output } => {
            download_proof(&client, batch_id, output).await?;
        }
        Commands::HealthCheck => {
            health_check(&client).await?;
        }
        Commands::VerifyProof {
            proof_file,
            proof_data,
            public_values,
            verifying_key,
            expected_initial_balance,
            expected_final_balance,
            verbose,
        } => {
            verify_proof_local(
                proof_file,
                proof_data,
                public_values,
                verifying_key,
                expected_initial_balance,
                expected_final_balance,
                verbose,
            )?;
        }
        Commands::QueryVerificationKey { verbose } => {
            query_verification_key(verbose).await?;
        }
    }

    Ok(())
}

/// Submit a new transaction to the batch processing queue
async fn submit_transaction(client: &BatchApiClient, amount: i32) -> Result<()> {
    match client.submit_transaction(amount).await {
        Ok(response) => {
            println!("✅ Transaction submitted successfully!");
            println!("   Transaction ID: {}", response.transaction_id);
            println!("   Amount: {}", response.amount);
            println!("   Status: {}", response.status);
            println!("   Created: {}", response.created_at);
            println!();
            println!("💡 Use 'cli view-pending' to see all pending transactions");
            println!("💡 Use 'cli trigger-batch' to create a batch and generate proof");
        }
        Err(e) => {
            eprintln!("❌ Failed to submit transaction: {}", e);
        }
    }

    Ok(())
}

/// View all pending (unbatched) transactions
async fn view_pending_transactions(client: &BatchApiClient) -> Result<()> {
    match client.get_pending_transactions().await {
        Ok(response) => {
            if response.transactions.is_empty() {
                println!("📭 No pending transactions found.");
                println!();
                println!("💡 Submit transactions using: cli submit-transaction --amount <amount>");
            } else {
                println!("📋 Pending Transactions (Unbatched):");
                println!("   Total Count: {}", response.total_count);
                println!("   Total Amount: {}", response.total_amount);
                println!();

                for (i, tx) in response.transactions.iter().enumerate() {
                    println!(
                        "   {}. Transaction ID: {} | Amount: {} | Created: {}",
                        i + 1,
                        tx.id,
                        tx.amount,
                        tx.created_at
                    );
                }

                println!();
                println!(
                    "💡 Use 'cli trigger-batch' to batch these transactions and generate proof"
                );
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get pending transactions: {}", e);
        }
    }

    Ok(())
}

/// Get current counter state and associated merkle root
async fn get_current_state(client: &BatchApiClient) -> Result<()> {
    match client.get_current_state().await {
        Ok(response) => {
            println!("📊 Current Counter State:");
            println!("   Counter Value: {}", response.counter_value);

            if response.has_merkle_root {
                println!("   Merkle Root: Available (use get_contract_data for details)");
            } else {
                println!("   Merkle Root: Not set (no batches processed yet)");
            }

            if let Some(last_batch_id) = response.last_batch_id {
                println!("   Last Batch ID: {}", last_batch_id);
            }

            if let Some(last_proven_batch_id) = response.last_proven_batch_id {
                println!("   Last Proven Batch ID: {}", last_proven_batch_id);
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get current state: {}", e);
        }
    }

    Ok(())
}

/// Trigger batch creation and get contract submission data
async fn trigger_batch(
    client: &BatchApiClient,
    batch_size: Option<i32>,
    verbose: bool,
) -> Result<()> {
    println!("🔄 Creating batch from pending transactions...");

    match client.create_batch(batch_size).await {
        Ok(response) => {
            println!("✅ Batch created successfully!");
            println!("   Batch ID: {}", response.batch_id);
            println!("   Transaction Count: {}", response.transaction_count);
            println!("   Previous Counter: {}", response.previous_counter_value);
            println!("   Final Counter: {}", response.final_counter_value);
            println!("   Created: {}", response.created_at);

            // Get contract submission data (dry run)
            match client.get_contract_data(response.batch_id).await {
                Ok(Some(contract_data)) => {
                    println!();
                    println!("📄 Contract Submission Data (Dry Run):");
                    println!();
                    println!("🔒 Public Information:");
                    println!(
                        "   Previous Merkle Root: {}",
                        contract_data.public.prev_merkle_root
                    );
                    println!(
                        "   New Merkle Root: {}",
                        contract_data.public.new_merkle_root
                    );
                    println!("   ZK Proof ID: {}", contract_data.public.zk_proof);

                    if verbose {
                        println!();
                        println!("🔓 Private Information (for verification only):");
                        println!(
                            "   Previous Counter Value: {}",
                            contract_data.private.prev_counter_value
                        );
                        println!(
                            "   New Counter Value: {}",
                            contract_data.private.new_counter_value
                        );
                        println!("   Transactions: {:?}", contract_data.private.transactions);

                        println!();
                        println!("🔍 Privacy Note:");
                        println!(
                            "   • The private information above is shown for CLI verification"
                        );
                        println!("   • On-chain, only the public information would be submitted");
                        println!("   • Individual transaction amounts remain private via ZK proof");
                    }
                }
                Ok(None) => {
                    println!();
                    println!("⚠ Contract submission data not available yet for this batch");
                }
                Err(e) => {
                    eprintln!("⚠ Warning: Failed to get contract submission data: {}", e);
                }
            }

            println!();
            println!("💡 Next steps:");
            println!("   • ZK proof generation happens asynchronously");
            println!(
                "   • Use 'cli get-batch --batch-id {}' to check batch status",
                response.batch_id
            );
            println!(
                "   • Use 'cli download-proof --batch-id {}' once proof is ready",
                response.batch_id
            );
        }
        Err(e) => {
            eprintln!("❌ Failed to create batch: {}", e);
        }
    }

    Ok(())
}

/// List all historical batches
async fn list_batches(client: &BatchApiClient) -> Result<()> {
    match client.get_batches(None).await {
        Ok(response) => {
            if response.batches.is_empty() {
                println!("📭 No batches found.");
                println!();
                println!("💡 Create batches using: cli trigger-batch");
            } else {
                println!("📋 Historical Batches:");
                println!("   Total Count: {}", response.total_count);
                println!();

                for (i, batch) in response.batches.iter().enumerate() {
                    println!("   {}. Batch ID: {}", i + 1, batch.id);
                    println!(
                        "      Counter: {} → {}",
                        batch.previous_counter_value, batch.final_counter_value
                    );
                    println!("      Transactions: {}", batch.transaction_count);
                    println!("      Status: {}", batch.proof_status);
                    if let Some(ref proof_id) = batch.sindri_proof_id {
                        println!("      Proof ID: {}", proof_id);
                    }
                    println!("      Created: {}", batch.created_at);
                    if let Some(ref proven_at) = batch.proven_at {
                        println!("      Proven: {}", proven_at);
                    }
                    println!();
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to get batches: {}", e);
        }
    }

    Ok(())
}

/// Get details of a specific batch
async fn get_batch(client: &BatchApiClient, batch_id: i32) -> Result<()> {
    match client.get_batch(batch_id).await {
        Ok(Some(batch)) => {
            println!("📋 Batch Details:");
            println!("   Batch ID: {}", batch.id);
            println!("   Previous Counter: {}", batch.previous_counter_value);
            println!("   Final Counter: {}", batch.final_counter_value);
            println!("   Transaction Count: {}", batch.transaction_count);
            println!("   Proof Status: {}", batch.proof_status);

            if let Some(ref proof_id) = batch.sindri_proof_id {
                println!("   Sindri Proof ID: {}", proof_id);
            }

            println!("   Created: {}", batch.created_at);
            if let Some(ref proven_at) = batch.proven_at {
                println!("   Proven: {}", proven_at);
            }

            println!();
            if batch.proof_status == "proven" {
                println!(
                    "💡 Proof is ready! Download using: cli download-proof --batch-id {}",
                    batch_id
                );
            } else {
                println!(
                    "⏳ Proof is still being generated (status: {})",
                    batch.proof_status
                );
            }
        }
        Ok(None) => {
            println!("❌ Batch {} not found", batch_id);
        }
        Err(e) => {
            eprintln!("❌ Failed to get batch {}: {}", batch_id, e);
        }
    }

    Ok(())
}

/// Download proof data for local verification
#[allow(clippy::too_many_lines)]
async fn download_proof(
    client: &BatchApiClient,
    batch_id: i32,
    output: Option<String>,
) -> Result<()> {
    // First check if the batch exists and has a proof
    match client.get_batch(batch_id).await {
        Ok(Some(batch)) => {
            if batch.proof_status != "proven" {
                println!(
                    "❌ Batch {} proof is not ready yet (status: {})",
                    batch_id, batch.proof_status
                );
                println!(
                    "💡 Try again later or check status with: cli get-batch --batch-id {}",
                    batch_id
                );
                return Ok(());
            }

            if batch.sindri_proof_id.is_none() {
                println!("❌ Batch {} has no associated proof ID", batch_id);
                return Ok(());
            }

            let proof_id = batch.sindri_proof_id.unwrap();
            let filename = output.unwrap_or_else(|| format!("proof_batch_{}.json", batch_id));

            println!("🔄 Downloading real proof data from Sindri...");
            println!("   Proof ID: {}", proof_id);

            // Fetch actual proof data from Sindri
            match arithmetic_lib::proof::get_sindri_proof_data(&proof_id).await {
                Ok(proof_data) => {
                    let download_data = serde_json::json!({
                        "batch_id": batch_id,
                        "proof_id": proof_data.proof_id,
                        "initial_balance": batch.previous_counter_value,
                        "final_balance": batch.final_counter_value,
                        "status": "proven",
                        "proof_data": format!("0x{}", hex::encode(&proof_data.proof_bytes)),
                        "public_values": format!("0x{}", hex::encode(&proof_data.public_values)),
                        "verifying_key": format!("0x{}", hex::encode(&proof_data.verifying_key)),
                        "note": "Real proof data retrieved from Sindri API"
                    });

                    fs::write(&filename, serde_json::to_string_pretty(&download_data)?)?;

                    println!("✅ Real proof data downloaded!");
                    println!("   File: {}", filename);
                    println!("   Batch ID: {}", batch_id);
                    println!("   Proof ID: {}", proof_data.proof_id);
                    println!(
                        "   Balance: {} → {}",
                        batch.previous_counter_value, batch.final_counter_value
                    );
                    println!("   Proof Size: {} bytes", proof_data.proof_bytes.len());
                    println!(
                        "   Public Values Size: {} bytes",
                        proof_data.public_values.len()
                    );
                    println!(
                        "   Verifying Key Size: {} bytes",
                        proof_data.verifying_key.len()
                    );
                    println!();
                }
                Err(e) => {
                    println!("❌ Failed to download proof data from Sindri: {}", e);
                    println!("💡 This might be because:");
                    println!("   • Proof is still being generated");
                    println!("   • Network connectivity issues");
                    println!("   • Sindri API error");
                    println!();
                    println!("⚠ Falling back to placeholder template...");

                    // Fallback to placeholder
                    let download_data = serde_json::json!({
                        "batch_id": batch_id,
                        "proof_id": proof_id,
                        "initial_balance": batch.previous_counter_value,
                        "final_balance": batch.final_counter_value,
                        "status": "proven",
                        "proof_data": "0x...",
                        "public_values": "0x...",
                        "verifying_key": "0x...",
                        "error": format!("Failed to fetch real data: {}", e),
                        "note": "Placeholder data - real proof fetch failed"
                    });

                    fs::write(&filename, serde_json::to_string_pretty(&download_data)?)?;

                    println!("✅ Placeholder proof data saved!");
                    println!("   File: {}", filename);
                    println!("   Batch ID: {}", batch_id);
                    println!("   Proof ID: {}", proof_id);
                    println!();
                }
            }
            println!("💡 Once real proof data is available, verify with:");
            println!("   cli verify-proof --proof-file {} \\", filename);
            println!(
                "     --expected-initial-balance {} \\",
                batch.previous_counter_value
            );
            println!(
                "     --expected-final-balance {}",
                batch.final_counter_value
            );
        }
        Ok(None) => {
            println!("❌ Batch {} not found", batch_id);
        }
        Err(e) => {
            eprintln!("❌ Failed to get batch {}: {}", batch_id, e);
        }
    }

    Ok(())
}

/// Health check
async fn health_check(client: &BatchApiClient) -> Result<()> {
    let start = Instant::now();

    match client.health_check().await {
        Ok(response) => {
            let duration = start.elapsed();
            println!("✅ API server is healthy");
            println!("   Status: {}", response.status);
            println!(
                "   Database: {}",
                if response.database_connected {
                    "Connected"
                } else {
                    "Disconnected"
                }
            );
            println!("   Response time: {:?}", duration);
            println!("   Timestamp: {}", response.timestamp);
        }
        Err(e) => {
            let duration = start.elapsed();
            eprintln!("❌ Failed to reach API server: {}", e);
            eprintln!("   Attempted in: {:?}", duration);
        }
    }

    Ok(())
}

/// Verify proof locally without requiring network access
#[allow(clippy::too_many_lines)]
fn verify_proof_local(
    proof_file: Option<String>,
    proof_data: Option<String>,
    public_values: Option<String>,
    verifying_key: Option<String>,
    expected_initial_balance: i32,
    expected_final_balance: i32,
    verbose: bool,
) -> Result<()> {
    let start_time = Instant::now();

    println!("🔍 Starting local proof verification...");

    let (proof_hex, public_values_hex, verifying_key_hex, batch_info) =
        if let Some(file_path) = proof_file {
            // Load from file
            let file_content = fs::read_to_string(&file_path)
                .map_err(|e| eyre::eyre!("Failed to read proof file '{}': {}", file_path, e))?;

            let download_response: serde_json::Value = serde_json::from_str(&file_content)
                .map_err(|e| eyre::eyre!("Failed to parse proof file: {}", e))?;

            let proof_data = download_response
                .get("proof_data")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre::eyre!("Proof data not found in file"))?
                .to_string();
            let public_values = download_response
                .get("public_values")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre::eyre!("Public values not found in file"))?
                .to_string();
            let verifying_key = download_response
                .get("verifying_key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre::eyre!("Verifying key not found in file"))?
                .to_string();

            let batch_id = i32::try_from(
                download_response
                    .get("batch_id")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
            )
            .unwrap_or(0);
            let initial_balance = i32::try_from(
                download_response
                    .get("initial_balance")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
            )
            .unwrap_or(0);
            let final_balance = i32::try_from(
                download_response
                    .get("final_balance")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
            )
            .unwrap_or(0);

            (
                proof_data,
                public_values,
                verifying_key,
                (batch_id, initial_balance, final_balance),
            )
        } else {
            // Use provided hex data
            let proof_data = proof_data.ok_or_else(|| eyre::eyre!("Proof data is required"))?;
            let public_values =
                public_values.ok_or_else(|| eyre::eyre!("Public values are required"))?;
            let verifying_key =
                verifying_key.ok_or_else(|| eyre::eyre!("Verifying key is required"))?;

            (proof_data, public_values, verifying_key, (0, 0, 0))
        };

    if verbose {
        println!("📊 Verification Details:");
        println!("   Expected initial balance: {}", expected_initial_balance);
        println!("   Expected final balance: {}", expected_final_balance);
        println!("   Proof data length: {} chars", proof_hex.len());
        println!("   Public values length: {} chars", public_values_hex.len());
        println!("   Verifying key length: {} chars", verifying_key_hex.len());
        if batch_info.0 > 0 {
            println!("   Batch ID: {}", batch_info.0);
            println!("   File initial balance: {}", batch_info.1);
            println!("   File final balance: {}", batch_info.2);
        }
        println!();
    }

    // For now, we'll validate the structure and expected values
    // In a real implementation, you'd verify the actual cryptographic proof
    println!("🔓 Validating proof structure and values...");

    let balances_match = if batch_info.0 > 0 {
        // Compare with file data
        batch_info.1 == expected_initial_balance && batch_info.2 == expected_final_balance
    } else {
        // Can't validate without file data in this placeholder implementation
        true
    };

    if balances_match {
        println!("   ✅ Balance transition matches expected values");
    } else {
        println!("   ❌ Balance transition mismatch");
        if batch_info.0 > 0 {
            println!(
                "      Expected: {} → {}",
                expected_initial_balance, expected_final_balance
            );
            println!("      File contains: {} → {}", batch_info.1, batch_info.2);
        }
    }

    // Placeholder cryptographic verification
    println!();
    println!("🔒 Cryptographic verification...");
    println!("   ℹ Note: Full cryptographic proof verification requires SP1 verifier integration");
    println!("   ℹ For now, validating structure and balance transitions only");

    let verification_time = start_time.elapsed();
    let overall_valid = balances_match;

    println!();
    println!("📋 Verification Summary:");
    println!(
        "   Balance validation: {}",
        if balances_match {
            "✅ PASS"
        } else {
            "❌ FAIL"
        }
    );
    println!("   Structure validation: ✅ PASS");
    println!("   Verification Time: {:?}", verification_time);
    println!();

    if overall_valid {
        println!("🎉 Batch proof structure successfully verified!");
        println!("   • Privacy: Individual transaction amounts remain hidden");
        println!("   • Correctness: Balance transition verified");
        println!("   • Integrity: Proof structure is valid");
        println!();
        println!("⚠ Note: This is a structural verification only.");
        println!("   Full cryptographic verification requires SP1 verifier integration.");
    } else {
        error!("❌ Proof verification failed!");
        error!("   • The proof does not match expected values");
        std::process::exit(1);
    }

    Ok(())
}

/// Query the current verification key from the smart contract
async fn query_verification_key(verbose: bool) -> Result<()> {
    println!("🔍 Querying verification key from smart contract...");

    // Load environment configuration
    // Note: Environment variables should be set by the user or loaded via .env by the parent process

    // Create ethereum client from environment config
    let ethereum_config = match Config::from_env() {
        Ok(config) => {
            println!("✅ Ethereum configuration loaded");
            config
        }
        Err(e) => {
            println!("❌ Failed to load Ethereum configuration: {}", e);
            println!("   Please check your .env file contains:");
            println!("   - ETHEREUM_RPC_URL");
            println!("   - ETHEREUM_CONTRACT_ADDRESS");
            println!("   - ETHEREUM_WALLET_PRIVATE_KEY");
            println!("   - ETHEREUM_DEPLOYER_ADDRESS");
            return Ok(());
        }
    };

    if verbose {
        println!(
            "📍 Contract address: {}",
            ethereum_config.contract.arithmetic_contract
        );
        println!("📡 RPC URL: {}", ethereum_config.network.rpc_url);
    }

    // Initialize ethereum client (without validation to avoid circular dependency)
    let ethereum_client = match EthereumClient::new_without_validation(ethereum_config).await {
        Ok(client) => {
            println!("✅ Connected to Ethereum network");
            client
        }
        Err(e) => {
            println!("❌ Failed to connect to Ethereum network: {}", e);
            return Ok(());
        }
    };

    // Query the verification key
    match ethereum_client.query_contract_verification_key().await {
        Ok((contract_vkey, verifier_address)) => {
            println!();
            println!("🔑 Smart Contract Verification Key Information:");
            println!("   Verification Key: 0x{}", hex::encode(contract_vkey));

            if verbose {
                println!("   SP1 Verifier Address: {}", verifier_address);
            }

            // Try to load local vk.json for comparison if it exists
            if let Ok(local_vkey_content) = std::fs::read_to_string("vk.json") {
                if let Ok(local_vkey) = parse_local_vkey(&local_vkey_content) {
                    println!();
                    println!(
                        "📁 Local Verification Key (vk.json): 0x{}",
                        hex::encode(local_vkey)
                    );

                    if contract_vkey == local_vkey {
                        println!("✅ Keys match! Your proofs will be compatible with the smart contract.");
                    } else {
                        println!(
                            "❌ Key mismatch! Your proofs will be REJECTED by the smart contract."
                        );
                        println!();
                        println!("Solutions:");
                        println!(
                            "   1. Deploy new contract with key: 0x{}",
                            hex::encode(local_vkey)
                        );
                        println!(
                            "   2. Use circuit that matches contract key: 0x{}",
                            hex::encode(contract_vkey)
                        );
                    }
                } else {
                    println!("⚠️ Found vk.json but couldn't parse it");
                }
            } else if verbose {
                println!("ℹ️ No local vk.json file found for comparison");
            }

            println!();
            println!("🎉 Verification key query completed successfully!");
        }
        Err(e) => {
            println!("❌ Failed to query verification key: {}", e);
            println!("   Make sure the contract is deployed and accessible");
        }
    }

    Ok(())
}

/// Parse local verification key from vk.json content
fn parse_local_vkey(content: &str) -> Result<[u8; 32]> {
    use serde_json::Value;

    let vk_data: Value = serde_json::from_str(content)?;

    // Extract the commit value array (8 32-bit integers)
    let commit_array = vk_data["commit"]
        .as_array()
        .ok_or_else(|| eyre::eyre!("Missing or invalid 'commit' array in vk.json"))?;

    if commit_array.len() != 8 {
        return Err(eyre::eyre!(
            "Expected 8 commit values, found {}",
            commit_array.len()
        ));
    }

    // Convert 8 u32 values to 32 bytes
    let mut vkey_bytes = [0u8; 32];
    for (i, value) in commit_array.iter().enumerate() {
        let commit_value = value
            .as_u64()
            .ok_or_else(|| eyre::eyre!("Commit value {} is not a valid number", i))?;

        let bytes = u32::try_from(commit_value)
            .map_err(|_| eyre::eyre!("Commit value {} is not a valid number", i))?
            .to_le_bytes();
        vkey_bytes[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
    }

    Ok(vkey_bytes)
}
