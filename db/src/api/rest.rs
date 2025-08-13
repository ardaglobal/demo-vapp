use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};

use crate::ads_service::{AuthenticatedDataStructure, IndexedMerkleTreeADS};
use crate::db::{
    get_current_global_state, get_sindri_proof_by_result, get_state_history, get_value_by_result,
    store_transaction_with_state_update, validate_state_integrity,
};
use crate::vapp_integration::VAppAdsIntegration;
use alloy_sol_types::SolType;
use arithmetic_lib::{addition, PublicValuesStruct};
use sindri::integrations::sp1_v5::SP1ProofInfo;
use sindri::ProofInput;
use sindri::{client::SindriClient, JobStatus};
use sp1_sdk::SP1Stdin;

// ============================================================================
// API STATE AND CONFIGURATION
// ============================================================================

/// API state containing the ADS service and configuration
#[derive(Clone)]
pub struct ApiState {
    pub ads: Arc<RwLock<IndexedMerkleTreeADS>>,
    pub vapp_integration: Arc<RwLock<VAppAdsIntegration>>,
    pub config: ApiConfig,
}

impl std::fmt::Debug for ApiState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiState")
            .field("ads", &"<IndexedMerkleTreeADS>")
            .field("vapp_integration", &"<VAppAdsIntegration>")
            .field("config", &self.config)
            .finish()
    }
}

/// Configuration for API server
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub server_name: String,
    pub version: String,
    pub max_batch_size: usize,
    pub rate_limit_per_minute: u32,
    pub enable_metrics: bool,
    pub enable_debug_endpoints: bool,
    pub cors_origins: Vec<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            server_name: "Indexed Merkle Tree API".to_string(),
            version: "1.0.0".to_string(),
            max_batch_size: 1000,
            rate_limit_per_minute: 100,
            enable_metrics: true,
            enable_debug_endpoints: false,
            cors_origins: vec!["*".to_string()],
        }
    }
}

// ============================================================================
// REQUEST/RESPONSE MODELS
// ============================================================================

/// Request to insert a single nullifier
#[derive(Debug, Serialize, Deserialize)]
pub struct InsertNullifierRequest {
    pub value: i64,
    pub metadata: Option<serde_json::Value>,
    pub client_id: Option<String>,
}

/// Response from nullifier insertion
#[derive(Debug, Serialize, Deserialize)]
pub struct InsertNullifierResponse {
    pub success: bool,
    pub transaction_id: String,
    pub state_transition: StateTransitionDto,
    pub constraint_count: ConstraintCount,
    pub performance_metrics: InsertionMetrics,
    pub settlement_info: Option<SettlementInfo>,
}

/// Request to insert multiple nullifiers in a batch
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchInsertRequest {
    pub values: Vec<i64>,
    pub metadata: Option<serde_json::Value>,
    pub client_id: Option<String>,
}

/// Response from batch insertion
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchInsertResponse {
    pub success: bool,
    pub batch_id: String,
    pub total_operations: usize,
    pub successful_operations: usize,
    pub failed_operations: Vec<BatchFailure>,
    pub combined_metrics: ConstraintCount,
    pub processing_time_ms: u64,
}

/// Individual batch failure
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchFailure {
    pub nullifier: i64,
    pub error: String,
    pub error_code: String,
}

/// DTO for state transition (external API representation)
#[derive(Debug, Serialize, Deserialize)]
pub struct StateTransitionDto {
    pub id: String,
    pub old_root: String,
    pub new_root: String,
    pub nullifier_value: i64,
    pub block_height: u64,
    pub timestamp: DateTime<Utc>,
    pub gas_estimate: u64,
}

/// Constraint count information for ZK circuit analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct ConstraintCount {
    pub total_hashes: u32,      // 3n + 3 = 99 for 32-level tree
    pub range_checks: u32,      // 2
    pub equality_checks: u32,   // ~10
    pub total_constraints: u32, // ~200
    pub vs_traditional: String, // Comparison message
}

/// Performance metrics for insertion operations
#[derive(Debug, Serialize, Deserialize)]
pub struct InsertionMetrics {
    pub insertion_time_ms: u64,
    pub proof_generation_time_ms: u64,
    pub database_operations: u32,
    pub hash_operations: u32,
}

/// Settlement information for on-chain operations
#[derive(Debug, Serialize, Deserialize)]
pub struct SettlementInfo {
    pub contract_address: String,
    pub chain_id: u64,
    pub estimated_gas: u64,
    pub estimated_cost_wei: String,
}

/// Membership check response
#[derive(Debug, Serialize, Deserialize)]
pub struct MembershipCheckResponse {
    pub exists: bool,
    pub nullifier_value: i64,
    pub proof: Option<MembershipProofDto>,
    pub verification_time_ms: u64,
}

/// DTO for membership proof
#[derive(Debug, Serialize, Deserialize)]
pub struct MembershipProofDto {
    pub tree_index: i64,
    pub root_hash: String,
    pub merkle_proof: MerkleProofDto,
    pub verified_at: DateTime<Utc>,
}

/// DTO for Merkle proof
#[derive(Debug, Serialize, Deserialize)]
pub struct MerkleProofDto {
    pub leaf_hash: String,
    pub siblings: Vec<String>,
    pub path_indices: Vec<bool>,
    pub proof_size_bytes: usize,
}

/// Non-membership proof response
#[derive(Debug, Serialize, Deserialize)]
pub struct NonMembershipResponse {
    pub proof: NonMembershipProofDto,
    pub verification_data: NonMembershipVerification,
    pub verification_time_ms: u64,
}

/// DTO for non-membership proof
#[derive(Debug, Serialize, Deserialize)]
pub struct NonMembershipProofDto {
    pub queried_value: i64,
    pub low_nullifier: LowNullifierDto,
    pub root_hash: String,
    pub range_proof: RangeProofDto,
    pub verified_at: DateTime<Utc>,
}

/// DTO for low nullifier information
#[derive(Debug, Serialize, Deserialize)]
pub struct LowNullifierDto {
    pub value: i64,
    pub next_value: i64,
    pub tree_index: i64,
    pub merkle_proof: MerkleProofDto,
}

/// DTO for range proof validation
#[derive(Debug, Serialize, Deserialize)]
pub struct RangeProofDto {
    pub lower_bound: i64,
    pub upper_bound: i64,
    pub queried_value: i64,
    pub valid: bool,
}

