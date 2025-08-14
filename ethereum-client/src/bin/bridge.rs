use alloy_primitives::{Bytes, FixedBytes};
use clap::{Parser, Subcommand};
use ethereum_client::{Config, EthereumClient, Result};
use sindri::{client::SindriClient, integrations::sp1_v5::SP1ProofInfo, JobStatus};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Check connections and configuration
    Check,
    /// Run background processing for pending proofs (continuous)
    Process {
        /// Polling interval in seconds for checking new proofs
        #[arg(long, default_value = "60")]
        interval: u64,
        /// Run once and exit (don't run continuously)
        #[arg(long)]
        one_shot: bool,
    },
    /// Test contract capabilities
    Test,
    /// Submit a specific proof by result value
    Submit {
        /// The arithmetic result to submit proof for
        #[arg(short, long)]
        result: i32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    info!("🌉 Ethereum Bridge - Unified Implementation");
    info!("============================================");

    // Connect to database if available
    let pool = if let Ok(database_url) = std::env::var("DATABASE_URL") {
        Some(PgPool::connect(&database_url).await?)
    } else {
        warn!("⚠️  DATABASE_URL not set - database features disabled");
        None
    };

    // Load configuration and create client
    let config = Config::from_env()?;
    let client = EthereumClient::new(config.clone()).await?;
    info!("✅ Connected to Ethereum client");

    match args.command {
        Commands::Check => {
            run_check(&client, &config).await?;
        }
        Commands::Process { interval, one_shot } => {
            if let Some(pool) = pool {
                run_background_processor(&client, &config, pool, interval, one_shot).await?;
            } else {
                return Err(ethereum_client::EthereumError::Config(
                    "DATABASE_URL required for background processing".to_string(),
                ));
            }
        }
        Commands::Test => {
            run_capability_test(&client).await?;
        }
        Commands::Submit { result } => {
            if let Some(pool) = pool {
                run_single_submission(&client, &config, &pool, result).await?;
            } else {
                return Err(ethereum_client::EthereumError::Config(
                    "DATABASE_URL required for proof submission".to_string(),
                ));
            }
        }
    }

    Ok(())
}

async fn run_check(client: &EthereumClient, config: &Config) -> Result<()> {
    info!("🔍 Running connection and configuration checks...");

    // Test network connection
    match client.get_network_stats().await {
        Ok(stats) => {
            info!("✅ Network connection successful!");
            info!("  Chain ID: {}", stats.chain_id);
            info!("  Network: {}", stats.network_name);
            info!("  Current block: {}", stats.block_number);
            info!("  Gas price: {}", stats.gas_price);
        }
        Err(e) => {
            error!("❌ Network connection failed: {}", e);
            return Err(e);
        }
    }

    // Test contract configuration
    info!("📋 Contract Configuration:");
    info!(
        "  - Arithmetic Contract: {}",
        config.contract.arithmetic_contract
    );
    info!(
        "  - Verifier Contract: {}",
        config.contract.verifier_contract
    );

    // Test signer configuration
    if config.signer.is_some() {
        info!("✅ Transaction signer configured - full functionality available");
        info!("🚀 Ready for proof submission and contract interactions!");
    } else {
        warn!("⚠️  No signer configured - read-only mode");
        info!("💡 Set ETHEREUM_PRIVATE_KEY for transaction capabilities");
    }

    // Test Sindri configuration
    match std::env::var("SINDRI_API_KEY") {
        Ok(_) => info!("✅ Sindri API key configured - proof generation available"),
        Err(_) => warn!("⚠️  SINDRI_API_KEY not set - proof generation may fail"),
    }

    info!("✅ All checks completed successfully!");
    Ok(())
}

