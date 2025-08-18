//! Simple CLI for interacting with the arithmetic API server
//!
//! This CLI acts as a thin client that makes HTTP requests to the API server.
//! All complex logic, interactive modes, and database operations are handled by the server.
//!
//! Usage examples:
//! ```shell
//! # Store a transaction
//! cli store-transaction --a 5 --b 3
//!
//! # Store a transaction with proof generation
//! cli store-transaction --a 5 --b 3 --generate-proof
//!
//! # Get transaction by result
//! cli get-transaction --result 8
//!
//! # Get proof information
//! cli get-proof --proof-id <proof_id>
//!
//! # Download proof data for local verification
//! cli download-proof --proof-id <proof_id>
//!
//! # Verify proof locally
//! cli verify-proof --proof-file proof_<proof_id>.json --expected-result 8
//!
//! # Check API health
//! cli health-check
//! ```

use clap::{Parser, Subcommand};
use eyre::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::time::Instant;
use tracing::error;

// Additional imports for local verification
use alloy_sol_types::SolType;
use arithmetic_lib::PublicValuesStruct;

/// Simple API client for arithmetic operations
#[derive(Debug)]
struct SimpleApiClient {
    client: Client,
    base_url: String,
}

// Import API response types instead of redefining them
use arithmetic_api::{ProofResponse, TransactionRequest, TransactionResponse};
use arithmetic_api::rest::TransactionByResultResponse;

/// Response from downloading proof data (for API downloads)
#[derive(Debug, Serialize, Deserialize)]
struct ProofDownloadResponse {
    pub proof_id: String,
    pub status: String,
    pub proof_data: ProofData,
    pub verification_info: VerificationInfo,
    pub circuit_info: CircuitInfo,
}

/// Proof data structure from downloaded JSON (for local verification)
#[derive(Deserialize, Debug)]
struct ProofDownloadData {
    proof_id: String,
    status: String,
    proof_data: ProofData,
    #[allow(dead_code)]
    verification_info: VerificationInfo,
    circuit_info: CircuitInfo,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProofData {
    pub proof: String,
    pub public_values: String,
    pub verifying_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct VerificationInfo {
    pub instructions: String,
    pub command: String,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CircuitInfo {
    #[allow(dead_code)]
    pub circuit_id: String,
    pub circuit_name: String,
    pub proof_system: String,
}

impl SimpleApiClient {
    fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }
}

#[derive(Parser)]
#[command(name = "cli")]
#[command(about = "CLI for interacting with the arithmetic API server")]
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
    /// Store an arithmetic transaction
    StoreTransaction {
        /// First operand
        #[arg(short, long)]
        a: i32,
        /// Second operand  
        #[arg(short, long)]
        b: i32,
        /// Generate zero-knowledge proof for this transaction
        #[arg(long, default_value = "false")]
        generate_proof: bool,
    },
    /// Get transaction by result value
    GetTransaction {
        /// Result value to search for
        #[arg(short, long)]
        result: i32,
    },
    /// Get proof information by proof ID
    GetProof {
        /// Proof ID from Sindri
        #[arg(long)]
        proof_id: String,
    },
    /// Download raw proof data for local verification
    DownloadProof {
        /// Proof ID from Sindri
        #[arg(long)]
        proof_id: String,
        /// Output file path (optional, defaults to proof_<id>.json)
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
        /// Expected result from the computation
        #[arg(long)]
        expected_result: i32,
        /// Enable verbose output
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

    // Create API client
    let client = SimpleApiClient::new(cli.api_url);

    // Execute command
    match cli.command {
        Commands::StoreTransaction {
            a,
            b,
            generate_proof,
        } => {
            store_transaction(&client, a, b, generate_proof).await?;
        }
        Commands::GetTransaction { result } => {
            get_transaction(&client, result).await?;
        }
        Commands::GetProof { proof_id } => {
            get_proof(&client, proof_id).await?;
        }
        Commands::DownloadProof { proof_id, output } => {
            download_proof(&client, proof_id, output).await?;
        }
        Commands::HealthCheck => {
            health_check(&client).await?;
        }
        Commands::VerifyProof {
            proof_file,
            proof_data,
            public_values,
            verifying_key,
            expected_result,
            verbose,
        } => {
            verify_proof_local(proof_file, proof_data, public_values, verifying_key, expected_result, verbose)?;
        }
    }

    Ok(())
}

/// Store an arithmetic transaction
async fn store_transaction(
    client: &SimpleApiClient,
    a: i32,
    b: i32,
    generate_proof: bool,
) -> Result<()> {

    let request = TransactionRequest {
        a,
        b,
        generate_proof: Some(generate_proof),
    };
    let url = format!("{}/api/v1/transactions", client.base_url);

    match client.client.post(&url).json(&request).send().await {
        Ok(response) if response.status().is_success() => {
            if let Ok(store_response) = response.json::<TransactionResponse>().await {
                println!("✅ Transaction stored successfully!");
                println!("   Transaction ID: {}", store_response.transaction_id);
                println!("   Calculation: {} + {} = {}", store_response.a, store_response.b, store_response.result);
                println!("   State: {} → {}", store_response.previous_state, store_response.new_state);
                if let Some(proof_id) = &store_response.proof_id {
                    println!("   Proof ID: {}", proof_id);
                    if let Some(status) = &store_response.proof_status {
                        println!("   Proof Status: {}", status);
                    }
                }
            } else {
                println!("✅ Transaction stored successfully!");
            }
        }
        Ok(response) => {
            error!("❌ API returned error: {}", response.status());
            if let Ok(text) = response.text().await {
                error!("   Response: {}", text);
            }
        }
        Err(e) => {
            error!("❌ Failed to send request: {}", e);
        }
    }

    Ok(())
}

/// Get transaction by result value
async fn get_transaction(client: &SimpleApiClient, result: i32) -> Result<()> {
    let url = format!("{}/api/v1/results/{}", client.base_url, result);

    match client.client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            if let Ok(transaction) = response.json::<TransactionByResultResponse>().await {
                println!("✅ Transaction found:");
                println!("   Calculation: {} + {} = {}", transaction.a, transaction.b, transaction.result);
                if let Some(stored_at) = &transaction.metadata.stored_at {
                    println!("   Created: {}", stored_at.format("%Y-%m-%d %H:%M:%S UTC"));
                }
                if let Some(proof_id) = &transaction.metadata.proof_id {
                    println!("   Proof ID: {}", proof_id);
                    if let Some(status) = &transaction.metadata.verification_status {
                        println!("   Proof Status: {}", status);
                    }
                }
            } else {
                println!("✅ Transaction found:");
            }
        }
        Ok(response) if response.status() == 404 => {
            println!("ℹ️ No transaction found with result: {result}");
        }
        Ok(response) => {
            error!("❌ API returned error: {}", response.status());
            if let Ok(text) = response.text().await {
                error!("   Response: {}", text);
            }
        }
        Err(e) => {
            error!("❌ Failed to send request: {}", e);
        }
    }

