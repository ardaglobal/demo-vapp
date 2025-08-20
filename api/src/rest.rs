use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use humantime::format_duration;
use serde::{Deserialize, Serialize};
use sqlx::{Error as SqlxError, PgPool};
use tracing::{error, info, instrument, warn};

use crate::batch_processor::BatchProcessorHandle;
use arithmetic_db::{
    get_all_batches, get_batch_by_id, get_contract_submission_data,
    get_current_state, get_pending_transactions, submit_transaction,
    update_batch_proof, store_ads_state_commit, ContractSubmissionData,
    IndexedMerkleTreeADS,
};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// API STATE
// ============================================================================

/// API state containing the database pool, configuration, and ADS service
#[derive(Clone)]
pub struct ApiState {
    pub pool: PgPool,
    pub config: ApiConfig,
    pub batch_processor: Option<BatchProcessorHandle>,
    pub ads_service: Arc<RwLock<IndexedMerkleTreeADS>>,
}

/// Configuration for API server
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub server_name: String,
    pub version: String,
    pub max_batch_size: u32,
    pub enable_debug_endpoints: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            server_name: "Batch Processing API".to_string(),
            version: "2.0.0".to_string(),
            max_batch_size: 50,
            enable_debug_endpoints: false,
        }
    }
}

// ============================================================================
// REQUEST/RESPONSE MODELS
// ============================================================================

/// Request to submit a single transaction
#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitTransactionRequest {
    pub amount: i32,
}

/// Response from transaction submission
#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitTransactionResponse {
    pub transaction_id: i32,
    pub amount: i32,
    pub status: String, // "pending"
    pub created_at: DateTime<Utc>,
}

/// Request to create a batch
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBatchRequest {
    pub batch_size: Option<i32>,
}

/// Response from batch creation
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBatchResponse {
    pub batch_id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub proof_status: String,
    pub created_at: DateTime<Utc>,
}

/// Response for pending transactions
#[derive(Debug, Serialize, Deserialize)]
pub struct PendingTransactionsResponse {
    pub transactions: Vec<TransactionInfo>,
    pub total_count: usize,
    pub total_amount: i32,
}

/// Transaction info for API responses
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub id: i32,
    pub amount: i32,
    pub created_at: DateTime<Utc>,
}

/// Response for batch listing
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchListResponse {
    pub batches: Vec<BatchInfo>,
    pub total_count: usize,
}

/// Batch info for API responses
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchInfo {
    pub id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub proof_status: String,
    pub sindri_proof_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub proven_at: Option<DateTime<Utc>>,
}

/// Response for current state
#[derive(Debug, Serialize, Deserialize)]
pub struct CurrentStateResponse {
    pub counter_value: i64,
    pub has_merkle_root: bool,
    pub last_batch_id: Option<i32>,
    pub last_proven_batch_id: Option<i32>,
}

/// Request to update batch with proof
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateBatchProofRequest {
    pub sindri_proof_id: String,
    pub status: String,              // "proven", "failed"
    pub merkle_root: Option<String>, // hex encoded
}

/// Query parameters for batch listing
#[derive(Debug, Deserialize)]
pub struct BatchListQuery {
    pub limit: Option<i32>,
}

/// API information response
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiInfoResponse {
    pub server_name: String,
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub endpoints: Vec<EndpointInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointInfo {
    pub method: String,
    pub path: String,
    pub description: String,
}

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub database_connected: bool,
}

/// Batch processor stats response
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchProcessorStatsResponse {
    pub total_batches_created: u64,
    pub total_transactions_processed: u64,
    pub timer_triggers: u64,
    pub count_triggers: u64,
    pub manual_triggers: u64,
    pub errors: u64,
    pub last_batch_time: Option<String>,
}

/// Manual batch trigger response
#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerBatchResponse {
    pub triggered: bool,
    pub message: String,
}

// ============================================================================
// ROUTER SETUP
// ============================================================================

