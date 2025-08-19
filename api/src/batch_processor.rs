//! Background Batch Processing Service
//!
//! This module provides automatic batch processing with multiple triggers:
//! - Timer-based: Every 1 minute
//! - Count-based: When 10+ transactions are pending
//! - Manual: Via API trigger
//!
//! The service runs in the background alongside the API server.

use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, instrument};

use crate::rest::ApiConfig;
use arithmetic_db::{create_batch, get_pending_transactions, get_batch_by_id, update_batch_proof};
use arithmetic_lib::proof::{generate_sindri_proof, ProofGenerationRequest, ProofSystem};

// ============================================================================
// BATCH PROCESSOR CONFIGURATION
// ============================================================================

/// Configuration for the background batch processor
#[derive(Debug, Clone)]
pub struct BatchProcessorConfig {
    /// Timer interval for periodic batching (default: 1 minute)
    pub timer_interval_seconds: u64,

    /// Minimum number of pending transactions to trigger batching (default: 10)
    pub count_trigger_threshold: usize,

    /// Maximum batch size (should match API config)
    pub max_batch_size: u32,

    /// Whether to enable the background processor
    pub enabled: bool,

    /// Minimum time between batches to avoid too frequent processing
    pub min_batch_interval_seconds: u64,
}

impl Default for BatchProcessorConfig {
    fn default() -> Self {
        Self {
            timer_interval_seconds: 60, // 1 minute
            count_trigger_threshold: 10,
            max_batch_size: 50,
            enabled: true,
            min_batch_interval_seconds: 5, // Minimum 5 seconds between batches
        }
    }
}

impl From<&ApiConfig> for BatchProcessorConfig {
    fn from(api_config: &ApiConfig) -> Self {
        Self {
            max_batch_size: api_config.max_batch_size,
            ..Default::default()
        }
    }
}

// ============================================================================
// BATCH PROCESSOR COMMANDS
// ============================================================================

/// Commands that can be sent to the batch processor
#[derive(Debug, Clone)]
pub enum BatchProcessorCommand {
    /// Trigger manual batch processing
    TriggerBatch,

    /// Stop the batch processor
    Stop,

    /// Update configuration
    UpdateConfig(BatchProcessorConfig),

    /// Get current status
    GetStatus,
}

/// Response from batch processor operations
#[derive(Debug, Clone)]
pub struct BatchProcessorResponse {
    pub success: bool,
    pub message: String,
    pub batch_id: Option<i32>,
    pub transaction_count: Option<usize>,
}

/// Statistics about the batch processor
#[derive(Debug, Clone)]
pub struct BatchProcessorStats {
    pub total_batches_created: u64,
    pub total_transactions_processed: u64,
    pub last_batch_time: Option<Instant>,
    pub timer_triggers: u64,
    pub count_triggers: u64,
    pub manual_triggers: u64,
    pub errors: u64,
}

impl Default for BatchProcessorStats {
    fn default() -> Self {
        Self {
            total_batches_created: 0,
            total_transactions_processed: 0,
            last_batch_time: None,
            timer_triggers: 0,
            count_triggers: 0,
            manual_triggers: 0,
            errors: 0,
        }
    }
}

// ============================================================================
// BACKGROUND BATCH PROCESSOR
// ============================================================================

/// Background batch processor service
pub struct BackgroundBatchProcessor {
    config: BatchProcessorConfig,
    pool: PgPool,
    command_rx: mpsc::UnboundedReceiver<BatchProcessorCommand>,
    stats: Arc<RwLock<BatchProcessorStats>>,
}

/// Handle for communicating with the background batch processor
#[derive(Clone)]
pub struct BatchProcessorHandle {
    command_tx: mpsc::UnboundedSender<BatchProcessorCommand>,
    stats: Arc<RwLock<BatchProcessorStats>>,
}

impl BatchProcessorHandle {
    /// Trigger manual batch processing
    pub async fn trigger_batch(&self) -> Result<(), String> {
        self.command_tx
            .send(BatchProcessorCommand::TriggerBatch)
            .map_err(|e| format!("Failed to send trigger command: {}", e))
    }

    /// Stop the batch processor
    pub async fn stop(&self) -> Result<(), String> {
        self.command_tx
            .send(BatchProcessorCommand::Stop)
            .map_err(|e| format!("Failed to send stop command: {}", e))
    }

    /// Update processor configuration
    pub async fn update_config(&self, config: BatchProcessorConfig) -> Result<(), String> {
        self.command_tx
            .send(BatchProcessorCommand::UpdateConfig(config))
            .map_err(|e| format!("Failed to send config update: {}", e))
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> BatchProcessorStats {
        self.stats.read().await.clone()
    }
}

impl BackgroundBatchProcessor {
    /// Create a new background batch processor
    pub fn new(pool: PgPool, config: BatchProcessorConfig) -> (Self, BatchProcessorHandle) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let stats = Arc::new(RwLock::new(BatchProcessorStats::default()));

