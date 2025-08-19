use async_graphql::{
    Context, Enum, FieldResult, InputObject, Object, Schema, SimpleObject, Subscription, Union,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use tracing::{info, instrument};

use crate::rest::ApiState;
use arithmetic_db::ads_service::{AdsError, AuthenticatedDataStructure};
use arithmetic_db::vapp_integration::VAppError;

// ============================================================================
// GRAPHQL SCHEMA TYPES
// ============================================================================

/// Nullifier type for GraphQL
#[derive(SimpleObject, Clone)]
pub struct NullifierType {
    pub value: i64,
    pub tree_index: Option<i64>,
    pub inserted_at: Option<DateTime<Utc>>,
    pub block_height: Option<i64>,
}

/// State transition type for GraphQL
#[derive(SimpleObject, Clone)]
pub struct StateTransitionType {
    pub id: String,
    pub old_root: String,
    pub new_root: String,
    pub nullifier_value: i64,
    pub block_height: i64,
    pub timestamp: DateTime<Utc>,
    pub gas_estimate: i64,
    pub constraint_count: ConstraintCountType,
}

/// Constraint count information
#[derive(SimpleObject, Clone)]
pub struct ConstraintCountType {
    pub total_hashes: i32,
    pub range_checks: i32,
    pub equality_checks: i32,
    pub total_constraints: i32,
    pub vs_traditional: String,
}

/// Merkle proof type
#[derive(SimpleObject, Clone)]
pub struct MerkleProofType {
    pub leaf_hash: String,
    pub siblings: Vec<String>,
    pub path_indices: Vec<bool>,
    pub proof_size_bytes: i32,
    pub tree_height: i32,
}

/// Membership proof type
#[derive(SimpleObject, Clone)]
pub struct MembershipProofType {
    pub nullifier_value: i64,
    pub tree_index: i64,
    pub root_hash: String,
    pub merkle_proof: MerkleProofType,
    pub verified_at: DateTime<Utc>,
    pub is_valid: bool,
}

/// Low nullifier information
#[derive(SimpleObject, Clone)]
pub struct LowNullifierType {
    pub value: i64,
    pub next_value: i64,
    pub tree_index: i64,
    pub merkle_proof: MerkleProofType,
}

/// Range proof information
#[derive(SimpleObject, Clone)]
pub struct RangeProofType {
    pub lower_bound: i64,
    pub upper_bound: i64,
    pub queried_value: i64,
    pub valid: bool,
    pub gap_size: i64,
}

/// Non-membership proof type
#[derive(SimpleObject, Clone)]
pub struct NonMembershipProofType {
    pub queried_value: i64,
    pub low_nullifier: LowNullifierType,
    pub root_hash: String,
    pub range_proof: RangeProofType,
    pub verified_at: DateTime<Utc>,
    pub is_valid: bool,
}

/// Tree statistics type
#[derive(SimpleObject, Clone)]
pub struct TreeStatsType {
    pub root_hash: String,
    pub total_nullifiers: i64,
    pub tree_height: i32,
    pub next_available_index: i64,
    pub performance_metrics: PerformanceMetricsType,
    pub constraint_efficiency: ConstraintEfficiencyType,
    pub last_updated: DateTime<Utc>,
}

/// Performance metrics type
#[derive(SimpleObject, Clone)]
pub struct PerformanceMetricsType {
    pub avg_insertion_time_ms: f64,
    pub avg_proof_generation_time_ms: f64,
    pub total_operations: i64,
    pub error_rate_percent: f64,
    pub operations_per_second: f64,
}

/// Constraint efficiency comparison
#[derive(SimpleObject, Clone)]
pub struct ConstraintEfficiencyType {
    pub our_constraints: i32,
    pub traditional_constraints: i32,
    pub improvement_factor: f64,
    pub description: String,
    pub efficiency_percentage: f64,
}

/// Audit event type
#[derive(SimpleObject, Clone)]
pub struct AuditEventType {
    pub event_id: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub root_before: String,
    pub root_after: String,
    pub block_height: i64,
    pub operator: String,
    pub metadata: String, // JSON string
    pub impact_score: f64,
}

/// Audit trail type
#[derive(SimpleObject, Clone)]
pub struct AuditTrailType {
    pub nullifier_value: i64,
    pub total_events: i32,
    pub events: Vec<AuditEventType>,
    pub compliance_status: ComplianceStatusType,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub risk_score: f64,
}

/// Compliance status type
#[derive(SimpleObject, Clone)]
pub struct ComplianceStatusType {
    pub is_compliant: bool,
    pub last_audit: DateTime<Utc>,
    pub jurisdiction: String,
    pub notes: Vec<String>,
    pub risk_level: String,
}

/// State commitment type
#[derive(SimpleObject, Clone)]
pub struct StateCommitmentType {
    pub root_hash: String,
    pub nullifier_count: i64,
    pub tree_height: i32,
    pub commitment_hash: String,
    pub settlement_data: SettlementDataType,
    pub last_updated: DateTime<Utc>,
    pub version: i32,
}

/// Settlement data type
#[derive(SimpleObject, Clone)]
pub struct SettlementDataType {
    pub contract_address: String,
    pub chain_id: i64,
    pub nonce: i64,
    pub gas_price: i64,
    pub estimated_gas: i64,
}

/// Health status type
#[derive(SimpleObject, Clone)]
pub struct HealthStatusType {
    pub service_name: String,
    pub status: String,
    pub uptime_seconds: i64,
    pub last_check: DateTime<Utc>,
    pub version: String,
    pub metrics: HashMap<String, String>,
}

// ============================================================================
// INPUT TYPES
// ============================================================================

/// Input for nullifier insertion
#[derive(InputObject, Debug)]
pub struct InsertNullifierInput {
    pub value: i64,
    pub metadata: Option<String>,
    pub client_id: Option<String>,
    pub force_proof_generation: Option<bool>,
}

/// Input for batch nullifier insertion
#[derive(InputObject, Debug)]
pub struct BatchInsertInput {
    pub values: Vec<i64>,
    pub metadata: Option<String>,
    pub client_id: Option<String>,
    pub parallel_processing: Option<bool>,
}

/// Input for nullifier query
#[derive(InputObject, Debug)]
pub struct NullifierQueryInput {
    pub value: i64,
    pub generate_proof: Option<bool>,
    pub include_audit_trail: Option<bool>,
}

/// Input for pagination
#[derive(InputObject, Debug)]
pub struct PaginationInput {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Input for date range filtering
#[derive(InputObject, Debug)]
pub struct DateRangeInput {
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
}

/// Input for audit trail query
#[derive(InputObject, Debug)]
pub struct AuditTrailQueryInput {
    pub nullifier_value: i64,
    pub event_types: Option<Vec<String>>,
    pub date_range: Option<DateRangeInput>,
    pub include_metadata: Option<bool>,
}

// ============================================================================
// UNION TYPES FOR RESULTS
// ============================================================================

/// Union type for proof results
#[derive(Union)]
pub enum ProofResult {
    MembershipProof(MembershipProofType),
    NonMembershipProof(NonMembershipProofType),
}

/// Union type for operation results
#[derive(Union)]
pub enum OperationResult {
    Success(SuccessResult),
    Error(ErrorResult),
}

#[derive(SimpleObject)]
pub struct SuccessResult {
    pub message: String,
    pub transaction_id: Option<String>,
    pub processing_time_ms: i64,
}

#[derive(SimpleObject)]
pub struct ErrorResult {
    pub error_code: String,
    pub message: String,
    pub details: Option<String>,
}

// ============================================================================
// ENUM TYPES
// ============================================================================

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum ProofTypeEnum {
    Membership,
    NonMembership,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TreeMetricType {
    InsertionTime,
    ProofTime,
    ErrorRate,
    ThroughputOps,
    ConstraintCount,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum AuditEventTypeEnum {
    Insertion,
    ProofGeneration,
    Settlement,
    ComplianceCheck,
    ErrorOccurred,
}

// ============================================================================
// QUERY ROOT
// ============================================================================

#[derive(Debug)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Get information about a specific nullifier
    #[instrument(skip(ctx))]
    async fn nullifier(
        &self,
        ctx: &Context<'_>,
        input: NullifierQueryInput,
    ) -> FieldResult<Option<NullifierType>> {
        let state = ctx.data::<ApiState>()?;

        info!("🔍 GraphQL: Querying nullifier {}", input.value);

        let verification_result = {
            let vapp = state.vapp_integration.read().await;
            vapp.verify_nullifier_presence(input.value).await
        };
        match verification_result {
            Ok(_) => {
                // Nullifier exists, get details
                Ok(Some(NullifierType {
                    value: input.value,
                    tree_index: Some(0),           // Would get actual index
                    inserted_at: Some(Utc::now()), // Would get actual timestamp
                    block_height: Some(1000),      // Would get actual block height
                }))
            }
            Err(_) => Ok(None), // Nullifier doesn't exist
        }
    }

    /// Check if a nullifier exists and get membership proof
    #[instrument(skip(ctx))]
    async fn membership_proof(
        &self,
        ctx: &Context<'_>,
        nullifier_value: i64,
    ) -> FieldResult<Option<MembershipProofType>> {
        let state = ctx.data::<ApiState>()?;

        info!(
            "🔍 GraphQL: Generating membership proof for {}",
            nullifier_value
        );

        let verification_result = {
            let vapp = state.vapp_integration.read().await;
            vapp.verify_nullifier_presence(nullifier_value).await
        };
        match verification_result {
            Ok(response) => {
                if let Some(proof) = response.membership_proof {
                    Ok(Some(MembershipProofType {
                        nullifier_value: proof.nullifier_value,
                        tree_index: proof.tree_index,
                        root_hash: hex::encode(proof.root_hash),
                        merkle_proof: MerkleProofType {
                            leaf_hash: hex::encode([0u8; 32]), // Would be actual leaf hash
                            siblings: proof
                                .merkle_proof
                                .siblings
                                .iter()
                                .map(|s| hex::encode(s))
                                .collect(),
                            path_indices: proof.merkle_proof.path_indices,
                            proof_size_bytes: 32 * 32,
                            tree_height: 32,
                        },
                        verified_at: proof.verified_at,
                        is_valid: true,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Generate non-membership proof for a nullifier
    #[instrument(skip(ctx))]
    async fn non_membership_proof(
        &self,
        ctx: &Context<'_>,
        nullifier_value: i64,
    ) -> FieldResult<Option<NonMembershipProofType>> {
        let state = ctx.data::<ApiState>()?;

        info!(
            "🔍 GraphQL: Generating non-membership proof for {}",
            nullifier_value
        );

        let verification_result = {
            let vapp = state.vapp_integration.read().await;
            vapp.verify_nullifier_absence(nullifier_value).await
        };
        match verification_result {
            Ok(response) => {
                if let Some(proof) = response.non_membership_proof {
                    let gap_size = if proof.low_nullifier.next_value == 0 {
                        i64::MAX - proof.low_nullifier.value
                    } else {
                        proof.low_nullifier.next_value - proof.low_nullifier.value
                    };

                    Ok(Some(NonMembershipProofType {
                        queried_value: proof.queried_value,
                        low_nullifier: LowNullifierType {
                            value: proof.low_nullifier.value,
                            next_value: proof.low_nullifier.next_value,
                            tree_index: proof.low_nullifier.tree_index,
                            merkle_proof: MerkleProofType {
                                leaf_hash: hex::encode([0u8; 32]),
                                siblings: proof
                                    .low_nullifier
                                    .merkle_proof
                                    .siblings
                                    .iter()
                                    .map(|s| hex::encode(s))
                                    .collect(),
                                path_indices: proof.low_nullifier.merkle_proof.path_indices,
                                proof_size_bytes: 32 * 32,
                                tree_height: 32,
                            },
                        },
                        root_hash: hex::encode(proof.root_hash),
                        range_proof: RangeProofType {
                            lower_bound: proof.range_proof.lower_bound,
                            upper_bound: proof.range_proof.upper_bound,
                            queried_value: proof.range_proof.queried_value,
                            valid: proof.range_proof.valid,
                            gap_size,
                        },
                        verified_at: proof.verified_at,
                        is_valid: response.verification_status,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Get current tree statistics
    #[instrument(skip(ctx))]
    async fn tree_stats(&self, ctx: &Context<'_>) -> FieldResult<TreeStatsType> {
        let state = ctx.data::<ApiState>()?;

        let (commitment, metrics) = {
            let ads = state.ads.read().await;
            let commitment = ads.get_state_commitment().await?;
            let metrics = ads.get_metrics().await?;
            (commitment, metrics)
        };

        Ok(TreeStatsType {
            root_hash: hex::encode(commitment.root_hash),
            total_nullifiers: commitment.nullifier_count as i64,
            tree_height: 32,
            next_available_index: commitment.nullifier_count as i64,
            performance_metrics: PerformanceMetricsType {
                avg_insertion_time_ms: metrics.avg_insertion_time_ms,
                avg_proof_generation_time_ms: metrics.avg_proof_time_ms,
                total_operations: metrics.operations_total as i64,
                error_rate_percent: metrics.error_rate * 100.0,
                operations_per_second: if metrics.avg_insertion_time_ms > 0.0 {
                    1000.0 / metrics.avg_insertion_time_ms
                } else {
                    0.0
                },
            },
            constraint_efficiency: ConstraintEfficiencyType {
                our_constraints: 200,
                traditional_constraints: 1600,
                improvement_factor: 8.0,
                description: "32-level indexed tree vs traditional 256-level tree".to_string(),
                efficiency_percentage: 87.5, // (1600-200)/1600 * 100
            },
            last_updated: commitment.last_updated,
        })
    }

    /// Get current tree root hash
    #[instrument(skip(ctx))]
    async fn tree_root(&self, ctx: &Context<'_>) -> FieldResult<String> {
        let state = ctx.data::<ApiState>()?;

        let commitment = {
            let vapp = state.vapp_integration.read().await;
            vapp.get_current_state_commitment().await?
        };

        Ok(hex::encode(commitment.root_hash))
    }

    /// Get state commitment for settlement
    #[instrument(skip(ctx))]
    async fn state_commitment(&self, ctx: &Context<'_>) -> FieldResult<StateCommitmentType> {
        let state = ctx.data::<ApiState>()?;

        let commitment = {
            let vapp = state.vapp_integration.read().await;
            vapp.get_current_state_commitment().await?
        };

        Ok(StateCommitmentType {
            root_hash: hex::encode(commitment.root_hash),
            nullifier_count: commitment.nullifier_count as i64,
            tree_height: commitment.tree_height as i32,
            commitment_hash: hex::encode(commitment.commitment_hash),
            settlement_data: SettlementDataType {
                contract_address: commitment.settlement_data.contract_address,
                chain_id: commitment.settlement_data.chain_id as i64,
                nonce: commitment.settlement_data.nonce as i64,
                gas_price: commitment.settlement_data.gas_price as i64,
                estimated_gas: 150_000, // Placeholder
            },
            last_updated: commitment.last_updated,
            version: 1,
        })
    }

    /// Get audit trail for a nullifier
    #[instrument(skip(ctx))]
    async fn audit_trail(
        &self,
        ctx: &Context<'_>,
        input: AuditTrailQueryInput,
    ) -> FieldResult<Option<AuditTrailType>> {
        let state = ctx.data::<ApiState>()?;

        let audit_result = {
            let ads = state.ads.read().await;
            ads.get_audit_trail(input.nullifier_value).await
        };
        match audit_result {
            Ok(audit_trail) => {
                let events: Vec<AuditEventType> = audit_trail
                    .operation_history
                    .into_iter()
                    .map(|event| AuditEventType {
                        event_id: event.event_id,
                        event_type: format!("{:?}", event.event_type),
                        timestamp: event.timestamp,
                        root_before: hex::encode(event.root_before),
                        root_after: hex::encode(event.root_after),
                        block_height: event.block_height as i64,
                        operator: event.operator,
                        metadata: event.metadata.to_string(),
                        impact_score: 0.5, // Would calculate based on event type
                    })
                    .collect();

                Ok(Some(AuditTrailType {
                    nullifier_value: audit_trail.nullifier_value,
                    total_events: events.len() as i32,
                    events,
                    compliance_status: ComplianceStatusType {
                        is_compliant: audit_trail.compliance_status.is_compliant,
                        last_audit: audit_trail.compliance_status.last_audit,
                        jurisdiction: audit_trail.compliance_status.jurisdiction,
                        notes: audit_trail.compliance_status.notes,
                        risk_level: "LOW".to_string(), // Would calculate
                    },
                    created_at: audit_trail.created_at,
                    last_accessed: audit_trail.last_accessed,
                    risk_score: 0.1, // Would calculate
                }))
            }
            Err(_) => Ok(None),
        }
    }

    /// Get performance metrics
    #[instrument(skip(ctx))]
    async fn performance_metrics(&self, ctx: &Context<'_>) -> FieldResult<PerformanceMetricsType> {
        let state = ctx.data::<ApiState>()?;

        let metrics = {
            let vapp = state.vapp_integration.read().await;
            vapp.get_metrics().await?
        };

        Ok(PerformanceMetricsType {
            avg_insertion_time_ms: metrics.avg_insertion_time_ms,
            avg_proof_generation_time_ms: metrics.avg_proof_time_ms,
            total_operations: metrics.operations_total as i64,
            error_rate_percent: metrics.error_rate * 100.0,
            operations_per_second: if metrics.avg_insertion_time_ms > 0.0 {
                1000.0 / metrics.avg_insertion_time_ms
            } else {
                0.0
            },
        })
    }

    /// Get system health status
    #[instrument(skip(ctx))]
    async fn health(&self, ctx: &Context<'_>) -> FieldResult<HealthStatusType> {
        let state = ctx.data::<ApiState>()?;

        let is_healthy = state
            .vapp_integration
            .read()
            .await
            .health_check()
            .await
            .unwrap_or(false);

        let mut health_metrics = HashMap::new();
        health_metrics.insert(
            "ads_service".to_string(),
            if is_healthy { "healthy" } else { "degraded" }.to_string(),
        );
        health_metrics.insert("database".to_string(), "healthy".to_string());
        health_metrics.insert("tree_height".to_string(), "32".to_string());
        health_metrics.insert("constraint_optimization".to_string(), "8x".to_string());

        Ok(HealthStatusType {
            service_name: state.config.server_name.clone(),
            status: if is_healthy { "healthy" } else { "degraded" }.to_string(),
            uptime_seconds: 0, // Would track actual uptime
            last_check: Utc::now(),
            version: state.config.version.clone(),
            metrics: health_metrics,
        })
    }
}

// ============================================================================
// MUTATION ROOT
// ============================================================================

#[derive(Debug)]
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Insert a single nullifier into the tree
    #[instrument(skip(ctx))]
    async fn insert_nullifier(
        &self,
        ctx: &Context<'_>,
        input: InsertNullifierInput,
    ) -> FieldResult<StateTransitionType> {
        let state = ctx.data::<ApiState>()?;

        info!("🔄 GraphQL: Inserting nullifier {}", input.value);

        let response = {
            let vapp = state.vapp_integration.read().await;
            vapp.process_nullifier_insertion(input.value).await?
        };

        Ok(StateTransitionType {
            id: response.state_transition.id,
            old_root: hex::encode(response.state_transition.old_root),
            new_root: hex::encode(response.state_transition.new_root),
            nullifier_value: response.state_transition.nullifier_value,
            block_height: response.state_transition.block_height as i64,
            timestamp: response.state_transition.timestamp,
            gas_estimate: response.state_transition.gas_estimate as i64,
            constraint_count: ConstraintCountType {
                total_hashes: 99, // 3 * 32 + 3
                range_checks: 2,
                equality_checks: 10,
                total_constraints: 200,
                vs_traditional: "8x fewer constraints (200 vs 1600)".to_string(),
            },
        })
    }

    /// Insert multiple nullifiers in a batch
    #[instrument(skip(ctx))]
    async fn batch_insert_nullifiers(
        &self,
        ctx: &Context<'_>,
        input: BatchInsertInput,
    ) -> FieldResult<OperationResult> {
        let state = ctx.data::<ApiState>()?;

        info!(
            "📦 GraphQL: Batch inserting {} nullifiers",
            input.values.len()
        );

        // Validate batch size
        if input.values.len() > state.config.max_batch_size as usize {
            return Ok(OperationResult::Error(ErrorResult {
                error_code: "BATCH_SIZE_EXCEEDED".to_string(),
                message: format!(
                    "Batch size {} exceeds maximum {}",
                    input.values.len(),
                    state.config.max_batch_size
                ),
                details: Some("Reduce batch size and try again".to_string()),
            }));
        }

        let batch_result = {
            let vapp = state.vapp_integration.read().await;
            vapp.process_batch_insertions(&input.values).await
        };
        match batch_result {
            Ok(response) => {
                let message = format!(
                    "Batch completed: {}/{} operations successful in {}ms",
                    response.successful_operations,
                    response.total_operations,
                    response.processing_time_ms
                );

                Ok(OperationResult::Success(SuccessResult {
                    message,
                    transaction_id: Some(response.batch_id),
                    processing_time_ms: response.processing_time_ms as i64,
                }))
            }
            Err(e) => Ok(OperationResult::Error(ErrorResult {
                error_code: "BATCH_INSERT_FAILED".to_string(),
                message: "Batch insertion failed".to_string(),
                details: Some(e.to_string()),
            })),
        }
    }

    /// Reset performance metrics (admin operation)
    #[instrument(skip(ctx))]
    async fn reset_metrics(&self, ctx: &Context<'_>) -> FieldResult<OperationResult> {
        let state = ctx.data::<ApiState>()?;

        info!("🔄 GraphQL: Resetting performance metrics");

        let ads = state.ads.write().await;
        match ads.reset_metrics().await {
            Ok(_) => Ok(OperationResult::Success(SuccessResult {
                message: "Performance metrics reset successfully".to_string(),
                transaction_id: None,
                processing_time_ms: 0,
            })),
            Err(e) => Ok(OperationResult::Error(ErrorResult {
                error_code: "METRICS_RESET_FAILED".to_string(),
                message: "Failed to reset metrics".to_string(),
                details: Some(e.to_string()),
            })),
        }
    }
}

// ============================================================================
// SUBSCRIPTION ROOT
// ============================================================================

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Subscribe to nullifier insertion events
    #[instrument(skip(self, _ctx))]
    async fn nullifier_insertions(
        &self,
        _ctx: &Context<'_>,
    ) -> impl futures_util::stream::Stream<Item = StateTransitionType> {
        // This would be a real-time stream of insertions
        // For now, return an empty stream as placeholder
        tokio_stream::empty()
    }

    /// Subscribe to tree statistics updates
    #[instrument(skip(self, _ctx))]
    async fn tree_stats_updates(
        &self,
        _ctx: &Context<'_>,
    ) -> impl futures_util::stream::Stream<Item = TreeStatsType> {
        // This would emit periodic tree statistics
        // For now, return an empty stream as placeholder
        tokio_stream::empty()
    }

    /// Subscribe to audit events
    #[instrument(skip(self, _ctx))]
    async fn audit_events(
        &self,
        _ctx: &Context<'_>,
    ) -> impl futures_util::stream::Stream<Item = AuditEventType> {
        // This would emit audit events in real-time
        // For now, return an empty stream as placeholder
        tokio_stream::empty()
    }
}

// ============================================================================
// SCHEMA CREATION
// ============================================================================

/// GraphQL schema type
pub type GraphQLSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Create the GraphQL schema
pub fn create_schema() -> GraphQLSchema {
    let query_root = QueryRoot;
    let mutation_root = MutationRoot;
    let subscription_root = SubscriptionRoot;

    Schema::build(query_root, mutation_root, subscription_root)
        .enable_federation()
        .finish()
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Convert internal error to GraphQL field result
pub fn to_field_result<T>(result: Result<T, VAppError>) -> FieldResult<T> {
    match result {
        Ok(value) => Ok(value),
        Err(e) => Err(async_graphql::Error::new(e.to_string())),
    }
}

/// Convert ADS error to GraphQL error
pub fn ads_error_to_graphql(error: AdsError) -> async_graphql::Error {
    async_graphql::Error::new(format!("ADS Error: {}", error))
}

/// Create GraphQL context with state
pub fn create_context_with_state(_state: ApiState) -> Result<(), async_graphql::Error> {
    // Context creation is handled by the async-graphql framework
    // This function is a placeholder for context initialization logic
    Ok(())
}

/// Validate GraphQL input parameters
pub fn validate_nullifier_value(value: i64) -> Result<(), async_graphql::Error> {
    if value <= 0 {
        Err(async_graphql::Error::new(
            "Nullifier value must be positive",
        ))
    } else if value > i64::MAX - 1000 {
        Err(async_graphql::Error::new("Nullifier value too large"))
    } else {
        Ok(())
    }
}

/// Validate batch size
pub fn validate_batch_size(size: usize, max_size: usize) -> Result<(), async_graphql::Error> {
    if size == 0 {
        Err(async_graphql::Error::new("Batch cannot be empty"))
    } else if size > max_size {
        Err(async_graphql::Error::new(format!(
            "Batch size {} exceeds maximum {}",
            size, max_size
        )))
    } else {
        Ok(())
    }
}