/// Create the main API router with all batch processing endpoints
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        // Health and info endpoints (additional legacy paths)
        .route("/health", get(health_check))
        .route("/", get(api_info))
        .route("/api/v2/health", get(health_check))
        .route("/api/v2/info", get(api_info))
        // Transaction operations
        .route("/api/v2/transactions", post(submit_transaction_endpoint))
        .route(
            "/api/v2/transactions/pending",
            get(get_pending_transactions_endpoint),
        )
        // Batch operations
        .route("/api/v2/batches", post(create_batch_endpoint))
        .route("/api/v2/batches", get(get_batches_endpoint))
        .route("/api/v2/batches/{batch_id}", get(get_batch_endpoint))
        .route(
            "/api/v2/batches/{batch_id}/proof",
            post(update_batch_proof_endpoint),
        )
        .route("/api/v2/batches/trigger", post(trigger_batch_endpoint))
        .route(
            "/api/v2/batches/stats",
            get(get_batch_processor_stats_endpoint),
        )
        // State operations
        .route("/api/v2/state/current", get(get_current_state_endpoint))
        .route(
            "/api/v2/state/{batch_id}/contract",
            get(get_contract_data_endpoint),
        )
        .with_state(state)
}

// ============================================================================
// ENDPOINT HANDLERS
// ============================================================================

/// Health check endpoint
#[instrument(skip(state), level = "info")]
async fn health_check(
    State(state): State<ApiState>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    info!("🔍 API: Health check requested");

    // Test database connection
    let db_connected = match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => true,
        Err(e) => {
            error!("Database health check failed: {}", e);
            false
        }
    };

    let response = HealthResponse {
        status: if db_connected {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        timestamp: Utc::now(),
        database_connected: db_connected,
    };

    if db_connected {
        info!("✅ API: Health check passed");
        Ok(Json(response))
    } else {
        error!("❌ API: Health check failed - database not connected");
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Database not available".to_string(),
        ))
    }
}

/// API information endpoint
#[instrument(skip(state), level = "info")]
async fn api_info(State(state): State<ApiState>) -> Json<ApiInfoResponse> {
    info!("📋 API: API info requested");

    let endpoints = vec![
        EndpointInfo {
            method: "GET".to_string(),
            path: "/api/v2/health".to_string(),
            description: "Health check".to_string(),
        },
        EndpointInfo {
            method: "POST".to_string(),
            path: "/api/v2/transactions".to_string(),
            description: "Submit a new transaction".to_string(),
        },
        EndpointInfo {
            method: "GET".to_string(),
            path: "/api/v2/transactions/pending".to_string(),
            description: "Get pending (unbatched) transactions".to_string(),
        },
        EndpointInfo {
            method: "POST".to_string(),
            path: "/api/v2/batches".to_string(),
            description: "Create a new batch from pending transactions".to_string(),
        },
        EndpointInfo {
            method: "GET".to_string(),
            path: "/api/v2/batches".to_string(),
            description: "List historical batches".to_string(),
        },
        EndpointInfo {
            method: "GET".to_string(),
            path: "/api/v2/batches/{batch_id}".to_string(),
            description: "Get specific batch details".to_string(),
        },
        EndpointInfo {
            method: "POST".to_string(),
            path: "/api/v2/batches/{batch_id}/proof".to_string(),
            description: "Update batch with ZK proof".to_string(),
        },
        EndpointInfo {
            method: "GET".to_string(),
            path: "/api/v2/state/current".to_string(),
            description: "Get current counter state".to_string(),
        },
        EndpointInfo {
            method: "GET".to_string(),
            path: "/api/v2/state/{batch_id}/contract".to_string(),
            description: "Get contract submission data (dry run)".to_string(),
        },
    ];

    let response = ApiInfoResponse {
        server_name: state.config.server_name.clone(),
        version: state.config.version.clone(),
        timestamp: Utc::now(),
        endpoints,
    };

    info!("✅ API: API info returned");
    Json(response)
}

/// Submit a new transaction
#[instrument(skip(state), level = "info")]
async fn submit_transaction_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<SubmitTransactionRequest>,
) -> Result<Json<SubmitTransactionResponse>, (StatusCode, String)> {
    info!("💰 API: Submitting transaction: amount={}", request.amount);

    match submit_transaction(&state.pool, request.amount).await {
        Ok(transaction) => {
            let response = SubmitTransactionResponse {
                transaction_id: transaction.id,
                amount: transaction.amount,
                status: "pending".to_string(),
                created_at: transaction.created_at,
            };

            info!("✅ API: Transaction submitted: id={}", transaction.id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to submit transaction: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to submit transaction: {}", e),
            ))
        }
    }
}