        let processor = Self {
            config,
            pool,
            command_rx,
            stats: stats.clone(),
        };

        let handle = BatchProcessorHandle { command_tx, stats };

        (processor, handle)
    }

    /// Start the background batch processor
    #[instrument(skip(self), level = "info")]
    pub async fn run(mut self) {
        if !self.config.enabled {
            info!("🔄 Background batch processor is disabled");
            return;
        }

        info!("🚀 Starting background batch processor");
        info!(
            "⏰ Timer interval: {} seconds",
            self.config.timer_interval_seconds
        );
        info!(
            "📊 Count trigger threshold: {} transactions",
            self.config.count_trigger_threshold
        );
        info!("📦 Max batch size: {}", self.config.max_batch_size);

        let mut timer = interval(Duration::from_secs(self.config.timer_interval_seconds));
        let mut last_batch_time = Instant::now();

        loop {
            tokio::select! {
                // Handle timer-based triggers
                _ = timer.tick() => {
                    if self.should_process_batch(last_batch_time).await {
                        match self.process_batch("timer").await {
                            Ok(Some(_)) => {
                                last_batch_time = Instant::now();
                                self.increment_timer_triggers().await;
                            }
                            Ok(None) => {
                                debug!("Timer trigger: No transactions to batch");
                            }
                            Err(e) => {
                                error!("Timer trigger batch processing failed: {}", e);
                                self.increment_errors().await;
                            }
                        }
                    }
                }

                // Handle commands
                Some(command) = self.command_rx.recv() => {
                    match command {
                        BatchProcessorCommand::TriggerBatch => {
                            info!("🔄 Manual batch trigger received");
                            match self.process_batch("manual").await {
                                Ok(Some(_)) => {
                                    last_batch_time = Instant::now();
                                    self.increment_manual_triggers().await;
                                }
                                Ok(None) => {
                                    debug!("Manual trigger: No transactions to batch");
                                }
                                Err(e) => {
                                    error!("Manual trigger batch processing failed: {}", e);
                                    self.increment_errors().await;
                                }
                            }
                        }

                        BatchProcessorCommand::Stop => {
                            info!("🛑 Stopping background batch processor");
                            break;
                        }

                        BatchProcessorCommand::UpdateConfig(new_config) => {
                            info!("⚙️ Updating batch processor configuration");
                            self.config = new_config;
                            timer = interval(Duration::from_secs(self.config.timer_interval_seconds));
                        }

                        BatchProcessorCommand::GetStatus => {
                            let stats = self.stats.read().await;
                            debug!("📊 Batch processor stats: {:?}", *stats);
                        }
                    }
                }

                // Check count-based trigger periodically (every 10 seconds)
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    if self.should_process_batch(last_batch_time).await {
                        match self.check_count_trigger().await {
                            Ok(true) => {
                                match self.process_batch("count").await {
                                    Ok(Some(_)) => {
                                        last_batch_time = Instant::now();
                                        self.increment_count_triggers().await;
                                    }
                                    Ok(None) => {
                                        debug!("Count trigger: No transactions to batch");
                                    }
                                    Err(e) => {
                                        error!("Count trigger batch processing failed: {}", e);
                                        self.increment_errors().await;
                                    }
                                }
                            }
                            Ok(false) => {
                                // Not enough transactions for count trigger
                            }
                            Err(e) => {
                                error!("Failed to check count trigger: {}", e);
                                self.increment_errors().await;
                            }
                        }
                    }
                }
            }
        }