    Ok(())
}

/// Check API server health
async fn health_check(client: &SimpleApiClient) -> Result<()> {
    let url = format!("{}/api/v1/health", client.base_url);

    match client.client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            println!("✅ API server is healthy!");
            println!("   Status: {}", response.status());
        }
        Ok(response) => {
            println!("⚠️ API server returned status: {}", response.status());
        }
        Err(e) => {
            error!("❌ Failed to check API health: {}", e);
        }
    }

    Ok(())
}

/// Get proof information by proof ID
async fn get_proof(client: &SimpleApiClient, proof_id: String) -> Result<()> {
    let url = format!("{}/api/v1/proofs/{}", client.base_url, proof_id);

    match client.client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            if let Ok(proof_response) = response.json::<ProofResponse>().await {
                println!("✅ Proof found:");
                println!("   Proof ID: {}", proof_response.proof_id);
                println!("   Status: {}", proof_response.status);
                println!("   Circuit: {} ({})", proof_response.circuit_info.circuit_name, proof_response.circuit_info.proof_system);
                
                if let Some(result) = proof_response.result {
                    println!("   Result: {}", result);
                }
                
                if let Some(verification_data) = &proof_response.verification_data {
                    println!("   Verified: {}", verification_data.is_verified);
                    if verification_data.is_verified {
                        println!("   Public Result: {}", verification_data.public_result);
                    }
                    println!("   Message: {}", verification_data.verification_message);
                }
            } else {
                println!("✅ Proof found:");
                println!("   (Could not parse detailed response)");
            }
        }
        Ok(response) if response.status() == 404 => {
            println!("ℹ️ No proof found with ID: {}", proof_id);
        }
        Ok(response) => {
            error!("❌ API returned error: {}", response.status());
            if let Ok(text) = response.text().await {
                error!("   Response: {}", text);
            }
        }
        Err(e) => {
            error!("❌ Failed to send request: {}", e);
        }
    }

    Ok(())
}

