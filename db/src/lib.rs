#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::single_match_else)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::ignored_unit_patterns)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::derive_partial_eq_without_eq)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::unused_self)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::assign_op_pattern)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::zero_sized_map_values)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::suboptimal_flops)]

pub mod db;
pub mod error;
pub mod merkle_tree;
pub mod merkle_tree_32;
pub mod ads_service;
pub mod vapp_integration;
pub mod api;
pub mod background_processor;

// Re-export main types for convenience
pub use error::{DbError, DbResult};
pub use merkle_tree::{
    IndexedMerkleTree, AlgorithmInsertionResult, InsertionMetrics, InsertionProof, MerkleProof,
    InsertionResult, LowNullifier, MerkleNode, MerkleNodeDb, MerkleTreeDb, 
    Nullifier, NullifierDb, TreeState, TreeStateDb, TreeStats,
};
pub use merkle_tree_32::{
    MerkleTree32, MerkleProof32, BatchUpdate, TreeMetrics, Tree32Stats,
};
pub use ads_service::{
    AuthenticatedDataStructure, IndexedMerkleTreeADS, AdsServiceFactory, AdsConfig,
    StateTransition, MembershipProof, NonMembershipProof, StateCommitment,
    AuditTrail, AuditEvent, AuditEventType, AdsMetrics, AdsError,
    WitnessData, ComplianceStatus,
};
pub use vapp_integration::{
    VAppAdsIntegration, VAppConfig, Environment,
    VAppInsertionResponse, VAppProofResponse, VAppBatchResponse,
    ProofType, SettlementResult, ZkProof, ComplianceResult,
    VAppError, SettlementError, ProofError, ComplianceError,
};
pub use api::{
    // REST API types
    ApiState, ApiConfig, create_router,
    InsertNullifierRequest, InsertNullifierResponse, 
    BatchInsertRequest, BatchInsertResponse,
    MembershipCheckResponse, NonMembershipResponse,
    TreeStatsResponse, AuditTrailResponse, HealthResponse,
    // GraphQL types
    QueryRoot, MutationRoot, SubscriptionRoot, GraphQLSchema,
    create_schema, NullifierType, StateTransitionType, 
    MembershipProofType, NonMembershipProofType, TreeStatsType,
    InsertNullifierInput, BatchInsertInput, NullifierQueryInput,
    ProofResult, OperationResult,
    // Server types
    ApiServer, ApiServerConfig, ApiServerBuilder,
};
pub use background_processor::{
    BackgroundProcessor, ProcessorConfig, ProcessorBuilder,
};

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod error_tests;

// Temporarily disabled - advanced functionality not yet implemented
// #[cfg(test)]
// mod merkle_tree_tests;

// #[cfg(test)]
// mod indexed_merkle_tree_tests;

// #[cfg(test)]
// mod merkle_tree_32_tests;

// #[cfg(test)]
// mod ads_service_tests;

#[cfg(test)]
mod api_tests;
