use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{info, instrument, warn};

use crate::ads_service::{
    AdsConfig, AdsError, AdsMetrics, AdsServiceFactory, AuditTrail, AuthenticatedDataStructure,
    IndexedMerkleTreeADS, MembershipProof, NonMembershipProof, StateCommitment, StateTransition,
    WitnessData,
};

// ============================================================================
// VAPP SERVER INTEGRATION LAYER
// ============================================================================

/// Main vApp server integration for ADS services
pub struct VAppAdsIntegration {
    ads: Arc<RwLock<IndexedMerkleTreeADS>>,
    config: VAppConfig,
    settlement_service: Arc<dyn SettlementService>,
    proof_service: Arc<dyn ProofGenerationService>,
    compliance_service: Arc<dyn ComplianceService>,
    notification_service: Arc<dyn NotificationService>,
}

/// Configuration for vApp integration
#[derive(Debug, Clone)]
pub struct VAppConfig {
    pub server_id: String,
    pub environment: Environment,
    pub settlement_enabled: bool,
    pub compliance_checks_enabled: bool,
    pub auto_proof_generation: bool,
    pub batch_processing_enabled: bool,
    pub max_concurrent_operations: usize,
    pub proof_generation_timeout_ms: u64,
    pub settlement_confirmation_blocks: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Default for VAppConfig {
    fn default() -> Self {
        Self {
            server_id: "vapp-ads-server".to_string(),
            environment: Environment::Development,
            settlement_enabled: false,
            compliance_checks_enabled: true,
            auto_proof_generation: true,
            batch_processing_enabled: true,
            max_concurrent_operations: 100,
            proof_generation_timeout_ms: 30_000, // 30 seconds
            settlement_confirmation_blocks: 12,
        }
    }
}

// ============================================================================
// SERVICE TRAITS FOR DEPENDENCY INJECTION
// ============================================================================

/// Settlement service for on-chain state commitments
#[async_trait]
pub trait SettlementService: Send + Sync {
    async fn submit_state_commitment(
        &self,
        commitment: &StateCommitment,
    ) -> Result<SettlementResult, SettlementError>;
    async fn get_settlement_status(
        &self,
        transaction_hash: &str,
    ) -> Result<SettlementStatus, SettlementError>;
    async fn estimate_gas(&self, commitment: &StateCommitment) -> Result<u64, SettlementError>;
}

/// Proof generation service for ZK circuits
#[async_trait]
pub trait ProofGenerationService: Send + Sync {
    async fn generate_zk_proof(&self, witnesses: &[WitnessData]) -> Result<ZkProof, ProofError>;
    async fn verify_zk_proof(&self, proof: &ZkProof) -> Result<bool, ProofError>;
    async fn get_proving_key(&self, circuit_type: &str) -> Result<ProvingKey, ProofError>;
}

/// Compliance service for regulatory requirements
#[async_trait]
pub trait ComplianceService: Send + Sync {
    async fn validate_nullifier(&self, nullifier: i64)
        -> Result<ComplianceResult, ComplianceError>;
    async fn audit_operation(&self, audit_trail: &AuditTrail) -> Result<(), ComplianceError>;
    async fn generate_compliance_report(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ComplianceReport, ComplianceError>;
}

/// Notification service for alerts and monitoring
#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn notify_insertion(&self, transition: &StateTransition)
        -> Result<(), NotificationError>;
    async fn notify_settlement(&self, result: &SettlementResult) -> Result<(), NotificationError>;
    async fn notify_error(&self, error: &VAppError) -> Result<(), NotificationError>;
}

// ============================================================================
// RESPONSE TYPES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementResult {
    pub transaction_hash: String,
    pub block_number: u64,
    pub gas_used: u64,
    pub status: SettlementStatus,
    pub confirmation_blocks: u64,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SettlementStatus {
    Pending,
    Confirmed,
    Failed,
    Reverted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkProof {
    pub circuit_type: String,
    pub proof_data: Vec<u8>,
    pub public_inputs: Vec<String>,
    pub verification_key_hash: [u8; 32],
    pub proving_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvingKey {
    pub circuit_type: String,
    pub key_data: Vec<u8>,
    pub constraint_count: u32,
    pub key_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceResult {
    pub is_valid: bool,
    pub jurisdiction: String,
    pub risk_score: f64,
    pub flags: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_operations: u64,
    pub compliant_operations: u64,
    pub flagged_operations: Vec<FlaggedOperation>,
    pub risk_assessment: RiskAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlaggedOperation {
    pub nullifier: i64,
    pub reason: String,
    pub severity: ComplianceSeverity,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub overall_score: f64,
    pub risk_level: RiskLevel,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

// ============================================================================
// VAPP OPERATION RESPONSES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VAppInsertionResponse {
    pub transaction_id: String,
    pub state_transition: StateTransition,
    pub settlement_result: Option<SettlementResult>,
    pub zk_proof: Option<ZkProof>,
    pub compliance_result: ComplianceResult,
    pub processing_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VAppProofResponse {
    pub proof_type: ProofType,
    pub membership_proof: Option<MembershipProof>,
    pub non_membership_proof: Option<NonMembershipProof>,
    pub zk_proof: Option<ZkProof>,
    pub verification_status: bool,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofType {
    Membership,
    NonMembership,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VAppBatchResponse {
    pub batch_id: String,
    pub total_operations: usize,
    pub successful_operations: usize,
    pub failed_operations: Vec<BatchFailure>,
    pub combined_state_transition: Option<StateTransition>,
    pub processing_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFailure {
    pub nullifier: i64,
    pub error: String,
    pub error_code: String,
}

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, Error)]
pub enum VAppError {
    #[error("ADS error: {0}")]
    AdsError(#[from] AdsError),

    #[error("Settlement error: {0}")]
    SettlementError(#[from] SettlementError),

    #[error("Proof generation error: {0}")]
    ProofError(#[from] ProofError),

    #[error("Compliance error: {0}")]
    ComplianceError(#[from] ComplianceError),

    #[error("Notification error: {0}")]
    NotificationError(#[from] NotificationError),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("Concurrency error: {0}")]
    ConcurrencyError(String),
}

#[derive(Debug, Error)]
pub enum SettlementError {
    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Gas estimation failed: {0}")]
    GasEstimationError(String),

    #[error("Transaction failed: {0}")]
    TransactionError(String),

    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),
}

#[derive(Debug, Error)]
pub enum ProofError {
    #[error("Circuit compilation failed: {0}")]
    CircuitError(String),

    #[error("Witness generation failed: {0}")]
    WitnessError(String),

    #[error("Proving failed: {0}")]
    ProvingError(String),

    #[error("Verification failed: {0}")]
    VerificationError(String),
}

#[derive(Debug, Error)]
pub enum ComplianceError {
    #[error("Validation failed: {0}")]
    ValidationError(String),

    #[error("Regulatory violation: {0}")]
    RegulatoryViolation(String),

    #[error("Audit failed: {0}")]
    AuditError(String),
}

#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("Delivery failed: {0}")]
    DeliveryError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

// ============================================================================
// VAPP INTEGRATION IMPLEMENTATION
// ============================================================================

impl VAppAdsIntegration {
    /// Create new vApp ADS integration
    #[instrument(
        skip(
            pool,
            settlement_service,
            proof_service,
            compliance_service,
            notification_service
        ),
        level = "info"
    )]
    pub async fn new(
        pool: PgPool,
        config: VAppConfig,
        settlement_service: Arc<dyn SettlementService>,
        proof_service: Arc<dyn ProofGenerationService>,
        compliance_service: Arc<dyn ComplianceService>,
        notification_service: Arc<dyn NotificationService>,
    ) -> Result<Self, VAppError> {
        info!("🚀 Initializing vApp ADS integration");

        // Create ADS configuration
        let ads_config = AdsConfig {
            settlement_contract: "0x742d35cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1".to_string(), // Example
            chain_id: match config.environment {
                Environment::Development => 31337, // Local
                Environment::Staging => 11155111,  // Sepolia
                Environment::Production => 1,      // Mainnet
            },
            audit_enabled: config.compliance_checks_enabled,
            metrics_enabled: true,
            cache_size_limit: 50_000,
            batch_size_limit: 1_000,
            gas_price: 20_000_000_000, // 20 gwei
        };

        // Create ADS service
        let factory = AdsServiceFactory::with_config(pool, ads_config);
        let ads = factory.create_indexed_merkle_tree().await?;

        let integration = Self {
            ads: Arc::new(RwLock::new(ads)),
            config,
            settlement_service,
            proof_service,
            compliance_service,
            notification_service,
        };

        info!("✅ vApp ADS integration initialized successfully");
        Ok(integration)
    }

    /// Process nullifier insertion with full vApp workflow
    #[instrument(skip(self), level = "info")]
    pub async fn process_nullifier_insertion(
        &self,
        nullifier: i64,
    ) -> Result<VAppInsertionResponse, VAppError> {
        info!("🔄 Processing nullifier insertion: {}", nullifier);
        let start_time = std::time::Instant::now();

        // Step 1: Compliance validation
        let compliance_result = if self.config.compliance_checks_enabled {
            self.compliance_service
                .validate_nullifier(nullifier)
                .await?
        } else {
            ComplianceResult {
                is_valid: true,
                jurisdiction: "NONE".to_string(),
                risk_score: 0.0,
                flags: vec![],
                notes: vec!["Compliance checks disabled".to_string()],
            }
        };

        if !compliance_result.is_valid {
            return Err(VAppError::ComplianceError(
                ComplianceError::ValidationError(format!(
                    "Nullifier {} failed compliance validation",
                    nullifier
                )),
            ));
        }

        // Step 2: Insert into ADS
        let mut ads_guard = self.ads.write().await;
        let state_transition = ads_guard.insert(nullifier).await?;
        drop(ads_guard); // Release lock early

        info!(
            "📊 Nullifier {} inserted. Root: {:02x?} -> {:02x?}",
            nullifier,
            &state_transition.old_root[..8],
            &state_transition.new_root[..8]
        );

        // Step 3: Generate ZK proof if enabled
        let zk_proof = if self.config.auto_proof_generation {
            match self
                .proof_service
                .generate_zk_proof(&state_transition.witnesses)
                .await
            {
                Ok(proof) => Some(proof),
                Err(e) => {
                    warn!("ZK proof generation failed: {:?}", e);
                    None
                }
            }
        } else {
            None
        };

        // Step 4: Settlement if enabled
        let settlement_result = if self.config.settlement_enabled {
            let ads_guard = self.ads.read().await;
            let commitment = ads_guard.get_state_commitment().await?;
            drop(ads_guard);

            match self
                .settlement_service
                .submit_state_commitment(&commitment)
                .await
            {
                Ok(result) => Some(result),
                Err(e) => {
                    warn!("Settlement failed: {:?}", e);
                    None
                }
            }
        } else {
            None
        };

        // Step 5: Audit trail update
        if self.config.compliance_checks_enabled {
            let ads_guard = self.ads.read().await;
            if let Ok(audit_trail) = ads_guard.get_audit_trail(nullifier).await {
                if let Err(e) = self.compliance_service.audit_operation(&audit_trail).await {
                    warn!("Audit operation failed: {:?}", e);
                }
            }
            drop(ads_guard);
        }

        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        let response = VAppInsertionResponse {
            transaction_id: state_transition.id.clone(),
            state_transition: state_transition.clone(),
            settlement_result,
            zk_proof,
            compliance_result,
            processing_time_ms,
        };

        // Step 6: Notifications
        if let Err(e) = self
            .notification_service
            .notify_insertion(&state_transition)
            .await
        {
            warn!("Notification failed: {:?}", e);
        }

        info!(
            "✅ Nullifier {} processed successfully in {}ms",
            nullifier, processing_time_ms
        );
        Ok(response)
    }

    /// Verify nullifier absence with comprehensive proofs
    #[instrument(skip(self), level = "info")]
    pub async fn verify_nullifier_absence(
        &self,
        nullifier: i64,
    ) -> Result<VAppProofResponse, VAppError> {
        info!("🔍 Verifying nullifier absence: {}", nullifier);
        let start_time = std::time::Instant::now();

        let ads_guard = self.ads.read().await;
        let non_membership_proof = ads_guard.prove_non_membership(nullifier).await?;
        drop(ads_guard);

        // Generate ZK proof for non-membership
        let zk_proof = if self.config.auto_proof_generation {
            // Create witness data for non-membership circuit
            let witness_data = WitnessData {
                circuit_type: "non_membership".to_string(),
                inputs: serde_json::json!({
                    "queried_value": nullifier,
                    "low_nullifier": non_membership_proof.low_nullifier.value,
                    "next_nullifier": non_membership_proof.low_nullifier.next_value,
                    "root": hex::encode(non_membership_proof.root_hash),
                }),
                constraints: 150, // Estimated constraints for non-membership
                proving_key_hash: [0u8; 32],
            };

            match self.proof_service.generate_zk_proof(&[witness_data]).await {
                Ok(proof) => Some(proof),
                Err(e) => {
                    warn!("ZK proof generation for non-membership failed: {:?}", e);
                    None
                }
            }
        } else {
            None
        };

        // Verify the proof
        let verification_status = non_membership_proof.range_proof.valid;

        let response = VAppProofResponse {
            proof_type: ProofType::NonMembership,
            membership_proof: None,
            non_membership_proof: Some(non_membership_proof),
            zk_proof,
            verification_status,
            generated_at: Utc::now(),
        };

        let processing_time_ms = start_time.elapsed().as_millis() as u64;
        info!(
            "✅ Non-membership proof generated in {}ms",
            processing_time_ms
        );

        Ok(response)
    }

    /// Verify nullifier presence with membership proof
    #[instrument(skip(self), level = "info")]
    pub async fn verify_nullifier_presence(
        &self,
        nullifier: i64,
    ) -> Result<VAppProofResponse, VAppError> {
        info!("🔍 Verifying nullifier presence: {}", nullifier);

        let ads_guard = self.ads.read().await;
        let membership_proof = ads_guard.prove_membership(nullifier).await?;
        drop(ads_guard);

        // Generate ZK proof for membership
        let zk_proof = if self.config.auto_proof_generation {
            let witness_data = WitnessData {
                circuit_type: "membership".to_string(),
                inputs: serde_json::json!({
                    "nullifier": nullifier,
                    "tree_index": membership_proof.tree_index,
                    "root": hex::encode(membership_proof.root_hash),
                    "merkle_proof": membership_proof.merkle_proof.siblings.iter()
                        .map(|s| hex::encode(s))
                        .collect::<Vec<_>>(),
                }),
                constraints: 256, // Estimated constraints for membership
                proving_key_hash: [0u8; 32],
            };

            match self.proof_service.generate_zk_proof(&[witness_data]).await {
                Ok(proof) => Some(proof),
                Err(e) => {
                    warn!("ZK proof generation for membership failed: {:?}", e);
                    None
                }
            }
        } else {
            None
        };

        let response = VAppProofResponse {
            proof_type: ProofType::Membership,
            membership_proof: Some(membership_proof),
            non_membership_proof: None,
            zk_proof,
            verification_status: true, // If we got here, the proof exists
            generated_at: Utc::now(),
        };

        info!("✅ Membership proof generated successfully");
        Ok(response)
    }

    /// Process batch nullifier insertions
    #[instrument(skip(self, nullifiers), level = "info")]
    pub async fn process_batch_insertions(
        &self,
        nullifiers: &[i64],
    ) -> Result<VAppBatchResponse, VAppError> {
        info!(
            "📦 Processing batch insertion of {} nullifiers",
            nullifiers.len()
        );
        let start_time = std::time::Instant::now();

        if !self.config.batch_processing_enabled {
            return Err(VAppError::ConfigurationError(
                "Batch processing is disabled".to_string(),
            ));
        }

        let batch_id = format!("batch_{}", Utc::now().timestamp_millis());
        let mut successful_operations = 0;
        let mut failed_operations = Vec::new();
        let mut last_state_transition = None;

        // Process batch insertions
        let mut ads_guard = self.ads.write().await;
        let transitions = match ads_guard.batch_insert(nullifiers).await {
            Ok(transitions) => {
                successful_operations = transitions.len();
                last_state_transition = transitions.last().cloned();
                transitions
            }
            Err(e) => {
                // Handle partial failures
                for &nullifier in nullifiers {
                    failed_operations.push(BatchFailure {
                        nullifier,
                        error: e.to_string(),
                        error_code: "BATCH_INSERT_FAILED".to_string(),
                    });
                }
                vec![]
            }
        };
        drop(ads_guard);

        // Submit state commitment if batch succeeded and settlement is enabled
        if self.config.settlement_enabled && !transitions.is_empty() {
            let ads_guard = self.ads.read().await;
            if let Ok(commitment) = ads_guard.get_state_commitment().await {
                if let Err(e) = self
                    .settlement_service
                    .submit_state_commitment(&commitment)
                    .await
                {
                    warn!("Batch settlement failed: {:?}", e);
                }
            }
            drop(ads_guard);
        }

        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        let response = VAppBatchResponse {
            batch_id,
            total_operations: nullifiers.len(),
            successful_operations,
            failed_operations,
            combined_state_transition: last_state_transition,
            processing_time_ms,
        };

        info!(
            "✅ Batch processing completed: {}/{} successful in {}ms",
            successful_operations,
            nullifiers.len(),
            processing_time_ms
        );

        Ok(response)
    }

    /// Get current state commitment for settlement
    #[instrument(skip(self), level = "info")]
    pub async fn get_current_state_commitment(&self) -> Result<StateCommitment, VAppError> {
        let ads_guard = self.ads.read().await;
        let commitment = ads_guard.get_state_commitment().await?;
        drop(ads_guard);

        info!("📊 Current state commitment retrieved");
        Ok(commitment)
    }

    /// Get performance metrics
    #[instrument(skip(self), level = "info")]
    pub async fn get_metrics(&self) -> Result<AdsMetrics, VAppError> {
        let ads_guard = self.ads.read().await;
        let metrics = ads_guard.get_metrics().await?;
        drop(ads_guard);

        Ok(metrics)
    }

    /// Health check for all integrated services
    #[instrument(skip(self), level = "info")]
    pub async fn health_check(&self) -> Result<bool, VAppError> {
        // Check ADS health
        let ads_guard = self.ads.read().await;
        ads_guard.health_check().await?;
        drop(ads_guard);

        // Could add additional health checks for other services
        info!("✅ vApp ADS integration health check passed");
        Ok(true)
    }
}

// ============================================================================
// MOCK IMPLEMENTATIONS FOR TESTING
// ============================================================================

/// Mock settlement service for testing
pub struct MockSettlementService;

#[async_trait]
impl SettlementService for MockSettlementService {
    async fn submit_state_commitment(
        &self,
        _commitment: &StateCommitment,
    ) -> Result<SettlementResult, SettlementError> {
        Ok(SettlementResult {
            transaction_hash: "0x1234567890abcdef".to_string(),
            block_number: 1000,
            gas_used: 150_000,
            status: SettlementStatus::Pending,
            confirmation_blocks: 0,
            submitted_at: Utc::now(),
        })
    }

    async fn get_settlement_status(
        &self,
        _transaction_hash: &str,
    ) -> Result<SettlementStatus, SettlementError> {
        Ok(SettlementStatus::Confirmed)
    }

    async fn estimate_gas(&self, _commitment: &StateCommitment) -> Result<u64, SettlementError> {
        Ok(150_000)
    }
}

/// Mock proof generation service for testing
pub struct MockProofService;

#[async_trait]
impl ProofGenerationService for MockProofService {
    async fn generate_zk_proof(&self, _witnesses: &[WitnessData]) -> Result<ZkProof, ProofError> {
        Ok(ZkProof {
            circuit_type: "mock".to_string(),
            proof_data: vec![0u8; 256],
            public_inputs: vec!["0x1234".to_string()],
            verification_key_hash: [0u8; 32],
            proving_time_ms: 1000,
        })
    }

    async fn verify_zk_proof(&self, _proof: &ZkProof) -> Result<bool, ProofError> {
        Ok(true)
    }

    async fn get_proving_key(&self, _circuit_type: &str) -> Result<ProvingKey, ProofError> {
        Ok(ProvingKey {
            circuit_type: "mock".to_string(),
            key_data: vec![0u8; 1024],
            constraint_count: 1000,
            key_hash: [0u8; 32],
        })
    }
}

/// Mock compliance service for testing
pub struct MockComplianceService;

#[async_trait]
impl ComplianceService for MockComplianceService {
    async fn validate_nullifier(
        &self,
        _nullifier: i64,
    ) -> Result<ComplianceResult, ComplianceError> {
        Ok(ComplianceResult {
            is_valid: true,
            jurisdiction: "US".to_string(),
            risk_score: 0.1,
            flags: vec![],
            notes: vec!["Mock validation".to_string()],
        })
    }

    async fn audit_operation(&self, _audit_trail: &AuditTrail) -> Result<(), ComplianceError> {
        Ok(())
    }

    async fn generate_compliance_report(
        &self,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<ComplianceReport, ComplianceError> {
        Ok(ComplianceReport {
            period_start: Utc::now(),
            period_end: Utc::now(),
            total_operations: 100,
            compliant_operations: 100,
            flagged_operations: vec![],
            risk_assessment: RiskAssessment {
                overall_score: 0.1,
                risk_level: RiskLevel::Low,
                recommendations: vec!["Continue monitoring".to_string()],
            },
        })
    }
}

/// Mock notification service for testing
pub struct MockNotificationService;

#[async_trait]
impl NotificationService for MockNotificationService {
    async fn notify_insertion(
        &self,
        _transition: &StateTransition,
    ) -> Result<(), NotificationError> {
        Ok(())
    }

    async fn notify_settlement(&self, _result: &SettlementResult) -> Result<(), NotificationError> {
        Ok(())
    }

    async fn notify_error(&self, _error: &VAppError) -> Result<(), NotificationError> {
        Ok(())
    }
}