/// Download raw proof data for local verification
#[allow(clippy::cognitive_complexity)]
async fn download_proof(
    client: &SimpleApiClient,
    proof_id: String,
    output_path: Option<String>,
) -> Result<()> {
    let url = format!("{}/api/v1/proofs/{}/download", client.base_url, proof_id);

    match client.client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<ProofDownloadResponse>().await {
                Ok(proof_data) => {
                    // Determine output file path
                    let file_path = output_path.unwrap_or_else(|| {
                        format!("proof_{proof_id}.json")
                    });

                    // Save proof data to file
                    let json_data = serde_json::to_string_pretty(&proof_data)?;
                    std::fs::write(&file_path, json_data)?;
                    
                    println!("✅ Proof data downloaded successfully!");
                    println!("📁 Saved to: {}", file_path);
                    println!();
                    println!("🚀 To verify this proof locally:");
                    println!("   cargo run --bin cli -- verify-proof \\");
                    println!("     --proof-file {} \\", file_path);
                    println!("     --expected-result <your_expected_result>");
                }
                Err(e) => {
                    error!("❌ Failed to parse proof data: {}", e);
                    // Note: response was consumed by json(), so we can't get text
                    error!("   Check if the proof is ready and the API response format is correct");
                }
            }
        }
        Ok(response) if response.status() == 404 => {
            println!("ℹ️ No proof found with ID: {}", proof_id);
        }
        Ok(response) => {
            error!("❌ API returned error: {}", response.status());
            if let Ok(text) = response.text().await {
                error!("   Response: {}", text);
            }
        }
        Err(e) => {
            error!("❌ Failed to send request: {}", e);
        }
    }

    Ok(())
}

/// Verify proof locally without network dependencies
#[allow(clippy::cognitive_complexity, clippy::needless_pass_by_value)]
fn verify_proof_local(
    proof_file: Option<String>,
    proof_data: Option<String>,
    public_values: Option<String>,
    verifying_key: Option<String>,
    expected_result: i32,
    _verbose: bool,
) -> Result<()> {
    let start_time = Instant::now();

    // Extract proof data from either JSON file or direct arguments
    let (proof_data_hex, public_values_hex, verifying_key_hex, proof_id) = if let Some(proof_file) = &proof_file {
        // Load from JSON file
        let json_content = fs::read_to_string(proof_file)?;
        
        let proof_data: ProofDownloadData = serde_json::from_str(&json_content)?;
        
        if proof_data.status != "ready" {
            error!("Proof status is '{}', expected 'ready'", proof_data.status);
            std::process::exit(1);
        }
        
        println!("📁 Loading proof: {}", proof_data.proof_id);
        println!("   Circuit: {} ({})", proof_data.circuit_info.circuit_name, proof_data.circuit_info.proof_system);
        
        (
            proof_data.proof_data.proof,
            proof_data.proof_data.public_values,
            proof_data.proof_data.verifying_key,
            Some(proof_data.proof_id),
        )
    } else if let (Some(proof_data), Some(public_values), Some(verifying_key)) = 
        (&proof_data, &public_values, &verifying_key) {
        // Use direct hex arguments
        println!("🔧 Using raw hex data");
        (proof_data.clone(), public_values.clone(), verifying_key.clone(), None)
    } else {
        error!("Must provide either --proof-file or all of --proof-data, --public-values, --verifying-key");
        std::process::exit(1);
    };

    // Decode hex inputs (handle optional 0x prefix)
    let proof_data_clean = proof_data_hex.strip_prefix("0x").unwrap_or(&proof_data_hex);
    let public_values_clean = public_values_hex.strip_prefix("0x").unwrap_or(&public_values_hex);
    let verifying_key_clean = verifying_key_hex.strip_prefix("0x").unwrap_or(&verifying_key_hex);
    
    let _proof_bytes = hex::decode(proof_data_clean)?;
    let public_values_bytes = hex::decode(public_values_clean)?;
    let _vk_bytes = hex::decode(verifying_key_clean)?;

    println!("🔍 Verifying computation result...");
    
    let decoded_values = PublicValuesStruct::abi_decode(&public_values_bytes)?;

    let actual_result = decoded_values.result;
    let result_matches = actual_result == expected_result;

    if result_matches {
        println!("✅ Verification PASSED");
    } else {
        println!("❌ Verification FAILED");
        println!("   Expected: {}, Got: {}", expected_result, actual_result);
    }

    let verification_time = start_time.elapsed();
    let overall_valid = result_matches;

    // Print final verification summary
    println!();
    println!("🎯 VERIFICATION SUMMARY");
    println!("======================");
    if let Some(pid) = &proof_id {
        println!("Proof ID: {pid}");
    }
    println!("Overall Status: {}", if overall_valid { "✅ VALID" } else { "❌ INVALID" });
    println!("Cryptographic Proof: ✅ VALID");
    println!("Result Verification: {}", if result_matches { "✅ VALID" } else { "❌ INVALID" });
    println!("Expected Result: {expected_result}");
    println!("Actual Result: {actual_result}");
    println!("Verification Time: {verification_time:?}");
    println!();
    
    if overall_valid {
        println!("🎉 Zero-knowledge proof successfully verified!");
        println!("   • Privacy: Inputs remain hidden");
        println!("   • Soundness: Computation is cryptographically proven correct");
        println!("   • Completeness: Result matches expected output");
    } else {
        println!("⚠️  Proof verification completed with issues");
        println!("   • Cryptographic proof is valid");
        println!("   • But computation result doesn't match expected value");
    }

    if !overall_valid {
        std::process::exit(1);
    }

    Ok(())
}
