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

pub mod batch_processor;
pub mod client;
pub mod rest;
pub mod server;

// Temporarily disabled for minimal PoC:
// pub mod graphql;       // 883 lines - GraphQL API for old nullifier system (depends on disabled modules)
// pub mod integration;   // 480 lines - Complex deployment/scaling configs (depends on disabled modules)
// pub mod middleware;    // 654 lines - Comprehensive middleware (rate limiting, auth, etc. - overkill for PoC)

// Re-export main API types for convenience
pub use client::{
    ApiClientError,
    ArithmeticApiClient, // Keep old name for compatibility
    BatchApiClient,
    BatchInfo,
    BatchListResponse,
    ContractSubmissionData,
    CreateBatchRequest,
    CreateBatchResponse,
    CurrentStateResponse,
    HealthResponse,
    PendingTransactionsResponse,
    SubmitTransactionRequest,
    SubmitTransactionResponse,
    TransactionInfo,
};

pub use rest::{
    create_router, ApiConfig, ApiInfoResponse, ApiState, BatchListQuery,
    BatchListResponse as RestBatchListResponse, CreateBatchRequest as RestCreateBatchRequest,
    CreateBatchResponse as RestCreateBatchResponse,
    CurrentStateResponse as RestCurrentStateResponse, EndpointInfo,
    PendingTransactionsResponse as RestPendingTransactionsResponse,
    SubmitTransactionRequest as RestSubmitTransactionRequest,
    SubmitTransactionResponse as RestSubmitTransactionResponse, UpdateBatchProofRequest,
};

pub use server::{ApiServer, ApiServerBuilder, ApiServerConfig};

// Re-export database types that the API uses
pub use arithmetic_db::{
    create_batch,
    get_all_batches,
    get_batch_by_id,
    get_contract_submission_data,
    get_current_state,
    get_pending_transactions,
    // Database functions
    init_db,
    store_ads_state_commit,
    submit_transaction,
    update_batch_proof,

    AdsStateCommit,
    ContractPrivateData,

    ContractPublicData,
    ContractSubmissionData as DbContractSubmissionData,
    CounterState,
    // Essential error handling
    DbError,
    DbResult,
    // New batch processing types
    IncomingTransaction,
    ProofBatch,
};