        info!("✅ Background batch processor stopped");
    }

    /// Check if we should process a batch (respecting minimum interval)
    async fn should_process_batch(&self, last_batch_time: Instant) -> bool {
        let elapsed = last_batch_time.elapsed();
        elapsed.as_secs() >= self.config.min_batch_interval_seconds
    }

    /// Check if count trigger should fire
    async fn check_count_trigger(&self) -> Result<bool, String> {
        match get_pending_transactions(&self.pool).await {
            Ok(transactions) => {
                let count = transactions.len();
                debug!("📊 Current pending transactions: {}", count);
                Ok(count >= self.config.count_trigger_threshold)
            }
            Err(e) => Err(format!("Failed to get pending transactions: {}", e)),
        }
    }

    /// Process a batch of transactions
    #[instrument(skip(self), level = "info")]
    async fn process_batch(&self, trigger_type: &str) -> Result<Option<i32>, String> {
        info!("🔄 Processing batch (trigger: {})", trigger_type);

        // First check if there are any pending transactions
        let pending_count = match get_pending_transactions(&self.pool).await {
            Ok(transactions) => transactions.len(),
            Err(e) => return Err(format!("Failed to check pending transactions: {}", e)),
        };

        if pending_count == 0 {
            debug!("No pending transactions to batch");
            return Ok(None);
        }

        info!(
            "📦 Creating batch from {} pending transactions",
            pending_count
        );

        // Create batch with configured max size
        match create_batch(&self.pool, Some(self.config.max_batch_size as i32)).await {
            Ok(Some(batch)) => {
                let transaction_count = batch.transaction_ids.len();
                info!(
                    "✅ Batch created successfully: id={}, transactions={}",
                    batch.id, transaction_count
                );

                // Update statistics
                self.update_stats(batch.id, transaction_count).await;

                // Trigger proof generation asynchronously
                self.trigger_proof_generation(batch.id).await;

                Ok(Some(batch.id))
            }
            Ok(None) => {
                debug!("No transactions were available for batching");
                Ok(None)
            }
            Err(e) => {
                error!("Failed to create batch: {}", e);
                Err(format!("Failed to create batch: {}", e))
            }
        }
    }

    /// Update processor statistics
    async fn update_stats(&self, _batch_id: i32, transaction_count: usize) {
        let mut stats = self.stats.write().await;
        stats.total_batches_created += 1;
        stats.total_transactions_processed += transaction_count as u64;
        stats.last_batch_time = Some(Instant::now());

        info!(
            "📊 Updated stats: batches={}, transactions={}",
            stats.total_batches_created, stats.total_transactions_processed
        );
    }

    async fn increment_timer_triggers(&self) {
        self.stats.write().await.timer_triggers += 1;
    }

    async fn increment_count_triggers(&self) {
        self.stats.write().await.count_triggers += 1;
    }

    async fn increment_manual_triggers(&self) {
        self.stats.write().await.manual_triggers += 1;
    }

    async fn increment_errors(&self) {
        self.stats.write().await.errors += 1;
    }

    /// Trigger proof generation for a batch asynchronously
    async fn trigger_proof_generation(&self, batch_id: i32) {
        let pool = self.pool.clone();
        
        // Spawn proof generation in background to avoid blocking batch creation
        tokio::spawn(async move {
            if let Err(e) = Self::generate_proof_for_batch(&pool, batch_id).await {
                error!("Failed to generate proof for batch {}: {}", batch_id, e);
            }
        });
    }

    /// Generate a ZK proof for a specific batch
    async fn generate_proof_for_batch(pool: &PgPool, batch_id: i32) -> Result<(), String> {
        info!("🔐 Starting proof generation for batch: {}", batch_id);

        // Get batch details
        let batch = get_batch_by_id(pool, batch_id)
            .await
            .map_err(|e| format!("Failed to get batch {}: {}", batch_id, e))?;

        // For the arithmetic circuit, we need to prove the counter transition
        // The circuit takes: initial_value + sum_of_transactions = final_value
        // We'll use: a = initial_value, b = sum_of_transactions, result = final_value
        let initial_value = batch.previous_counter_value as i32;
        let final_value = batch.final_counter_value as i32;
        let sum_of_transactions = final_value - initial_value;

        info!(
            "📊 Batch {} proof parameters: {} + {} = {}",
            batch_id, initial_value, sum_of_transactions, final_value
        );

        // Create proof generation request
        let proof_request = ProofGenerationRequest {
            a: initial_value,
            b: sum_of_transactions,
            result: final_value,
            proof_system: ProofSystem::default(), // Use Groth16 by default
            generate_fixtures: false, // Don't generate fixtures in production
        };

        // Generate proof via Sindri
        match generate_sindri_proof(proof_request).await {
            Ok(proof_response) => {
                info!(
                    "✅ Proof submitted to Sindri for batch {}: proof_id={}",
                    batch_id, proof_response.proof_id
                );

                // Update batch with Sindri proof ID
                if let Err(e) = update_batch_proof(
                    pool,
                    batch_id,
                    &proof_response.proof_id,
                    "pending", // Sindri will update this to "proven" when ready
                )
                .await
                {
                    error!("Failed to update batch {} with proof ID: {}", batch_id, e);
                    return Err(format!("Failed to update batch with proof ID: {}", e));
                }

                info!(
                    "📝 Updated batch {} with Sindri proof ID: {}",
                    batch_id, proof_response.proof_id
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to generate proof for batch {}: {}", batch_id, e);

                // Update batch status to failed
                if let Err(update_err) = update_batch_proof(
                    pool,
                    batch_id,
                    &format!("failed_{}", batch_id),
                    "failed",
                )
                .await
                {
                    error!(
                        "Failed to update batch {} status to failed: {}",
                        batch_id, update_err
                    );
                }

                Err(format!("Proof generation failed: {}", e))
            }
        }
    }
}

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

/// Start the background batch processor service
pub async fn start_batch_processor(
    pool: PgPool,
    config: BatchProcessorConfig,
) -> BatchProcessorHandle {
    let (processor, handle) = BackgroundBatchProcessor::new(pool, config);

    // Spawn the processor in the background
    tokio::spawn(async move {
        processor.run().await;
    });

    handle
}

/// Create batch processor configuration from API config
pub fn create_batch_processor_config(api_config: &ApiConfig) -> BatchProcessorConfig {
    BatchProcessorConfig::from(api_config)
}