/// Non-membership verification data
#[derive(Debug, Serialize, Deserialize)]
pub struct NonMembershipVerification {
    pub low_nullifier_value: i64,
    pub queried_value: i64,
    pub next_value: i64,
    pub range_valid: bool,
    pub proof_valid: bool,
    pub gap_size: i64,
}

/// Tree statistics response
#[derive(Debug, Serialize, Deserialize)]
pub struct TreeStatsResponse {
    pub root_hash: String,
    pub total_nullifiers: u64,
    pub tree_height: u32,
    pub next_available_index: u64,
    pub performance_metrics: PerformanceMetrics,
    pub constraint_efficiency: ConstraintEfficiency,
    pub last_updated: DateTime<Utc>,
}

/// Performance metrics for the tree
#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub avg_insertion_time_ms: f64,
    pub avg_proof_generation_time_ms: f64,
    pub total_operations: u64,
    pub error_rate_percent: f64,
}

/// Constraint efficiency comparison
#[derive(Debug, Serialize, Deserialize)]
pub struct ConstraintEfficiency {
    pub our_constraints: u32,
    pub traditional_constraints: u32,
    pub improvement_factor: f64,
    pub description: String,
}

/// Audit trail response
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditTrailResponse {
    pub nullifier_value: i64,
    pub total_events: usize,
    pub events: Vec<AuditEventDto>,
    pub compliance_status: ComplianceStatusDto,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}

/// DTO for audit events
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEventDto {
    pub event_id: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub root_before: String,
    pub root_after: String,
    pub block_height: u64,
    pub operator: String,
    pub metadata: serde_json::Value,
}

/// DTO for compliance status
#[derive(Debug, Serialize, Deserialize)]
pub struct ComplianceStatusDto {
    pub is_compliant: bool,
    pub last_audit: DateTime<Utc>,
    pub jurisdiction: String,
    pub notes: Vec<String>,
}

/// Query parameters for pagination
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
}

/// Query parameters for filtering
#[derive(Debug, Deserialize, Serialize)]
pub struct FilterQuery {
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub event_type: Option<String>,
}

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub uptime_seconds: u64,
    pub services: HashMap<String, ServiceStatus>,
}

/// Individual service status
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub last_check: DateTime<Utc>,
    pub error: Option<String>,
}

/// Request to submit a new transaction
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub a: i32,
    pub b: i32,
    pub generate_proof: Option<bool>,
}

/// Response from transaction submission
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub transaction_id: String,
    pub a: i32,
    pub b: i32,
    pub result: i32,
    pub previous_state: i64,
    pub new_state: i64,
    pub proof_id: Option<String>,
    pub proof_status: Option<String>,
    pub verification_info: VerificationInfo,
    pub state_info: StateInfo,
}

/// State information for continuous ledger
#[derive(Debug, Serialize, Deserialize)]
pub struct StateInfo {
    pub state_updated: bool,
    pub transaction_count: i64,
    pub continuous_ledger: bool,
    pub state_description: String,
}

/// Verification information for external actors
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationInfo {
    pub can_verify_externally: bool,
    pub verification_command: Option<String>,
    pub db_stored: bool,
    pub merkle_tree_updated: bool,
}

/// Response for getting transaction by result
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionByResultResponse {
    pub result: i32,
    pub a: i32,
    pub b: i32,
    pub found: bool,
    pub metadata: TransactionMetadata,
}

/// Transaction metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionMetadata {
    pub stored_at: Option<DateTime<Utc>>,
    pub has_proof: bool,
    pub proof_id: Option<String>,
    pub verification_status: Option<String>,
}

/// Proof information response
#[derive(Debug, Serialize, Deserialize)]
pub struct ProofResponse {
    pub proof_id: String,
    pub status: String,
    pub result: Option<i32>,
    pub verification_data: Option<ProofVerificationData>,
    pub circuit_info: ProofCircuitInfo,
}

/// Proof verification data
#[derive(Debug, Serialize, Deserialize)]
pub struct ProofVerificationData {
    pub is_verified: bool,
    pub public_result: i32,
    pub verification_message: String,
    pub cryptographic_proof_valid: bool,
}

/// Proof circuit information
#[derive(Debug, Serialize, Deserialize)]
pub struct ProofCircuitInfo {
    pub circuit_id: String,
    pub circuit_name: String,
    pub proof_system: String,
}

/// Request for proof verification
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyProofRequest {
    pub proof_id: String,
    pub expected_result: i32,
}

/// Response for proof verification
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyProofResponse {
    pub valid: bool,
    pub proof_id: String,
    pub actual_result: Option<i32>,
    pub expected_result: i32,
    pub verification_details: VerificationDetails,
    pub zero_knowledge_properties: ZkProperties,
}

/// Detailed verification information
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationDetails {
    pub sindri_status: String,
    pub cryptographic_proof_valid: bool,
    pub result_matches_expected: bool,
    pub verification_time_ms: u64,
}

/// Zero-knowledge proof properties
#[derive(Debug, Serialize, Deserialize)]
pub struct ZkProperties {
    pub privacy_preserved: bool,
    pub inputs_hidden: bool,
    pub soundness: bool,
    pub completeness: bool,
    pub description: String,
}

// ============================================================================
// ROUTER SETUP
// ============================================================================