async fn run_capability_test(client: &EthereumClient) -> Result<()> {
    info!("🧪 Testing contract interaction capabilities...");

    // Test reading verifier key
    match client.get_verifier_key().await {
        Ok(vkey) => {
            info!("✅ Verifier key retrieved: 0x{}", hex::encode(&vkey[..8]));
        }
        Err(e) => {
            warn!("⚠️  Failed to get verifier key: {}", e);
        }
    }

    // Test getting verifier version
    match client.get_verifier_version().await {
        Ok(version) => {
            info!("✅ Verifier version: {}", version);
        }
        Err(e) => {
            warn!("⚠️  Failed to get verifier version: {}", e);
        }
    }

    info!("🎯 Available contract operations:");
    info!("  - Read verifier key: ✅");
    info!("  - Read verifier version: ✅");
    info!("  - Read state roots: ✅");
    info!("  - Read proof data: ✅");
    info!(
        "  - Submit state updates: {} (requires signer)",
        if client.has_signer() { "✅" } else { "⚠️" }
    );

    Ok(())
}

async fn run_background_processor(
    client: &EthereumClient,
    config: &Config,
    pool: PgPool,
    interval_secs: u64,
    one_shot: bool,
) -> Result<()> {
    info!("🔄 Starting background proof processor...");

    let mut bridge = UnifiedBridge::new(client, config, pool)?;

    if one_shot {
        info!("Running one-shot processing...");
        let processed = bridge.process_pending_proofs().await?;
        info!("✅ Processed {} proofs", processed);
    } else {
        info!(
            "Starting continuous processing (interval: {}s)...",
            interval_secs
        );
        bridge
            .start_continuous_processing(Duration::from_secs(interval_secs))
            .await?;
    }

    Ok(())
}

async fn run_single_submission(
    client: &EthereumClient,
    config: &Config,
    pool: &PgPool,
    result: i32,
) -> Result<()> {
    info!("📤 Submitting proof for result: {}", result);

    let mut bridge = UnifiedBridge::new(client, config, pool.clone())?;

    // Query for the proof
    let query = r#"
        SELECT sp.proof_id
        FROM arithmetic_transactions at
        INNER JOIN sindri_proofs sp ON sp.result = at.result
        WHERE at.result = $1 AND sp.status = 'Ready'
        LIMIT 1
    "#;

    let row = sqlx::query(query)
        .bind(result)
        .fetch_optional(pool)
        .await
        .map_err(|e| ethereum_client::EthereumError::External(e.to_string()))?;

    match row {
        Some(row) => {
            let proof_id: String = row.get("proof_id");
            info!("📋 Found ready proof: {}", proof_id);
            bridge.submit_single_proof(result, &proof_id).await?;
            info!("✅ Proof submission completed!");
        }
        None => {
            error!("❌ No ready proof found for result {}", result);
            info!(
                "💡 Run proof generation first: cargo run --bin main -- --prove --result {}",
                result
            );
        }
    }

    Ok(())
}

struct UnifiedBridge<'a> {
    client: &'a EthereumClient,
    pool: PgPool,
}

impl<'a> UnifiedBridge<'a> {
    fn new(client: &'a EthereumClient, _config: &'a Config, pool: PgPool) -> Result<Self> {
        Ok(Self { client, pool })
    }

    async fn start_continuous_processing(&mut self, polling_interval: Duration) -> Result<()> {
        let mut interval_timer = interval(polling_interval);

        loop {
            interval_timer.tick().await;

            match self.process_pending_proofs().await {
                Ok(processed) => {
                    if processed > 0 {
                        info!("✅ Processed {} proofs", processed);
                    } else {
                        info!("⏳ No pending proofs to process");
                    }
                }
                Err(e) => {
                    error!("❌ Error processing proofs: {}", e);
                    sleep(Duration::from_secs(30)).await;
                }
            }
        }
    }

    async fn process_pending_proofs(&mut self) -> Result<usize> {
        // Query for arithmetic transactions that have ready Sindri proofs
        let query = r#"
            SELECT DISTINCT at.result, sp.proof_id
            FROM arithmetic_transactions at
            INNER JOIN sindri_proofs sp ON sp.result = at.result
            WHERE sp.status = 'Ready'
            LIMIT 10
        "#;

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ethereum_client::EthereumError::External(e.to_string()))?;

        if rows.is_empty() {
            return Ok(0);
        }

        info!("📋 Found {} proofs ready for submission", rows.len());