/// Get pending transactions
#[instrument(skip(state), level = "info")]
async fn get_pending_transactions_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<PendingTransactionsResponse>, (StatusCode, String)> {
    info!("📋 API: Getting pending transactions");

    match get_pending_transactions(&state.pool).await {
        Ok(transactions) => {
            let total_amount: i32 = transactions.iter().map(|t| t.amount).sum();
            let transaction_infos: Vec<TransactionInfo> = transactions
                .into_iter()
                .map(|t| TransactionInfo {
                    id: t.id,
                    amount: t.amount,
                    created_at: t.created_at,
                })
                .collect();

            let response = PendingTransactionsResponse {
                total_count: transaction_infos.len(),
                total_amount,
                transactions: transaction_infos,
            };

            info!(
                "✅ API: Found {} pending transactions",
                response.total_count
            );
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get pending transactions: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get pending transactions: {}", e),
            ))
        }
    }
}

/// Create a new batch using unified ADS-integrated workflow
#[instrument(skip(state), level = "info")]
async fn create_batch_endpoint(
    State(state): State<ApiState>,
    Json(request): Json<CreateBatchRequest>,
) -> Result<Json<CreateBatchResponse>, (StatusCode, String)> {
    // Validate and clamp batch_size to ensure it's in the valid range [1, max_batch_size]
    let requested_size = request
        .batch_size
        .unwrap_or(state.config.max_batch_size as i32);
    let batch_size = requested_size
        .max(1)
        .min(state.config.max_batch_size as i32);
    
    info!("🔄 UNIFIED API: Creating batch with size: {} (using ADS integration)", batch_size);

    // Use unified batch service for consistent ADS integration
    let unified_service = crate::unified_batch_service::UnifiedBatchService::new(
        state.pool.clone(),
        state.ads_service.clone(),
        state.config.max_batch_size,
    );

    match unified_service.create_batch_with_ads(Some(batch_size), "api").await {
        Ok(Some(result)) => {
            let response = CreateBatchResponse {
                batch_id: result.batch_id,
                previous_counter_value: result.previous_counter_value,
                final_counter_value: result.final_counter_value,
                transaction_count: result.transaction_count,
                proof_status: "pending".to_string(), // New batches start as pending
                created_at: chrono::Utc::now(),
            };

            info!(
                "✅ UNIFIED API: Batch created with ADS integration: id={}, transactions={}, nullifiers={}, merkle_root=0x{}",
                result.batch_id, 
                result.transaction_count,
                result.nullifier_count,
                hex::encode(&result.merkle_root[..8])
            );

            // Trigger proof generation for the newly created batch
            if state.batch_processor.is_some() {
                info!("🚀 Triggering proof generation for batch {}", result.batch_id);
                tokio::spawn({
                    let pool = state.pool.clone();
                    let batch_id = result.batch_id;
                    async move {
                        if let Err(e) = crate::batch_processor::BackgroundBatchProcessor::generate_proof_for_batch(&pool, batch_id).await {
                            error!("Failed to generate proof for unified batch {}: {}", batch_id, e);
                        }
                    }
                });
            } else {
                warn!("Batch processor not available - proof generation skipped");
            }

            Ok(Json(response))
        }
        Ok(None) => {
            info!("ℹ️ UNIFIED API: No transactions available to batch");
            Err((
                StatusCode::BAD_REQUEST,
                "No pending transactions available to batch".to_string(),
            ))
        }
        Err(e) => {
            error!("UNIFIED API: Failed to create batch: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create batch: {}", e),
            ))
        }
    }
}

/// Get all batches (paginated)
#[instrument(skip(state), level = "info")]
async fn get_batches_endpoint(
    State(state): State<ApiState>,
    Query(params): Query<BatchListQuery>,
) -> Result<Json<BatchListResponse>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(20);
    info!("📋 API: Getting batches with limit: {}", limit);

    match get_all_batches(&state.pool, Some(limit)).await {
        Ok(batches) => {
            let batch_infos: Vec<BatchInfo> = batches
                .into_iter()
                .map(|b| BatchInfo {
                    id: b.id,
                    previous_counter_value: b.previous_counter_value,
                    final_counter_value: b.final_counter_value,
                    transaction_count: b.transaction_ids.len(),
                    proof_status: b.proof_status,
                    sindri_proof_id: b.sindri_proof_id,
                    created_at: b.created_at,
                    proven_at: b.proven_at,
                })
                .collect();

            let response = BatchListResponse {
                total_count: batch_infos.len(),
                batches: batch_infos,
            };

            info!("✅ API: Found {} batches", response.total_count);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get batches: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get batches: {}", e),
            ))
        }
    }
}