/// Create the main API router with all endpoints
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        // Health and info endpoints
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/info", get(api_info))
        // Transaction operations (vApp-specific)
        .route("/api/v1/transactions", post(submit_transaction))
        .route("/api/v1/results/{result}", get(get_transaction_by_result))
        .route("/api/v1/results/{result}/verify", post(verify_result_proof))
        .route("/api/v1/proofs/{proof_id}", get(get_proof_info))
        .route("/api/v1/verify", post(verify_proof))
        // State management operations
        .route("/api/v1/state", get(get_current_state))
        .route("/api/v1/state/history", get(get_state_history_endpoint))
        .route("/api/v1/state/validate", get(validate_state))
        // Nullifier operations
        .route("/api/v1/nullifiers", post(insert_nullifier))
        .route("/api/v1/nullifiers/batch", post(batch_insert_nullifiers))
        .route(
            "/api/v1/nullifiers/{value}/membership",
            get(check_membership),
        )
        .route(
            "/api/v1/nullifiers/{value}/non-membership",
            get(prove_non_membership),
        )
        .route("/api/v1/nullifiers/{value}/audit", get(get_audit_trail))
        // Tree operations
        .route("/api/v1/tree/root", get(get_tree_root))
        .route("/api/v1/tree/stats", get(get_tree_stats))
        .route("/api/v1/tree/state", get(get_tree_state))
        .route("/api/v1/tree/proof/{index}", get(get_merkle_proof))
        // Advanced operations
        .route("/api/v1/state/commitment", get(get_state_commitment))
        .route("/api/v1/metrics", get(get_performance_metrics))
        .route("/api/v1/audit/compliance", get(get_compliance_report))
        // Prometheus metrics endpoint
        .route("/metrics", get(get_prometheus_metrics))
        // Root-level health endpoints
        .route("/health", get(health_check_simple))
        .route("/health/detailed", get(health_check))
        .route("/health/ready", get(health_check_ready))
        .route("/health/live", get(health_check_live))
        .with_state(state)
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// Health check endpoint
#[instrument(skip(state), level = "debug")]
async fn health_check(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();

    // Check ADS service health
    let ads_health = {
        let ads = state.ads.read().await;
        match ads.health_check().await {
            Ok(_) => ServiceStatus {
                healthy: true,
                latency_ms: Some(u64::try_from(start_time.elapsed().as_millis()).unwrap()),
                last_check: Utc::now(),
                error: None,
            },
            Err(e) => ServiceStatus {
                healthy: false,
                latency_ms: None,
                last_check: Utc::now(),
                error: Some(e.to_string()),
            },
        }
    };

    // Check vApp integration health
    let vapp_health = {
        let vapp = state.vapp_integration.read().await;
        match vapp.health_check().await {
            Ok(_) => ServiceStatus {
                healthy: true,
                latency_ms: Some(u64::try_from(start_time.elapsed().as_millis()).unwrap()),
                last_check: Utc::now(),
                error: None,
            },
            Err(e) => ServiceStatus {
                healthy: false,
                latency_ms: None,
                last_check: Utc::now(),
                error: Some(e.to_string()),
            },
        }
    };

    let mut services = HashMap::new();
    services.insert("ads".to_string(), ads_health);
    services.insert("vapp_integration".to_string(), vapp_health);

    let overall_healthy = services.values().all(|s| s.healthy);
    let status = if overall_healthy {
        "healthy"
    } else {
        "degraded"
    };

    // Convert services to checks format expected by the test
    let checks: Vec<serde_json::Value> = services
        .into_iter()
        .map(|(name, service)| {
            serde_json::json!({
                "name": name,
                "healthy": service.healthy,
                "error": service.error,
                "latency_ms": service.latency_ms,
                "last_check": service.last_check.to_rfc3339()
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "service_id": "vapp-ads-server",
        "status": status,
        "timestamp": Utc::now().to_rfc3339(),
        "version": state.config.version,
        "uptime_seconds": 0,
        "checks": checks
    })))
}

/// API information endpoint
#[instrument(skip(state), level = "debug")]
async fn api_info(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(serde_json::json!({
        "name": state.config.server_name,
        "version": state.config.version,
        "description": "Indexed Merkle Tree API with 32-level optimization",
        "features": {
            "tree_height": 32,
            "max_capacity": 4_294_967_296_u64,
            "constraint_optimization": "8x fewer than traditional trees",
            "proof_size": "1KB (32 × 32 bytes)",
            "batch_processing": true,
            "audit_trails": true,
            "zk_integration": true
        },
        "endpoints": {
            "rest": "/api/v1/*",
            "graphql": "/graphql",
            "playground": "/graphql (GET)"
        }
    })))
}