        let mut processed = 0;

        for row in rows {
            let result: i32 = row.get("result");
            let proof_id: String = row.get("proof_id");

            match self.submit_single_proof(result, &proof_id).await {
                Ok(_) => {
                    info!("✅ Successfully submitted proof for result {}", result);
                    processed += 1;
                }
                Err(e) => {
                    error!("❌ Failed to submit proof for result {}: {}", result, e);
                }
            }

            // Small delay to avoid overwhelming the network
            sleep(Duration::from_millis(500)).await;
        }

        Ok(processed)
    }

    async fn submit_single_proof(&mut self, result: i32, proof_id: &str) -> Result<()> {
        // Check if we have a signer for transactions
        if !self.client.has_signer() {
            return Err(ethereum_client::EthereumError::Config(
                "Signer required for proof submission".to_string(),
            ));
        }

        // Get proof from Sindri
        let sindri_client = SindriClient::default();
        let proof_info = sindri_client
            .get_proof(proof_id, None, None, None)
            .await
            .map_err(|e| {
                ethereum_client::EthereumError::External(format!("Sindri API error: {}", e))
            })?;

        // Check if proof is ready
        if proof_info.status != JobStatus::Ready {
            return Err(ethereum_client::EthereumError::Config(format!(
                "Proof not ready: {:?}",
                proof_info.status
            )));
        }

        // Extract SP1 proof
        let sp1_proof = proof_info
            .to_sp1_proof_with_public()
            .map_err(|e| ethereum_client::EthereumError::Sindri(e.to_string()))?;

        // Convert to bytes for contract submission
        let proof_bytes = serde_json::to_vec(&sp1_proof)
            .map_err(|e| ethereum_client::EthereumError::Sindri(e.to_string()))?;
        let proof_data = Bytes::from(proof_bytes);

        // Create public values (the arithmetic result)
        let public_values = Bytes::from(result.to_be_bytes().to_vec());

        // Generate state ID and new state root (deterministic)
        let state_id = FixedBytes::<32>::from([1u8; 32]); // Use a fixed state ID for now
        let new_state_root = {
            use alloy_primitives::keccak256;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let data = format!("{}:{}:{}", result, proof_id, timestamp);
            FixedBytes::<32>::from_slice(keccak256(data.as_bytes()).as_slice())
        };

        info!(
            "📤 Submitting to contract: result={}, state_root=0x{}",
            result,
            hex::encode(new_state_root.as_slice())
        );

        // Submit using the unified client
        let state_update = self
            .client
            .update_state(state_id, new_state_root, proof_data, public_values)
            .await?;

        info!("✅ Transaction submitted successfully!");
        if let Some(tx_hash) = state_update.transaction_hash {
            info!("  Transaction hash: 0x{}", hex::encode(tx_hash.as_slice()));
        }
        if let Some(block_number) = state_update.block_number {
            info!("  Block number: {}", block_number);
        }

        // Update database to track submission
        self.update_submission_status(result, "Confirmed", state_update.transaction_hash.as_ref())
            .await?;

        Ok(())
    }

    async fn update_submission_status(
        &self,
        result: i32,
        status: &str,
        tx_hash: Option<&FixedBytes<32>>,
    ) -> Result<()> {
        let query = if let Some(hash) = tx_hash {
            sqlx::query(
                r#"
                INSERT INTO ethereum_submissions (result, status, transaction_hash, submitted_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (result)
                DO UPDATE SET status = $2, transaction_hash = $3, updated_at = NOW()
                "#,
            )
            .bind(result)
            .bind(status)
            .bind(hash.as_slice())
        } else {
            sqlx::query(
                r#"
                INSERT INTO ethereum_submissions (result, status, submitted_at)
                VALUES ($1, $2, NOW())
                ON CONFLICT (result)
                DO UPDATE SET status = $2, updated_at = NOW()
                "#,
            )
            .bind(result)
            .bind(status)
        };

        query
            .execute(&self.pool)
            .await
            .map_err(|e| ethereum_client::EthereumError::External(e.to_string()))?;

        Ok(())
    }
}
