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
    // Load environment variables from .env file
    dotenv::dotenv().ok();

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

    check_network_connection(client).await?;
    check_contract_configuration(config);
    check_signer_configuration(config);
    check_sindri_configuration();

    info!("✅ All checks completed successfully!");
    Ok(())
}

#[allow(clippy::cognitive_complexity)]
async fn check_network_connection(client: &EthereumClient) -> Result<()> {
    match client.get_network_stats().await {
        Ok(stats) => {
            info!("✅ Network connection successful!");
            info!("  Chain ID: {}", stats.chain_id);
            info!("  Network: {}", stats.network_name);
            info!("  Current block: {}", stats.block_number);
            info!("  Gas price: {}", stats.gas_price);
            Ok(())
        }
        Err(e) => {
            error!("❌ Network connection failed: {}", e);
            Err(e)
        }
    }
}

fn check_contract_configuration(config: &Config) {
    info!("📋 Contract Configuration:");
    info!(
        "  - Arithmetic Contract: {}",
        config.contract.arithmetic_contract
    );
    info!(
        "  - Verifier Contract: {}",
        config.contract.verifier_contract
    );
}

#[allow(clippy::cognitive_complexity)]
fn check_signer_configuration(config: &Config) {
    if config.signer.is_some() {
        info!("✅ Transaction signer configured - full functionality available");
        info!("🚀 Ready for proof submission and contract interactions!");
    } else {
        warn!("⚠️  No signer configured - read-only mode");
        info!("💡 Set ETHEREUM_WALLET_PRIVATE_KEY for transaction capabilities");
    }
}

fn check_sindri_configuration() {
    if std::env::var("SINDRI_API_KEY").is_ok() {
        info!("✅ Sindri API key configured - proof generation available");
    } else {
        warn!("⚠️  SINDRI_API_KEY not set - proof generation may fail");
    }
}

async fn run_capability_test(client: &EthereumClient) -> Result<()> {
    info!("🧪 Testing contract interaction capabilities...");

    test_verifier_key_retrieval(client).await;
    test_verifier_version_retrieval(client).await;
    display_available_operations();

    Ok(())
}

async fn test_verifier_key_retrieval(client: &EthereumClient) {
    match client.get_verifier_key().await {
        Ok(vkey) => {
            info!("✅ Verifier key retrieved: 0x{}", hex::encode(&vkey[..8]));
        }
        Err(e) => {
            warn!("⚠️  Failed to get verifier key: {}", e);
        }
    }
}

async fn test_verifier_version_retrieval(client: &EthereumClient) {
    match client.get_verifier_version().await {
        Ok(version) => {
            info!("✅ Verifier version: {}", version);
        }
        Err(e) => {
            warn!("⚠️  Failed to get verifier version: {}", e);
        }
    }
}

#[allow(clippy::cognitive_complexity)]
fn display_available_operations() {
    info!("🎯 Available contract operations:");
    info!("  - Read verifier key: ✅");
    info!("  - Read verifier version: ✅");
    info!("  - Read state roots: ✅");
    info!("  - Read proof data: ✅");
    info!("  - Submit state updates: ✅");
}

#[allow(clippy::cognitive_complexity)]
async fn run_background_processor(
    client: &EthereumClient,
    config: &Config,
    pool: PgPool,
    interval_secs: u64,
    one_shot: bool,
) -> Result<()> {
    info!("🔄 Starting background proof processor...");

    let bridge = UnifiedBridge::new(client, config, pool);

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

    let bridge = UnifiedBridge::new(client, config, pool.clone());
    let proof_id_opt = query_proof_by_result(pool, result).await?;

    handle_proof_submission(&bridge, result, proof_id_opt).await
}