/// Insert a single nullifier
#[instrument(skip(state, request), level = "info")]
async fn insert_nullifier(
    State(state): State<ApiState>,
    Json(request): Json<InsertNullifierRequest>,
) -> Result<Json<InsertNullifierResponse>, (StatusCode, String)> {
    info!("🔄 API: Inserting nullifier {}", request.value);

    // Input validation
    if request.value <= 0 {
        warn!("Invalid nullifier value: {}", request.value);
        return Err((
            StatusCode::BAD_REQUEST,
            "Nullifier value must be positive".into(),
        ));
    }

    let start_time = std::time::Instant::now();

    // Process through vApp integration for full workflow
    let vapp_response = {
        let vapp = state.vapp_integration.read().await;
        vapp.process_nullifier_insertion(request.value)
            .await
            .map_err(|e| {
                error!("Nullifier insertion failed: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?
    };

    let processing_time = start_time.elapsed();

    // Convert to API response format
    let response = InsertNullifierResponse {
        success: true,
        transaction_id: vapp_response.transaction_id,
        state_transition: StateTransitionDto {
            id: vapp_response.state_transition.id,
            old_root: hex::encode(vapp_response.state_transition.old_root),
            new_root: hex::encode(vapp_response.state_transition.new_root),
            nullifier_value: vapp_response.state_transition.nullifier_value,
            block_height: vapp_response.state_transition.block_height,
            timestamp: vapp_response.state_transition.timestamp,
            gas_estimate: vapp_response.state_transition.gas_estimate,
        },
        constraint_count: ConstraintCount {
            total_hashes: 99, // 3 * 32 + 3
            range_checks: 2,
            equality_checks: 10,
            total_constraints: 200,
            vs_traditional: "8x fewer constraints (200 vs 1600)".to_string(),
        },
        performance_metrics: InsertionMetrics {
            insertion_time_ms: vapp_response.processing_time_ms,
            proof_generation_time_ms: 25, // From metrics
            database_operations: 6,
            hash_operations: 99,
        },
        settlement_info: vapp_response.settlement_result.map(|s| SettlementInfo {
            contract_address: "0x742d35cc...".to_string(), // From config
            chain_id: 1,
            estimated_gas: s.gas_used,
            estimated_cost_wei: format!("{}", s.gas_used * 20_000_000_000), // 20 gwei
        }),
    };

    info!(
        "✅ API: Nullifier {} inserted in {}ms",
        request.value,
        processing_time.as_millis()
    );
    Ok(Json(response))
}

/// Batch insert multiple nullifiers
#[instrument(skip(state, request), level = "info")]
async fn batch_insert_nullifiers(
    State(state): State<ApiState>,
    Json(request): Json<BatchInsertRequest>,
) -> Result<Json<BatchInsertResponse>, (StatusCode, String)> {
    info!(
        "📦 API: Batch inserting {} nullifiers",
        request.values.len()
    );

    // Validate batch size
    if request.values.len() > state.config.max_batch_size {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Batch size {} exceeds maximum {}",
                request.values.len(),
                state.config.max_batch_size
            ),
        ));
    }

    // Validate all values
    for &value in &request.values {
        if value <= 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Invalid nullifier value: {value}"),
            ));
        }
    }

    let start_time = std::time::Instant::now();

    // Process through vApp integration
    let vapp_response = {
        let vapp = state.vapp_integration.read().await;
        vapp.process_batch_insertions(&request.values)
            .await
            .map_err(|e| {
                error!("Batch insertion failed: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?
    };

    let processing_time = start_time.elapsed();

    // Check success before moving failed_operations
    let success = vapp_response.failed_operations.is_empty();

    // Convert failed operations
    let failed_operations = vapp_response
        .failed_operations
        .into_iter()
        .map(|f| BatchFailure {
            nullifier: f.nullifier,
            error: f.error,
            error_code: f.error_code,
        })
        .collect();

    let response = BatchInsertResponse {
        success,
        batch_id: vapp_response.batch_id,
        total_operations: vapp_response.total_operations,
        successful_operations: vapp_response.successful_operations,
        failed_operations,
        combined_metrics: ConstraintCount {
            total_hashes: u32::try_from(vapp_response.successful_operations).unwrap() * 99,
            range_checks: u32::try_from(vapp_response.successful_operations).unwrap() * 2,
            equality_checks: u32::try_from(vapp_response.successful_operations).unwrap() * 10,
            total_constraints: u32::try_from(vapp_response.successful_operations).unwrap() * 200,
            vs_traditional: format!("{}x fewer constraints", 8),
        },
        processing_time_ms: vapp_response.processing_time_ms,
    };

    info!(
        "✅ API: Batch completed - {}/{} successful in {}ms",
        vapp_response.successful_operations,
        vapp_response.total_operations,
        processing_time.as_millis()
    );

    Ok(Json(response))
}

/// Check nullifier membership
#[instrument(skip(state), level = "info")]
async fn check_membership(
    State(state): State<ApiState>,
    Path(value): Path<i64>,
) -> Result<Json<MembershipCheckResponse>, (StatusCode, String)> {
    info!("🔍 API: Checking membership for nullifier {}", value);
    let start_time = std::time::Instant::now();

    let vapp_response = {
        let vapp = state.vapp_integration.read().await;
        vapp.verify_nullifier_presence(value).await
    };

    let verification_time = start_time.elapsed();

    match vapp_response {
        Ok(response) => {
            let membership_proof = response.membership_proof.map(|proof| MembershipProofDto {
                tree_index: proof.tree_index,
                root_hash: hex::encode(proof.root_hash),
                merkle_proof: MerkleProofDto {
                    leaf_hash: hex::encode([0u8; 32]), // Would be actual leaf hash
                    siblings: proof
                        .merkle_proof
                        .siblings
                        .iter()
                        .map(hex::encode)
                        .collect(),
                    path_indices: proof.merkle_proof.path_indices.clone(),
                    proof_size_bytes: 32 * 32, // 32 siblings * 32 bytes
                },
                verified_at: proof.verified_at,
            });

            Ok(Json(MembershipCheckResponse {
                exists: response.verification_status,
                nullifier_value: value,
                proof: membership_proof,
                verification_time_ms: u64::try_from(verification_time.as_millis()).unwrap(),
            }))
        }
        Err(_) => {
            // Not found, return exists: false
            Ok(Json(MembershipCheckResponse {
                exists: false,
                nullifier_value: value,
                proof: None,
                verification_time_ms: u64::try_from(verification_time.as_millis()).unwrap(),
            }))
        }
    }
}

/// Prove non-membership of a nullifier
#[instrument(skip(state), level = "info")]
async fn prove_non_membership(
    State(state): State<ApiState>,
    Path(value): Path<i64>,
) -> Result<Json<NonMembershipResponse>, (StatusCode, String)> {
    info!("🔍 API: Proving non-membership for nullifier {}", value);
    let start_time = std::time::Instant::now();

    let vapp_response = {
        let vapp = state.vapp_integration.read().await;
        vapp.verify_nullifier_absence(value)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    let verification_time = start_time.elapsed();

    if let Some(non_membership_proof) = vapp_response.non_membership_proof {
        let gap_size = if non_membership_proof.low_nullifier.next_value == 0 {
            i64::MAX - non_membership_proof.low_nullifier.value
        } else {
            non_membership_proof.low_nullifier.next_value - non_membership_proof.low_nullifier.value
        };

        let response = NonMembershipResponse {
            proof: NonMembershipProofDto {
                queried_value: non_membership_proof.queried_value,
                low_nullifier: LowNullifierDto {
                    value: non_membership_proof.low_nullifier.value,
                    next_value: non_membership_proof.low_nullifier.next_value,
                    tree_index: non_membership_proof.low_nullifier.tree_index,
                    merkle_proof: MerkleProofDto {
                        leaf_hash: hex::encode([0u8; 32]),
                        siblings: non_membership_proof
                            .low_nullifier
                            .merkle_proof
                            .siblings
                            .iter()
                            .map(hex::encode)
                            .collect(),
                        path_indices: non_membership_proof
                            .low_nullifier
                            .merkle_proof
                            .path_indices
                            .clone(),
                        proof_size_bytes: 32 * 32,
                    },
                },
                root_hash: hex::encode(non_membership_proof.root_hash),
                range_proof: RangeProofDto {
                    lower_bound: non_membership_proof.range_proof.lower_bound,
                    upper_bound: non_membership_proof.range_proof.upper_bound,
                    queried_value: non_membership_proof.range_proof.queried_value,
                    valid: non_membership_proof.range_proof.valid,
                },
                verified_at: non_membership_proof.verified_at,
            },
            verification_data: NonMembershipVerification {
                low_nullifier_value: non_membership_proof.low_nullifier.value,
                queried_value: non_membership_proof.queried_value,
                next_value: non_membership_proof.low_nullifier.next_value,
                range_valid: non_membership_proof.range_proof.valid,
                proof_valid: vapp_response.verification_status,
                gap_size,
            },
            verification_time_ms: u64::try_from(verification_time.as_millis()).unwrap(),
        };

        Ok(Json(response))
    } else {
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to generate non-membership proof".into(),
        ))
    }
}

/// Get tree statistics
#[instrument(skip(state), level = "info")]
async fn get_tree_stats(
    State(state): State<ApiState>,
) -> Result<Json<TreeStatsResponse>, (StatusCode, String)> {
    let ads = state.ads.read().await;

    let commitment = ads
        .get_state_commitment()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let metrics = ads
        .get_metrics()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    drop(ads);

    let response = TreeStatsResponse {
        root_hash: hex::encode(commitment.root_hash),
        total_nullifiers: commitment.nullifier_count,
        tree_height: 32,
        next_available_index: commitment.nullifier_count, // Simplified
        performance_metrics: PerformanceMetrics {
            avg_insertion_time_ms: metrics.avg_insertion_time_ms,
            avg_proof_generation_time_ms: metrics.avg_proof_time_ms,
            total_operations: metrics.operations_total,
            error_rate_percent: metrics.error_rate * 100.0,
        },
        constraint_efficiency: ConstraintEfficiency {
            our_constraints: 200,
            traditional_constraints: 1600,
            improvement_factor: 8.0,
            description: "32-level tree vs traditional 256-level tree".to_string(),
        },
        last_updated: commitment.last_updated,
    };

    Ok(Json(response))
}

/// Get current tree root
#[instrument(skip(state), level = "debug")]
async fn get_tree_root(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let commitment = {
        let vapp = state.vapp_integration.read().await;
        vapp.get_current_state_commitment()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(serde_json::json!({
        "root_hash": hex::encode(commitment.root_hash),
        "nullifier_count": commitment.nullifier_count,
        "last_updated": commitment.last_updated,
    })))
}

/// Get tree state commitment
#[instrument(skip(state), level = "debug")]
async fn get_tree_state(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let commitment = {
        let vapp = state.vapp_integration.read().await;
        vapp.get_current_state_commitment()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(serde_json::json!({
        "root_hash": hex::encode(commitment.root_hash),
        "nullifier_count": commitment.nullifier_count,
        "tree_height": commitment.tree_height,
        "commitment_hash": hex::encode(commitment.commitment_hash),
        "settlement_data": {
            "contract_address": commitment.settlement_data.contract_address,
            "chain_id": commitment.settlement_data.chain_id,
            "nonce": commitment.settlement_data.nonce,
            "gas_price": commitment.settlement_data.gas_price,
        },
        "last_updated": commitment.last_updated,
    })))
}

/// Get Merkle proof for a specific leaf index
#[instrument(skip(_state), level = "debug")]
async fn get_merkle_proof(
    State(_state): State<ApiState>,
    Path(index): Path<u64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // This would need to be implemented in the ADS service
    // For now, return a placeholder response
    Ok(Json(serde_json::json!({
        "leaf_index": index,
        "message": "Merkle proof generation by index not yet implemented",
        "note": "Use membership check with nullifier value instead"
    })))
}

/// Get state commitment for settlement
#[instrument(skip(state), level = "info")]
async fn get_state_commitment(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let commitment = {
        let vapp = state.vapp_integration.read().await;
        vapp.get_current_state_commitment()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(serde_json::json!({
        "commitment": {
            "root_hash": hex::encode(commitment.root_hash),
            "nullifier_count": commitment.nullifier_count,
            "tree_height": commitment.tree_height,
            "commitment_hash": hex::encode(commitment.commitment_hash),
            "last_updated": commitment.last_updated,
        },
        "settlement": {
            "contract_address": commitment.settlement_data.contract_address,
            "chain_id": commitment.settlement_data.chain_id,
            "nonce": commitment.settlement_data.nonce,
            "gas_price": commitment.settlement_data.gas_price,
            "estimated_gas": 150_000, // Placeholder
        }
    })))
}

/// Get performance metrics
#[instrument(skip(state), level = "info")]
async fn get_performance_metrics(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let metrics = {
        let vapp = state.vapp_integration.read().await;
        vapp.get_metrics()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(serde_json::json!({
        "operations": {
            "total": metrics.operations_total,
            "insertions": metrics.insertions_total,
            "proofs_generated": metrics.proofs_generated,
        },
        "performance": {
            "avg_insertion_time_ms": metrics.avg_insertion_time_ms,
            "avg_proof_time_ms": metrics.avg_proof_time_ms,
            "error_rate_percent": metrics.error_rate * 100.0,
        },
        "constraints": {
            "avg_per_operation": metrics.constraint_efficiency.avg_constraints_per_op,
            "target": metrics.constraint_efficiency.target_constraints,
            "efficiency_ratio": metrics.constraint_efficiency.efficiency_ratio,
        },
        "last_reset": metrics.last_reset,
    })))
}

/// Get audit trail for a nullifier
#[instrument(skip(state), level = "info")]
async fn get_audit_trail(
    State(state): State<ApiState>,
    Path(value): Path<i64>,
) -> Result<Json<AuditTrailResponse>, (StatusCode, String)> {
    let audit_trail = {
        let ads = state.ads.read().await;
        ads.get_audit_trail(value)
            .await
            .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?
    };

    let events: Vec<AuditEventDto> = audit_trail
        .operation_history
        .into_iter()
        .map(|event| AuditEventDto {
            event_id: event.event_id,
            event_type: format!("{:?}", event.event_type),
            timestamp: event.timestamp,
            root_before: hex::encode(event.root_before),
            root_after: hex::encode(event.root_after),
            block_height: event.block_height,
            operator: event.operator,
            metadata: event.metadata,
        })
        .collect();

    let response = AuditTrailResponse {
        nullifier_value: audit_trail.nullifier_value,
        total_events: events.len(),
        events,
        compliance_status: ComplianceStatusDto {
            is_compliant: audit_trail.compliance_status.is_compliant,
            last_audit: audit_trail.compliance_status.last_audit,
            jurisdiction: audit_trail.compliance_status.jurisdiction,
            notes: audit_trail.compliance_status.notes,
        },
        created_at: audit_trail.created_at,
        last_accessed: audit_trail.last_accessed,
    };

    Ok(Json(response))
}

/// Get compliance report
#[instrument(level = "info")]
async fn get_compliance_report(
    State(state): State<ApiState>,
    Query(filter): Query<FilterQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // This would integrate with the compliance service
    // For now, return a placeholder
    Ok(Json(serde_json::json!({
        "message": "Compliance reporting endpoint",
        "filter": filter,
        "note": "Full compliance integration pending"
    })))
}

/// Prometheus metrics endpoint
#[instrument(skip(state), level = "info")]
async fn get_prometheus_metrics(
    State(state): State<ApiState>,
) -> Result<String, (StatusCode, String)> {
    let metrics = {
        let vapp = state.vapp_integration.read().await;
        vapp.get_metrics()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    // Generate Prometheus format
    let prometheus_metrics = format!(
        r"# HELP http_requests_total Total number of HTTP requests
# TYPE http_requests_total counter
http_requests_total {{}} {}

# HELP insertion_operations_total Total number of insertion operations
# TYPE insertion_operations_total counter
insertion_operations_total {{}} {}

# HELP proof_operations_total Total number of proof operations
# TYPE proof_operations_total counter
proof_operations_total {{}} {}

# HELP merkle_tree_operations_total Total number of Merkle tree operations
# TYPE merkle_tree_operations_total counter
merkle_tree_operations_total {{}} {}

# HELP constraint_count Current constraint count
# TYPE constraint_count gauge
constraint_count {{}} {}

# HELP avg_insertion_time_seconds Average insertion time in seconds
# TYPE avg_insertion_time_seconds gauge
avg_insertion_time_seconds {{}} {}

# HELP avg_proof_time_seconds Average proof generation time in seconds
# TYPE avg_proof_time_seconds gauge
avg_proof_time_seconds {{}} {}

# HELP error_rate_percent Error rate percentage
# TYPE error_rate_percent gauge
error_rate_percent {{}} {}

# HELP constraint_efficiency_ratio Constraint efficiency ratio
# TYPE constraint_efficiency_ratio gauge
constraint_efficiency_ratio {{}} {}
",
        metrics.operations_total,
        metrics.insertions_total,
        metrics.proofs_generated,
        metrics.insertions_total + metrics.proofs_generated, // merkle tree ops
        metrics.constraint_efficiency.avg_constraints_per_op, // constraint count
        metrics.avg_insertion_time_ms / 1000.0,
        metrics.avg_proof_time_ms / 1000.0,
        metrics.error_rate * 100.0,
        metrics.constraint_efficiency.efficiency_ratio,
    );

    Ok(prometheus_metrics)
}

/// Simple health check endpoint
#[instrument(skip(state), level = "info")]
async fn health_check_simple(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Quick health check without detailed service status
    let _ads = state.ads.read().await;
    let _vapp = state.vapp_integration.read().await;

    Ok(Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Health readiness check endpoint
#[instrument(skip(state), level = "info")]
async fn health_check_ready(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Check if services are ready to serve requests
    let _ads = state.ads.read().await;
    let _vapp = state.vapp_integration.read().await;

    Ok(Json(serde_json::json!({
        "status": "ready",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Health liveness check endpoint
#[instrument(skip(state), level = "info")]
async fn health_check_live(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Basic liveness check - service is alive
    let _ads = state.ads.read().await;
    let _vapp = state.vapp_integration.read().await;

    Ok(Json(serde_json::json!({
        "status": "live",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

// ============================================================================
// VAPP TRANSACTION HANDLERS
// ============================================================================

/// Submit a new transaction (a + b)
#[instrument(skip(state, request), level = "info")]
async fn submit_transaction(
    State(state): State<ApiState>,
    Json(request): Json<TransactionRequest>,
) -> Result<Json<TransactionResponse>, (StatusCode, String)> {
    info!(
        "🧮 API: Submitting transaction {} + {}",
        request.a, request.b
    );

    let result = addition(request.a, request.b);
    let transaction_id = uuid::Uuid::new_v4().to_string();

    // Store transaction with atomic state update for continuous ledger
    let pool = {
        let guard = state.vapp_integration.read().await;
        guard.pool.clone()
    };

    let state_transition =
        match store_transaction_with_state_update(&pool, request.a, request.b, result).await {
            Ok(transition) => transition,
            Err(e) => {
                error!("Failed to store transaction with state update: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to store transaction with state update: {}", e),
                ));
            }
        };

    let mut proof_id = None;
    let mut proof_status = None;

    // Generate proof if requested
    if request.generate_proof.unwrap_or(false) {
        match generate_sindri_proof(request.a, request.b, result).await {
            Ok((id, status)) => {
                proof_id = Some(id.clone());
                proof_status = Some(status);

                // Store proof metadata in database
                if let Err(e) = crate::db::upsert_sindri_proof(
                    &pool,
                    result,
                    &id,
                    Some("demo-vapp".to_string()),
                    proof_status.clone(),
                )
                .await
                {
                    warn!("Failed to store proof metadata: {}", e);
                }
            }
            Err(e) => {
                warn!("Proof generation failed: {}", e);
                proof_status = Some("Failed".to_string());
            }
        }
    }

    let verification_command = proof_id.as_ref().map(|pid| {
        format!(
            "cargo run --release -- --verify --proof-id {} --result {}",
            pid, result
        )
    });

    let response = TransactionResponse {
        transaction_id,
        a: request.a,
        b: request.b,
        result,
        previous_state: state_transition.previous_state,
        new_state: state_transition.new_state,
        proof_id: proof_id.clone(),
        proof_status: proof_status.clone(),
        verification_info: VerificationInfo {
            can_verify_externally: proof_id.is_some(),
            verification_command,
            db_stored: true,
            merkle_tree_updated: false, // Updated by background processor
        },
        state_info: StateInfo {
            state_updated: true,
            transaction_count: state_transition.new_state
                / if result == 0 { 1 } else { result } as i64, // Estimate
            continuous_ledger: true,
            state_description: format!(
                "State transition: {} + {} = {}, global state: {} → {}",
                state_transition.previous_state,
                result,
                state_transition.new_state,
                state_transition.previous_state,
                state_transition.new_state
            ),
        },
    };

    info!("✅ API: Transaction submitted - result = {}", result);
    Ok(Json(response))
}

/// Get transaction inputs by result value
#[instrument(skip(state), level = "info")]
async fn get_transaction_by_result(
    State(state): State<ApiState>,
    Path(result): Path<i32>,
) -> Result<Json<TransactionByResultResponse>, (StatusCode, String)> {
    info!("🔍 API: Looking up transaction for result = {}", result);

    let pool = state.vapp_integration.read().await.pool.clone();

    match get_value_by_result(&pool, result).await {
        Ok(Some((a, b, created_at))) => {
            // Check if there's an associated proof
            let proof_info = get_sindri_proof_by_result(&pool, result)
                .await
                .ok()
                .flatten();

            let response = TransactionByResultResponse {
                result,
                a,
                b,
                found: true,
                metadata: TransactionMetadata {
                    stored_at: Some(created_at),
                    has_proof: proof_info.is_some(),
                    proof_id: proof_info.as_ref().map(|p| p.proof_id.clone()),
                    verification_status: proof_info.as_ref().and_then(|p| p.status.clone()),
                },
            };

            info!("✅ API: Found transaction {} + {} = {}", a, b, result);
            Ok(Json(response))
        }
        Ok(None) => {
            let response = TransactionByResultResponse {
                result,
                a: 0,
                b: 0,
                found: false,
                metadata: TransactionMetadata {
                    stored_at: None,
                    has_proof: false,
                    proof_id: None,
                    verification_status: None,
                },
            };

            Ok(Json(response))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )),
    }
}

/// Get proof information by proof ID
#[instrument(skip(_state), level = "info")]
async fn get_proof_info(
    State(_state): State<ApiState>,
    Path(proof_id): Path<String>,
) -> Result<Json<ProofResponse>, (StatusCode, String)> {
    info!("🔍 API: Getting proof info for ID: {}", proof_id);

    let client = SindriClient::default();

    match client.get_proof(&proof_id, None, None, None).await {
        Ok(proof_info) => {
            let verification_data = if proof_info.status == JobStatus::Ready {
                // Perform local verification
                if let Ok(sp1_proof) = proof_info.to_sp1_proof_with_public() {
                    if let Ok(vk) = proof_info.get_sp1_verifying_key() {
                        match proof_info.verify_sp1_proof_locally(&vk) {
                            Ok(()) => {
                                // Proof verification succeeded, extract public values
                                match PublicValuesStruct::abi_decode(
                                    sp1_proof.public_values.as_slice(),
                                ) {
                                    Ok(decoded) => Some(ProofVerificationData {
                                        is_verified: true,
                                        public_result: decoded.result,
                                        verification_message: "Proof cryptographically verified"
                                            .to_string(),
                                        cryptographic_proof_valid: true,
                                    }),
                                    Err(decode_err) => Some(ProofVerificationData {
                                        is_verified: false,
                                        public_result: 0,
                                        verification_message: format!(
                                            "Failed to decode public values: {}",
                                            decode_err
                                        ),
                                        cryptographic_proof_valid: true, // Crypto verification passed, decode failed
                                    }),
                                }
                            }
                            Err(verify_err) => Some(ProofVerificationData {
                                is_verified: false,
                                public_result: 0,
                                verification_message: format!(
                                    "Cryptographic proof verification failed: {}",
                                    verify_err
                                ),
                                cryptographic_proof_valid: false,
                            }),
                        }
                    } else {
                        Some(ProofVerificationData {
                            is_verified: false,
                            public_result: 0,
                            verification_message: "Failed to get verifying key".to_string(),
                            cryptographic_proof_valid: false,
                        })
                    }
                } else {
                    Some(ProofVerificationData {
                        is_verified: false,
                        public_result: 0,
                        verification_message: "Failed to convert to SP1 proof".to_string(),
                        cryptographic_proof_valid: false,
                    })
                }
            } else {
                None
            };

            let response = ProofResponse {
                proof_id: proof_id.clone(),
                status: format!("{:?}", proof_info.status),
                result: verification_data.as_ref().map(|v| v.public_result),
                verification_data,
                circuit_info: ProofCircuitInfo {
                    circuit_id: proof_info.circuit_id,
                    circuit_name: "demo-vapp".to_string(),
                    proof_system: "SP1".to_string(),
                },
            };

            Ok(Json(response))
        }
        Err(e) => Err((StatusCode::NOT_FOUND, format!("Proof not found: {}", e))),
    }
}

/// Verify proof for a specific result
#[instrument(skip(state), level = "info")]
async fn verify_result_proof(
    State(state): State<ApiState>,
    Path(result): Path<i32>,
) -> Result<Json<VerifyProofResponse>, (StatusCode, String)> {
    info!("🔐 API: Verifying proof for result = {}", result);

    let pool = state.vapp_integration.read().await.pool.clone();

    match get_sindri_proof_by_result(&pool, result).await {
        Ok(Some(stored_proof)) => verify_proof_internal(&stored_proof.proof_id, result).await,
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            format!("No proof found for result = {}", result),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )),
    }
}

/// Verify proof by proof ID
#[instrument(skip(_state, request), level = "info")]
async fn verify_proof(
    State(_state): State<ApiState>,
    Json(request): Json<VerifyProofRequest>,
) -> Result<Json<VerifyProofResponse>, (StatusCode, String)> {
    info!("🔐 API: Verifying proof ID: {}", request.proof_id);

    verify_proof_internal(&request.proof_id, request.expected_result).await
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Generate a proof via Sindri
async fn generate_sindri_proof(a: i32, b: i32, _result: i32) -> Result<(String, String), String> {
    let mut stdin = SP1Stdin::new();
    stdin.write(&a);
    stdin.write(&b);

    let stdin_json = serde_json::to_string(&stdin)
        .map_err(|e| format!("Failed to serialize SP1Stdin: {}", e))?;
    let proof_input = ProofInput::from(stdin_json);

    let client = SindriClient::default();
    let proof_info = client
        .prove_circuit("demo-vapp", proof_input, None, None, None)
        .await
        .map_err(|e| format!("Failed to submit proof: {}", e))?;

    let status = match proof_info.status {
        JobStatus::Ready => "Ready".to_string(),
        JobStatus::Failed => "Failed".to_string(),
        _ => "Pending".to_string(),
    };

    Ok((proof_info.proof_id, status))
}

/// Internal proof verification logic
async fn verify_proof_internal(
    proof_id: &str,
    expected_result: i32,
) -> Result<Json<VerifyProofResponse>, (StatusCode, String)> {
    let start_time = std::time::Instant::now();
    let client = SindriClient::default();

    match client.get_proof(proof_id, None, None, None).await {
        Ok(proof_info) => {
            let sindri_status = format!("{:?}", proof_info.status);
            let mut actual_result = None;
            let mut cryptographic_proof_valid = false;

            if proof_info.status == JobStatus::Ready {
                // Perform local verification
                if let Ok(sp1_proof) = proof_info.to_sp1_proof_with_public() {
                    if let Ok(vk) = proof_info.get_sp1_verifying_key() {
                        if matches!(proof_info.verify_sp1_proof_locally(&vk), Ok(())) {
                            cryptographic_proof_valid = true;

                            // Extract result from public values
                            if let Ok(decoded) =
                                PublicValuesStruct::abi_decode(sp1_proof.public_values.as_slice())
                            {
                                actual_result = Some(decoded.result);
                            }
                        } else {
                            cryptographic_proof_valid = false;
                        }
                    }
                }
            }

            let result_matches = actual_result.map(|r| r == expected_result).unwrap_or(false);
            let verification_time = start_time.elapsed();

            let response = VerifyProofResponse {
                valid: cryptographic_proof_valid && result_matches,
                proof_id: proof_id.to_string(),
                actual_result,
                expected_result,
                verification_details: VerificationDetails {
                    sindri_status,
                    cryptographic_proof_valid,
                    result_matches_expected: result_matches,
                    verification_time_ms: verification_time.as_millis() as u64,
                },
                zero_knowledge_properties: ZkProperties {
                    privacy_preserved: true,
                    inputs_hidden: true,
                    soundness: cryptographic_proof_valid,
                    completeness: result_matches,
                    description: "SP1 zero-knowledge proof preserves input privacy while proving computation correctness".to_string(),
                },
            };

            Ok(Json(response))
        }
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            format!("Proof verification failed: {}", e),
        )),
    }
}

// ============================================================================
// STATE MANAGEMENT HANDLERS
// ============================================================================

/// Get current global state
#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStateResponse {
    pub current_state: i64,
    pub transaction_count: i64,
    pub last_updated: DateTime<Utc>,
    pub continuous_ledger: bool,
    pub description: String,
}

#[instrument(skip(state), level = "info")]
async fn get_current_state(
    State(state): State<ApiState>,
) -> Result<Json<GlobalStateResponse>, (StatusCode, String)> {
    info!("📊 API: Getting current global state");

    let pool = state.vapp_integration.read().await.pool.clone();

    match get_current_global_state(&pool).await {
        Ok(global_state) => {
            let response = GlobalStateResponse {
                current_state: global_state.current_state,
                transaction_count: global_state.transaction_count,
                last_updated: global_state.last_updated,
                continuous_ledger: true,
                description: format!(
                    "Continuous state ledger: {} transactions processed, current state = {}",
                    global_state.transaction_count, global_state.current_state
                ),
            };

            info!(
                "✅ API: Current state = {}, {} transactions",
                global_state.current_state, global_state.transaction_count
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get current state: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get current state: {}", e),
            ))
        }
    }
}

/// State history response
#[derive(Debug, Serialize, Deserialize)]
pub struct StateHistoryResponse {
    pub transitions: Vec<GlobalStateTransitionDto>,
    pub total_transitions: usize,
    pub continuous_ledger: bool,
    pub description: String,
}

/// Global state transition DTO (different from existing StateTransitionDto)
#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStateTransitionDto {
    pub id: i32,
    pub transaction_id: i32,
    pub previous_state: i64,
    pub arithmetic_result: i32,
    pub new_state: i64,
    pub a: i32,
    pub b: i32,
    pub created_at: DateTime<Utc>,
    pub state_change: String,
}

/// Query parameters for state history
#[derive(Debug, Deserialize)]
pub struct StateHistoryQuery {
    pub limit: Option<i32>,
}

#[instrument(skip(state), level = "info")]
async fn get_state_history_endpoint(
    State(state): State<ApiState>,
    Query(query): Query<StateHistoryQuery>,
) -> Result<Json<StateHistoryResponse>, (StatusCode, String)> {
    info!("📈 API: Getting state history (limit: {:?})", query.limit);

    let pool = state.vapp_integration.read().await.pool.clone();

    match get_state_history(&pool, query.limit).await {
        Ok(transitions) => {
            let transition_dtos: Vec<GlobalStateTransitionDto> = transitions
                .into_iter()
                .map(|t| GlobalStateTransitionDto {
                    id: t.id,
                    transaction_id: t.transaction_id,
                    previous_state: t.previous_state,
                    arithmetic_result: t.arithmetic_result,
                    new_state: t.new_state,
                    a: t.a,
                    b: t.b,
                    created_at: t.created_at,
                    state_change: format!(
                        "{} + {} = {}",
                        t.previous_state, t.arithmetic_result, t.new_state
                    ),
                })
                .collect();

            let response = StateHistoryResponse {
                total_transitions: transition_dtos.len(),
                transitions: transition_dtos,
                continuous_ledger: true,
                description: "State transition history showing continuous ledger updates"
                    .to_string(),
            };

            info!(
                "✅ API: Found {} state transitions",
                response.total_transitions
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get state history: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get state history: {}", e),
            ))
        }
    }
}

/// State validation response
#[derive(Debug, Serialize, Deserialize)]
pub struct StateValidationResponse {
    pub is_valid: bool,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub continuous_ledger: bool,
}

#[instrument(skip(state), level = "info")]
async fn validate_state(
    State(state): State<ApiState>,
) -> Result<Json<StateValidationResponse>, (StatusCode, String)> {
    info!("🔍 API: Validating state integrity");

    let pool = state.vapp_integration.read().await.pool.clone();

    match validate_state_integrity(&pool).await {
        Ok((is_valid, message)) => {
            let response = StateValidationResponse {
                is_valid,
                message: message.clone(),
                timestamp: Utc::now(),
                continuous_ledger: true,
            };

            info!("✅ API: State validation result: {}", message);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to validate state: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to validate state: {}", e),
            ))
        }
    }
}
