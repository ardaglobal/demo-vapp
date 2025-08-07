pub mod db;
pub mod error;
pub mod merkle_tree;
pub mod merkle_tree_32;
pub mod ads_service;
pub mod vapp_integration;
pub mod api;

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

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod error_tests;

#[cfg(test)]
mod merkle_tree_tests;

#[cfg(test)]
mod indexed_merkle_tree_tests;

#[cfg(test)]
mod merkle_tree_32_tests;

#[cfg(test)]
mod ads_service_tests;

#[cfg(test)]
mod api_tests;