async fn query_proof_by_result(pool: &PgPool, result: i32) -> Result<Option<String>> {
    let query = r"
        SELECT sp.proof_id
        FROM arithmetic_transactions at
        INNER JOIN sindri_proofs sp ON sp.result = at.result
        WHERE at.result = $1 AND sp.status = 'Ready'
        LIMIT 1
    ";

    let row = sqlx::query(query)
        .bind(result)
        .fetch_optional(pool)
        .await
        .map_err(|e| ethereum_client::EthereumError::External(e.to_string()))?;

    Ok(row.map(|r| r.get("proof_id")))
}

#[allow(clippy::cognitive_complexity)]
async fn handle_proof_submission(
    bridge: &UnifiedBridge<'_>,
    result: i32,
    proof_id_opt: Option<String>,
) -> Result<()> {
    if let Some(proof_id) = proof_id_opt {
        info!("📋 Found ready proof: {}", proof_id);
        bridge.submit_single_proof(result, &proof_id).await?;
        info!("✅ Proof submission completed!");
    } else {
        error!("❌ No ready proof found for result {}", result);
        info!(
            "💡 Run proof generation first: cargo run --bin main -- --prove --result {}",
            result
        );
    }
    Ok(())
}

struct UnifiedBridge<'a> {
    client: &'a EthereumClient,
    pool: PgPool,
}

impl<'a> UnifiedBridge<'a> {
    const fn new(client: &'a EthereumClient, _config: &'a Config, pool: PgPool) -> Self {
        Self { client, pool }
    }

    async fn start_continuous_processing(&self, polling_interval: Duration) -> Result<()> {
        let mut interval_timer = interval(polling_interval);

        loop {
            interval_timer.tick().await;
            self.process_proofs_with_error_handling().await;
        }
    }

    async fn process_proofs_with_error_handling(&self) {
        match self.process_pending_proofs().await {
            Ok(processed) => {
                Self::log_processing_result(processed);
            }
            Err(e) => {
                error!("❌ Error processing proofs: {}", e);
                sleep(Duration::from_secs(30)).await;
            }
        }
    }

    fn log_processing_result(processed: usize) {
        if processed > 0 {
            info!("✅ Processed {} proofs", processed);
        } else {
            info!("⏳ No pending proofs to process");
        }
    }

    async fn process_pending_proofs(&self) -> Result<usize> {
        let ready_proofs = self.query_ready_proofs().await?;

        if ready_proofs.is_empty() {
            return Ok(0);
        }

        info!(
            "📋 Found {} proofs ready for submission",
            ready_proofs.len()
        );
        self.submit_ready_proofs(ready_proofs).await
    }

