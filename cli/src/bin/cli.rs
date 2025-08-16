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
//! # Get transaction by result
//! cli get-transaction --result 8
//!
//! # Check API health
//! cli health-check
//! ```

use clap::{Parser, Subcommand};
use eyre::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{error, info};

/// Simple API client for arithmetic operations
#[derive(Debug)]
struct SimpleApiClient {
    client: Client,
    base_url: String,
}

/// Request to store an arithmetic transaction
#[derive(Debug, Serialize)]
struct StoreTransactionRequest {
    pub a: i32,
    pub b: i32,
    pub generate_proof: bool,
}

/// Response from storing an arithmetic transaction
#[derive(Debug, Deserialize)]
struct StoreTransactionResponse {
    pub transaction_id: i32,
    pub success: bool,
}

/// Transaction data response
#[derive(Debug, Deserialize)]
struct Transaction {
    pub id: i32,
    pub a: i32,
    pub b: i32,
    pub result: i32,
    pub created_at: String,
}

/// Request to verify a proof
#[derive(Debug, Serialize)]
struct VerifyProofRequest {
    pub proof_id: String,
    pub expected_result: i32,
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
    /// Verify proof with proof ID and expected result
    VerifyProof {
        /// Proof ID from Sindri
        #[arg(long)]
        proof_id: String,
        /// Expected result to verify against
        #[arg(long)]
        expected_result: i32,
    },
    /// Check API server health
    HealthCheck,
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
        Commands::VerifyProof {
            proof_id,
            expected_result,
        } => {
            verify_proof(&client, proof_id, expected_result).await?;
        }
        Commands::HealthCheck => {
            health_check(&client).await?;
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
    let result = a + b;

    info!(
        "Storing transaction: {} + {} = {} (generate_proof: {})",
        a, b, result, generate_proof
    );

    let request = StoreTransactionRequest {
        a,
        b,
        generate_proof,
    };
    let url = format!("{}/api/v1/transactions", client.base_url);

    match client.client.post(&url).json(&request).send().await {
        Ok(response) if response.status().is_success() => {
            info!("✅ Transaction stored successfully!");
            info!("   Status: {}", response.status());
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
    info!("Looking up transaction with result: {}", result);

    let url = format!("{}/api/v1/results/{}", client.base_url, result);

    match client.client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            info!("✅ Transaction found:");
            if let Ok(text) = response.text().await {
                info!("   Response: {}", text);
            }
        }
        Ok(response) if response.status() == 404 => {
            info!("ℹ️ No transaction found with result: {}", result);
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
    info!("Checking API server health...");

    let url = format!("{}/api/v1/health", client.base_url);

    match client.client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            info!("✅ API server is healthy!");
            info!("   Status: {}", response.status());
        }
        Ok(response) => {
            info!("⚠️ API server returned status: {}", response.status());
        }
        Err(e) => {
            error!("❌ Failed to check API health: {}", e);
        }
    }

    Ok(())
}

/// Get proof information by proof ID
async fn get_proof(client: &SimpleApiClient, proof_id: String) -> Result<()> {
    info!("Getting proof information for: {}", proof_id);

    let url = format!("{}/api/v1/proofs/{}", client.base_url, proof_id);

    match client.client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            info!("✅ Proof found:");
            if let Ok(text) = response.text().await {
                info!("   Response: {}", text);
            }
        }
        Ok(response) if response.status() == 404 => {
            info!("ℹ️ No proof found with ID: {}", proof_id);
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

/// Verify proof with proof ID and expected result
async fn verify_proof(
    client: &SimpleApiClient,
    proof_id: String,
    expected_result: i32,
) -> Result<()> {
    info!(
        "Verifying proof {} with expected result: {}",
        proof_id, expected_result
    );

    let request = VerifyProofRequest {
        proof_id: proof_id.clone(),
        expected_result,
    };
    let url = format!("{}/api/v1/verify", client.base_url);

    match client.client.post(&url).json(&request).send().await {
        Ok(response) if response.status().is_success() => {
            info!("✅ Proof verification completed:");
            if let Ok(text) = response.text().await {
                info!("   Response: {}", text);
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
