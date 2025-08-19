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

// Core modules for batch processing functionality
// Temporarily disabled legacy modules due to dropped database tables:
// pub mod ads_service;
// pub mod background_processor;
// pub mod merkle_tree;
// pub mod merkle_tree_32;
// pub mod vapp_integration;

pub mod db;
pub mod error;

// Re-export batch processing types and functions
pub use db::{
    // Batch functions
    create_batch,
    get_all_batches,
    get_batch_by_id,
    get_contract_submission_data,
    // State functions
    get_current_counter_value,
    get_current_state,

    get_pending_transactions,

    // Database connection
    init_db,
    init_db_with_url,

    // ADS/Merkle functions
    store_ads_state_commit,
    // Transaction functions
    submit_transaction,
    update_batch_proof,

    // Types
    AdsStateCommit,
    ContractPrivateData,
    ContractPublicData,
    ContractSubmissionData,
    CounterState,
    IncomingTransaction,
    ProofBatch,
};

// Re-export essential error types
pub use error::{DbError, DbResult};

// Legacy modules temporarily disabled due to dropped database tables:
/*
pub use ads_service::{
    AdsConfig, AdsError, AdsMetrics, AdsServiceFactory, AuditEvent, AuditEventType, AuditTrail,
    AuthenticatedDataStructure, ComplianceStatus, IndexedMerkleTreeADS, MembershipProof,
    NonMembershipProof, StateCommitment, StateTransition, WitnessData,
};

pub use background_processor::{BackgroundProcessor, ProcessorBuilder, ProcessorConfig};
pub use merkle_tree::{
    AlgorithmInsertionResult, IndexedMerkleTree, InsertionMetrics, InsertionProof, InsertionResult,
    LowNullifier, MerkleNode, MerkleNodeDb, MerkleProof, MerkleTreeDb, Nullifier, NullifierDb,
    TreeState, TreeStateDb, TreeStats,
};
pub use merkle_tree_32::{BatchUpdate, MerkleProof32, MerkleTree32, Tree32Stats, TreeMetrics};
pub use vapp_integration::{
    ComplianceError, ComplianceResult, Environment, ProofError, ProofType, SettlementError,
    SettlementResult, VAppAdsIntegration, VAppBatchResponse, VAppConfig, VAppError,
    VAppInsertionResponse, VAppProofResponse, ZkProof,
};
*/

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod error_tests;

#[cfg(test)]
mod proof_verification_tests;