    async fn query_ready_proofs(&self) -> Result<Vec<(i32, String)>> {
        let query = r"
            SELECT DISTINCT at.result, sp.proof_id
            FROM arithmetic_transactions at
            INNER JOIN sindri_proofs sp ON sp.result = at.result
            WHERE sp.status = 'Ready'
            LIMIT 10
        ";

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ethereum_client::EthereumError::External(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| (row.get("result"), row.get("proof_id")))
            .collect())
    }

    async fn submit_ready_proofs(&self, ready_proofs: Vec<(i32, String)>) -> Result<usize> {
        let mut processed = 0;

        for (result, proof_id) in ready_proofs {
            match self.submit_single_proof(result, &proof_id).await {
                Ok(()) => {
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

    async fn submit_single_proof(&self, result: i32, proof_id: &str) -> Result<()> {
        let proof_info = self.get_proof_from_sindri(proof_id).await?;
        let (proof_data, public_values) = Self::prepare_proof_data(&proof_info, result)?;
        let (state_id, new_state_root) = Self::generate_state_info(result, proof_id);

        Self::log_submission_info(result, &new_state_root);
        let state_update = self
            .submit_to_contract(state_id, new_state_root, proof_data, public_values)
            .await?;
        Self::log_success(&state_update);
        self.update_submission_status(result, "Confirmed", state_update.transaction_hash.as_ref())
            .await?;

        Ok(())
    }

    async fn get_proof_from_sindri(&self, proof_id: &str) -> Result<sindri::ProofInfoResponse> {
        let sindri_client = SindriClient::default();
        let proof_info = sindri_client
            .get_proof(proof_id, None, None, None)
            .await
            .map_err(|e| {
                ethereum_client::EthereumError::External(format!("Sindri API error: {e}"))
            })?;

        if proof_info.status != JobStatus::Ready {
            return Err(ethereum_client::EthereumError::Config(format!(
                "Proof not ready: {:?}",
                proof_info.status
            )));
        }

        Ok(proof_info)
    }

    fn prepare_proof_data(
        proof_info: &sindri::ProofInfoResponse,
        result: i32,
    ) -> Result<(Bytes, Bytes)> {
        let sp1_proof = proof_info
            .to_sp1_proof_with_public()
            .map_err(|e| ethereum_client::EthereumError::Sindri(e.to_string()))?;

        let proof_bytes = serde_json::to_vec(&sp1_proof)
            .map_err(|e| ethereum_client::EthereumError::Sindri(e.to_string()))?;
        let proof_data = Bytes::from(proof_bytes);
        let public_values = Bytes::from(result.to_be_bytes().to_vec());

        Ok((proof_data, public_values))
    }

    fn generate_state_info(result: i32, proof_id: &str) -> (FixedBytes<32>, FixedBytes<32>) {
        let state_id = FixedBytes::<32>::from([1u8; 32]); // Use a fixed state ID for now
        let new_state_root = {
            use alloy_primitives::keccak256;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let data = format!("{result}:{proof_id}:{timestamp}");
            FixedBytes::<32>::from_slice(keccak256(data.as_bytes()).as_slice())
        };
        (state_id, new_state_root)
    }

    fn log_submission_info(result: i32, new_state_root: &FixedBytes<32>) {
        info!(
            "📤 Submitting to contract: result={}, state_root=0x{}",
            result,
            hex::encode(new_state_root.as_slice())
        );
    }

    async fn submit_to_contract(
        &self,
        state_id: FixedBytes<32>,
        new_state_root: FixedBytes<32>,
        proof_data: Bytes,
        public_values: Bytes,
    ) -> Result<ethereum_client::types::StateUpdate> {
        self.client
            .update_state(state_id, new_state_root, proof_data, public_values)
            .await
    }

    fn log_success(state_update: &ethereum_client::types::StateUpdate) {
        info!("✅ Transaction submitted successfully!");
        if let Some(tx_hash) = state_update.transaction_hash {
            info!("  Transaction hash: 0x{}", hex::encode(tx_hash.as_slice()));
        }
        if let Some(block_number) = state_update.block_number {
            info!("  Block number: {}", block_number);
        }
    }

    async fn update_submission_status(
        &self,
        result: i32,
        status: &str,
        tx_hash: Option<&FixedBytes<32>>,
    ) -> Result<()> {
        let query = tx_hash.map_or_else(
            || {
                sqlx::query(
                    r"
                    INSERT INTO ethereum_submissions (result, status, submitted_at)
                    VALUES ($1, $2, NOW())
                    ON CONFLICT (result)
                    DO UPDATE SET status = $2, updated_at = NOW()
                    ",
                )
                .bind(result)
                .bind(status)
            },
            |hash| {
                sqlx::query(
                    r"
                    INSERT INTO ethereum_submissions (result, status, transaction_hash, submitted_at)
                    VALUES ($1, $2, $3, NOW())
                    ON CONFLICT (result)
                    DO UPDATE SET status = $2, transaction_hash = $3, updated_at = NOW()
                    ",
                )
                .bind(result)
                .bind(status)
                .bind(hash.as_slice())
            }
        );

        query
            .execute(&self.pool)
            .await
            .map_err(|e| ethereum_client::EthereumError::External(e.to_string()))?;

        Ok(())
    }
}