/// Get specific batch by ID
#[instrument(skip(state), level = "info")]
async fn get_batch_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
) -> Result<Json<BatchInfo>, (StatusCode, String)> {
    info!("🔍 API: Getting batch: id={}", batch_id);

    match get_batch_by_id(&state.pool, batch_id).await {
        Ok(batch) => {
            let batch_info = BatchInfo {
                id: batch.id,
                previous_counter_value: batch.previous_counter_value,
                final_counter_value: batch.final_counter_value,
                transaction_count: batch.transaction_ids.len(),
                proof_status: batch.proof_status,
                sindri_proof_id: batch.sindri_proof_id,
                created_at: batch.created_at,
                proven_at: batch.proven_at,
            };

            info!("✅ API: Found batch: id={}", batch.id);
            Ok(Json(batch_info))
        }
        Err(SqlxError::RowNotFound) => {
            info!("❌ API: Batch not found: id={}", batch_id);
            Err((
                StatusCode::NOT_FOUND,
                format!("Batch not found: {}", batch_id),
            ))
        }
        Err(e) => {
            error!("Failed to get batch: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get batch: {}", e),
            ))
        }
    }
}

/// Update batch with ZK proof information
///
/// Validates:
/// - Status must be one of: "pending", "proven", "failed"
/// - When status is "proven", merkle_root is required and must be exactly 32 bytes
/// - When status is not "proven", merkle_root is optional but if provided must be valid hex and 32 bytes
/// - Hex values can optionally start with "0x" prefix
#[instrument(skip(state), level = "info")]
async fn update_batch_proof_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
    Json(request): Json<UpdateBatchProofRequest>,
) -> Result<Json<BatchInfo>, (StatusCode, String)> {
    info!(
        "🔐 API: Updating batch proof: id={}, proof={}",
        batch_id, request.sindri_proof_id
    );

    // Validate status against allowed values
    const ALLOWED_STATUSES: &[&str] = &["pending", "proven", "failed"];
    if !ALLOWED_STATUSES.contains(&request.status.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid status '{}'. Allowed values: {}",
                request.status,
                ALLOWED_STATUSES.join(", ")
            ),
        ));
    }

    // Validate merkle_root requirements based on status
    let validated_merkle_root = if request.status == "proven" {
        // For "proven" status, merkle_root is required
        match &request.merkle_root {
            Some(merkle_root_hex) => {
                // Decode hex string to bytes
                let merkle_root =
                    hex::decode(merkle_root_hex.trim_start_matches("0x")).map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            format!("Invalid merkle root hex: {}", e),
                        )
                    })?;

                // Validate that merkle_root is exactly 32 bytes
                if merkle_root.len() != 32 {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!(
                            "Merkle root must be exactly 32 bytes, got {} bytes",
                            merkle_root.len()
                        ),
                    ));
                }

                Some(merkle_root)
            }
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Merkle root is required when status is 'proven'".to_string(),
                ));
            }
        }
    } else {
        // For other statuses, merkle_root is not required (but allowed)
        if let Some(merkle_root_hex) = &request.merkle_root {
            // If provided, still validate the format
            let merkle_root =
                hex::decode(merkle_root_hex.trim_start_matches("0x")).map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid merkle root hex: {}", e),
                    )
                })?;

            // Validate that merkle_root is exactly 32 bytes
            if merkle_root.len() != 32 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "Merkle root must be exactly 32 bytes, got {} bytes",
                        merkle_root.len()
                    ),
                ));
            }

            Some(merkle_root)
        } else {
            None
        }
    };

    // Update batch with proof
    match update_batch_proof(
        &state.pool,
        batch_id,
        &request.sindri_proof_id,
        &request.status,
    )
    .await
    {
        Ok(()) => {
            // Store Merkle root if validated and provided
            if let Some(merkle_root) = validated_merkle_root {
                if let Err(e) = store_ads_state_commit(&state.pool, batch_id, &merkle_root).await {
                    error!("Failed to store ADS state commit: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to store ADS state commit: {}", e),
                    ));
                }
            }

            // Return updated batch info
            match get_batch_by_id(&state.pool, batch_id).await {
                Ok(batch) => {
                    let batch_info = BatchInfo {
                        id: batch.id,
                        previous_counter_value: batch.previous_counter_value,
                        final_counter_value: batch.final_counter_value,
                        transaction_count: batch.transaction_ids.len(),
                        proof_status: batch.proof_status,
                        sindri_proof_id: batch.sindri_proof_id,
                        created_at: batch.created_at,
                        proven_at: batch.proven_at,
                    };

                    info!("✅ API: Batch proof updated: id={}", batch_id);
                    Ok(Json(batch_info))
                }
                Err(e) => {
                    error!("Failed to get updated batch: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to get updated batch: {}", e),
                    ))
                }
            }
        }
        Err(e) => {
            error!("Failed to update batch proof: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update batch proof: {}", e),
            ))
        }
    }
}

/// Get current counter state
#[instrument(skip(state), level = "info")]
async fn get_current_state_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<CurrentStateResponse>, (StatusCode, String)> {
    info!("🎯 API: Getting current state");

    match get_current_state(&state.pool).await {
        Ok(state_info) => {
            // Find last proven batch
            let last_proven_batch_id = if state_info.merkle_root.is_some() {
                state_info.last_batch_id
            } else {
                None
            };

            let response = CurrentStateResponse {
                counter_value: state_info.counter_value,
                has_merkle_root: state_info.merkle_root.is_some(),
                last_batch_id: state_info.last_batch_id,
                last_proven_batch_id,
            };

            info!(
                "✅ API: Current state: counter={}, has_root={}",
                response.counter_value, response.has_merkle_root
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

/// Get contract submission data (dry run)
#[instrument(skip(state), level = "info")]
async fn get_contract_data_endpoint(
    State(state): State<ApiState>,
    Path(batch_id): Path<i32>,
) -> Result<Json<ContractSubmissionData>, (StatusCode, String)> {
    info!("📄 API: Getting contract data for batch: id={}", batch_id);

    match get_contract_submission_data(&state.pool, batch_id).await {
        Ok(contract_data) => {
            info!("✅ API: Contract data prepared for batch: id={}", batch_id);
            Ok(Json(contract_data))
        }
        Err(SqlxError::RowNotFound) => {
            info!("❌ API: Batch not found or not proven: id={}", batch_id);
            Err((
                StatusCode::NOT_FOUND,
                format!("Batch not found or not proven: {}", batch_id),
            ))
        }
        Err(e) => {
            error!("Failed to get contract data: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get contract data: {}", e),
            ))
        }
    }
}

/// Manually trigger batch processing
#[instrument(skip(state), level = "info")]
async fn trigger_batch_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<TriggerBatchResponse>, (StatusCode, String)> {
    info!("🔄 API: Manual batch trigger requested");

    if let Some(batch_processor) = &state.batch_processor {
        match batch_processor.trigger_batch().await {
            Ok(()) => {
                info!("✅ API: Batch trigger sent successfully");
                Ok(Json(TriggerBatchResponse {
                    triggered: true,
                    message: "Batch processing triggered successfully".to_string(),
                }))
            }
            Err(e) => {
                error!("Failed to trigger batch processing: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to trigger batch processing: {}", e),
                ))
            }
        }
    } else {
        warn!("Batch processor is not available");
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Background batch processor is not available".to_string(),
        ))
    }
}

/// Get batch processor statistics
#[instrument(skip(state), level = "info")]
async fn get_batch_processor_stats_endpoint(
    State(state): State<ApiState>,
) -> Result<Json<BatchProcessorStatsResponse>, (StatusCode, String)> {
    info!("📊 API: Batch processor stats requested");

    if let Some(batch_processor) = &state.batch_processor {
        let stats = batch_processor.get_stats().await;

        let response = BatchProcessorStatsResponse {
            total_batches_created: stats.total_batches_created,
            total_transactions_processed: stats.total_transactions_processed,
            timer_triggers: stats.timer_triggers,
            count_triggers: stats.count_triggers,
            manual_triggers: stats.manual_triggers,
            errors: stats.errors,
            last_batch_time: stats
                .last_batch_time
                .map(|t| format!("{} ago", format_duration(t.elapsed()))),
        };

        info!("✅ API: Batch processor stats returned");
        Ok(Json(response))
    } else {
        warn!("Batch processor is not available");
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Background batch processor is not available".to_string(),
        ))
    }
}
